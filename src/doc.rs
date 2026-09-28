use std::path::{Path, PathBuf};

use crate::builtins;
use crate::interp::Interp;
use crate::plot::PlotSpec;
use crate::value::{fmt_num, Value};

#[derive(Clone, Debug, Default)]
pub struct Meta {
    pub title: String,
    pub subtitle: String,
    pub author: Vec<String>,
    pub date: String,
    pub description: String,
    pub theme: String,
    pub keywords: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum Inline {
    Text(String),
    Code(String),
    Strong(Vec<Inline>),
    Em(Vec<Inline>),
    Strike(Vec<Inline>),
    Link { text: Vec<Inline>, href: String },
    Image { alt: String, src: String },
    Math(String),
    Interp(String),
    Break,
    Ref(String),
}

impl Inline {
    pub fn plain(&self) -> String {
        match self {
            Inline::Text(t) | Inline::Code(t) | Inline::Math(t) => t.clone(),
            Inline::Strong(v) | Inline::Em(v) | Inline::Strike(v) => {
                v.iter().map(|i| i.plain()).collect()
            }
            Inline::Link { text, .. } => text.iter().map(|i| i.plain()).collect(),
            Inline::Image { alt, .. } => alt.clone(),
            Inline::Interp(_) | Inline::Break => String::new(),
            Inline::Ref(r) => format!("[{}]", r),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct CellResult {
    pub stdout: String,
    pub value: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
    pub align: Vec<Align>,
    pub caption: String,
    pub zebra: bool,
    pub max_rows: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Video,
    Audio,
    Embed,
    Iframe,
}

#[derive(Clone, Debug)]
pub struct Media {
    pub kind: MediaKind,
    pub src: String,
    pub alt: String,
    pub caption: String,
    pub width: Option<String>,
    pub poster: Option<String>,
    pub autoplay: bool,
    pub loop_media: bool,
    pub muted: bool,
    pub controls: bool,
    pub float: Option<String>,
    pub attrs: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub struct Note {
    pub kind: String,
    pub title: String,
    pub body: Vec<Inline>,
    /// Set when nano itself generated this panel because something failed, so
    /// `nano check` can report it instead of only showing it in the output.
    pub error: bool,
}

#[derive(Clone, Debug)]
pub struct Widget {
    pub kind: String,
    pub name: String,
    pub label: String,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub value: f64,
    pub options: Vec<String>,
    pub checked: bool,
}

#[derive(Clone, Debug)]
pub enum Block {
    Heading { level: u8, inlines: Vec<Inline>, id: String },
    Para(Vec<Inline>),
    List { ordered: bool, start: u32, items: Vec<Vec<Inline>> },
    Checklist { items: Vec<(bool, Vec<Inline>)> },
    Quote(Vec<Inline>),
    Rule,
    Code { lang: String, text: String, runnable: bool, result: Option<CellResult> },
    Plot(Box<PlotBlock>),
    Table(Table),
    Media(Media),
    Math { display: bool, text: String },
    Note(Note),
    Widget(Widget),
    Toc { title: String },
    Raw(String),
    PageBreak,
    Directive(Raw),
}

#[derive(Clone, Debug)]
pub struct Raw {
    pub name: String,
    pub head: String,
    pub lines: Vec<String>,
    pub body: String,
}

#[derive(Clone, Debug)]
pub struct PlotBlock {
    pub spec: PlotSpec,
    pub caption: String,
    pub id: String,
}

#[derive(Clone, Debug, Default)]
pub struct Doc {
    pub meta: Meta,
    pub blocks: Vec<Block>,
    pub base_dir: PathBuf,
    pub source: String,
    pub name: String,
}

impl Doc {
    pub fn resolve(&self, rel: &str) -> PathBuf {
        if rel.starts_with("http://") || rel.starts_with("https://") || rel.starts_with("data:") {
            return PathBuf::from(rel);
        }
        let p = Path::new(rel);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.base_dir.join(p)
        }
    }

    pub fn headings(&self) -> Vec<(u8, String, String)> {
        let mut out = Vec::new();
        for b in &self.blocks {
            if let Block::Heading { level, inlines, id } = b {
                out.push((*level, inlines.iter().map(|i| i.plain()).collect(), id.clone()));
            }
        }
        out
    }
}

pub fn parse_document(src: &str, base_dir: &Path) -> Doc {
    let mut doc = Doc {
        meta: Meta::default(),
        blocks: Vec::new(),
        base_dir: base_dir.to_path_buf(),
        source: src.to_string(),
        name: "Untitled".into(),
    };
    let body = front_matter(src, &mut doc.meta);
    if doc.meta.title.is_empty() {
        doc.name = "Untitled".to_string();
    } else {
        doc.name = doc.meta.title.clone();
    }
    doc.blocks = parse_blocks(body);
    doc
}

fn front_matter<'a>(src: &'a str, meta: &mut Meta) -> &'a str {
    let trimmed = src.trim_start_matches('\u{feff}');
    if !trimmed.starts_with("---") {
        return src;
    }
    let rest = &trimmed[3..];
    let rest = rest.trim_start_matches('\r').trim_start_matches('\n');
    let end = match rest.find("\n---") {
        Some(k) => k,
        None => return src,
    };
    let block = &rest[..end];
    let after = &rest[end + 4..];
    let after = after.trim_start_matches(['-', '\r', '\n']);
    for line in block.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (k, v) = match line.split_once(':') {
            Some((k, v)) => (k.trim().to_lowercase(), v.trim().trim_matches('"').to_string()),
            None => continue,
        };
        match k.as_str() {
            "title" => meta.title = v,
            "subtitle" => meta.subtitle = v,
            "author" | "authors" => {
                meta.author = v
                    .split(',')
                    .map(|s| s.trim().trim_matches('"').to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            }
            "date" => meta.date = v,
            "description" | "abstract" => meta.description = v,
            "theme" => meta.theme = v,
            "keywords" => {
                meta.keywords = v
                    .split(',')
                    .map(|s| s.trim().trim_matches('"').to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            }
            _ => {}
        }
    }
    after
}

struct Cursor<'a> {
    lines: Vec<&'a str>,
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn peek(&self) -> Option<&'a str> {
        self.lines.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<&'a str> {
        let l = self.peek();
        if l.is_some() {
            self.pos += 1;
        }
        l
    }

