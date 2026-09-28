use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use crate::ast::Expr;
use crate::interp::Closure;

pub type Dict = Vec<(String, Value)>;

#[derive(Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Num(f64),
    Str(Rc<String>),
    List(Rc<RefCell<Vec<Value>>>),
    Map(Rc<RefCell<Dict>>),
    Fn(Rc<Closure>),
    Native(Rc<NativeFn>),
    Partial(Rc<Closure>, Vec<Option<Value>>),
    Range(f64, f64, f64),
}

pub struct NativeFn {
    pub name: &'static str,
    pub arity: usize,
    pub call: Rc<dyn Fn(&[Value]) -> Result<Value, String>>,
    pub doc: &'static str,
}

pub fn native(
    name: &'static str,
    arity: usize,
    call: impl Fn(&[Value]) -> Result<Value, String> + 'static,
) -> Value {
    Value::Native(Rc::new(NativeFn { name, arity, call: Rc::new(call), doc: "" }))
}

/// Wraps `base` so that it first fills in `holes` from the arguments it is
/// called with. This is what makes `sqrt(_)` work for builtins, not just for
/// functions defined with `fn`.
pub fn native_partial(base: Rc<NativeFn>, holes: Vec<Option<Value>>) -> Value {
    Value::Native(Rc::new(NativeFn {
        name: base.name,
        arity: holes.len(),
        doc: base.doc,
        call: Rc::new(move |args: &[Value]| {
            let mut rest = args.iter();
            let mut full = Vec::with_capacity(holes.len());
            for h in &holes {
                match h {
                    Some(v) => full.push(v.clone()),
                    None => full.push(
                        rest.next()
                            .cloned()
                            .ok_or_else(|| format!("{} still needs {} argument(s)", base.name, holes.iter().filter(|h| h.is_none()).count()))?,
                    ),
                }
            }
            if rest.next().is_some() {
                return Err(format!("too many arguments for {}", base.name));
            }
            (base.call)(&full)
        }),
    }))
}

impl Value {
    pub fn str(s: impl Into<String>) -> Value {
        Value::Str(Rc::new(s.into()))
    }

    pub fn list(items: Vec<Value>) -> Value {
        Value::List(Rc::new(RefCell::new(items)))
    }

    pub fn map(pairs: Vec<(String, Value)>) -> Value {
        Value::Map(Rc::new(RefCell::new(pairs)))
    }

    pub fn num(&self) -> Option<f64> {
        match self {
            Value::Num(n) => Some(*n),
            Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Value::Str(s) => s.trim().parse::<f64>().ok(),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<String> {
        match self {
            Value::Str(s) => Some(s.as_str().to_string()),
            _ => None,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Num(_) => "number",
            Value::Str(_) => "string",
            Value::List(_) | Value::Range(..) => "list",
            Value::Map(_) => "map",
            Value::Fn(_) | Value::Native(_) | Value::Partial(..) => "function",
        }
    }

    pub fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Num(n) => *n != 0.0 && !n.is_nan(),
            Value::Str(s) => !s.is_empty(),
            Value::List(l) => !l.borrow().is_empty(),
            Value::Map(m) => !m.borrow().is_empty(),
            Value::Range(a, b, s) => (*s > 0.0 && a < b) || (*s < 0.0 && a > b),
            _ => true,
        }
    }

    pub fn items(&self) -> Vec<Value> {
        match self {
            Value::List(l) => l.borrow().clone(),
            Value::Range(a, b, s) => range_items(*a, *b, *s),
            Value::Map(m) => m
                .borrow()
                .iter()
                .map(|(k, v)| Value::list(vec![Value::str(k.clone()), v.clone()]))
                .collect(),
            Value::Str(s) => s.chars().map(|c| Value::str(c.to_string())).collect(),
            _ => Vec::new(),
        }
    }

    pub fn as_list(&self) -> Option<Vec<Value>> {
        match self {
            Value::List(l) => Some(l.borrow().clone()),
            Value::Range(a, b, s) => Some(range_items(*a, *b, *s)),
            _ => None,
        }
    }

    pub fn call(&self, args: Vec<Value>) -> Result<Value, String> {
        match self {
            Value::Native(f) => (f.call)(&args),
            Value::Fn(f) => crate::interp::call_closure(f, args),
            Value::Partial(f, holes) => {
                let mut slots: Vec<Option<Value>> = holes.clone();
                let mut iter = args.into_iter();
                for slot in slots.iter_mut() {
                    if slot.is_none() {
                        *slot = iter.next();
                    }
                }
                if iter.next().is_some() {
                    return Err("too many arguments for a partially applied function".into());
                }
                let mut filled = Vec::with_capacity(slots.len());
                for (i, slot) in slots.into_iter().enumerate() {
                    let v = match slot {
                        Some(v) => v,
                        None => match f.lambda.params.get(i).and_then(|p| p.default.clone()) {
                            Some(e) => crate::interp::Interp::new().eval(&e)?,
                            None => Value::Null,
                        },
                    };
                    filled.push(v);
                }
                crate::interp::call_closure(f, filled)
            }
            other => Err(format!("{} is not callable", other.type_name())),
        }
    }

