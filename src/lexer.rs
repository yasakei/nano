use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    Kw(&'static str),
    Punct(&'static str),
    Newline,
    Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    pub line: usize,
    pub col: usize,
}

pub struct Lexer {
    src: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
    depth: usize,
}

pub type LexResult<T> = Result<T, String>;

const KEYWORDS: &[&str] = &[
    "let", "if", "else", "for", "in", "while", "true", "false", "null", "fn", "return", "and", "or",
    "not", "break", "continue",
];

const PUNCTS: &[&str] = &[
    "=>", "==", "!=", "<=", ">=", "+=", "-=", "*=", "/=", "->", "..", "+", "-", "*", "/", "%", "^",
    "(", ")", "[", "]", "{", "}", ",", ":", ".", ";", "=", "<", ">", "!", "@", "&", "|", "?",
];

impl Lexer {
    pub fn new(src: &str) -> Self {
        Lexer { src: src.chars().collect(), pos: 0, line: 1, col: 1, depth: 0 }
    }

    fn peek(&self) -> Option<char> {
        self.src.get(self.pos).copied()
    }

    fn peek_at(&self, n: usize) -> Option<char> {
        self.src.get(self.pos + n).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.src.get(self.pos).copied();
        if let Some(ch) = c {
            self.pos += 1;
            if ch == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
        c
    }

    pub fn tokenize(mut self) -> LexResult<Vec<Token>> {
        let mut out = Vec::new();
        loop {
            self.skip_trivia();
            let line = self.line;
            let col = self.col;
            let c = match self.peek() {
                None => {
                    out.push(Token { tok: Tok::Eof, line, col });
                    return Ok(out);
                }
                Some(c) => c,
            };
            if c == '\n' {
                self.bump();
                if self.depth == 0 {
                    out.push(Token { tok: Tok::Newline, line, col });
                }
                continue;
            }
            if c.is_ascii_digit() || (c == '.' && self.peek_at(1).map_or(false, |d| d.is_ascii_digit())) {
                let n = self.lex_number()?;
                out.push(Token { tok: Tok::Num(n), line, col });
                continue;
            }
            if c == '"' || c == '\'' {
                let s = self.lex_string(c)?;
                out.push(Token { tok: Tok::Str(s), line, col });
                continue;
            }
            if c == '`' {
                let s = self.lex_backtick()?;
                out.push(Token { tok: Tok::Str(s), line, col });
                continue;
            }
            if c.is_alphabetic() || c == '_' {
                let mut word = String::new();
                while let Some(ch) = self.peek() {
                    if ch.is_alphanumeric() || ch == '_' {
                        word.push(ch);
                        self.bump();
                    } else {
                        break;
                    }
                }
                let tok = match KEYWORDS.iter().find(|k| **k == word) {
                    Some(k) => Tok::Kw(k),
                    None => Tok::Ident(word),
                };
                out.push(Token { tok, line, col });
                continue;
            }
            let rest: String = self.src[self.pos..].iter().take(2).collect();
            let mut matched = None;
            for p in PUNCTS {
                if rest.starts_with(p) {
                    matched = Some(*p);
                    break;
                }
            }
            match matched {
                Some(p) => {
                    for _ in 0..p.chars().count() {
                        self.bump();
                    }
                    if p == "(" || p == "[" {
                        self.depth += 1;
                    } else if p == ")" || p == "]" {
                        self.depth = self.depth.saturating_sub(1);
                    }
                    out.push(Token { tok: Tok::Punct(p), line, col });
                }
                None => {
                    return Err(format!("line {}: unexpected character `{}`", line, c));
                }
            }
        }
    }

    fn skip_trivia(&mut self) {
        loop {
            match self.peek() {
                Some(' ') | Some('\t') | Some('\r') => {
                    self.bump();
                }
                Some('/') if self.peek_at(1) == Some('/') => {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                Some('#') => {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                Some('/') if self.peek_at(1) == Some('*') => {
                    self.bump();
                    self.bump();
                    while self.pos < self.src.len() {
                        if self.peek() == Some('*') && self.peek_at(1) == Some('/') {
                            self.bump();
                            self.bump();
                            break;
                        }
                        self.bump();
                    }
                }
                _ => break,
            }
        }
    }

    fn lex_number(&mut self) -> LexResult<f64> {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.bump();
            } else if c == '_' {
                self.bump();
            } else if c == '.' && self.peek_at(1).map_or(false, |d| d.is_ascii_digit() || d == '_') {
                s.push('.');
                self.bump();
            } else if (c == 'e' || c == 'E')
                && self
                    .peek_at(1)
                    .map_or(false, |d| d.is_ascii_digit() || d == '+' || d == '-')
            {
                s.push('e');
                self.bump();
                if let Some(d) = self.peek() {
                    if d == '+' || d == '-' {
                        s.push(d);
                        self.bump();
                    }
                }
            } else {
                break;
            }
        }
        let cleaned: String = s.chars().filter(|c| *c != '_').collect();
        cleaned
            .parse::<f64>()
            .map_err(|e| format!("line {}: bad number `{}` ({})", self.line, s, e))
    }

    fn lex_string(&mut self, quote: char) -> LexResult<String> {
        self.bump();
        let mut s = String::new();
        loop {
            let c = match self.bump() {
                None => return Err(format!("line {}: unterminated string", self.line)),
                Some(c) => c,
            };
            if c == quote {
                return Ok(s);
            }
            if c == '\\' {
                let e = self.bump().ok_or("unterminated escape")?;
                s.push(match e {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    '0' => '\0',
                    '\\' => '\\',
                    '"' => '"',
                    '\'' => '\'',
                    'e' => '\x1b',
                    other => other,
                });
                continue;
            }
            s.push(c);
        }
    }

    fn lex_backtick(&mut self) -> LexResult<String> {
        self.bump();
        let mut s = String::new();
        loop {
            let c = match self.bump() {
                None => return Err(format!("line {}: unterminated backtick string", self.line)),
                Some(c) => c,
            };
            if c == '`' {
                return Ok(s);
            }
            if c == '\\' {
                let e = self.bump().ok_or("unterminated escape")?;
                s.push(match e {
                    'n' => '\n',
                    't' => '\t',
                    '`' => '`',
                    '\\' => '\\',
                    other => other,
                });
                continue;
            }
            s.push(c);
        }
    }
}

impl fmt::Display for Tok {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Tok::Num(n) => write!(f, "number {}", n),
            Tok::Str(s) => write!(f, "string \"{}\"", s),
            Tok::Ident(s) => write!(f, "identifier `{}`", s),
            Tok::Kw(k) => write!(f, "keyword `{}`", k),
            Tok::Punct(p) => write!(f, "`{}`", p),
            Tok::Newline => write!(f, "end of line"),
            Tok::Eof => write!(f, "end of file"),
        }
    }
}
