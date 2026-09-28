use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::*;
use crate::value::*;

pub type Scope = HashMap<String, Value>;

pub struct Env {
    pub vars: RefCell<Scope>,
    pub parent: Option<Rc<Env>>,
}

impl Env {
    pub fn root(vars: Scope) -> Rc<Env> {
        Rc::new(Env { vars: RefCell::new(vars), parent: None })
    }

    pub fn child(parent: &Rc<Env>, vars: Scope) -> Rc<Env> {
        Rc::new(Env { vars: RefCell::new(vars), parent: Some(parent.clone()) })
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        if let Some(v) = self.vars.borrow().get(name) {
            return Some(v.clone());
        }
        self.parent.as_ref().and_then(|p| p.get(name))
    }

    pub fn set(&self, name: &str, v: Value) -> bool {
        if self.vars.borrow().contains_key(name) {
            self.vars.borrow_mut().insert(name.to_string(), v);
            return true;
        }
        match &self.parent {
            Some(p) => p.set(name, v),
            None => false,
        }
    }

    pub fn define(&self, name: &str, v: Value) {
        self.vars.borrow_mut().insert(name.to_string(), v);
    }
}

pub struct Closure {
    pub lambda: Rc<Lambda>,
    pub env: Rc<Env>,
}

pub struct Interp {
    pub env: Rc<Env>,
    pub out: Vec<String>,
    pub depth: usize,
}

pub enum Flow {
    Normal,
    Break,
    Continue,
    Return(Value),
}

pub fn call_closure(c: &Rc<Closure>, args: Vec<Value>) -> Result<Value, String> {
    let mut interp = Interp::with_env(c.env.clone());
    interp.run(&c.lambda, args)
}

impl Default for Interp {
    fn default() -> Self {
        Self::new()
    }
}

impl Interp {
    pub fn new() -> Self {
        Interp::with_env(Env::root(crate::builtins::globals()))
    }

    pub fn with_env(env: Rc<Env>) -> Self {
        Interp { env, out: Vec::new(), depth: 0 }
    }

    pub fn define(&self, name: &str, v: Value) {
        self.env.define(name, v);
    }

    pub fn run(&mut self, f: &Rc<Lambda>, args: Vec<Value>) -> Result<Value, String> {
        let mut vars = Scope::new();
        for (i, p) in f.params.iter().enumerate() {
            let v = match args.get(i) {
                Some(v) => v.clone(),
                None => match &p.default {
                    Some(e) => self.eval(e)?,
                    None => return Err(format!("missing argument `{}`", p.name)),
                },
            };
            vars.insert(p.name.clone(), v);
        }
        let parent = self.env.clone();
        let outer = std::mem::replace(&mut self.env, Env::child(&parent, vars));
        let result = (|| -> Result<Value, String> {
            if let Some(e) = &f.expr_body {
                return self.eval(e);
            }
            let mut last = Value::Null;
            let n = f.body.len();
            for (i, st) in f.body.iter().enumerate() {
                if i + 1 == n {
                    if let Some(v) = self.value_of(st)? {
                        last = v;
                    }
                    continue;
                }
                match self.exec(st)? {
                    Flow::Normal => {}
                    Flow::Return(v) => return Ok(v),
                    _ => break,
                }
            }
            Ok(last)
        })();
        self.env = outer;
        result
    }

    pub fn print(&mut self, text: String) {
        self.out.push(text);
    }

    fn lookup(&self, name: &str) -> Option<Value> {
        self.env.get(name)
    }

    fn assign(&mut self, name: &str, v: Value) {
        if !self.env.set(name, v.clone()) {
            self.env.define(name, v);
        }
    }