    fn blank(&self) -> bool {
        self.peek().map_or(true, |l| l.trim().is_empty())
    }
}

fn is_directive_start(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with('@') && t[1..].chars().next().map_or(false, |c| c.is_alphabetic())
}

fn parse_blocks(body: &str) -> Vec<Block> {
    let mut cur = Cursor { lines: body.lines().collect(), pos: 0 };
    let mut out: Vec<Block> = Vec::new();
    let mut para: Vec<&str> = Vec::new();

    fn flush<'a>(para: &mut Vec<&'a str>, out: &mut Vec<Block>) {
        if !para.is_empty() {
            let text = para.join("\n");
            out.push(Block::Para(inline_parse(&text)));
            para.clear();
        }
    }

    while let Some(line) = cur.next() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            flush(&mut para, &mut out);
            continue;
        }
        if is_directive_start(line) {
            flush(&mut para, &mut out);
            if let Some(b) = parse_directive(&mut cur, line) {
                out.push(b);
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("```") {
            flush(&mut para, &mut out);
            let info = rest.trim().to_string();
            let mut code = String::new();
            while let Some(l) = cur.next() {
                if l.trim_start().starts_with("```") {
                    break;
                }
                code.push_str(l);
                code.push('\n');
            }
            let runnable = info.is_empty()
                || info.starts_with("nano")
                || info.starts_with("run")
                || info.starts_with("{");
            out.push(Block::Code {
                lang: info.trim_start_matches('{').trim_end_matches('}').to_string(),
                text: code.trim_end_matches('\n').to_string(),
                runnable,
                result: None,
            });
            continue;
        }
        if trimmed.starts_with("$$") {
            // `$$ ... $$` on its own is a display formula, and may span lines
            flush(&mut para, &mut out);
            let mut body = trimmed.trim_start_matches('$').to_string();
            let mut closed = trimmed.trim_end().ends_with("$$") && trimmed.trim().len() > 4;
            if closed {
                body = trimmed.trim().trim_start_matches('$').trim_end_matches('$').to_string();
            } else {
                while let Some(l) = cur.next() {
                    let t = l.trim();
                    if t.ends_with("$$") {
                        body.push('\n');
                        body.push_str(t.trim_end_matches('$'));
                        closed = true;
                        break;
                    }
                    body.push('\n');
                    body.push_str(t);
                }
            }
            let _ = closed;
            out.push(Block::Math { display: true, text: body.trim().to_string() });
            continue;
        }
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            flush(&mut para, &mut out);
            out.push(Block::Rule);
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix('#') {
            flush(&mut para, &mut out);
            let level = (rest.chars().take_while(|c| *c == '#').count() + 1).min(6) as u8;
            let text = rest.trim_start_matches('#').trim();
            let (id_flag, text) = match text.split_once(" {#") {
                Some((t, id)) => (format!("{{#{}}}", id.trim_end_matches('}')), t.trim().to_string()),
                None => (String::new(), text.to_string()),
            };
            let inlines = inline_parse(&text);
            let id = if id_flag.is_empty() {
                slugify(&inlines.iter().map(|i| i.plain()).collect::<String>())
            } else {
                id_flag.trim_start_matches("{#").trim_end_matches('}').to_string()
            };
            out.push(Block::Heading { level, inlines, id });
            continue;
        }
        if trimmed.starts_with("> ") || trimmed == ">" {
            flush(&mut para, &mut out);
            let mut q = vec![trimmed.trim_start_matches('>').trim().to_string()];
            while let Some(l) = cur.peek() {
                if l.trim_start().starts_with('>') {
                    q.push(l.trim_start().trim_start_matches('>').trim().to_string());
                    cur.next();
                } else {
                    break;
                }
            }
            out.push(Block::Quote(inline_parse(&q.join("\n"))));
            continue;
        }
        if let Some(kind) = list_marker(trimmed) {
            flush(&mut para, &mut out);
            let (ordered, start, _, item) = kind;
            let mut items = vec![inline_parse(&item)];
            while let Some(l) = cur.peek() {
                if l.trim().is_empty() {
                    if let Some(nextl) = cur.lines.get(cur.pos + 1) {
                        if list_marker(nextl.trim()).is_some() {
                            break;
                        }
                    }
                    break;
                }
                match list_marker(l.trim()) {
                    Some((o2, _, _, item2)) if o2 == ordered => {
                        cur.next();
                        items.push(inline_parse(&item2));
                    }
                    Some(_) => break,
                    None => {
                        if l.starts_with("  ") {
                            cur.next();
                            if let Some(last) = items.last_mut() {
                                let extra = inline_parse(l.trim());
                                last.extend(extra);
                            }
                        } else {
                            break;
                        }
                    }
                }
            }
            let is_check = !ordered
                && !items.is_empty()
                && items.iter().all(|it| {
                    let t = it.iter().map(|i| i.plain()).collect::<String>();
                    let t = t.trim_start();
                    t.starts_with("[ ]") || t.starts_with("[x]") || t.starts_with("[X]")
                });
            if is_check {
                let checks = items
                    .into_iter()
                    .map(|it| {
                        let text = it.iter().map(|i| i.plain()).collect::<String>();
                        let t = text.trim_start();
                        let done = t.starts_with("[x]") || t.starts_with("[X]");
                        let rest = t
                            .trim_start_matches("[ ]")
                            .trim_start_matches("[x]")
                            .trim_start_matches("[X]")
                            .trim_start();
                        (done, inline_parse(rest))
                    })
                    .collect();
                out.push(Block::Checklist { items: checks });
                continue;
            }
            out.push(Block::List { ordered, start, items });
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix(":::") {
            flush(&mut para, &mut out);
            out.push(Block::Raw(rest.trim().to_string()));
            continue;
        }
        para.push(line);
    }
    flush(&mut para, &mut out);
    out
}

type Marker = (bool, u32, usize, String);

fn list_marker(t: &str) -> Option<Marker> {
    if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
        return Some((false, 0, 2, t[2..].to_string()));
    }
    let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    if !digits.is_empty() {
        let rest = &t[digits.len()..];
        if let Some(after) = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")) {
            return Some((true, digits.parse().unwrap_or(1), digits.len() + 2, after.to_string()));
        }
    }
    None
}

pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if c == ' ' || c == '-' || c == '_' {
            if !out.ends_with('-') {
                out.push('-');
            }
        }
    }
    out.trim_matches('-').to_string()
}

pub fn inline_parse(text: &str) -> Vec<Inline> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut lit = String::new();
    let mut i = 0;

    macro_rules! flush {
        () => {
            if !lit.is_empty() {
                out.push(Inline::Text(std::mem::take(&mut lit)));
            }
        };
    }

    while i < chars.len() {
        let c = chars[i];
        let prev = if i > 0 { chars[i - 1] } else { '\0' };
        if c == '\\' && i + 1 < chars.len() && "\\`*_[]()#!$~|{".contains(chars[i + 1]) {
            lit.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if c == '\n' && chars.get(i + 1) == Some(&'\n') {
            flush!();
            out.push(Inline::Break);
            i += 2;
            while i < chars.len() && chars[i] == '\n' {
                i += 1;
            }
            continue;
        }
        if c == '`' {
            if let Some(end) = find_from(&chars, i + 1, '`') {
                flush!();
                out.push(Inline::Code(chars[i + 1..end].iter().collect()));
                i = end + 1;
                continue;
            }
        }
        if c == '$' {
            if let Some(end) = find_from(&chars, i + 1, '$') {
                let inner: String = chars[i + 1..end].iter().collect();
                if !inner.contains('\n') {
                    flush!();
                    out.push(Inline::Math(inner));
                    i = end + 1;
                    continue;
                }
            }
        }
        if (c == '{' && chars.get(i + 1) == Some(&'{')) && prev != '$' {
            if let Some(end) = find_seq(&chars, i + 2, "}}") {
                flush!();
                out.push(Inline::Interp(chars[i + 2..end].iter().collect()));
                i = end + 2;
                continue;
            }
        }
        if c == '!' && chars.get(i + 1) == Some(&'[') {
            if let Some((alt, target, next)) = parse_link(&chars, i + 1) {
                flush!();
                out.push(Inline::Image { alt, src: target });
                i = next;
                continue;
            }
        }
        if c == '[' {
            if chars.get(i + 1) == Some(&'[') {
                if let Some(end) = find_seq(&chars, i + 2, "]]") {
                    flush!();
                    out.push(Inline::Ref(chars[i + 2..end].iter().collect()));
                    i = end + 2;
                    continue;
                }
            }
            if let Some((label, target, next)) = parse_link(&chars, i) {
                flush!();
                out.push(Inline::Link { text: inline_parse(&label), href: target });
                i = next;
                continue;
            }
        }
        if (c == '*' || c == '_') && chars.get(i + 1) == Some(&c) && !is_space(prev) && !lit.ends_with('*') {
            if let Some(end) = find_pair(&chars, i + 2, c) {
                flush!();
                out.push(Inline::Strong(inline_parse(&chars[i + 2..end].iter().collect::<String>())));
                i = end + 2;
                continue;
            }
        }
        if c == '~' && chars.get(i + 1) == Some(&'~') {
            if let Some(end) = find_pair(&chars, i + 2, '~') {
                flush!();
                out.push(Inline::Strike(inline_parse(&chars[i + 2..end].iter().collect::<String>())));
                i = end + 2;
                continue;
            }
        }
        if (c == '*' || c == '_') && !is_space(prev) && !is_space(chars.get(i + 1).copied().unwrap_or(' ')) {
            if let Some(end) = find_pair(&chars, i + 1, c) {
                flush!();
                out.push(Inline::Em(inline_parse(&chars[i + 1..end].iter().collect::<String>())));
                i = end + 1;
                continue;
            }
        }
        if c == 'h' && starts_with(&chars, i, "http://") || c == 'h' && starts_with(&chars, i, "https://") {
            let mut j = i;
            while j < chars.len() && !chars[j].is_whitespace() && !"()[]".contains(chars[j]) {
                j += 1;
            }
            let url: String = chars[i..j].iter().collect();
            let trimmed = url.trim_end_matches(['.', ',', ';', ':', '"', '\'']);
            flush!();
            out.push(Inline::Link {
                text: vec![Inline::Text(trimmed.to_string())],
                href: trimmed.to_string(),
            });
            i += trimmed.chars().count();
            continue;
        }
        lit.push(c);
        i += 1;
    }
    flush!();
    out
}

