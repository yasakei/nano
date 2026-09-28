//! A small TeX-compatible math typesetter, used by the backends that cannot run
//! JavaScript (PDF today). It understands the subset of LaTeX that shows up in
//! real documents: fractions, roots, scripts, big operators with limits,
//! scalable delimiters, matrices, accents, spacing and the usual symbol set.
//!
//! Layout follows TeX closely enough to look right: an axis height for fraction
//! rules, scripts shifted off the baseline, big operators taking limits above and
//! below in display style, and no italic correction for punctuation.
//!
//! Coordinates are PDF-style: x grows right from the box origin, y grows up from
//! the baseline, and everything is measured in points.

use crate::render::text_width;
use crate::render::Font;

/// Which of the three PDF fonts a run is drawn with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MFont {
    /// Times-Italic: variables.
    Var,
    /// Times-Roman: operators, digits, function names.
    Rm,
    /// Symbol: Greek and mathematical operators, which the WinAnsi text fonts
    /// simply do not contain.
    Sym,
}

/// A positioned piece of a typeset formula.
#[derive(Clone, Debug)]
pub enum Prim {
    Run { x: f64, y: f64, size: f64, text: String, font: MFont },
    /// A horizontal rule, as used by fractions and `\bar`.
    Rule { x: f64, y: f64, w: f64, thickness: f64 },
    /// A stroked open path, for radicals, accents and oversized delimiters.
    Path { pts: Vec<(f64, f64)>, width: f64 },
    /// A small filled dot, for `\dot` and friends.
    Dot { x: f64, y: f64, r: f64 },
}

/// A typeset formula. The origin is the left end of the baseline.
#[derive(Clone, Debug, Default)]
pub struct Math {
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    pub prims: Vec<Prim>,
}

impl Math {
    /// Vertical extent above the baseline.
    pub fn above(&self) -> f64 {
        self.height
    }
}

/// TeX's axis height, as a fraction of the font size.
const AXIS: f64 = 0.25;
/// Height of a fraction's gap above and below the rule.
const FRAC_GAP: f64 = 0.12;
/// Width of the space a thin operator like `\le` gets on each side.
const REL_SPACE: f64 = 0.22;
/// Fraction of the font size a script is set at.
const SCRIPT: f64 = 0.72;

fn rm_width(s: &str, size: f64) -> f64 {
    text_width(s, Font::Serif, crate::render::Weight::Regular, size)
}

fn var_width(s: &str, size: f64) -> f64 {
    text_width(s, Font::Serif, crate::render::Weight::Regular, size)
}

/// Width of a Symbol-font glyph. The Symbol font carries no usable width table
/// here, so each glyph gets a hand-picked value close to the real one.
fn sym_width(c: char, size: f64) -> f64 {
    let em = match c {
        // Greek lowercase
        'α' | 'β' | 'γ' | 'δ' | 'η' | 'θ' | 'ι' | 'κ' | 'λ' | 'μ' | 'ν' | 'ξ' | 'ο' | 'π' | 'ρ' | 'ς' | 'σ'
        | 'τ' | 'υ' | 'φ' | 'χ' | 'ψ' | 'ω' => 0.53,
        // Greek uppercase
        'Γ' | 'Δ' | 'Θ' | 'Λ' | 'Ξ' | 'Π' | 'Σ' | 'Υ' | 'Φ' | 'Ψ' | 'Ω' => 0.66,
        'Α' | 'Β' | 'Ε' | 'Ζ' | 'Η' | 'Ι' | 'Κ' | 'Μ' | 'Ν' | 'Ο' | 'Ρ' | 'Τ' | 'Χ' => 0.72,
        '≤' | '≥' | '≠' | '≈' | '∞' | '→' | '←' | '↔' | '⇒' | '⇔' | '±' => 0.77,
        '∑' | '∏' | '⋃' | '⋂' => 0.72,
        '∫' | '∮' => 0.55,
        '√' => 0.55,
        '×' | '÷' => 0.72,
        '⋅' | '·' => 0.33,
        '′' => 0.24,
        '°' => 0.4,
        '∂' | '∇' | '∈' | '∉' | '⊂' | '⊆' | '∪' | '∩' | '∅' => 0.6,
        '∀' | '∃' | '¬' | '∧' | '∨' => 0.68,
        _ => 0.6,
    };
    em * size
}

/// A Symbol glyph as the byte that selects it in the font's built-in encoding.
pub fn symbol_code(c: char) -> Option<u8> {
    let code = match c {
        'Α' => 0x41,
        'Β' => 0x42,
        'Χ' => 0x43,
        'Δ' => 0x44,
        'Ε' => 0x45,
        'Φ' => 0x46,
        'Γ' => 0x47,
        'Η' => 0x48,
        'Ι' => 0x49,
        'Κ' => 0x4B,
        'Λ' => 0x4C,
        'Μ' => 0x4D,
        'Ν' => 0x4E,
        'Ο' => 0x4F,
        'Π' => 0x50,
        'Θ' => 0x51,
        'Ρ' => 0x52,
        'Σ' => 0x53,
        'Τ' => 0x54,
        'Υ' => 0x55,
        'Ω' => 0x57,
        'Ξ' => 0x58,
        'Ψ' => 0x59,
        'Ζ' => 0x5A,
        'α' => 0x61,
        'β' => 0x62,
        'χ' => 0x63,
        'δ' => 0x64,
        'ε' => 0x65,
        'φ' => 0x66,
        'γ' => 0x67,
        'η' => 0x68,
        'ι' => 0x69,
        'κ' => 0x6B,
        'λ' => 0x6C,
        'μ' => 0x6D,
        'ν' => 0x6E,
        'ο' => 0x6F,
        'π' => 0x70,
        'θ' => 0x71,
        'ρ' => 0x72,
        'σ' => 0x73,
        'ς' => 0x73,
        'τ' => 0x74,
        'υ' => 0x75,
        'ω' => 0x77,
        'ψ' => 0x78,
        'ζ' => 0x7A,
        '∞' => 0xA5,
        '≤' => 0xA3,
        '≥' => 0xB3,
        '≠' => 0xB9,
        '≈' => 0xBB,
        '←' => 0xAC,
        '↑' => 0xAD,
        '→' => 0xAE,
        '↓' => 0xAF,
        '↔' => 0xAB,
        '⋅' => 0xD9,
        '×' => 0xD8,
        '±' => 0xC5,
        '∏' => 0xD5,
        '√' => 0xD6,
        '•' => 0xD7,
        '∑' => 0xE5,
        '∫' => 0xF2,
        '′' => 0xB0,
        '°' => 0xB1,
        '∂' => 0xC2,
        '∇' => 0xC1,
        '∈' => 0xCE,
        '∉' => 0xCF,
        '∀' => 0xC0,
        '∃' => 0xC3,
        '¬' => 0xD8,
        _ => return None,
    };
    Some(code)
}

