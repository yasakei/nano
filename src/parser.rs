use crate::ast::*;
use crate::lexer::{Lexer, Tok, Token};

pub struct Parser {
    toks: Vec<Token>,
    pos: usize,
}

const BINARY_LEVELS: &[&[&'static str]] = &[
    &["or"],
    &["and"],
    &["==", "!=", "<=", ">=", "<", ">"],
    &[".."],
    &["+", "-"],
    &["*", "/", "%"],
];

pub fn parse_program(src: &str) -> PResult<Vec<Stmt>> {
    let toks = Lexer::new(src).tokenize()?;
    let mut p = Parser { toks, pos: 0 };
    p.stmts_until_eof()
}

pub fn parse_expr_str(src: &str) -> PResult<Expr> {
    let toks = Lexer::new(src).tokenize()?;
    let mut p = Parser { toks, pos: 0 };
    p.skip_newlines();
    let e = p.expr()?;
    p.skip_newlines();
    Ok(e)
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos.min(self.toks.len() - 1)].tok
    }

    fn peek_at(&self, n: usize) -> &Tok {
        &self.toks[(self.pos + n).min(self.toks.len() - 1)].tok
    }

    fn line(&self) -> usize {
        self.toks[self.pos.min(self.toks.len() - 1)].line
    }

    fn bump(&mut self) -> Tok {
        let t = self.toks[self.pos.min(self.toks.len() - 1)].tok.clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn at_punct(&self, p: &str) -> bool {
        matches!(self.peek(), Tok::Punct(x) if *x == p)
    }

    fn at_kw(&self, k: &str) -> bool {
        matches!(self.peek(), Tok::Kw(x) if *x == k)
    }

    fn eat_punct(&mut self, p: &str) -> bool {
        if self.at_punct(p) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn eat_kw(&mut self, k: &str) -> bool {
        if self.at_kw(k) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect_punct(&mut self, p: &'static str) -> PResult<()> {
        if self.eat_punct(p) {
            Ok(())
        } else {
            let mut ahead = String::new();
            for k in 0..6 {
                ahead.push_str(&format!(" {}", self.peek_at(k)));
            }
            Err(format!(
                "line {}, col {}: expected `{}` but found {}[{}]",
                self.line(),
                self.col(),
                p,
                self.peek(),
                ahead
            ))
        }
    }

    fn col(&self) -> usize {
        self.toks[self.pos.min(self.toks.len() - 1)].col
    }

    fn ident(&mut self) -> PResult<String> {
        match self.bump() {
            Tok::Ident(s) => Ok(s),
            other => Err(format!("line {}: expected identifier, found {}", self.line(), other)),
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek(), Tok::Newline) {
            self.bump();
        }
    }

    fn skip_terminators(&mut self) {
        loop {
            if matches!(self.peek(), Tok::Newline) || self.at_punct(";") {
                self.bump();
            } else {
                break;
            }
        }
    }

    fn stmts_until_eof(&mut self) -> PResult<Vec<Stmt>> {
        let mut out = Vec::new();
        loop {
            self.skip_terminators();
            if matches!(self.peek(), Tok::Eof) {
                break;
            }
            out.push(self.stmt()?);
        }
        Ok(out)
    }

    pub fn block(&mut self) -> PResult<Vec<Stmt>> {
        self.expect_punct("{")?;
        let mut out = Vec::new();
        loop {
            self.skip_terminators();
            if self.at_punct("}") {
                self.bump();
                break;
            }
            if matches!(self.peek(), Tok::Eof) {
                return Err(format!("line {}: unclosed `{{`", self.line()));
            }
            out.push(self.stmt()?);
        }
        Ok(out)
    }

    fn lambda_body_as_block(&mut self) -> PResult<Lambda> {
        let params = self.params()?;
        if self.eat_punct("=>") {
            self.skip_newlines();
            if self.at_punct("{") && !self.brace_is_dict() {
                let body = self.block()?;
                Ok(Lambda { params, body, expr_body: None })
            } else {
                let e = self.expr()?;
                Ok(Lambda { params, body: Vec::new(), expr_body: Some(Box::new(e)) })
            }
        } else if self.at_punct("{") {
            let body = self.block()?;
            Ok(Lambda { params, body, expr_body: None })
        } else {
            Err(format!(
                "line {}: expected `=>` or `{{` in function body",
                self.line()
            ))
        }
    }

    fn brace_is_dict(&self) -> bool {
        match (self.peek_at(1), self.peek_at(2)) {
            (Tok::Punct("}"), _) => true,
            (Tok::Ident(_), Tok::Punct(":")) => true,
            (Tok::Str(_), Tok::Punct(":")) => true,
            _ => false,
        }
    }

    fn params(&mut self) -> PResult<Vec<Param>> {
        self.expect_punct("(")?;
        let mut out = Vec::new();
        self.skip_newlines();
        while !self.at_punct(")") {
            let name = self.ident()?;
            let default = if self.eat_punct("=") { Some(self.expr()?) } else { None };
            out.push(Param { name, default });
            self.skip_newlines();
            if !self.eat_punct(",") {
                break;
            }
            self.skip_newlines();
        }
        self.skip_newlines();
        self.expect_punct(")")?;
        Ok(out)
    }

    pub fn stmt(&mut self) -> PResult<Stmt> {
        if self.at_kw("fn")
            && matches!(self.peek_at(1), Tok::Ident(_))
            && matches!(self.peek_at(2), Tok::Punct("("))
        {
            self.bump();
            let name = self.ident()?;
            let lam = self.lambda_body_as_block()?;
            return Ok(Stmt::Let(name, Expr::Lambda(Box::new(lam))));
        }
        if self.at_kw("let") {
            self.bump();
            let name = self.ident()?;
            if self.eat_punct(":") {
                self.expr()?;
            }
            self.expect_punct("=")?;
            let value = self.expr()?;
            return Ok(Stmt::Let(name, value));
        }
        if self.at_kw("if") {
            self.bump();
            let cond = self.expr()?;
            let then = self.block()?;
            let save = self.pos;
            self.skip_terminators();
            let alt = if self.eat_kw("else") {
                if self.at_kw("if") {
                    Some(vec![self.stmt()?])
                } else {
                    Some(self.block()?)
                }
            } else {
                self.pos = save;
                None
            };
            return Ok(Stmt::If(cond, then, alt));
        }
        if self.at_kw("for") {
            self.bump();
            let name = self.ident()?;
            if !self.eat_kw("in") {
                return Err(format!("line {}: expected `in` in for loop", self.line()));
            }
            let iter = self.expr()?;
            let body = self.block()?;
            return Ok(Stmt::For(name, iter, body));
        }
        if self.at_kw("while") {
            self.bump();
            let cond = self.expr()?;
            let body = self.block()?;
            return Ok(Stmt::While(cond, body));
        }
        if self.at_kw("return") {
            self.bump();
            if matches!(self.peek(), Tok::Newline | Tok::Eof) || self.at_punct("}") {
                return Ok(Stmt::Return(Expr::Null));
            }
            return Ok(Stmt::Return(self.expr()?));
        }
        if self.eat_kw("break") {
            return Ok(Stmt::Break);
        }
        if self.eat_kw("continue") {
            return Ok(Stmt::Continue);
        }
        if self.at_punct("{") {
            let body = self.block()?;
            return Ok(Stmt::Block(body));
        }
        let target = self.expr()?;
        if self.at_punct("=") && !matches!(self.peek_at(1), Tok::Punct("=")) {
            self.bump();
            self.skip_newlines();
            let v = self.expr()?;
            return Ok(Stmt::Assign(target, "=", v));
        }
        for op in ["+=", "-=", "*=", "/="] {
            if self.at_punct(op) {
                self.bump();
                let v = self.expr()?;
                let bin: &'static str = match op {
                    "+=" => "+",
                    "-=" => "-",
                    "*=" => "*",
                    _ => "/",
                };
                return Ok(Stmt::Assign(target, bin, v));
            }
        }
        Ok(Stmt::Expr(target))
    }

    pub fn expr(&mut self) -> PResult<Expr> {
        let lhs = self.binary(0)?;
        if self.at_punct("?") {
            self.bump();
            self.skip_newlines();
            let a = self.expr()?;
            if !self.eat_punct(":") {
                return Err(format!("line {}: expected `:` in ternary", self.line()));
            }
            self.skip_newlines();
            let b = self.expr()?;
            return Ok(Expr::Ternary(Box::new(lhs), Box::new(a), Box::new(b)));
        }
        Ok(lhs)
    }

    fn binary(&mut self, level: usize) -> PResult<Expr> {
        if level >= BINARY_LEVELS.len() {
            return self.unary();
        }
        let mut lhs = self.binary(level + 1)?;
        loop {
            let op = match self.peek() {
                Tok::Punct(p) if BINARY_LEVELS[level].contains(p) => *p,
                Tok::Kw(k) if BINARY_LEVELS[level].contains(k) => *k,
                _ => break,
            };
            self.bump();
            self.skip_newlines();
            let rhs = self.binary(level + 1)?;
            lhs = if op == ".." {
                let hi = rhs;
                if self.eat_punct("..") {
                    self.skip_newlines();
                    let step = self.binary(level + 1)?;
                    Expr::Range(Box::new(lhs), Box::new(hi), Some(Box::new(step)))
                } else {
                    Expr::Range(Box::new(lhs), Box::new(hi), None)
                }
            } else {
                Expr::Binary(op, Box::new(lhs), Box::new(rhs))
            };
        }
        Ok(lhs)
    }

    fn power(&mut self) -> PResult<Expr> {
        let base = self.postfix()?;
        if self.eat_punct("^") {
            self.skip_newlines();
            let exp = self.unary()?;
            return Ok(Expr::Binary("^", Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }

    fn unary(&mut self) -> PResult<Expr> {
        if self.eat_punct("-") {
            self.skip_newlines();
            let e = self.unary()?;
            return Ok(Expr::Unary("-", Box::new(e)));
        }
        if self.eat_punct("!") {
            self.skip_newlines();
            let e = self.unary()?;
            return Ok(Expr::Unary("not", Box::new(e)));
        }
        if self.eat_kw("not") {
            self.skip_newlines();
            let e = self.unary()?;
            return Ok(Expr::Unary("not", Box::new(e)));
        }
        self.power()
    }

    fn postfix(&mut self) -> PResult<Expr> {
        let mut e = self.primary()?;
        loop {
            if self.at_punct(".") {
                self.bump();
                let name = self.ident()?;
                if self.at_punct("(") {
                    let args = self.call_args()?;
                    e = Expr::Call(
                        Box::new(Expr::Member(Box::new(e), name)),
                        args,
                    );
                } else {
                    e = Expr::Member(Box::new(e), name);
                }
                continue;
            }
            if self.at_punct("[") {
                self.bump();
                self.skip_newlines();
                let idx = self.expr()?;
                self.skip_newlines();
                self.expect_punct("]")?;
                e = Expr::Index(Box::new(e), Box::new(idx));
                continue;
            }
            if self.at_punct("(") {
                let args = self.call_args()?;
                e = Expr::Call(Box::new(e), args);
                continue;
            }
            break;
        }
        Ok(e)
    }

    fn call_args(&mut self) -> PResult<Vec<Arg>> {
        self.expect_punct("(")?;
        let mut out = Vec::new();
        self.skip_newlines();
        while !self.at_punct(")") {
            if matches!(self.peek(), Tok::Ident(_)) && matches!(self.peek_at(1), Tok::Punct("=")) {
                let name = self.ident()?;
                self.bump();
                self.skip_newlines();
                out.push(Arg::Named(name, self.expr()?));
            } else {
                out.push(Arg::Pos(self.expr()?));
            }
            self.skip_newlines();
            if !self.eat_punct(",") {
                break;
            }
            self.skip_newlines();
        }
        self.skip_newlines();
        self.expect_punct(")")?;
        Ok(out)
    }

    fn primary(&mut self) -> PResult<Expr> {
        match self.bump() {
            Tok::Num(n) => Ok(Expr::Num(n)),
            Tok::Str(s) => {
                if s.contains('{') {
                    Ok(fstring(s))
                } else {
                    Ok(Expr::Str(s))
                }
            }
            Tok::Ident(name) => {
                if self.at_punct("=>") {
                    self.bump();
                    self.skip_newlines();
                    let body = self.expr()?;
                    Ok(Expr::Lambda(Box::new(Lambda {
                        params: vec![Param { name, default: None }],
                        body: Vec::new(),
                        expr_body: Some(Box::new(body)),
                    })))
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            Tok::Kw("true") => Ok(Expr::Bool(true)),
            Tok::Kw("false") => Ok(Expr::Bool(false)),
            Tok::Kw("null") => Ok(Expr::Null),
            Tok::Kw("fn") => Ok(Expr::Lambda(Box::new(self.lambda_body_as_block()?))),
            Tok::Punct("(") => {
                self.skip_newlines();
                let e = self.expr()?;
                self.skip_newlines();
                self.expect_punct(")")?;
                Ok(e)
            }
            Tok::Punct("[") => {
                let mut items = Vec::new();
                self.skip_newlines();
                while !self.at_punct("]") {
                    items.push(self.expr()?);
                    self.skip_newlines();
                    if !self.eat_punct(",") {
                        break;
                    }
                    self.skip_newlines();
                }
                self.skip_newlines();
                self.expect_punct("]")?;
                Ok(Expr::List(items))
            }
            Tok::Punct("{") => {
                let mut items = Vec::new();
                self.skip_newlines();
                while !self.at_punct("}") {
                    let key = match self.bump() {
                        Tok::Ident(k) => k,
                        Tok::Str(k) => k,
                        Tok::Num(n) => crate::value::fmt_num(n),
                        other => {
                            return Err(format!("line {}: bad dict key {}", self.line(), other))
                        }
                    };
                    if !self.eat_punct(":") {
                        return Err(format!(
                            "line {}: expected `:` after dict key `{}`",
                            self.line(),
                            key
                        ));
                    }
                    self.skip_newlines();
                    items.push((key, self.expr()?));
                    self.skip_newlines();
                    if !self.eat_punct(",") {
                        break;
                    }
                    self.skip_newlines();
                }
                self.skip_newlines();
                self.expect_punct("}")?;
                Ok(Expr::Dict(items))
            }
            other => Err(format!("line {}: unexpected {}", self.line(), other)),
        }
    }
}

fn fstring(s: String) -> Expr {
    let chars: Vec<char> = s.chars().collect();
    let mut parts = Vec::new();
    let mut lit = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '{' {
            if chars.get(i + 1) == Some(&'{') {
                lit.push('{');
                i += 2;
                continue;
            }
            let mut depth = 1;
            let mut j = i + 1;
            let mut inner = String::new();
            while j < chars.len() && depth > 0 {
                let d = chars[j];
                if d == '{' {
                    depth += 1;
                } else if d == '}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                inner.push(d);
                j += 1;
            }
            if !lit.is_empty() {
                parts.push(FPart::Lit(std::mem::take(&mut lit)));
            }
            let (code, spec) = split_spec(&inner);
            match crate::parser::parse_expr_str(&code) {
                Ok(e) => {
                    if spec.is_empty() {
                        parts.push(FPart::Expr(e));
                    } else {
                        parts.push(FPart::Spec(e, spec));
                    }
                }
                Err(_) => parts.push(FPart::Lit(inner)),
            }
            i = j + 1;
            continue;
        }
        if c == '}' && chars.get(i + 1) == Some(&'}') {
            lit.push('}');
            i += 2;
            continue;
        }
        lit.push(c);
        i += 1;
    }
    if !lit.is_empty() {
        parts.push(FPart::Lit(lit));
    }
    Expr::FString(parts)
}

fn split_spec(inner: &str) -> (String, String) {
    let chars: Vec<char> = inner.chars().collect();
    let mut depth = 0;
    let mut quote: Option<char> = None;
    let mut split: Option<usize> = None;
    for (i, c) in chars.iter().enumerate() {
        match quote {
            Some(q) => {
                if *c == q {
                    quote = None;
                }
            }
            None => match c {
                '\'' | '"' => quote = Some(*c),
                '[' | '(' | '{' => depth += 1,
                ']' | ')' | '}' => depth -= 1,
                ':' if depth == 0 => split = Some(i),
                _ => {}
            },
        }
    }
    match split {
        Some(i) => (chars[..i].iter().collect(), chars[i + 1..].iter().collect()),
        None => (inner.to_string(), String::new()),
    }
}