fn is_space(c: char) -> bool {
    c == ' ' || c == '\n' || c == '\t'
}

fn starts_with(chars: &[char], i: usize, pat: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    if i + p.len() > chars.len() {
        return false;
    }
    chars[i..i + p.len()] == p[..]
}

fn find_from(chars: &[char], from: usize, target: char) -> Option<usize> {
    (from..chars.len()).find(|k| chars[*k] == target)
}

fn find_seq(chars: &[char], from: usize, pat: &str) -> Option<usize> {
    (from..chars.len()).find(|k| starts_with(chars, *k, pat))
}

fn find_pair(chars: &[char], from: usize, target: char) -> Option<usize> {
    let mut k = from;
    while k < chars.len() {
        if chars[k] == '\\' {
            k += 2;
            continue;
        }
        if chars[k] == target && k + 1 < chars.len() && chars[k + 1] == target {
            return Some(k);
        }
        k += 1;
    }
    None
}

fn parse_link(chars: &[char], start: usize) -> Option<(String, String, usize)> {
    if chars.get(start) != Some(&'[') {
        return None;
    }
    let mut depth = 0;
    let mut label_end = None;
    for k in start..chars.len() {
        match chars[k] {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    label_end = Some(k);
                    break;
                }
            }
            _ => {}
        }
    }
    let label_end = label_end?;
    if chars.get(label_end + 1) != Some(&'(') {
        return None;
    }
    let mut par = 0;
    let mut target_end = None;
    for k in label_end + 1..chars.len() {
        match chars[k] {
            '(' => par += 1,
            ')' => {
                par -= 1;
                if par == 0 {
                    target_end = Some(k);
                    break;
                }
            }
            _ => {}
        }
    }
    let target_end = target_end?;
    let label: String = chars[start + 1..label_end].iter().collect();
    let target: String = chars[label_end + 2..target_end].iter().collect();
    Some((label, target.trim().to_string(), target_end + 1))
}