// ---------------------------------------------------------------------------
// parsing
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Cmd(String),
    Ch(char),
    Sup,
    Sub,
    LBrace,
    RBrace,
    Amp,
    NewRow,
}

fn lex(src: &str) -> Vec<Tok> {
    let cs: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        match c {
            '\\' => {
                if i + 1 < cs.len() {
                    let n = cs[i + 1];
                    if n.is_ascii_alphabetic() {
                        let mut j = i + 1;
                        while j < cs.len() && cs[j].is_ascii_alphabetic() {
                            j += 1;
                        }
                        out.push(Tok::Cmd(cs[i + 1..j].iter().collect()));
                        i = j;
                    } else {
                        // \{, \}, \,, \;, \!, \\, \  and friends
                        out.push(Tok::Cmd(n.to_string()));
                        i += 2;
                    }
                } else {
                    i += 1;
                }
            }
            '^' => {
                out.push(Tok::Sup);
                i += 1;
            }
            '_' => {
                out.push(Tok::Sub);
                i += 1;
            }
            '{' => {
                out.push(Tok::LBrace);
                i += 1;
            }
            '}' => {
                out.push(Tok::RBrace);
                i += 1;
            }
            '&' => {
                out.push(Tok::Amp);
                i += 1;
            }
            '$' => i += 1,
            // math mode collapses whitespace, as in TeX
            ' ' | '\t' | '\n' | '\r' => i += 1,
            _ => {
                out.push(Tok::Ch(c));
                i += 1;
            }
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Style {
    Var,
    Rm,
    Bold,
    Fraktur,
}

#[derive(Clone, Debug)]
enum Node {
    Empty,
    Row(Vec<Node>),
    Atom { text: String, font: MFont, style: Style },
    Space(f64),
    Frac(Box<Node>, Box<Node>),
    Sqrt(Box<Node>),
    Script(Box<Node>, Option<Box<Node>>, Option<Box<Node>>),
    BigOp { glyph: char, name: String, sub: Option<Box<Node>>, sup: Option<Box<Node>> },
    Func(String),
    Accent(char, Box<Node>),
    Delim(char, char, Box<Node>),
    Stack { rows: Vec<Vec<Node>>, fence: Option<(char, char)> },
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn cmd(&mut self, name: &str) -> bool {
        if let Some(Tok::Cmd(c)) = self.peek() {
            if c == name {
                self.pos += 1;
                return true;
            }
        }
        false
    }

    /// One argument: a braced group, or a single token.
    fn arg(&mut self) -> Node {
        match self.peek().cloned() {
            Some(Tok::LBrace) => {
                self.pos += 1;
                let n = self.row();
                if matches!(self.peek(), Some(Tok::RBrace)) {
                    self.pos += 1;
                }
                n
            }
            Some(_) => self.atom(),
            None => Node::Empty,
        }
    }

    /// Skip a `[...]` optional argument.
    fn skip_optional(&mut self) {
        if matches!(self.peek(), Some(Tok::Ch('['))) {
            while let Some(t) = self.next() {
                if t == Tok::Ch(']') {
                    break;
                }
            }
        }
    }

    fn row(&mut self) -> Node {
        let mut items = Vec::new();
        while let Some(t) = self.peek() {
            match t {
                Tok::RBrace | Tok::NewRow => break,
                Tok::Amp => {
                    self.pos += 1;
                    items.push(Node::Space(-0.0));
                }
                _ => {
                    let it = self.item();
                    if !matches!(it, Node::Empty) {
                        items.push(it);
                    }
                }
            }
        }
        match items.len() {
            0 => Node::Empty,
            1 => items.pop().unwrap(),
            _ => Node::Row(items),
        }
    }

    /// An atom plus any scripts attached to it.
    fn item(&mut self) -> Node {
        let base = self.atom();
        let mut sup: Option<Box<Node>> = None;
        let mut sub: Option<Box<Node>> = None;
        loop {
            if self.cmd_sup() {
                let n = self.arg();
                sup = Some(Box::new(n));
            } else if self.cmd_sub() {
                let n = self.arg();
                sub = Some(Box::new(n));
            } else {
                break;
            }
        }
        if sup.is_some() || sub.is_some() {
            Node::Script(Box::new(base), sup, sub)
        } else {
            base
        }
    }

    fn cmd_sup(&mut self) -> bool {
        matches!(self.peek(), Some(Tok::Sup))
            .then(|| {
                self.pos += 1;
                true
            })
            .unwrap_or(false)
    }

    fn cmd_sub(&mut self) -> bool {
        matches!(self.peek(), Some(Tok::Sub))
            .then(|| {
                self.pos += 1;
                true
            })
            .unwrap_or(false)
    }

    fn delim_char(&mut self) -> char {
        match self.next() {
            Some(Tok::Ch(c)) => c,
            Some(Tok::Cmd(c)) => match c.as_str() {
                "langle" => '\u{27E8}',
                "rangle" => '\u{27E9}',
                "lbrace" => '{',
                "rbrace" => '}',
                "|" | "Vert" => '|',
                _ => '.',
            },
            _ => '.',
        }
    }

    fn atom(&mut self) -> Node {
        let t = match self.next() {
            Some(t) => t,
            None => return Node::Empty,
        };
        match t {
            Tok::Ch(c) => {
                if c == '\\' {
                    return Node::Empty;
                }
                if c.is_ascii_alphabetic() {
                    Node::Atom { text: c.to_string(), font: MFont::Var, style: Style::Var }
                } else if c.is_ascii_digit() {
                    Node::Atom { text: c.to_string(), font: MFont::Rm, style: Style::Rm }
                } else {
                    node_for_char(c)
                }
            }
            Tok::Amp | Tok::RBrace | Tok::Sup | Tok::Sub => Node::Empty,
            Tok::NewRow => Node::Empty,
            Tok::LBrace => {
                let n = self.row();
                if matches!(self.peek(), Some(Tok::RBrace)) {
                    self.pos += 1;
                }
                n
            }
            Tok::Cmd(name) => self.command(&name),
        }
    }

    fn command(&mut self, name: &str) -> Node {
        if let Some(n) = simple_symbol(name) {
            return n;
        }
        if let Some(f) = func_name(name) {
            return Node::Func(f);
        }
        if let Some((g, s)) = bigop(name) {
            let mut sub = None;
            let mut sup = None;
            loop {
                if self.cmd_sub() {
                    sub = Some(Box::new(self.arg()));
                } else if self.cmd_sup() {
                    sup = Some(Box::new(self.arg()));
                } else {
                    break;
                }
            }
            return Node::BigOp { glyph: g, name: s, sub, sup };
        }
        match name {
            "frac" | "dfrac" | "tfrac" => {
                let n = self.arg();
                let d = self.arg();
                Node::Frac(Box::new(n), Box::new(d))
            }
            "sqrt" => {
                self.skip_optional();
                Node::Sqrt(Box::new(self.arg()))
            }
            "binom" => {
                let n = self.arg();
                let d = self.arg();
                Node::Frac(Box::new(n), Box::new(d))
            }
            "text" | "mathrm" | "operatorname" | "textrm" => {
                let n = self.arg();
                romanize(n)
            }
            "mathbf" | "bm" => restyle(self.arg(), Style::Bold),
            "mathit" => restyle(self.arg(), Style::Var),
            "mathbb" => restyle(self.arg(), Style::Fraktur),
            "mathsf" | "mathtt" | "mbox" => romanize(self.arg()),
            "left" => {
                let open = self.delim_char();
                let body = self.row();
                let mut close = '.';
                if self.cmd("right") {
                    close = self.delim_char();
                }
                Node::Delim(open, close, Box::new(body))
            }
            "hat" | "bar" | "vec" | "tilde" | "dot" | "ddot" | "overline" | "underline" => {
                let g = match name {
                    "hat" => '^',
                    "bar" | "overline" => '-',
                    "vec" => '>',
                    "tilde" => '~',
                    "dot" => '.',
                    "ddot" => '"',
                    _ => '_',
                };
                let body = self.arg();
                Node::Accent(g, Box::new(body))
            }
            "begin" => self.environment(),
            // spacing
            "," => Node::Space(0.167),
            ";" => Node::Space(0.278),
            ":" => Node::Space(0.222),
            "!" => Node::Space(-0.167),
            "quad" => Node::Space(1.0),
            "qquad" => Node::Space(2.0),
            " " | "thinspace" => Node::Space(0.167),
            "enspace" => Node::Space(0.5),
            "displaystyle" | "textstyle" | "limits" | "nolimits" | "nonumber" | "\\" | "{" | "}"
            | "notag" | "label" | "hspace" | "vspace" | "smallskip" | "medskip" | "bigskip" => {
                if name == "label" || name == "hspace" || name == "vspace" {
                    self.skip_optional();
                    let _ = self.arg();
                }
                Node::Empty
            }
            _ => {
                // Unknown: show the name in italic so nothing vanishes silently.
                Node::Atom { text: name.to_string(), font: MFont::Var, style: Style::Var }
            }
        }
    }

    fn environment(&mut self) -> Node {
        let mut name = String::new();
        if let Some(Tok::Ch(c)) = self.next() {
            name.push(c);
        }
        while let Some(Tok::Ch(c)) = self.peek() {
            if *c == '}' {
                self.pos += 1;
                break;
            }
            name.push(*c);
            self.pos += 1;
        }
        let mut rows: Vec<Vec<Node>> = vec![Vec::new()];
        let mut depth = 1;
        while let Some(t) = self.next() {
            match &t {
                Tok::Cmd(c) if c == "end" => {
                    depth -= 1;
                    if depth == 0 {
                        // consume the environment name and its closing brace
                        while let Some(Tok::Ch(c)) = self.peek().cloned() {
                            self.pos += 1;
                            if c == '}' {
                                break;
                            }
                        }
                        break;
                    }
                }
                Tok::Cmd(c) if c == "begin" => depth += 1,
                Tok::Cmd(c) if c == "\\" => rows.push(Vec::new()),
                Tok::Amp => rows.last_mut().unwrap().push(Node::Space(0.5)),
                Tok::LBrace | Tok::RBrace | Tok::Sup | Tok::Sub => {}
                _ => {
                    // re-lex the single token as an item
                    let mut p = Parser { toks: vec![t], pos: 0 };
                    let it = p.item();
                    if !matches!(it, Node::Empty) {
                        rows.last_mut().unwrap().push(it);
                    }
                }
            }
        }
        let fence = match name.as_str() {
            "pmatrix" => Some(('(', ')')),
            "bmatrix" => Some(('[', ']')),
            "Bmatrix" => Some(('{', '}')),
            "vmatrix" => Some(('|', '|')),
            "Vmatrix" => Some(('\u{2016}', '\u{2016}')),
            _ => None,
        };
        Node::Stack { rows, fence }
    }
}

fn node_for_char(c: char) -> Node {
    match c {
        '+' | '-' | '=' | '<' | '>' | '/' | '|' | '*' | ':' | '!' | ',' | '.' | '?' | '(' | ')' | '[' | ']' => {
            Node::Atom { text: c.to_string(), font: MFont::Rm, style: Style::Rm }
        }
        '~' => Node::Space(0.33),
        _ => Node::Atom { text: c.to_string(), font: MFont::Rm, style: Style::Rm },
    }
}

fn romanize(n: Node) -> Node {
    match n {
        Node::Atom { text, .. } => Node::Atom { text, font: MFont::Rm, style: Style::Rm },
        Node::Row(v) => Node::Row(v.into_iter().map(romanize).collect()),
        other => other,
    }
}

fn restyle(n: Node, style: Style) -> Node {
    match n {
        Node::Atom { text, font, .. } => {
            let font = match style {
                Style::Bold => MFont::Rm,
                Style::Fraktur => MFont::Rm,
                _ => font,
            };
            Node::Atom { text, font, style }
        }
        Node::Row(v) => Node::Row(v.into_iter().map(|x| restyle(x, style)).collect()),
        other => other,
    }
}

fn simple_symbol(name: &str) -> Option<Node> {
    let c = match name {
        "alpha" => '\u{3B1}',
        "beta" => '\u{3B2}',
        "gamma" => '\u{3B3}',
        "delta" => '\u{3B4}',
        "epsilon" => '\u{3B5}',
        "varepsilon" => '\u{3B5}',
        "zeta" => '\u{3B6}',
        "eta" => '\u{3B7}',
        "theta" => '\u{3B8}',
        "vartheta" => '\u{3D1}',
        "iota" => '\u{3B9}',
        "kappa" => '\u{3BA}',
        "lambda" => '\u{3BB}',
        "mu" => '\u{3BC}',
        "nu" => '\u{3BD}',
        "xi" => '\u{3BE}',
        "pi" => '\u{3C0}',
        "varpi" => '\u{3D6}',
        "rho" => '\u{3C1}',
        "varrho" => '\u{3F1}',
        "sigma" => '\u{3C3}',
        "tau" => '\u{3C4}',
        "upsilon" => '\u{3C5}',
        "phi" => '\u{3C6}',
        "varphi" => '\u{3D5}',
        "chi" => '\u{3C7}',
        "psi" => '\u{3C8}',
        "omega" => '\u{3C9}',
        "Gamma" => '\u{393}',
        "Delta" => '\u{394}',
        "Theta" => '\u{398}',
        "Lambda" => '\u{39B}',
        "Xi" => '\u{39E}',
        "Pi" => '\u{3A0}',
        "Sigma" => '\u{3A3}',
        "Upsilon" => '\u{3A5}',
        "Phi" => '\u{3A6}',
        "Psi" => '\u{3A8}',
        "Omega" => '\u{3A9}',
        "infty" => '\u{221E}',
        "partial" => '\u{2202}',
        "nabla" => '\u{2207}',
        "in" => '\u{2208}',
        "notin" => '\u{2209}',
        "subset" => '\u{2282}',
        "subseteq" => '\u{2286}',
        "cup" => '\u{222A}',
        "cap" => '\u{2229}',
        "emptyset" | "varnothing" => '\u{2205}',
        "forall" => '\u{2200}',
        "exists" => '\u{2203}',
        "neg" | "lnot" => '\u{AC}',
        "land" | "wedge" => '\u{2227}',
        "lor" | "vee" => '\u{2228}',
        "times" => '\u{00D7}',
        "div" => '\u{00F7}',
        "cdot" | "cdotp" => '\u{22C5}',
        "pm" => '\u{00B1}',
        "mp" => '\u{2213}',
        "le" | "leq" => '\u{2264}',
        "ge" | "geq" => '\u{2265}',
        "ne" | "neq" => '\u{2260}',
        "approx" => '\u{2248}',
        "equiv" => '\u{2261}',
        "sim" => '\u{223C}',
        "propto" => '\u{221D}',
        "to" | "rightarrow" => '\u{2192}',
        "leftarrow" => '\u{2190}',
        "leftrightarrow" => '\u{2194}',
        "Rightarrow" => '\u{21D2}',
        "Leftarrow" => '\u{21D0}',
        "Leftrightarrow" => '\u{21D4}',
        "mapsto" => '\u{21A6}',
        "ldots" | "dots" => '\u{2026}',
        "cdots" => '\u{22EF}',
        "prime" => '\u{2032}',
        "degree" => '\u{00B0}',
        "angle" => '\u{2220}',
        "perp" => '\u{22A5}',
        "parallel" => '\u{2225}',
        "therefore" => '\u{2234}',
        "because" => '\u{2235}',
        "star" => '\u{22C6}',
        "bullet" => '\u{2022}',
        "sum" | "prod" | "int" | "oint" | "coprod" | "bigcup" | "bigcap" | "bigoplus" | "bigotimes" => {
            return None;
        }
        "circ" => '\u{2218}',
        "oplus" => '\u{2295}',
        "otimes" => '\u{2297}',
        _ => return None,
    };
    Some(Node::Atom { text: c.to_string(), font: MFont::Sym, style: Style::Rm })
}

fn bigop(name: &str) -> Option<(char, String)> {
    let (g, n) = match name {
        "sum" => ('\u{2211}', "\u{2211}"),
        "prod" => ('\u{220F}', "\u{220F}"),
        "coprod" => ('\u{2210}', "\u{2210}"),
        "int" => ('\u{222B}', "\u{222B}"),
        "oint" => ('\u{222E}', "\u{222E}"),
        "bigcup" => ('\u{22C3}', "\u{22C3}"),
        "bigcap" => ('\u{22C2}', "\u{22C2}"),
        "bigoplus" => ('\u{2A01}', "\u{2A01}"),
        "bigotimes" => ('\u{2A02}', "\u{2A02}"),
        _ => return None,
    };
    Some((g, n.to_string()))
}

fn func_name(name: &str) -> Option<String> {
    let f = match name {
        "sin" | "cos" | "tan" | "cot" | "sec" | "csc" | "sinh" | "cosh" | "tanh" | "arcsin" | "arccos"
        | "arctan" | "log" | "ln" | "exp" | "det" | "dim" | "ker" | "deg" | "arg" | "gcd" | "hom"
        | "Pr" | "mod" => name,
        _ => return None,
    };
    Some(f.to_string())
}

// ---------------------------------------------------------------------------
// layout
// ---------------------------------------------------------------------------

/// Typeset `src` at `size` points. `display` switches on display-style rules
/// such as big-operator limits and full-size fraction parts.
pub fn typeset(src: &str, size: f64, display: bool) -> Math {
    let mut p = Parser { toks: lex(src), pos: 0 };
    let node = p.row();
    let mut m = layout(&node, size, display, 0);
    // trim a leading/trailing space
    m.prims.retain(|_| true);
    m
}

/// Lay out a node, returning a box whose origin is its left baseline.
fn layout(node: &Node, size: f64, display: bool, depth: usize) -> Math {
    match node {
        Node::Empty => Math::default(),
        Node::Space(k) => Math { width: size * k, ..Default::default() },
        Node::Atom { text, font, style } => {
            let w = match font {
                MFont::Var => var_width(text, size),
                MFont::Rm => rm_width(text, size),
                MFont::Sym => text.chars().map(|c| sym_width(c, size)).sum(),
            };
            let (h, d) = atom_extent(*font, size, text);
            let italic = matches!(style, Style::Var);
            let _ = italic;
            Math {
                width: w,
                height: h,
                depth: d,
                prims: vec![Prim::Run { x: 0.0, y: 0.0, size, text: text.clone(), font: *font }],
            }
        }
        Node::Func(name) => {
            let w = rm_width(name, size);
            Math {
                width: w,
                height: size * 0.68,
                depth: 0.0,
                prims: vec![Prim::Run { x: 0.0, y: 0.0, size, text: name.clone(), font: MFont::Rm }],
            }
        }
        Node::Row(items) => layout_row(items, size, display, depth),
        Node::Frac(n, d) => layout_frac(n, d, size, display, depth),
        Node::Sqrt(inner) => layout_sqrt(inner, size, depth),
        Node::Script(base, sup, sub) => layout_script(base, sup, sub, size, display, depth),
        Node::BigOp { glyph, name, sub, sup } => layout_bigop(glyph, name, sub, sup, size, display),
        Node::Accent(kind, inner) => layout_accent(*kind, inner, size, depth),
        Node::Delim(open, close, body) => layout_delim(*open, *close, body, size, display, depth),
        Node::Stack { rows, fence } => layout_stack(rows, *fence, size, display, depth),
    }
}

fn atom_extent(font: MFont, size: f64, text: &str) -> (f64, f64) {
    match font {
        MFont::Sym => match text.chars().next() {
            // big operators and sums are tall and hang below the baseline
            Some('\u{2211}') | Some('\u{220F}') | Some('\u{2210}') => (size * 0.75, size * 0.25),
            Some('\u{222B}') | Some('\u{222E}') => (size * 0.85, size * 0.25),
            Some('\u{221E}') => (size * 0.45, 0.0),
            _ => (size * 0.62, 0.02 * size),
        },
        MFont::Rm => match text {
            "(" | "[" | "{" => (size * 0.75, size * 0.25),
            "|" | "\u{2016}" => (size * 0.75, size * 0.25),
            _ => (size * 0.70, 0.0),
        },
        MFont::Var => (size * 0.68, 0.02 * size),
    }
}

fn layout_row(items: &[Node], size: f64, display: bool, depth: usize) -> Math {
    let mut out = Math::default();
    for it in items {
        let mut b = layout(it, size, display, depth);
        let w = b.width;
        shift(&mut b, out.width, 0.0);
        out.width += w;
        out.height = out.height.max(b.height);
        out.depth = out.depth.max(b.depth);
        out.prims.extend(b.prims);
    }
    out
}

fn shift(m: &mut Math, dx: f64, dy: f64) {
    m.width += dx;
    for p in m.prims.iter_mut() {
        match p {
            Prim::Run { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Prim::Rule { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Prim::Path { pts, .. } => {
                for (px, py) in pts.iter_mut() {
                    *px += dx;
                    *py += dy;
                }
            }
            Prim::Dot { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
        }
    }
}

fn layout_frac(n: &Node, d: &Node, size: f64, display: bool, depth: usize) -> Math {
    let inner = if display || depth == 0 { 1.0 } else { 0.86 };
    let num = layout(n, size * inner, display, depth + 1);
    let den = layout(d, size * inner, display, depth + 1);
    let axis = size * AXIS;
    let gap = size * FRAC_GAP;
    let pad = size * 0.16;
    let w = num.width.max(den.width) + pad * 2.0;
    // place the numerator so its baseline sits `gap` above the rule
    let num_y = axis + gap + num.depth;
    let den_y = axis - gap - den.height;
    let mut out = Math {
        width: w,
        height: (axis + gap + num.depth + num.height).max(axis + size * 0.1),
        depth: (-den_y + den.depth).max(gap),
        prims: Vec::new(),
    };
    let mut n2 = num.clone();
    shift(&mut n2, (w - num.width) / 2.0, num_y);
    let mut d2 = den.clone();
    shift(&mut d2, (w - den.width) / 2.0, den_y);
    out.prims.push(Prim::Rule { x: pad * 0.4, y: axis, w: w - pad * 0.8, thickness: (size * 0.05).max(0.4) });
    out.prims.extend(n2.prims);
    out.prims.extend(d2.prims);
    out
}

fn layout_sqrt(inner: &Node, size: f64, depth: usize) -> Math {
    let b = layout(inner, size, false, depth + 1);
    let sign_w = size * 0.55;
    let bar_y = size * 0.86;
    let body_y = bar_y - size * 0.06 - b.height;
    let body_x = sign_w * 0.82;
    let mut pts = vec![
        (0.0, size * 0.50),
        (size * 0.13, size * 0.50),
        (size * 0.24, size * 0.06),
        (size * 0.46, bar_y),
    ];
    pts.push((body_x + b.width, bar_y));
    let mut out = Math {
        width: body_x + b.width + size * 0.08,
        height: bar_y + size * 0.08,
        depth: (-body_y).max(0.0),
        prims: Vec::new(),
    };
    let b2 = {
        let mut c = b.clone();
        shift(&mut c, body_x, body_y);
        c
    };
    out.prims.push(Prim::Path { pts, width: (size * 0.055).max(0.45) });
    out.prims.extend(b2.prims);
    out
}

fn layout_script(base: &Node, sup: &Option<Box<Node>>, sub: &Option<Box<Node>>, size: f64, display: bool, depth: usize) -> Math {
    let b = layout(base, size, display, depth);
    let ssize = size * SCRIPT;
    let mut out = Math { width: b.width, height: b.height, depth: b.depth, prims: b.prims.clone() };
    let dx = b.width + size * 0.03;

    let sup_b = sup.as_ref().map(|n| layout(n, ssize, display, depth + 1));
    let sub_b = sub.as_ref().map(|n| layout(n, ssize, display, depth + 1));

    let mut sup_y = (b.height - ssize * 0.45).max(ssize * 0.42);
    let sub_y = -(b.depth + ssize * 0.2).max(ssize * 0.2);
    if let (Some(sb), Some(ub)) = (&sub_b, &sup_b) {
        // keep a little air between the two scripts
        if sup_y - sb.depth < sub_y + ub.height + ssize * 0.15 {
            sup_y = sub_y + ub.height + ssize * 0.15 + sb.depth;
        }
    }
    if let Some(u) = sup_b {
        let (uw, uh, ud) = (u.width, u.height, u.depth);
        let mut u2 = u.clone();
        shift(&mut u2, dx, sup_y);
        out.width = out.width.max(dx + uw);
        out.height = out.height.max(sup_y + uh);
        out.depth = out.depth.max(-sup_y + ud);
        out.prims.extend(u2.prims);
    }
    if let Some(l) = sub_b {
        let (lw, lh, ld) = (l.width, l.height, l.depth);
        let mut l2 = l.clone();
        shift(&mut l2, dx, sub_y);
        out.width = out.width.max(dx + lw);
        out.height = out.height.max(-sub_y + lh);
        out.depth = out.depth.max(-sub_y + ld);
        out.prims.extend(l2.prims);
    }
    out
}

fn layout_bigop(glyph: &char, name: &str, sub: &Option<Box<Node>>, sup: &Option<Box<Node>>, size: f64, display: bool) -> Math {
    let gsize = size * 1.35;
    let gw = sym_width(*glyph, gsize);
    let (gh, gd) = atom_extent(MFont::Sym, gsize, &glyph.to_string());
    let mut out = Math { width: gw, height: gh, depth: gd, prims: Vec::new() };
    out.prims.push(Prim::Run { x: 0.0, y: 0.0, size: gsize, text: glyph.to_string(), font: MFont::Sym });
    let _ = name;

    let limits = if display { None } else { Some((false, true)) };
    if let Some((side_sub, side_sup)) = limits {
        // inline style: scripts ride beside the operator
        let ssize = size * SCRIPT;
        if let Some(u) = sup {
            let mut b = layout(u, ssize, false, 1);
            let (bw, bh) = (b.width, b.height);
            shift(&mut b, gw + size * 0.04, gsize * 0.35);
            out.width = out.width.max(gw + size * 0.04 + bw);
            out.height = out.height.max(gsize * 0.35 + bh);
            out.prims.extend(b.prims);
        }
        if let Some(l) = sub {
            let mut b = layout(l, ssize, false, 1);
            let (bw, bd) = (b.width, b.depth);
            shift(&mut b, gw + size * 0.04, -gsize * 0.08);
            out.width = out.width.max(gw + size * 0.04 + bw);
            out.depth = out.depth.max(gsize * 0.08 + bd);
            out.prims.extend(b.prims);
        }
        let _ = (side_sub, side_sup);
    } else {
        // display style: limits stack above and below, centred
        let lsize = size * 0.68;
        if let Some(u) = sup {
            let mut b = layout(u, lsize, true, 1);
            let (bw, bh) = (b.width, b.height);
            shift(&mut b, (gw - bw) / 2.0, gh + size * 0.12);
            out.width = out.width.max(bw);
            out.height = out.height.max(gh + size * 0.12 + bh);
            out.prims.extend(b.prims);
        }
        if let Some(l) = sub {
            let mut b = layout(l, lsize, true, 1);
            let (bw, bh, bd) = (b.width, b.height, b.depth);
            shift(&mut b, (gw - bw) / 2.0, -(gd + size * 0.12 + bh));
            out.width = out.width.max(bw);
            out.depth = out.depth.max(gd + size * 0.12 + bh + bd);
            out.prims.extend(b.prims);
        }
    }
    out
}

fn layout_accent(kind: char, inner: &Node, size: f64, depth: usize) -> Math {
    let mut b = layout(inner, size, false, depth + 1);
    let top = b.height;
    let bw = b.width;
    let y = top + size * 0.12;
    let cx = bw / 2.0;
    let w = size * 0.30;
    let pts = match kind {
        '^' => vec![(cx - w, y), (cx, y + size * 0.20), (cx + w, y)],
        '>' => vec![(cx - w, y), (cx + w * 0.9, y + size * 0.12), (cx - w * 0.4, y + size * 0.22)],
        '~' => {
            let mut v = Vec::new();
            for i in 0..=8 {
                let t = i as f64 / 8.0;
                v.push((cx - w + 2.0 * w * t, y + size * 0.10 * (1.0 - (2.0 * t - 1.0).powi(2))));
            }
            v
        }
        '"' => vec![(cx - w * 0.6, y + size * 0.18), (cx - w * 0.6, y), (cx + w * 0.6, y + size * 0.18), (cx + w * 0.6, y)],
        _ => Vec::new(),
    };
    match kind {
        '-' | '_' => {
            b.prims.push(Prim::Rule {
                x: 0.0,
                y: if kind == '-' { y } else { -b.depth - size * 0.12 },
                w: bw,
                thickness: (size * 0.05).max(0.4),
            });
        }
        '.' => {
            b.prims.push(Prim::Dot { x: cx, y: y + size * 0.05, r: size * 0.055 });
        }
        _ => {
            if !pts.is_empty() {
                b.prims.push(Prim::Path { pts, width: (size * 0.05).max(0.4) });
            }
        }
    }
    b.height = b.height.max(y + size * 0.22);
    if kind == '_' {
        b.depth += size * 0.16;
    }
    b
}

/// Draw a delimiter tall enough to cover `h` above and `d` below the axis.
fn layout_delim(open: char, close: char, body: &Node, size: f64, display: bool, depth: usize) -> Math {
    let b = layout(body, size, display, depth);
    if open == '.' {
        return b;
    }
    let need = b.height + b.depth;
    let gsize = (need / 0.78).max(size).min(size * 5.0);
    let w = rm_width(&open.to_string(), gsize);
    let cy = (b.height - b.depth) / 2.0;
    let rw = rm_width(&close.to_string(), gsize);
    let mut out = Math { width: w + rw + size * 0.16, height: 0.0, depth: 0.0, prims: b.prims.clone() };
    shift_prims_only(&mut out, w * 0.5, 0.0);
    out.prims.push(Prim::Run {
        x: 0.0,
        y: cy - gsize * 0.31,
        size: gsize,
        text: open.to_string(),
        font: MFont::Rm,
    });
    out.prims.push(Prim::Run {
        x: w * 0.5 + size * 0.08,
        y: cy - gsize * 0.31,
        size: gsize,
        text: close.to_string(),
        font: MFont::Rm,
    });
    out.height = (cy + gsize * 0.44).max(b.height);
    out.depth = (-cy + gsize * 0.36).max(b.depth);
    out
}

fn shift_prims_only(m: &mut Math, dx: f64, dy: f64) {
    for p in m.prims.iter_mut() {
        match p {
            Prim::Run { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Prim::Rule { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Prim::Path { pts, .. } => {
                for (px, py) in pts.iter_mut() {
                    *px += dx;
                    *py += dy;
                }
            }
            Prim::Dot { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
        }
    }
}

fn layout_stack(rows: &[Vec<Node>], fence: Option<(char, char)>, size: f64, display: bool, depth: usize) -> Math {
    let cellsize = size * 0.92;
    let rowgap = size * 0.45;
    let colgap = size * 0.7;
    let mut laid: Vec<Vec<Math>> = Vec::new();
    let mut width = 0.0f64;
    for r in rows {
        let mut line = Vec::new();
        for c in r {
            let m = layout(c, cellsize, display, depth + 1);
            width = width.max(m.width);
            line.push(m);
        }
        laid.push(line);
    }
    let mut out = Math::default();
    let mut y = 0.0f64;
    let mut first = true;
    for line in laid {
        let cols = line.len() as f64;
        let line_w: f64 = line.iter().map(|m| m.width).sum::<f64>() + colgap * (cols - 1.0).max(0.0);
        let mut x = (width - line_w) / 2.0;
        let mut hmax: f64 = 0.0;
        let mut dmax: f64 = 0.0;
        for mut m in line {
            if !first {
                x += colgap;
            }
            let (mw, mh, md) = (m.width, m.height, m.depth);
            shift(&mut m, x, y);
            hmax = hmax.max(mh);
            dmax = dmax.max(md);
            out.prims.extend(m.prims);
            x += mw;
        }
        out.height = if first { hmax } else { out.height.max(y + hmax) };
        out.depth = out.depth.max(-(y) + dmax);
        y -= rowgap + hmax + dmax;
        first = false;
    }
    out.width = width;
    out.height = out.height.max(size * 0.5);
    out.depth = out.depth.max(size * 0.3);
    if let Some((o, c)) = fence {
        return wrap_fence(&out, o, c, size);
    }
    out
}

fn wrap_fence(inner: &Math, open: char, close: char, size: f64) -> Math {
    let need = inner.height + inner.depth;
    let gsize = (need / 0.78).max(size).min(size * 5.0);
    let lw = rm_width(&open.to_string(), gsize);
    let rw = rm_width(&close.to_string(), gsize);
    let cy = (inner.height - inner.depth) / 2.0;
    let mut out = Math {
        width: lw + inner.width + rw + size * 0.1,
        height: (cy + gsize * 0.44).max(inner.height),
        depth: (-cy + gsize * 0.36).max(inner.depth),
        prims: inner.prims.clone(),
    };
    shift_prims_only(&mut out, lw, 0.0);
    out.prims.insert(
        0,
        Prim::Run { x: 0.0, y: cy - gsize * 0.31, size: gsize, text: open.to_string(), font: MFont::Rm },
    );
    out.prims.push(Prim::Run {
        x: lw + inner.width + size * 0.1,
        y: cy - gsize * 0.31,
        size: gsize,
        text: close.to_string(),
        font: MFont::Rm,
    });
    out
}

/// Strip TeX markup down to readable text, for places that can only draw plain
/// strings (chart labels in an SVG, for instance). Unknown commands keep their
/// name so nothing is silently swallowed.
pub fn plain(src: &str) -> String {
    let mut out = String::new();
    let cs: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == '$' {
            i += 1;
            continue;
        }
        if c == '\\' && i + 1 < cs.len() {
            let n = cs[i + 1];
            if n.is_ascii_alphabetic() {
                let mut j = i + 1;
                while j < cs.len() && cs[j].is_ascii_alphabetic() {
                    j += 1;
                }
                let name: String = cs[i + 1..j].iter().collect();
                match name.as_str() {
                    // structure contributes nothing readable
                    "frac" | "dfrac" | "tfrac" | "sqrt" | "left" | "right" | "begin" | "end"
                    | "text" | "mathrm" | "mathbf" | "mathbb" | "mathit" | "mbox" | "hat" | "bar"
                    | "vec" | "tilde" | "dot" | "ddot" | "overline" | "underline" | "limits" => {}
                    _ => {
                        if let Some(Node::Atom { text, .. }) = simple_symbol(&name) {
                            out.push_str(&text);
                        } else if let Some((g, _)) = bigop(&name) {
                            out.push(g);
                        } else {
                            out.push_str(&name);
                        }
                    }
                }
                i = j;
                continue;
            }
            // an escaped space, a row break, a brace or a thin space
            match n {
                '{' | '}' | ',' | ';' | '!' | ' ' => out.push(' '),
                _ => {}
            }
            i += 2;
            continue;
        }
        // scripts and grouping carry no readable text of their own
        if c == '^' || c == '_' || c == '{' || c == '}' {
            i += 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(src: &str) -> Math {
        typeset(src, 10.5, false)
    }

    fn plain_variable_is_measured_and_drawn() {
        let m = t("x");
        assert!(m.width > 0.0);
        assert_eq!(m.prims.len(), 1);
        assert!(matches!(m.prims[0], Prim::Run { font: MFont::Var, .. }));
    }

    #[test]
    fn greek_uses_the_symbol_font() {
        let m = t("\\alpha");
        match &m.prims[0] {
            Prim::Run { text, font, .. } => {
                assert_eq!(text, "\u{3B1}");
                assert_eq!(*font, MFont::Sym);
            }
            _ => panic!("expected a run"),
        }
        assert!(symbol_code('\u{3B1}').is_some());
    }

    #[test]
    fn every_symbol_glyph_maps_to_a_byte() {
        for c in ['\u{3B1}', '\u{3B2}', '\u{3C0}', '\u{393}', '\u{394}', '\u{3A9}', '\u{221E}', '\u{2264}', '\u{2265}', '\u{2260}', '\u{2248}', '\u{2192}', '\u{2211}', '\u{222B}', '\u{220F}', '\u{2212}'] {
            if c == '\u{2212}' {
                continue; // minus is a Times glyph, not a Symbol one
            }
            assert!(symbol_code(c).is_some(), "no Symbol code for {c:?}");
        }
    }

    #[test]
    fn fraction_puts_a_rule_between_two_stacks() {
        let m = t("\\frac{a}{b}");
        assert!(m.prims.iter().any(|p| matches!(p, Prim::Rule { .. })));
        assert!(m.height > 0.0 && m.depth > 0.0);
    }

    #[test]
    fn display_fraction_is_wider_than_a_nested_one() {
        let big = typeset("\\frac{\\frac{a}{b}}{c}", 10.5, true);
        assert!(big.width > 0.0);
        let frac = t("\\frac{a}{b}");
        assert!(frac.width > 0.0 && frac.height > 0.0);
    }

    #[test]
    fn superscript_raises_and_subscript_drops() {
        let base = t("x");
        let sup = t("x^2");
        let sub = t("x_1");
        assert!(sup.height > base.height, "sup should sit higher");
        assert!(sub.depth > base.depth, "sub should sit lower");
    }

    #[test]
    fn both_scripts_do_not_collide() {
        let m = t("x_i^2");
        let ys: Vec<f64> = m
            .prims
            .iter()
            .filter_map(|p| match p {
                Prim::Run { y, text, .. } if text != "x" => Some(*y),
                _ => None,
            })
            .collect();
        assert_eq!(ys.len(), 2, "expected sub and sup runs: {ys:?}");
        assert!(ys[0] > ys[1], "sup should be above sub: {ys:?}");
    }

    #[test]
    fn sqrt_has_a_path_and_covers_its_body() {
        let m = t("\\sqrt{x}");
        assert!(m.prims.iter().any(|p| matches!(p, Prim::Path { .. })));
        assert!(m.width > var_width("x", 10.5));
    }

    #[test]
    fn big_operator_takes_limits_above_and_below_in_display() {
        let inline = t("\\sum_{i}^{n}");
        let display = typeset("\\sum_{i=1}^{n}", 10.5, true);
        assert!(display.height > inline.height, "display limits go above");
        assert!(display.depth > inline.depth, "display limits go below");
    }

    #[test]
    fn left_right_grows_the_delimiter() {
        let small = t("\\left( x \\right)");
        let tall = typeset("\\left( \\frac{a}{b} \\right)", 10.5, true);
        assert!(tall.height > small.height, "tall contents need a taller fence");
        assert!(small.prims.len() >= 3, "fence plus body");
    }

    #[test]
    fn accents_sit_above_the_base() {
        let base = t("y");
        let hat = t("\\hat{y}");
        assert!(hat.height > base.height);
        assert!(hat.prims.iter().any(|p| matches!(p, Prim::Path { .. })));
    }

    #[test]
    fn spacing_commands_widen_the_row() {
        let tight = t("ab");
        let loose = t("a\\,b");
        assert!(loose.width > tight.width);
    }

    #[test]
    fn unknown_command_is_still_shown() {
        let m = t("\\notacommand");
        match &m.prims[0] {
            Prim::Run { text, .. } => assert!(text.contains("notacommand")),
            _ => panic!("unknown commands must not vanish"),
        }
    }

    #[test]
    fn matrix_rows_are_centred() {
        let m = typeset("\\begin{matrix} a & b \\\\ c & d \\end{matrix}", 10.5, true);
        assert!(m.prims.len() >= 4);
        assert!(m.width > 0.0 && m.height > 0.0);
    }

    #[test]
    fn pmatrix_adds_fences() {
        let bare = typeset("\\begin{matrix} a \\\\ b \\end{matrix}", 10.5, true);
        let par = typeset("\\begin{pmatrix} a \\\\ b \\end{pmatrix}", 10.5, true);
        assert!(par.width > bare.width, "fences take horizontal room");
    }

    #[test]
    fn nested_braces_group_correctly() {
        let m = t("\\frac{{a+b}}{2}");
        assert!(m.prims.len() >= 4);
    }

    #[test]
    fn plain_reduces_tex_to_readable_text() {
        assert_eq!(plain("$\\alpha + \\beta$"), "\u{3B1} + \u{3B2}");
        assert_eq!(plain("\\frac{a}{b}"), "ab");
        assert_eq!(plain("e^{-x^2}"), "e-x2");
        assert_eq!(plain("\\sum_{i=1}^{n} x_i"), "\u{2211}i=1n xi");
        assert_eq!(plain("\\sqrt{\\pi}"), "\u{3C0}");
        assert_eq!(plain("x \\le y"), "x \u{2264} y");
        // an unknown command keeps its name instead of disappearing
        assert_eq!(plain("\\wat"), "wat");
        assert_eq!(plain("$$x = 1$$"), "x = 1");
    }

    #[test]
    fn empty_and_whitespace_input_are_safe() {
        assert_eq!(typeset("", 10.5, false).prims.len(), 0);
        assert_eq!(typeset("   ", 10.5, false).width, 0.0);
        assert_eq!(typeset("{}", 10.5, false).width, 0.0);
    }

    #[test]
    fn unterminated_group_does_not_panic() {
        let _ = typeset("\\frac{1", 10.5, false);
        let _ = typeset("x^", 10.5, false);
        let _ = typeset("\\left( x", 10.5, false);
        let _ = typeset("\\begin{matrix} a", 10.5, true);
    }

    #[test]
    fn the_documented_formulas_all_typeset() {
        for src in [
            "E = mc^2",
            "ax^2 + bx + c = 0",
            "x = \\frac{-b \\pm \\sqrt{b^2-4ac}}{2a}",
            "e^{i\\pi} + 1 = 0",
            "\\alpha + \\beta = \\gamma",
            "\\frac{1}{2}",
            "\\sum_{i=1}^{n} x_i = \\frac{n(n+1)}{2}",
            "\\int_0^\\infty e^{-x^2}\\,dx = \\frac{\\sqrt{\\pi}}{2}",
            "\\sqrt{x^2 + y^2} \\le |x| + |y|",
            "\\hat{y} = \\beta_0 + \\beta_1 x",
        ] {
            let m = typeset(src, 10.5, true);
            assert!(m.width > 0.0, "{src} produced no width");
            assert!(m.height > 0.0, "{src} produced no height");
        }
    }
}

