#[derive(Clone, Debug)]
pub enum Expr {
    Num(f64),
    Str(String),
    Bool(bool),
    Null,
    Ident(String),
    List(Vec<Expr>),
    Dict(Vec<(String, Expr)>),
    Unary(&'static str, Box<Expr>),
    Binary(&'static str, Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Arg>),
    Index(Box<Expr>, Box<Expr>),
    Member(Box<Expr>, String),
    Lambda(Box<Lambda>),
    Range(Box<Expr>, Box<Expr>, Option<Box<Expr>>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    FString(Vec<FPart>),
}

#[derive(Clone, Debug)]
pub enum FPart {
    Lit(String),
    Expr(Expr),
    Spec(Expr, String),
}

#[derive(Clone, Debug)]
pub enum Arg {
    Pos(Expr),
    Named(String, Expr),
}

#[derive(Clone, Debug)]
pub struct Lambda {
    pub params: Vec<Param>,
    pub body: Vec<Stmt>,
    pub expr_body: Option<Box<Expr>>,
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: String,
    pub default: Option<Expr>,
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Let(String, Expr),
    Assign(Expr, &'static str, Expr),
    Expr(Expr),
    If(Expr, Vec<Stmt>, Option<Vec<Stmt>>),
    For(String, Expr, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    Return(Expr),
    Break,
    Continue,
    Block(Vec<Stmt>),
}

pub type PResult<T> = Result<T, String>;