fn is_named_arg(t: &str) -> bool {
    if let Some((k, _)) = t.split_once('=') {
        let k = k.trim();
        return !k.is_empty()
            && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !k.chars().next().map_or(true, |c| c.is_ascii_digit());
    }
    if t.ends_with(':') {
        let k = t[..t.len() - 1].trim();
        return !k.is_empty()
            && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !k.starts_with("http");
    }
    false
}

fn is_block_start(line: &str) -> bool {
    let t = line.trim();
    is_directive_start(line)
        || t.starts_with("```")
        || t.starts_with('#')
        || t.starts_with('>')
        || t.starts_with(":::")
        || t == "---"
        || t == "***"
        || t == "___"
        || list_marker(t).is_some()
}

fn arg_lines<'a>(cur: &mut Cursor<'a>, first: &str, multiline: bool) -> (Vec<String>, String) {
    let mut args = Vec::new();
    if !first.trim().is_empty() {
        args.push(first.trim().to_string());
    }
    let mut body = String::new();
    while let Some(l) = cur.peek() {
        if l.trim().is_empty() {
            if !multiline {
                break;
            }
            let next = cur.lines.get(cur.pos + 1).copied().unwrap_or("");
            if next.trim().is_empty() {
                break;
            }
            if is_block_start(next) {
                break;
            }
            let indented = next.starts_with("    ") || next.starts_with('\t');
            if body.is_empty() {
                cur.next();
                continue;
            }
            if !indented {
                break;
            }
            body.push('\n');
            cur.next();
            continue;
        }
        if is_block_start(l) {
            break;
        }
        let t = l.trim();
        if is_named_arg(t) {
            args.push(t.to_string());
        } else {
            body.push_str(t);
            body.push('\n');
        }
        cur.next();
    }
    (args, body.trim().to_string())
}

pub struct ArgSet {
    pub named: Vec<(String, String)>,
    pub positional: Vec<String>,
}

pub fn parse_args(lines: &[String]) -> ArgSet {
    let mut named = Vec::new();
    let mut positional = Vec::new();
    for l in lines {
        if let Some((k, v)) = l.split_once('=') {
            let k = k.trim().to_lowercase();
            if !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                named.push((k, v.trim().to_string()));
                continue;
            }
        }
        if let Some(k) = l.strip_suffix(':') {
            if k.trim().chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !k.trim().is_empty() {
                named.push((k.trim().to_lowercase(), String::new()));
                continue;
            }
        }
        positional.push(l.clone());
    }
    ArgSet { named, positional }
}

fn looks_like_literal(src: &str) -> bool {
    let t = src.trim();
    if t.is_empty() {
        return true;
    }
    if t.chars().count() >= 2 {
        let b = t.as_bytes();
        if (b[0] == b'"' && b[b.len() - 1] == b'"') || (b[0] == b'\'' && b[b.len() - 1] == b'\'') {
            return true;
        }
    }
    if t.contains("://") || t.starts_with('/') || t.starts_with("./") || t.starts_with("../") {
        return true;
    }
    if t.chars().any(|c| c.is_whitespace()) {
        return false;
    }
    !t.chars().any(|c| "()[]{}=+*^<>,!?\"'`$@".contains(c))
}