    pub fn get_member(&self, name: &str) -> Result<Value, String> {
        match self {
            Value::Map(m) => {
                let m = m.borrow();
                Ok(m.iter()
                    .find(|(k, _)| k == name)
                    .map(|(_, v)| v.clone())
                    .unwrap_or(Value::Null))
            }
            other => crate::builtins::member(other, name),
        }
    }

    pub fn eq(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Num(a), Value::Num(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::List(a), Value::List(b)) => {
                let (a, b) = (a.borrow(), b.borrow());
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.eq(y))
            }
            (Value::Map(a), Value::Map(b)) => {
                let (a, b) = (a.borrow(), b.borrow());
                a.len() == b.len()
                    && a.iter().all(|(k, v)| {
                        b.iter().find(|(k2, _)| k2 == k).map_or(false, |(_, v2)| v.eq(v2))
                    })
            }
            (Value::Null, Value::Num(n)) | (Value::Num(n), Value::Null) => *n == 0.0,
            _ => false,
        }
    }

    pub fn cmp(&self, other: &Value) -> Result<std::cmp::Ordering, String> {
        match (self, other) {
            (Value::Str(a), Value::Str(b)) => Ok(a.as_str().cmp(b.as_str())),
            _ => {
                let (a, b) = match (self.num(), other.num()) {
                    (Some(a), Some(b)) => (a, b),
                    _ => {
                        if self.eq(other) {
                            return Ok(std::cmp::Ordering::Equal);
                        }
                        return Err(format!(
                            "cannot compare {} with {}",
                            self.type_name(),
                            other.type_name()
                        ));
                    }
                };
                a.partial_cmp(&b).ok_or_else(|| "cannot compare NaN".to_string())
            }
        }
    }

    pub fn to_display(&self) -> String {
        match self {
            Value::Str(s) => s.as_str().to_string(),
            Value::List(_) | Value::Range(..) => {
                let parts: Vec<String> = self.items().iter().take(50).map(|v| v.to_display()).collect();
                let n = self.as_list().map(|l| l.len()).unwrap_or(0);
                if n > 50 {
                    format!("[{}, ... {} more]", parts.join(", "), n - 50)
                } else {
                    format!("[{}]", parts.join(", "))
                }
            }
            _ => self.to_plain(),
        }
    }

    pub fn to_plain(&self) -> String {
        match self {
            Value::Null => "null".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Num(n) => fmt_num(*n),
            Value::Str(s) => s.as_str().to_string(),
            Value::List(l) => {
                let parts: Vec<String> = l.borrow().iter().map(|v| v.to_plain()).collect();
                format!("[{}]", parts.join(", "))
            }
            Value::Range(a, b, s) => {
                let items = range_items(*a, *b, *s);
                let parts: Vec<String> = items.iter().take(50).map(|v| v.to_plain()).collect();
                if items.len() > 50 {
                    format!("[{}, ... {} more]", parts.join(", "), items.len() - 50)
                } else {
                    format!("[{}]", parts.join(", "))
                }
            }
            Value::Map(m) => {
                let parts: Vec<String> = m
                    .borrow()
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v.to_plain()))
                    .collect();
                format!("{{{}}}", parts.join(", "))
            }
            Value::Fn(_) | Value::Native(_) | Value::Partial(..) => "<function>".to_string(),
        }
    }

    pub fn format(&self, spec: &str) -> String {
        crate::builtins::apply_spec(self, spec)
            .unwrap_or_else(|_| self.to_display())
    }
}

pub fn range_items(a: f64, b: f64, step: f64) -> Vec<Value> {
    let mut out = Vec::new();
    if step == 0.0 {
        return out;
    }
    let n = (((b - a) / step).ceil()).max(0.0) as usize;
    let n = n.min(10_000_000);
    for i in 0..n {
        out.push(Value::Num(a + step * i as f64));
    }
    out
}

pub fn fmt_num(n: f64) -> String {
    if n.is_nan() {
        return "NaN".to_string();
    }
    if n.is_infinite() {
        return if n > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if n == n.trunc() && n.abs() < 1e15 {
        return format!("{}", n as i64);
    }
    let mut s = format!("{}", n);
    if s.contains('e') {
        s = format!("{:.*}", 10, n).trim_end_matches('0').trim_end_matches('.').to_string();
    }
    s
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_plain())
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_display())
    }
}

pub fn eval_expr_src(src: &str) -> Result<Value, String> {
    let e: Expr = crate::parser::parse_expr_str(src)?;
    crate::interp::Interp::new().eval(&e)
}