    pub fn value_of(&mut self, st: &Stmt) -> Result<Option<Value>, String> {
        match st {
            Stmt::Expr(e) => Ok(Some(self.eval(e)?)),
            Stmt::Block(b) => self.last_value(b),
            Stmt::If(c, t, e) => {
                if self.eval(c)?.truthy() {
                    self.last_value(t)
                } else {
                    match e {
                        Some(a) => self.last_value(a),
                        None => Ok(Some(Value::Null)),
                    }
                }
            }
            other => {
                self.exec(other)?;
                Ok(None)
            }
        }
    }

    fn last_value(&mut self, stmts: &[Stmt]) -> Result<Option<Value>, String> {
        match stmts.last() {
            Some(st) => self.value_of(st),
            None => Ok(Some(Value::Null)),
        }
    }

    pub fn exec_block(&mut self, stmts: &[Stmt]) -> Result<Flow, String> {
        for st in stmts {
            match self.exec(st)? {
                Flow::Normal => {}
                other => return Ok(other),
            }
        }
        Ok(Flow::Normal)
    }

    pub fn exec(&mut self, st: &Stmt) -> Result<Flow, String> {
        match st {
            Stmt::Let(name, e) => {
                let v = self.eval(e)?;
                self.assign(name, v);
                Ok(Flow::Normal)
            }
            Stmt::Assign(target, op, e) => {
                let rhs = self.eval(e)?;
                match target {
                    Expr::Ident(name) => {
                        let v = if *op == "=" {
                            rhs
                        } else {
                            let cur = self.lookup(name).unwrap_or(Value::Null);
                            binary_op(op, cur, rhs)?
                        };
                        self.assign(name, v);
                    }
                    Expr::Index(base, idx) => {
                        let b = self.eval(base)?;
                        let i = self.eval(idx)?;
                        if let Value::List(l) = &b {
                            let n = l.borrow().len() as i64;
                            let k = i.num().unwrap_or(0.0) as i64;
                            let k = if k < 0 { k + n } else { k };
                            let mut l = l.borrow_mut();
                            if k < 0 || k as usize >= l.len() {
                                return Err(format!("index {} out of range (len {})", k, n));
                            }
                            let v = if *op == "=" {
                                rhs
                            } else {
                                let cur = l[k as usize].clone();
                                binary_op(op, cur, rhs)?
                            };
                            l[k as usize] = v;
                        } else {
                            return Err("cannot assign into this value".into());
                        }
                    }
                    Expr::Member(base, name) => {
                        let b = self.eval(base)?;
                        if let Value::Map(m) = b {
                            let cur = m.borrow().iter().find(|(k, _)| k == name).map(|(_, v)| v.clone());
                            let v = if *op == "=" {
                                rhs
                            } else {
                                match cur {
                                    Some(c) => binary_op(op, c, rhs)?,
                                    None => rhs,
                                }
                            };
                            crate::builtins::map_set(&m, name, v);
                        } else {
                            return Err("cannot assign to this field".into());
                        }
                    }
                    _ => return Err("invalid assignment target".into()),
                }
                Ok(Flow::Normal)
            }
            Stmt::Expr(e) => {
                self.eval(e)?;
                Ok(Flow::Normal)
            }
            Stmt::If(cond, then, alt) => {
                let c = self.eval(cond)?;
                if c.truthy() {
                    self.exec_block(then)
                } else if let Some(a) = alt {
                    self.exec_block(a)
                } else {
                    Ok(Flow::Normal)
                }
            }
            Stmt::For(name, iter, body) => {
                let seq = self.eval(iter)?;
                let items = seq.items();
                for item in items {
                    let mut vars = Scope::new();
                    vars.insert(name.clone(), item);
                    let parent = self.env.clone();
        let outer = std::mem::replace(&mut self.env, Env::child(&parent, vars));
                    let flow = (|| -> Result<Flow, String> {
                        for st in body {
                            match self.exec(st)? {
                                Flow::Normal => {}
                                Flow::Continue => break,
                                other => return Ok(other),
                            }
                        }
                        Ok(Flow::Normal)
                    })();
                    self.env = outer;
                    match flow? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        _ => {}
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::While(cond, body) => {
                let mut guard = 0u64;
                loop {
                    guard += 1;
                    if guard > 100_000_000 {
                        return Err("while loop exceeded 100M iterations".into());
                    }
                    let c = self.eval(cond)?;
                    if !c.truthy() {
                        break;
                    }
                    let mut brk = false;
                    for st in body {
                        match self.exec(st)? {
                            Flow::Normal => {}
                            Flow::Break => {
                                brk = true;
                                break;
                            }
                            Flow::Continue => break,
                            Flow::Return(v) => return Ok(Flow::Return(v)),
                        }
                    }
                    if brk {
                        break;
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::Return(e) => {
                let v = self.eval(e)?;
                Ok(Flow::Return(v))
            }
            Stmt::Break => Ok(Flow::Break),
            Stmt::Continue => Ok(Flow::Continue),
            Stmt::Block(b) => self.exec_block(b),
        }
    }

    pub fn eval(&mut self, e: &Expr) -> Result<Value, String> {
        match e {
            Expr::Num(n) => Ok(Value::Num(*n)),
            Expr::Str(s) => Ok(Value::str(s.clone())),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Null => Ok(Value::Null),
            Expr::Ident(name) => match name.as_str() {
                "pi" => Ok(Value::Num(std::f64::consts::PI)),
                "e" => Ok(Value::Num(std::f64::consts::E)),
                "tau" => Ok(Value::Num(std::f64::consts::TAU)),
                "inf" => Ok(Value::Num(f64::INFINITY)),
                "nan" => Ok(Value::Num(f64::NAN)),
                "_" => Err(
                    "`_` is only a hole in a partial application: write `f(_, 2)`,\n\
                     not `_ + 1`. Use `fn(x) => x + 1` for a one-off function."
                        .into(),
                ),
                _ => self
                    .lookup(name)
                    .ok_or_else(|| format!("undefined variable `{}`", name)),
            },
            Expr::List(items) => {
                let mut out = Vec::with_capacity(items.len());
                for i in items {
                    out.push(self.eval(i)?);
                }
                Ok(Value::list(out))
            }
            Expr::Dict(pairs) => {
                let mut out = Vec::with_capacity(pairs.len());
                for (k, v) in pairs {
                    out.push((k.clone(), self.eval(v)?));
                }
                Ok(Value::map(out))
            }
            Expr::Unary(op, a) => {
                let v = self.eval(a)?;
                match *op {
                    "-" => Ok(Value::Num(-v.num().ok_or("cannot negate non-number")?)),
                    "not" => Ok(Value::Bool(!v.truthy())),
                    _ => Err(format!("unknown operator {}", op)),
                }
            }
            Expr::Binary(op, a, b) if *op == ".." => {
                let lo = self.eval(a)?.num().unwrap_or(0.0);
                let hi = self.eval(b)?.num().unwrap_or(0.0);
                Ok(Value::Range(lo, hi, 1.0))
            }
            Expr::Binary(op, a, b) => {
                if *op == "and" {
                    let l = self.eval(a)?;
                    return if l.truthy() { self.eval(b) } else { Ok(l) };
                }
                if *op == "or" {
                    let l = self.eval(a)?;
                    return if l.truthy() { Ok(l) } else { self.eval(b) };
                }
                let l = self.eval(a)?;
                let r = self.eval(b)?;
                binary_op(op, l, r)
            }
            Expr::Ternary(c, a, b) => {
                if self.eval(c)?.truthy() {
                    self.eval(a)
                } else {
                    self.eval(b)
                }
            }
            Expr::Range(a, b, step) => {
                let lo = self.eval(a)?.num().unwrap_or(0.0);
                let hi = self.eval(b)?.num().unwrap_or(0.0);
                let st = match step {
                    Some(s) => self.eval(s)?.num().unwrap_or(1.0),
                    None => 1.0,
                };
                Ok(Value::Range(lo, hi, st))
            }
            Expr::Index(base, idx) => {
                let b = self.eval(base)?;
                let i = self.eval(idx)?;
                index(&b, &i)
            }
            Expr::Member(base, name) => {
                let b = self.eval(base)?;
                b.get_member(name)
            }
            Expr::Lambda(l) => Ok(Value::Fn(Rc::new(Closure {
                lambda: Rc::new((**l).clone()),
                env: self.env.clone(),
            }))),
            Expr::Call(callee, args) => {
                let (f, receiver) = match &**callee {
                    Expr::Member(base, _) => {
                        let recv = self.eval(base)?;
                        (self.eval(callee)?, Some(recv))
                    }
                    other => (self.eval(other)?, None),
                };
                let mut pos = Vec::new();
                if let Some(r) = receiver {
                    pos.push(r);
                }
                let mut named: Vec<(String, Value)> = Vec::new();
                let mut holes: Vec<Option<Value>> = Vec::new();
                let mut saw_hole = false;
                for a in args {
                    match a {
                        Arg::Pos(e) => {
                            if is_hole(e) {
                                saw_hole = true;
                                holes.push(None);
                            } else {
                                let v = self.eval(e)?;
                                holes.push(Some(v.clone()));
                                pos.push(v);
                            }
                        }
                        Arg::Named(n, e) => named.push((n.clone(), self.eval(e)?)),
                    }
                }
                if saw_hole {
                    match f.clone() {
                        Value::Fn(c) => return Ok(Value::Partial(c, holes)),
                        Value::Native(nf) => {
                            return Ok(crate::value::native_partial(nf, holes));
                        }
                        _ => {}
                    }
                    return Err("`_` can only be used to partially apply a function".into());
                }
                if !named.is_empty() {
                    match &f {
                        Value::Native(nf) => {
                            let mut full = pos.clone();
                            for (n, v) in named {
                                match arg_index(nf.name, &n) {
                                    Some(i) => {
                                        while full.len() <= i {
                                            full.push(Value::Null);
                                        }
                                        full[i] = v;
                                    }
                                    None => {
                                        return Err(format!(
                                            "function `{}` has no argument `{}`",
                                            nf.name, n
                                        ))
                                    }
                                }
                            }
                            if full.len() < nf.arity {
                                full.resize(nf.arity, Value::Null);
                            }
                            pos = full;
                        }
                        _ => {}
                    }
                }
                if self.depth > 400 {
                    return Err("maximum call depth exceeded".into());
                }
                self.depth += 1;
                let r = f.call(pos);
                self.depth -= 1;
                r
            }
            Expr::FString(parts) => {
                let mut s = String::new();
                for part in parts {
                    match part {
                        FPart::Lit(l) => s.push_str(l),
                        FPart::Expr(e) => s.push_str(&self.eval(e)?.to_display()),
                        FPart::Spec(e, spec) => s.push_str(&self.eval(e)?.format(spec)),
                    }
                }
                Ok(Value::str(s))
            }
        }
    }
}

fn is_hole(e: &Expr) -> bool {
    matches!(e, Expr::Ident(n) if n == "_" || n.starts_with('_') && n[1..].chars().all(|c| c.is_ascii_digit()))
}

fn arg_index(fname: &str, arg: &str) -> Option<usize> {
    crate::builtins::arg_index(fname, arg)
}

pub fn index(base: &Value, idx: &Value) -> Result<Value, String> {
    match (base, idx) {
        (Value::List(l), _) => {
            let n = l.borrow().len() as i64;
            let k = idx.num().ok_or("list index must be a number")? as i64;
            let k = if k < 0 { k + n } else { k };
            l.borrow()
                .get(k.max(0) as usize)
                .cloned()
                .ok_or_else(|| format!("index {} out of range (len {})", k, n))
        }
        (Value::Str(s), _) => {
            let chars: Vec<char> = s.chars().collect();
            let n = chars.len() as i64;
            let k = idx.num().ok_or("string index must be a number")? as i64;
            let k = if k < 0 { k + n } else { k };
            chars
                .get(k.max(0) as usize)
                .map(|c| Value::str(c.to_string()))
                .ok_or_else(|| format!("index {} out of range (len {})", k, n))
        }
        (Value::Map(m), _) => {
            let k = match idx {
                Value::Str(s) => s.as_str().to_string(),
                other => other.to_plain(),
            };
            Ok(m.borrow()
                .iter()
                .find(|(kk, _)| *kk == k)
                .map(|(_, v)| v.clone())
                .unwrap_or(Value::Null))
        }
        (Value::Range(a, b, s), _) => {
            let items = range_items(*a, *b, *s);
            let n = items.len() as i64;
            let k = idx.num().ok_or("index must be a number")? as i64;
            let k = if k < 0 { k + n } else { k };
            items
                .get(k.max(0) as usize)
                .cloned()
                .ok_or_else(|| format!("index {} out of range (len {})", k, n))
        }
        _ => Err(format!("cannot index a {}", base.type_name())),
    }
}

pub fn binary_op(op: &str, l: Value, r: Value) -> Result<Value, String> {
    match op {
        "+" => {
            if let (Some(a), Some(b)) = (l.as_str(), r.as_str()) {
                return Ok(Value::str(a + &b));
            }
            if let (Value::List(a), Value::List(b)) = (&l, &r) {
                let mut v = a.borrow().clone();
                v.extend(b.borrow().iter().cloned());
                return Ok(Value::list(v));
            }
            num_op(op, &l, &r)
        }
        "-" | "*" | "/" | "%" | "^" => num_op(op, &l, &r),
        "==" => Ok(Value::Bool(l.eq(&r))),
        "!=" => Ok(Value::Bool(!l.eq(&r))),
        "<" | "<=" | ">" | ">=" => {
            let ord = l.cmp(&r)?;
            use std::cmp::Ordering::*;
            Ok(Value::Bool(match op {
                "<" => ord == Less,
                "<=" => ord != Greater,
                ">" => ord == Greater,
                _ => ord != Less,
            }))
        }
        other => Err(format!("unknown operator `{}`", other)),
    }
}

fn num_op(op: &str, l: &Value, r: &Value) -> Result<Value, String> {
    if op == "*" {
        if let Some(a) = l.as_str() {
            if let Some(k) = r.num() {
                if k >= 0.0 && k.fract() == 0.0 && k < 1000.0 {
                    return Ok(Value::str(a.repeat(k as usize)));
                }
            }
        }
        if let Some(b) = r.as_str() {
            if let Some(n) = l.num() {
                if n >= 0.0 && n.fract() == 0.0 && n < 1000.0 {
                    return Ok(Value::str(b.repeat(n as usize)));
                }
            }
        }
        if let (Value::List(items), Some(k)) = (l, r.num()) {
            if k >= 0.0 && k.fract() == 0.0 && k < 100_000.0 {
                let base = items.borrow().clone();
                let mut out = Vec::new();
                for _ in 0..(k as usize) {
                    out.extend(base.iter().cloned());
                }
                return Ok(Value::list(out));
            }
        }
    }
    let a = l
        .num()
        .ok_or_else(|| format!("cannot apply `{}` to {} and {}", op, l.type_name(), r.type_name()))?;
    let b = r
        .num()
        .ok_or_else(|| format!("cannot apply `{}` to {} and {}", op, l.type_name(), r.type_name()))?;
    Ok(Value::Num(match op {
        "+" => a + b,
        "-" => a - b,
        "*" => a * b,
        "/" => {
            if b == 0.0 {
                return Err("division by zero".into());
            }
            a / b
        }
        "%" => {
            if b == 0.0 {
                return Err("modulo by zero".into());
            }
            a % b
        }
        "^" => a.powf(b),
        _ => return Err(format!("unknown operator `{}`", op)),
    }))
}

pub fn shared() -> Rc<RefCell<Interp>> {
    Rc::new(RefCell::new(Interp::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_src(src: &str) -> Result<Vec<String>, String> {
        let stmts = crate::parser::parse_program(src)?;
        let mut interp = Interp::new();
        for st in &stmts {
            match st {
                Stmt::Expr(e) => {
                    interp.eval(e)?;
                }
                other => {
                    interp.exec(other)?;
                }
            }
        }
        Ok(crate::builtins::take_output())
    }

    #[test]
    fn partially_applies_builtins() {
        // `_` must work for builtins, not only for `fn` functions, because
        // `map(xs, sqrt(_))` is the obvious thing to write.
        assert_eq!(out("print(map([1.0, 4.0, 9.0], sqrt(_)))"), "[1, 2, 3]");
        assert_eq!(out("print(map(sqrt(_), [1.0, 4.0, 9.0]))"), "[1, 2, 3]");
        assert_eq!(out("let f = pow(_, 2)\nprint(f(5))"), "25");
        assert_eq!(out("let f = max(_, _)\nprint(f(1, 2))"), "2");
        assert_eq!(out("let f = clamp(_, 0, 1)\nprint(f(9))"), "1");
        assert_eq!(out("print(map([[1, 2], [3, 4]], sum(_)))"), "[3, 7]");
        assert_eq!(out("print(type(sqrt(_)))"), "function");
        // still callable directly
        assert_eq!(out("print(map([4.0, 9.0], sqrt))"), "[2, 3]");
    }

    #[test]
    fn partial_builtin_arity_errors() {
        // a hole cannot be filled twice
        let e = run_src("let f = max(_, _)\nprint(f(1, 2, 3))").unwrap_err();
        assert!(e.contains("too many arguments"), "{}", e);
        // and the error names the function
        let e2 = run_src("let f = sqrt(_)\nprint(f(2, 3))").unwrap_err();
        assert!(e2.contains("sqrt"), "{}", e2);
        // `_` is still only valid for callables
        let e3 = run_src("let n = 1\nn(_ + 1)").unwrap_err();
        assert!(e3.contains("only a hole in a partial application"), "{}", e3);
        // a bare `_` outside a call gets the same guidance
        let e4 = run_src("let x = _").unwrap_err();
        assert!(e4.contains("only a hole in a partial application"), "{}", e4);
    }

    fn out(src: &str) -> String {
        match run_src(src) {
            Ok(lines) => lines.join("\n"),
            Err(e) => panic!("{} => {}", src, e),
        }
    }

    #[test]
    fn arithmetic_and_precedence() {
        assert_eq!(out("print(1 + 2 * 3)"), "7");
        assert_eq!(out("print(2 ^ 3 ^ 2)"), "512");
        assert_eq!(out("print(-2 ^ 2)"), "-4");
        assert_eq!(out("print(7 % 3, 7 / 2)"), "1 3.5");
        assert_eq!(out("print(1 / 2)"), "0.5");
    }

    #[test]
    fn closures_capture_and_recurse() {
        assert_eq!(out("fn f(x, g) { g / x }\nlet h = f(_, 9)\nprint(h(3))"), "3");
        assert_eq!(
            out("fn fib(n) { if n < 2 { n } else { fib(n-1) + fib(n-2) } }\nprint(fib(10))"),
            "55"
        );
        assert_eq!(
            out("fn outer(a) { fn inner(b) { a + b }\ninner(10) }\nprint(outer(5))"),
            "15"
        );
        assert_eq!(out("let k = 2\nfn f(b) { k * b }\nprint(f(21))"), "42");
    }

    #[test]
    fn dict_return_and_nesting() {
        assert_eq!(out("let f = fn(v) => {n: len(v), mean: mean(v)}\nprint(f([1,2,3]))"), "{n: 3, mean: 2}");
    }

    #[test]
    fn list_pipeline_works_in_either_order() {
        assert_eq!(out("print(map([1,2,3], fn(v) => v * 2))"), "[2, 4, 6]");
        assert_eq!(out("print(map(fn(v) => v * 2, [1,2,3]))"), "[2, 4, 6]");
        assert_eq!(out("print(filter([1,2,3,4], fn(v) => v > 2))"), "[3, 4]");
        assert_eq!(out("print(reduce([1,2,3,4], fn(a, b) => a + b, 0))"), "10");
        assert_eq!(out("print(reduce([1,2,3,4], fn(a, b) => a + b))"), "10");
    }

    #[test]
    fn control_flow() {
        assert_eq!(out("let s = 0\nfor i in 1..5 { s += i }\nprint(s)"), "10");
        assert_eq!(out("let i = 0\nwhile i < 3 { i += 1 }\nprint(i)"), "3");
        assert_eq!(out("let a = []\nfor x in [1,2,3] { if x == 2 { continue }\n a.push(x) }\nprint(a)"), "[1, 3]");
        assert_eq!(out("let i = 0\nlet n = 0\nwhile true { i += 1\nif i > 4 { break }\nn += i }\nprint(n)"), "10");
    }

    #[test]
    fn strings_and_interpolation() {
        assert_eq!(out("let x = 1.5\nprint(\"x={x} y={x:.3f} z={x*2}\")"), "x=1.5 y=1.500 z=3");
        assert_eq!(out("print(\"a\" + \"b\", \"ab\" * 3)"), "ab ababab");
        assert_eq!(out("print(upper(\"ab\"), len(\"héllo\"))"), "AB 5");
        assert_eq!(out("print(join([1,2,3], \"-\"))"), "1-2-3");
        assert_eq!(out("print(split(\"a,b,,c\", \",\"))"), "[a, b, , c]");
        assert_eq!(out("print(slug(\"Hello, World! 42\"))"), "hello-world-42");
    }

    #[test]
    fn maps_and_grouping() {
        assert_eq!(
            out("let g = group_by([{k:\"a\",v:1},{k:\"b\",v:2},{k:\"a\",v:3}], fn(r) => r.k)\nprint(keys(g), len(g.a))"),
            "[a, b] 2"
        );
        assert_eq!(out("let m = {a: 1, b: 2}\nprint(m.b, has(m, \"c\"), keys(m))"), "2 false [a, b]");
        assert_eq!(out("let m = {a: 1}\nm.b = 9\nprint(m)"), "{a: 1, b: 9}");
        assert_eq!(out("print(sort([{n:2},{n:1}], \"n\"))"), "[{n: 1}, {n: 2}]");
    }

    #[test]
    fn statistics_helpers() {
        assert_eq!(out("print(mean([1,2,3,4]), median([1,2,3,4]), round(std([2,4,4,4,5,5,7,9]), 4))"), "2.5 2.5 2.1381");
        assert_eq!(out("print(quantile([1,2,3,4,5], 0.5), quantile([1,2,3,4,5], 0.25))"), "3 2");
        assert_eq!(out("print(corr([1,2,3,4], [2,4,6,8]))"), "1");
        assert_eq!(out("print(len(linspace(0, 1, 5)))"), "5");
    }

    #[test]
    fn errors_are_reported_not_panics() {
        assert!(run_src("print(undefined_thing)").is_err());
        assert!(run_src("let x = [1]\nprint(x[9])").is_err());
        assert!(run_src("print(1 / 0)").is_err());
        assert!(run_src("print(nope(1))").is_err());
        assert!(run_src("let f = fn(x) => x\nf()").is_err());
    }

    #[test]
    fn random_is_seedable() {
        crate::builtins::take_output();
        let a = out("seed(42)\nprint(round(random(), 9))\nseed(42)\nprint(round(random(), 9))");
        let parts: Vec<&str> = a.split('\n').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], parts[1]);
        assert!(parts[0].parse::<f64>().is_ok());
    }
}