const STRICT_KEYS: &[&str] = &["x", "y", "rows", "columns", "domain", "labels", "options", "f", "formula", "var", "y2", "n"];

fn eval_strict(interp: &mut Interp, src: &str, errs: &mut Vec<String>) -> Option<Value> {
    match crate::parser::parse_expr_str(src) {
        Ok(e) => match interp.eval(&e) {
            Ok(v) => Some(v),
            Err(msg) => {
                errs.push(format!("`{}` ({})", src.trim(), msg));
                None
            }
        },
        Err(msg) => {
            errs.push(format!("`{}` ({})", src.trim(), msg));
            None
        }
    }
}

fn eval_checked(interp: &mut Interp, src: &str, errs: &mut Vec<String>) -> Option<Value> {
    match crate::parser::parse_expr_str(src) {
        Ok(e) => match interp.eval(&e) {
            Ok(v) => Some(v),
            Err(msg) => {
                if looks_like_literal(src) {
                    Some(Value::str(strip_quotes(src)))
                } else {
                    errs.push(format!("`{}` ({})", src.trim(), msg));
                    None
                }
            }
        },
        Err(msg) => {
            if looks_like_literal(src) {
                Some(Value::str(strip_quotes(src)))
            } else {
                errs.push(format!("`{}` ({})", src.trim(), msg));
                None
            }
        }
    }
}

fn eval_arg(interp: &mut Interp, src: &str) -> Value {
    let mut errs = Vec::new();
    eval_checked(interp, src, &mut errs).unwrap_or_else(|| Value::Null)
}

pub fn strip_quotes(s: &str) -> String {
    let t = s.trim();
    if t.len() >= 2 {
        let b = t.as_bytes();
        if (b[0] == b'"' && b[t.len() - 1] == b'"') || (b[0] == b'\'' && b[t.len() - 1] == b'\'') {
            return t[1..t.len() - 1].to_string();
        }
    }
    t.to_string()
}

fn str_of(v: &Value) -> String {
    v.to_display()
}

fn num_of(v: &Value) -> Option<f64> {
    v.num()
}

fn parse_directive(cur: &mut Cursor, line: &str) -> Option<Block> {
    let t = line.trim_start().trim_start_matches('@');
    let (name, rest) = match t.split_once(char::is_whitespace) {
        Some((n, r)) => (n.to_lowercase(), r.trim().to_string()),
        None => (t.to_lowercase(), String::new()),
    };
    let multiline = matches!(
        name.as_str(),
        "note" | "callout" | "tip" | "warning" | "info" | "math" | "eq" | "equation" | "raw" | "html"
    );
    let (lines, body) = arg_lines(cur, &rest, multiline);
    Some(Block::Directive(Raw { name, head: rest, lines, body }))
}

struct Args<'a> {
    set: &'a ArgSet,
    ip: &'a mut Interp,
    errs: Vec<String>,
}

impl<'a> Args<'a> {
    fn get(&mut self, k: &str) -> Option<Value> {
        let v = self.set.named.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
        match v {
            Some(src) if STRICT_KEYS.contains(&k) => eval_strict(self.ip, &src, &mut self.errs),
            Some(src) => eval_checked(self.ip, &src, &mut self.errs),
            None => None,
        }
    }

    fn pos(&mut self, i: usize) -> Option<Value> {
        let v = self.set.positional.get(i).cloned();
        v.and_then(|src| eval_checked(self.ip, &src, &mut self.errs))
    }

    fn raw(&self, k: &str) -> Option<String> {
        self.set.named.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone())
    }

    fn lit(&mut self, k: &str) -> Option<String> {
        self.raw(k).map(|v| strip_quotes(&v))
    }

    fn num(&mut self, k: &str, d: f64) -> f64 {
        self.get(k).and_then(|v| num_of(&v)).unwrap_or(d)
    }

    fn text(&mut self, k: &str, d: &str) -> String {
        self.get(k).map(|v| str_of(&v)).unwrap_or_else(|| d.to_string())
    }

    fn flag(&mut self, k: &str, d: bool) -> bool {
        self.get(k).map(|v| v.truthy()).unwrap_or(d)
    }

    fn src(&mut self) -> String {
        for k in ["src", "url", "path"] {
            if let Some((_, v)) = self.set.named.iter().find(|(n, _)| n == k) {
                let v = v.clone();
                return eval_checked(self.ip, &v, &mut self.errs).map(|x| x.to_display()).unwrap_or_default();
            }
        }
        self.pos(0).map(|x| x.to_display()).unwrap_or_default()
    }
}

fn build_directive(raw: &Raw, ip: &mut Interp) -> Block {
    let name = raw.name.clone();
    let body = raw.body.clone();
    let set = parse_args(&raw.lines);
    let mut a = Args { set: &set, ip, errs: Vec::new() };
    let _ = a.pos(0);

    let block = match name.as_str() {
        "plot" | "chart" | "figure" | "graph" => {
            let kind = a.pos(0).map(|v| v.to_display()).unwrap_or_else(|| "line".into());
            let mut spec = PlotSpec::from_args(&kind, &set, a.ip, &mut a.errs);
            spec.caption = a.text("caption", "");
            Block::Plot(Box::new(PlotBlock { spec, caption: String::new(), id: String::new() }))
        }
        "table" => {
            let columns = a
                .get("columns")
                .map(|v| v.items().iter().map(|x| str_of(&x)).collect::<Vec<_>>())
                .unwrap_or_default();
            let rows = a
                .get("rows")
                .map(|v| {
                    v.items()
                        .into_iter()
                        .map(|r| r.as_list().unwrap_or_else(|| vec![r]))
                        .collect()
                })
                .unwrap_or_default();
            let align = match a.raw("align") {
                Some(spec) => spec
                    .split(',')
                    .map(|x| match x.trim().to_lowercase().as_str() {
                        "r" | "right" => Align::Right,
                        "c" | "center" => Align::Center,
                        _ => Align::Left,
                    })
                    .collect(),
                None => Vec::new(),
            };
            let caption = a.text("caption", "");
            let zebra = a.flag("zebra", true);
            let max_rows = a.get("max_rows").and_then(|v| num_of(&v)).map(|x| x as usize);
            Block::Table(Table { columns, rows, align, caption, zebra, max_rows })
        }
        "image" | "img" | "video" | "audio" | "embed" | "iframe" => {
            let kind = match name.as_str() {
                "image" | "img" => MediaKind::Image,
                "video" => MediaKind::Video,
                "audio" => MediaKind::Audio,
                _ => MediaKind::Embed,
            };
            let src = a.src();
            let alt = a.text("alt", "");
            let caption = a.text("caption", "");
            let width = a.lit("width");
            let poster = a.lit("poster");
            let autoplay = a.flag("autoplay", false);
            let loop_media = a.flag("loop", false);
            let muted = a.flag("muted", false);
            let controls = a.flag("controls", true);
            let float = a.lit("float");
            Block::Media(Media {
                kind,
                src,
                alt,
                caption,
                width,
                poster,
                autoplay,
                loop_media,
                muted,
                controls,
                float,
                attrs: Vec::new(),
            })
        }
        "math" | "eq" | "equation" => Block::Math {
            display: !body.is_empty(),
            text: if body.is_empty() { set.positional.join(" ") } else { body },
        },
        "note" | "callout" | "tip" | "warning" | "info" => {
            let kind = if name == "note" || name == "callout" {
                match a.lit("kind") {
                    Some(k) if !k.trim().is_empty() => k.trim().to_lowercase(),
                    _ => a
                        .pos(0)
                        .map(|v| v.to_display().trim().to_lowercase())
                        .filter(|k| !k.is_empty())
                        .unwrap_or_else(|| "note".to_string()),
                }
            } else {
                name.clone()
            };
            let title = a.text("title", "");
            let joined = if body.is_empty() {
                title.clone()
            } else if title.is_empty() {
                body.clone()
            } else {
                format!("{}\n{}", title, body)
            };
            Block::Note(Note { kind, title, body: inline_parse(&joined), error: false })
        }
        "widget" | "control" | "slider" => {
            let kind = if name == "slider" {
                "slider".to_string()
            } else {
                a.pos(0).map(|v| v.to_display()).unwrap_or_else(|| "slider".into())
            };
            let var = a.text("name", "x");
            let min = a.num("min", 0.0);
            let max = a.num("max", 1.0);
            let fallback = if kind == "slider" { (min + max) / 2.0 } else { 0.0 };
            let value = a.get("value").and_then(|v| num_of(&v)).unwrap_or(fallback);
            let label = a.text("label", "");
            let step = a.num("step", 1.0);
            let options = a
                .get("options")
                .map(|v| v.items().iter().map(str_of).collect())
                .unwrap_or_default();
            let checked = a.flag("checked", false);
            Block::Widget(Widget { kind, name: var, label, min, max, step, value, options, checked })
        }
        "toc" => Block::Toc { title: a.text("title", "Contents") },
        "pagebreak" | "newpage" => Block::PageBreak,
        "raw" | "html" => Block::Raw(body),
        _ => Block::Raw(format!("@{}", name)),
    };
    if !a.errs.is_empty() {
        // an argument is often read twice (e.g. `x` for the axis and for the
        // data), so drop repeats but keep the order the failures happened in
        let mut seen: Vec<String> = Vec::new();
        for e in a.errs.iter() {
            if !seen.contains(e) {
                seen.push(e.clone());
            }
        }
        return Block::Note(Note {
            kind: "danger".into(),
            title: format!("@{} could not be evaluated", name),
            body: inline_parse(&format!("{}.", seen.join("; "))),
            error: true,
        });
    }
    block
}

fn src_of(args: &ArgSet, interp: &mut Interp) -> String {
    let mut errs = Vec::new();
    if let Some((_, v)) = args.named.iter().find(|(n, _)| n == "src" || n == "url" || n == "path") {
        return eval_checked(interp, v, &mut errs).map(|x| x.to_display()).unwrap_or_default();
    }
    args.positional
        .first()
        .and_then(|s| eval_checked(interp, s, &mut errs))
        .map(|x| x.to_display())
        .unwrap_or_default()
}

pub fn run_cells(doc: &mut Doc) {
    let mut interp = Interp::new();
    let blocks = std::mem::take(&mut doc.blocks);
    let mut out: Vec<Block> = Vec::with_capacity(blocks.len());
    let mut failed = false;
    for mut block in blocks {
        match &mut block {
            Block::Code { runnable, result, text, .. } if *runnable => {
                if failed {
                    *result = Some(CellResult {
                        stdout: String::new(),
                        value: None,
                        error: Some("not run: an earlier cell in this document failed".into()),
                    });
                    out.push(block);
                    continue;
                }
                let stmts = match crate::parser::parse_program(text) {
                    Ok(s) => s,
                    Err(e) => {
                        failed = true;
                        *result = Some(CellResult { stdout: String::new(), value: None, error: Some(e) });
                        out.push(block);
                        continue;
                    }
                };
                let mut last = Value::Null;
                let mut err = None;
                for st in &stmts {
                    let outcome = match st {
                        crate::ast::Stmt::Expr(e) => interp.eval(e),
                        other => interp.exec(other).map(|_| Value::Null),
                    };
                    match outcome {
                        Ok(v) => last = v,
                        Err(msg) => {
                            err = Some(msg);
                            break;
                        }
                    }
                }
                let stdout = builtins::take_output().join("\n");
                let value = match &last {
                    Value::Null | Value::Fn(_) | Value::Native(_) | Value::Partial(..) => None,
                    v => Some(match v {
                        Value::Str(s) => s.as_str().to_string(),
                        other => other.to_display(),
                    }),
                };
                *result = Some(CellResult { stdout, value, error: err });
                if result.as_ref().and_then(|r| r.error.as_ref()).is_some() {
                    failed = true;
                }
                out.push(block);
            }
            Block::Para(inlines) => {
                resolve_inlines(inlines, &mut interp);
                out.push(block);
            }
            Block::Heading { inlines, .. } => {
                resolve_inlines(inlines, &mut interp);
                out.push(block);
            }
            Block::Directive(raw) => {
                out.push(build_directive(raw, &mut interp));
            }
            _ => out.push(block),
        }
    }
    builtins::take_output();
    doc.blocks = out;
}

pub fn resolve_inlines(inlines: &mut [Inline], interp: &mut Interp) {
    for i in inlines.iter_mut() {
        match i {
            Inline::Interp(src) => {
                let v = eval_arg(interp, src);
                *i = Inline::Text(v.to_display());
            }
            Inline::Strong(v) | Inline::Em(v) | Inline::Strike(v) => resolve_inlines(v, interp),
            Inline::Link { text, .. } => resolve_inlines(text, interp),
            _ => {}
        }
    }
}

pub fn fmt_value(v: &Value) -> String {
    match v {
        Value::Num(n) => fmt_num(*n),
        other => other.to_display(),
    }
}
