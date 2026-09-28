use std::collections::HashMap;
use std::fs;

use crate::doc::{Align, Block, CellResult, Doc, Inline, Media, MediaKind, Note, PlotBlock, Table, Widget};
use crate::image;
use crate::plot;
use crate::render::*;

pub const PAGE_W: f64 = 595.276;
pub const PAGE_H: f64 = 841.89;
const MARGIN: f64 = 56.0;
const MARGIN_TOP: f64 = 64.0;
const MARGIN_BOTTOM: f64 = 64.0;
const BODY: f64 = 10.5;
const LEADING: f64 = 1.45;
const CODE_SIZE: f64 = 8.5;
const CODE_LEAD: f64 = 11.5;
const PAD: f64 = 4.0;
const ACCENT: Color = Color::rgb(52, 106, 190);
const CODE_BG: Color = Color::rgb(246, 247, 249);
const CODE_EDGE: Color = Color::rgb(228, 231, 238);
const COMPUTED: Color = Color::rgb(46, 92, 168);
const CODE_INK: Color = Color::rgb(43, 52, 64);
const GOOD: Color = Color::rgb(72, 160, 90);
const BAD: Color = Color::rgb(214, 69, 69);
const BAD_INK: Color = Color::rgb(138, 43, 43);
const HEAD_FILL: Color = Color::rgb(238, 241, 246);
const ZEBRA: Color = Color::rgb(250, 251, 252);
const SLATE: Color = Color::rgb(90, 100, 118);
const BOX_BG: Color = Color::rgb(241, 243, 247);
const BOX_EDGE: Color = Color::rgb(221, 226, 234);
const STAMP: &str = "D:20240101000000Z";
const MAX_TABLE_ROWS: usize = 500;

const FONTS: [(&str, &str); 12] = [
    ("F1", "Helvetica"),
    ("F2", "Helvetica-Bold"),
    ("F3", "Helvetica-Oblique"),
    ("F4", "Times-Roman"),
    ("F5", "Times-Bold"),
    ("F6", "Times-Italic"),
    ("F7", "Courier"),
    ("F8", "Courier-Bold"),
    ("F9", "Courier-Oblique"),
    ("F10", "Times-BoldItalic"),
    ("F11", "Helvetica-BoldOblique"),
    ("F12", "Symbol"),
];

fn font_res(font: Font, weight: Weight, italic: bool) -> &'static str {
    let name = font.pdf_name(weight, italic);
    for (res, base) in FONTS.iter() {
        if *base == name {
            return res;
        }
    }
    "F1"
}

fn base_letter(c: char) -> Option<char> {
    let lower: char = match c as u32 {
        0xC0..=0xC6 => 'a',
        0xC7 => 'c',
        0xC8..=0xCB => 'e',
        0xCC..=0xCF => 'i',
        0xD0 => 'd',
        0xD1 => 'n',
        0xD2..=0xD6 => 'o',
        0xD7 => 'e',
        0xD8 => 'o',
        0xD9..=0xDC => 'u',
        0xDD => 'y',
        0xDE => 'p',
        0xDF => 's',
        0xE0..=0xE6 => 'a',
        0xE7 => 'c',
        0xE8..=0xEB => 'e',
        0xEC..=0xEF => 'i',
        0xF0 => 'd',
        0xF1 => 'n',
        0xF2..=0xF6 => 'o',
        0xF7 => 'e',
        0xF8 => 'o',
        0xF9..=0xFC => 'u',
        0xFD | 0xFF => 'y',
        0xFE => 'p',
        _ => return None,
    };
    Some(if c.is_uppercase() { lower.to_ascii_uppercase() } else { lower })
}

pub fn pdf_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let code = c as u32;
        if (32..=126).contains(&code) {
            out.push(c);
            continue;
        }
        let repl: &str = match c {
            '\u{00A0}' | '\t' | '\n' | '\r' | '\u{200B}'..='\u{200D}' => {
                if c == '\u{200B}' || c == '\u{200C}' || c == '\u{200D}' {
                    ""
                } else {
                    " "
                }
            }
            '\u{2022}' => "\u{0095}",
            '\u{2013}' => "\u{0096}",
            '\u{2014}' => "\u{0097}",
            '\u{2018}' => "\u{0091}",
            '\u{2019}' => "\u{0092}",
            '\u{201C}' => "\u{0093}",
            '\u{201D}' => "\u{0094}",
            '\u{2026}' => "\u{0085}",
            '\u{2010}' | '\u{2011}' | '\u{2212}' => "-",
            '\u{2192}' => "->",
            '\u{2190}' | '\u{2194}' => "<->",
            '\u{2264}' => "<=",
            '\u{2265}' => ">=",
            '\u{2260}' => "!=",
            '\u{00D7}' => "x",
            '\u{00B1}' => "+-",
            '\u{2020}' | '\u{2021}' => "*",
            _ => match base_letter(c) {
                Some(b) => {
                    out.push(b);
                    continue;
                }
                None => {
                    if (128..=255).contains(&code) {
                        out.push(c);
                        continue;
                    }
                    "?"
                }
            },
        };
        out.push_str(repl);
    }
    out
}

fn measure(text: &str, font: Font, weight: Weight, size: f64) -> f64 {
    text_width(&pdf_text(text), font, weight, size)
}

fn escape_str(s: &str) -> Vec<u8> {
    let mut v = Vec::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => v.extend_from_slice(b"\\\\"),
            '(' => v.extend_from_slice(b"\\("),
            ')' => v.extend_from_slice(b"\\)"),
            '\r' => v.push(b'\\'),
            '\n' => v.extend_from_slice(b"\\n"),
            _ => {
                let n = c as u32;
                v.push(if n < 256 { n as u8 } else { b'?' });
            }
        }
    }
    v
}

fn pdf_string(s: &str) -> Vec<u8> {
    if s.chars().all(|c| (c as u32) < 128) {
        let mut v = vec![b'('];
        v.extend_from_slice(&escape_str(s));
        v.push(b')');
        v
    } else {
        let mut v = vec![b'<', 0xFE, 0xFF];
        for u in s.encode_utf16() {
            v.push((u >> 8) as u8);
            v.push((u & 0xFF) as u8);
        }
        v.push(b'>');
        v
    }
}

#[derive(Clone, Debug)]
enum Op {
    /// `symbol` marks text already encoded in the Symbol font's own encoding,
    /// which has to be written as raw bytes rather than as WinAnsi text.
    Text { res: &'static str, size: f64, x: f64, y: f64, text: String, color: Color, alpha: f64, angle: f64, symbol: bool },
    Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        fill: Option<Color>,
        stroke: Option<Color>,
        stroke_width: f64,
        radius: f64,
        alpha: f64,
    },
    Line { x1: f64, y1: f64, x2: f64, y2: f64, color: Color, width: f64, dash: Vec<f64>, alpha: f64 },
    Poly {
        points: Vec<Point>,
        closed: bool,
        fill: Option<Color>,
        stroke: Option<Color>,
        stroke_width: f64,
        dash: Vec<f64>,
        alpha: f64,
    },
    Image { x: f64, y: f64, w: f64, h: f64, key: String, alpha: f64 },
}

impl Op {
    fn alpha(&self) -> f64 {
        match self {
            Op::Text { alpha, .. } => *alpha,
            Op::Rect { alpha, .. } => *alpha,
            Op::Line { alpha, .. } => *alpha,
            Op::Poly { alpha, .. } => *alpha,
            Op::Image { alpha, .. } => *alpha,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct PageContent {
    ops: Vec<Op>,
}

struct Pdf {
    objects: Vec<Vec<u8>>,
}

impl Pdf {
    fn new() -> Pdf {
        Pdf { objects: Vec::new() }
    }

    fn alloc(&mut self, body: Vec<u8>) -> u32 {
        self.objects.push(body);
        self.objects.len() as u32
    }

    fn set(&mut self, id: u32, body: Vec<u8>) {
        if (id as usize) <= self.objects.len() {
            self.objects[(id - 1) as usize] = body;
        }
    }

    fn serialise(&self, root: u32, info: u32) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        out.extend_from_slice(b"%PDF-1.4\n");
        out.extend_from_slice(&[b'%', 0xE2, 0xE3, 0xCF, 0xD3, b'\n']);
        let mut offsets: Vec<usize> = Vec::with_capacity(self.objects.len());
        for (i, body) in self.objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            out.extend_from_slice(body);
            out.extend_from_slice(b"\nendobj\n");
        }
        let start = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n", self.objects.len() + 1).as_bytes());
        out.extend_from_slice(b"0000000000 65535 f \n");
        for off in &offsets {
            out.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root {} 0 R /Info {} 0 R /ID [<4E414E4F20202020202020202020202020> <4E414E4F20202020202020202020202020>] >>\nstartxref\n{}\n%%EOF\n",
                self.objects.len() + 1,
                root,
                info,
                start
            )
            .as_bytes(),
        );
        out
    }
}

fn n(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let r = (v * 1000.0).round() / 1000.0;
    let mut s = format!("{:.3}", r);
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    if s == "-0" {
        s = "0".into();
    }
    s
}

fn col(c: Color) -> String {
    format!("{} {} {}", n(c.r as f64 / 255.0), n(c.g as f64 / 255.0), n(c.b as f64 / 255.0))
}

fn round_rect(out: &mut String, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let k = 0.5523 * r;
    out.push_str(&format!("{} {} m\n", n(x), n(y)));
    out.push_str(&format!("{} {} l\n", n(x + w - r), n(y)));
    out.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        n(x + w - r + k),
        n(y),
        n(x + w),
        n(y + r - k),
        n(x + w),
        n(y + r)
    ));
    out.push_str(&format!("{} {} l\n", n(x + w), n(y + h - r)));
    out.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        n(x + w),
        n(y + h - r + k),
        n(x + w - r + k),
        n(y + h),
        n(x + w - r),
        n(y + h)
    ));
    out.push_str(&format!("{} {} l\n", n(x + r), n(y + h)));
    out.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        n(x + r - k),
        n(y + h),
        n(x),
        n(y + h - r + k),
        n(x),
        n(y + h - r)
    ));
    out.push_str(&format!("{} {} l\n", n(x), n(y + r)));
    out.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        n(x),
        n(y + r - k),
        n(x + r - k),
        n(y),
        n(x + r),
        n(y)
    ));
    out.push_str("h\n");
}

fn fmt_sig(v: f64, sig: usize) -> String {
    if !v.is_finite() {
        return format!("{}", v);
    }
    if v == 0.0 {
        return "0".into();
    }
    let exp = v.abs().log10().floor() as i32;
    let dec = sig as i32 - 1 - exp;
    if dec <= 0 {
        let p = 10f64.powi(exp - sig as i32 + 1);
        return format!("{:.0}", (v / p).round() * p);
    }
    let s = format!("{:.*}", dec as usize, v);
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

fn parse_width(spec: &Option<String>, content: f64) -> f64 {
    let raw = match spec {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => return content,
    };
    if let Some(pct) = raw.strip_suffix('%') {
        if let Ok(v) = pct.trim().parse::<f64>() {
            return (content * v / 100.0).clamp(24.0, content);
        }
    }
    let cleaned = raw.trim().trim_end_matches("px").trim();
    cleaned.parse::<f64>().map(|v| v.clamp(24.0, content)).unwrap_or(content)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Dec {
    Plain,
    Code,
    Math,
}

#[derive(Clone, Debug)]
struct Run {
    text: String,
    font: Font,
    weight: Weight,
    italic: bool,
    size: f64,
    color: Color,
    dec: Dec,
    strike: bool,
    /// Set for `$...$`; the formula is typeset up front and drawn as primitives
    /// rather than as a text run, so it can carry fractions, roots and scripts.
    math: Option<crate::math::Math>,
}

#[derive(Clone, Copy, Debug)]
struct Style {
    font: Font,
    weight: Weight,
    italic: bool,
    size: f64,
    color: Color,
    dec: Dec,
    strike: bool,
}

#[derive(Clone, Debug)]
struct Tok {
    text: String,
    run: usize,
    sep: bool,
    brk: bool,
}

#[derive(Debug)]
enum Embed {
    Png { w: u32, h: u32, channels: usize, pixels: Vec<u8> },
    Jpeg { w: u32, h: u32, comps: u8, bytes: Vec<u8> },
    Missing,
}

#[derive(Clone)]
struct HeadingInfo {
    level: u8,
    title: String,
    page: usize,
}

struct Layout<'a> {
    doc: &'a Doc,
    body_font: Font,
    pages: Vec<PageContent>,
    y: f64,
    embeds: HashMap<String, Embed>,
    headings: Vec<HeadingInfo>,
    toc: Option<&'a [HeadingInfo]>,
    figure: usize,
    tables: usize,
}

fn is_external(href: &str) -> bool {
    let h = href.trim();
    h.starts_with("http://") || h.starts_with("https://") || h.starts_with("ftp://") || h.starts_with("mailto:")
}

fn mk(text: impl Into<String>, st: Style) -> Run {
    Run {
        text: text.into(),
        font: st.font,
        weight: st.weight,
        italic: st.italic,
        size: st.size,
        color: st.color,
        dec: st.dec,
        strike: st.strike,
        math: None,
    }
}

fn push_runs(out: &mut Vec<Run>, inlines: &[Inline], st: Style) {
    for i in inlines {
        match i {
            Inline::Text(t) => {
                if !t.is_empty() {
                    out.push(mk(t.clone(), st));
                }
            }
            Inline::Code(t) => {
                out.push(mk(
                    t.clone(),
                    Style { font: Font::Mono, size: st.size - 1.0, color: CODE_INK, dec: Dec::Code, ..st },
                ));
            }
            Inline::Strong(v) => push_runs(out, v, Style { weight: Weight::Bold, ..st }),
            Inline::Em(v) => push_runs(out, v, Style { italic: true, ..st }),
            Inline::Strike(v) => push_runs(out, v, Style { strike: true, color: MUTED, ..st }),
            Inline::Link { text, href } => {
                push_runs(out, text, Style { color: ACCENT, ..st });
                if is_external(href) {
                    out.push(mk(
                        format!(" ({})", href.trim()),
                        Style { font: Font::Sans, size: 7.5, color: MUTED, ..st },
                    ));
                }
            }
            Inline::Math(t) => {
                // `$e^{i\\pi} + 1 = 0$` becomes a real formula, not its source
                let mut r = mk(String::new(), Style { color: INK, dec: Dec::Math, ..st });
                r.math = Some(crate::math::typeset(t, st.size, false));
                out.push(r);
            }
            Inline::Interp(s) => {
                out.push(mk(s.trim().to_string(), Style { color: COMPUTED, ..st }));
            }
            Inline::Break => out.push(mk("\n", st)),
            Inline::Ref(r) => out.push(mk(format!("[{}]", r.trim()), Style { color: ACCENT, ..st })),
            Inline::Image { .. } => {}
        }
    }
}

fn tokenize(runs: &[Run]) -> Vec<Tok> {
    let mut toks: Vec<Tok> = Vec::new();
    let mut word = String::new();
    let mut sep = false;
    for (ri, r) in runs.iter().enumerate() {
        if r.math.is_some() {
            // a formula never breaks across lines
            toks.push(Tok { text: String::new(), run: ri, sep, brk: false });
            sep = true;
            continue;
        }
        for c in r.text.chars() {
            match c {
                ' ' | '\t' => {
                    if word.is_empty() {
                        sep = true;
                    } else {
                        toks.push(Tok { text: std::mem::take(&mut word), run: ri, sep, brk: false });
                        sep = true;
                    }
                }
                '\n' => {
                    if !word.is_empty() {
                        toks.push(Tok { text: std::mem::take(&mut word), run: ri, sep, brk: false });
                    }
                    toks.push(Tok { text: String::new(), run: ri, sep: true, brk: true });
                    sep = true;
                }
                _ => word.push(c),
            }
        }
        if !word.is_empty() {
            toks.push(Tok { text: std::mem::take(&mut word), run: ri, sep, brk: false });
            sep = true;
        }
    }
    toks
}

fn wrap_runs(runs: &[Run], maxw: f64) -> (Vec<Tok>, Vec<Vec<usize>>) {
    let toks = tokenize(runs);
    let mut lines: Vec<Vec<usize>> = Vec::new();
    let mut cur: Vec<usize> = Vec::new();
    let mut curw = 0.0;
    for (ti, t) in toks.iter().enumerate() {
        if t.brk {
            if !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
            }
            curw = 0.0;
            continue;
        }
        let r = &runs[t.run];
        let w = run_width(r, &t.text);
        let sw = if t.sep { measure(" ", r.font, r.weight, r.size) } else { 0.0 };
        if !cur.is_empty() && curw + sw + w > maxw {
            lines.push(std::mem::take(&mut cur));
            cur.push(ti);
            curw = w;
        } else {
            cur.push(ti);
            curw += if cur.len() == 1 { w } else { sw + w };
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    (toks, lines)
}

fn line_width(runs: &[Run], toks: &[Tok], line: &[usize]) -> f64 {
    let mut w = 0.0;
    for (k, ti) in line.iter().enumerate() {
        let t = &toks[*ti];
        let r = &runs[t.run];
        if k > 0 && t.sep {
            w += measure(" ", r.font, r.weight, r.size);
        }
        w += run_width(r, &t.text);
    }
    w
}

/// Turn typeset math primitives into PDF operators, with (x, baseline) as the
/// origin of the formula.
///
/// `math::Prim` offsets are y-up (positive is above the axis), while the op
/// space is y-down from the top of the page, so every vertical offset is
/// subtracted here.
fn math_ops(m: &crate::math::Math, x: f64, baseline: f64, color: Color, out: &mut Vec<Op>) {
    for p in &m.prims {
        match p {
            crate::math::Prim::Run { x: px, y: py, size, text, font } => {
                let (res, symbol) = match font {
                    crate::math::MFont::Sym => ("F12", true),
                    crate::math::MFont::Var => (font_res(Font::Serif, Weight::Regular, true), false),
                    crate::math::MFont::Rm => (font_res(Font::Serif, Weight::Regular, false), false),
                };
                let body = if symbol {
                    // map each glyph to its byte in the Symbol encoding
                    text.chars()
                        .filter_map(crate::math::symbol_code)
                        .map(|b| b as char)
                        .collect::<String>()
                } else {
                    pdf_text(text)
                };
                out.push(Op::Text {
                    res,
                    size: *size,
                    x: x + px,
                    y: baseline - py,
                    text: body,
                    color,
                    alpha: 1.0,
                    angle: 0.0,
                    symbol,
                });
            }
            crate::math::Prim::Rule { x: px, y: py, w, thickness } => {
                out.push(Op::Line {
                    x1: x + px,
                    y1: baseline - py,
                    x2: x + px + w,
                    y2: baseline - py,
                    color,
                    width: *thickness,
                    dash: Vec::new(),
                    alpha: 1.0,
                });
            }
            crate::math::Prim::Path { pts, width } => {
                for pair in pts.windows(2) {
                    out.push(Op::Line {
                        x1: x + pair[0].0,
                        y1: baseline - pair[0].1,
                        x2: x + pair[1].0,
                        y2: baseline - pair[1].1,
                        color,
                        width: *width,
                        dash: Vec::new(),
                        alpha: 1.0,
                    });
                }
            }
            crate::math::Prim::Dot { x: px, y: py, r } => {
                let r = *r;
                out.push(Op::Rect {
                    x: x + px - r,
                    y: baseline - py - r,
                    w: r * 2.0,
                    h: r * 2.0,
                    fill: Some(color),
                    stroke: None,
                    stroke_width: 0.0,
                    radius: r,
                    alpha: 1.0,
                });
            }
        }
    }
}

/// Width of one laid-out token: a typeset formula keeps its own metrics.
fn run_width(r: &Run, text: &str) -> f64 {
    match &r.math {
        Some(m) => m.width,
        None => measure(text, r.font, r.weight, r.size),
    }
}

fn line_size(runs: &[Run], toks: &[Tok], line: &[usize]) -> f64 {
    line.iter().map(|ti| runs[toks[*ti].run].size).fold(0.0f64, f64::max)
}

impl<'a> Layout<'a> {
    fn new(doc: &'a Doc, toc: Option<&'a [HeadingInfo]>) -> Layout<'a> {
        let serif = doc.meta.theme.eq_ignore_ascii_case("serif")
            || doc.meta.theme.eq_ignore_ascii_case("times")
            || doc.meta.theme.eq_ignore_ascii_case("classic");
        Layout {
            doc,
            body_font: if serif { Font::Serif } else { Font::Sans },
            pages: vec![PageContent { ops: Vec::new() }],
            y: MARGIN_TOP,
            embeds: HashMap::new(),
            headings: Vec::new(),
            toc,
            figure: 0,
            tables: 0,
        }
    }

    fn x0(&self) -> f64 {
        MARGIN
    }

    fn content_w(&self) -> f64 {
        PAGE_W - 2.0 * MARGIN
    }

    fn limit(&self) -> f64 {
        PAGE_H - MARGIN_BOTTOM
    }

    fn push(&mut self, op: Op) {
        if self.pages.is_empty() {
            self.pages.push(PageContent { ops: Vec::new() });
        }
        if let Some(page) = self.pages.last_mut() {
            page.ops.push(op);
        }
    }

    fn new_page(&mut self) {
        self.pages.push(PageContent { ops: Vec::new() });
        self.y = MARGIN_TOP;
    }

    fn need(&mut self, h: f64) {
        if self.y + h > self.limit() {
            self.new_page();
        }
    }

    fn advance(&mut self, h: f64) {
        self.y += h;
    }

    fn text_op(&self, font: Font, weight: Weight, italic: bool, size: f64, x: f64, y: f64, text: &str, color: Color) -> Op {
        Op::Text { res: font_res(font, weight, italic), size, x, y, text: pdf_text(text), color, alpha: 1.0, angle: 0.0, symbol: false }
    }

    fn line_op(&self, x1: f64, y1: f64, x2: f64, y2: f64, color: Color, width: f64) -> Op {
        Op::Line { x1, y1, x2, y2, color, width, dash: Vec::new(), alpha: 1.0 }
    }

    fn centre(&mut self, baseline: f64, text: &str, font: Font, weight: Weight, italic: bool, size: f64, color: Color) {
        if text.is_empty() {
            return;
        }
        let w = measure(text, font, weight, size);
        let x = self.x0() + (self.content_w() - w).max(0.0) / 2.0;
        let op = self.text_op(font, weight, italic, size, x, baseline, text, color);
        self.push(op);
    }

    fn style(&self) -> Style {
        Style { font: self.body_font, weight: Weight::Regular, italic: false, size: BODY, color: INK, dec: Dec::Plain, strike: false }
    }

    fn runs_of(&self, inlines: &[Inline]) -> Vec<Run> {
        let mut out = Vec::new();
        push_runs(&mut out, inlines, self.style());
        out
    }

    fn runs_height(&self, runs: &[Run], maxw: f64, lmul: f64) -> f64 {
        let (toks, lines) = wrap_runs(runs, maxw);
        if toks.is_empty() {
            return 0.0;
        }
        lines.iter().map(|l| line_size(runs, &toks, l) * lmul).sum()
    }

    fn runs_block(&mut self, runs: &[Run], x0: f64, maxw: f64, lmul: f64, align: Align, hang: Option<(&str, Color, f64)>) {
        let (toks, lines) = wrap_runs(runs, maxw.max(1.0));
        if toks.is_empty() {
            if let Some((h, hc, hx)) = hang {
                self.need(BODY * lmul);
                let base = self.y + text_ascent(BODY);
                let op = self.text_op(self.body_font, Weight::Regular, false, BODY, hx, base, h, hc);
                self.push(op);
            }
            self.advance(BODY * lmul);
            return;
        }
        for (li, line) in lines.iter().enumerate() {
            let size = line_size(runs, &toks, line);
            let lh = size * lmul;
            self.need(lh);
            let lw = line_width(runs, &toks, line);
            let x = match align {
                Align::Left => x0,
                Align::Center => x0 + (maxw - lw).max(0.0) / 2.0,
                Align::Right => x0 + (maxw - lw).max(0.0),
            };
            let baseline = self.y + text_ascent(size);
            if li == 0 {
                if let Some((h, hc, hx)) = hang {
                    let op = self.text_op(self.body_font, Weight::Regular, false, size.min(BODY), hx, baseline, h, hc);
                    self.push(op);
                }
            }
            let mut cx = x;
            let mut ops: Vec<Op> = Vec::new();
            for (k, ti) in line.iter().enumerate() {
                let t = &toks[*ti];
                let r = &runs[t.run];
                if k > 0 && t.sep {
                    cx += measure(" ", r.font, r.weight, r.size);
                }
                let tw = run_width(r, &t.text);
                if let Some(m) = &r.math {
                    math_ops(&m, cx, baseline, r.color, &mut ops);
                    cx += tw;
                    continue;
                }
                if r.dec == Dec::Code {
                    ops.push(Op::Rect {
                        x: cx - 1.5,
                        y: baseline - text_ascent(r.size) - 1.5,
                        w: tw + 3.0,
                        h: r.size + 3.0,
                        fill: Some(GRID),
                        stroke: None,
                        stroke_width: 0.0,
                        radius: 2.0,
                        alpha: 1.0,
                    });
                }
                if r.strike {
                    let sy = baseline - r.size * 0.26;
                    ops.push(Op::Line { x1: cx, y1: sy, x2: cx + tw, y2: sy, color: MUTED, width: 0.6, dash: Vec::new(), alpha: 1.0 });
                }
                ops.push(Op::Text {
                    symbol: false,
                    res: font_res(r.font, r.weight, r.italic),
                    size: r.size,
                    x: cx,
                    y: baseline,
                    text: pdf_text(&t.text),
                    color: r.color,
                    alpha: 1.0,
                    angle: 0.0,
                });
                cx += tw;
            }
            for op in ops {
                self.push(op);
            }
            self.advance(lh);
        }
    }

    fn para(&mut self, inlines: &[Inline]) {
        if inlines.iter().any(has_image) {
            let mut seg: Vec<Inline> = Vec::new();
            for i in inlines {
                if let Inline::Image { alt, src } = i {
                    if !seg.is_empty() {
                        let runs = self.runs_of(&seg);
                        self.runs_block(&runs, self.x0(), self.content_w(), LEADING, Align::Left, None);
                        seg.clear();
                    }
                    self.advance(4.0);
                    self.figure_block(src, alt, "", None);
                    self.advance(10.0);
                } else {
                    seg.push(i.clone());
                }
            }
            if !seg.is_empty() {
                let runs = self.runs_of(&seg);
                self.runs_block(&runs, self.x0(), self.content_w(), LEADING, Align::Left, None);
            }
        } else {
            let runs = self.runs_of(inlines);
            self.runs_block(&runs, self.x0(), self.content_w(), LEADING, Align::Left, None);
        }
        self.advance(BODY * 0.55);
    }

    fn heading(&mut self, level: u8, inlines: &[Inline]) {
        let (size, above, below) = match level {
            1 => (15.0, 20.0, 7.0),
            2 => (12.5, 15.0, 5.0),
            _ => (11.0, 12.0, 4.0),
        };
        self.advance(above);
        if self.limit() - self.y < 40.0 {
            self.new_page();
        }
        let title = inlines.iter().map(|i| i.plain()).collect::<String>();
        let runs = vec![Run {
            math: None,
            text: title.clone(),
            font: self.body_font,
            weight: Weight::Bold,
            italic: false,
            size,
            color: INK,
            dec: Dec::Plain,
            strike: false,
        }];
        self.runs_block(&runs, self.x0(), self.content_w(), 1.25, Align::Left, None);
        self.advance(below);
        if level <= 2 {
            let page = self.pages.len().saturating_sub(1);
            self.headings.push(HeadingInfo { level, title, page });
        }
    }

    fn header(&mut self) {
        let meta = &self.doc.meta;
        let x = self.x0();
        let w = self.content_w();
        if !meta.title.trim().is_empty() {
            self.need(30.0);
            let base = self.y + text_ascent(22.0);
            self.centre(base, &meta.title, self.body_font, Weight::Bold, false, 22.0, INK);
            self.advance(text_line_height(22.0) + 2.0);
        }
        if !meta.subtitle.trim().is_empty() {
            self.need(20.0);
            let base = self.y + text_ascent(13.0);
            self.centre(base, &meta.subtitle, self.body_font, Weight::Regular, false, 13.0, MUTED);
            self.advance(text_line_height(13.0) + 2.0);
        }
        let authors = meta.author.join(", ");
        if !authors.trim().is_empty() {
            self.need(18.0);
            let base = self.y + text_ascent(BODY);
            self.centre(base, &authors, self.body_font, Weight::Regular, false, BODY, INK);
            self.advance(text_line_height(BODY));
        }
        if !meta.date.trim().is_empty() {
            self.need(16.0);
            let base = self.y + text_ascent(9.5);
            self.centre(base, &meta.date, self.body_font, Weight::Regular, false, 9.5, MUTED);
            self.advance(text_line_height(9.5) + 6.0);
        }
        self.advance(6.0);
        let op = self.line_op(x + w * 0.28, self.y, x + w * 0.72, self.y, GRID, 0.6);
        self.push(op);
        self.advance(18.0);
    }
}

fn has_image(i: &Inline) -> bool {
    match i {
        Inline::Image { .. } => true,
        Inline::Strong(v) | Inline::Em(v) | Inline::Strike(v) => v.iter().any(has_image),
        Inline::Link { text, .. } => text.iter().any(has_image),
        _ => false,
    }
}

fn stream_body(dict: &str, data: &[u8]) -> Vec<u8> {
    let mut body = format!("<< {} /Length {} >>\nstream\n", dict, data.len()).into_bytes();
    body.extend_from_slice(data);
    body.extend_from_slice(b"\nendstream");
    body
}

fn push_gs(s: &mut String, op: &Op, gs: &[(f64, String)]) {
    let a = op.alpha();
    if a > 0.0 && a < 0.999 {
        for (v, name) in gs {
            if (v - a).abs() < 0.005 {
                s.push_str(&format!("/{} gs\n", name));
                break;
            }
        }
    }
}

fn dash_op(dash: &[f64], s: &mut String) {
    if !dash.is_empty() {
        s.push_str(&format!("[{}] 0 d\n", dash.iter().map(|d| n(*d)).collect::<Vec<_>>().join(" ")));
    }
}

/// A literal PDF string built from raw single bytes, for the Symbol font.
fn symbol_string(s: &str) -> Vec<u8> {
    let mut v = vec![b'('];
    for c in s.chars() {
        let code = c as u32;
        match code {
            0x28 => v.extend_from_slice(b"\\("),
            0x29 => v.extend_from_slice(b"\\)"),
            0x5C => v.extend_from_slice(b"\\\\"),
            _ => v.push(if code < 256 { code as u8 } else { b'?' }),
        }
    }
    v.push(b')');
    v
}

/// Split label text into runs of (encoded body, width, uses Symbol font),
/// keeping glyphs the base-14 text fonts cannot encode in runs of their own.
fn split_symbol_runs(text: &str, font: Font, weight: Weight, size: f64) -> Vec<(String, f64, bool)> {
    fn width_of(s: &str, font: Font, weight: Weight, size: f64) -> f64 {
        s.chars()
            .map(|ch| {
                if crate::math::symbol_code(ch).is_some() {
                    0.55 * size
                } else {
                    text_width(&pdf_text(&ch.to_string()), font, weight, size)
                }
            })
            .sum()
    }
    let mut out: Vec<(String, f64, bool)> = Vec::new();
    let mut buf = String::new();
    let mut sym = crate::math::symbol_code(text.chars().next().unwrap_or('a')).is_some();
    for ch in text.chars() {
        let is_sym = crate::math::symbol_code(ch).is_some();
        if is_sym != sym && !buf.is_empty() {
            let body = if sym {
                buf.chars().filter_map(crate::math::symbol_code).map(|b| b as char).collect::<String>()
            } else {
                pdf_text(&buf)
            };
            out.push((body, width_of(&buf, font, weight, size), sym));
            buf.clear();
        }
        sym = is_sym;
        buf.push(ch);
    }
    if !buf.is_empty() {
        let body = if sym {
            buf.chars().filter_map(crate::math::symbol_code).map(|b| b as char).collect::<String>()
        } else {
            pdf_text(&buf)
        };
        out.push((body, width_of(&buf, font, weight, size), sym));
    }
    out
}

fn write_op(out: &mut Vec<u8>, op: &Op, gs: &[(f64, String)], imgs: &HashMap<String, String>) {
    if let Op::Text { res, size, x, y, text, color, angle, symbol, .. } = op {
        let mut head = String::from("q\n");
        push_gs(&mut head, op, gs);
        head.push_str(&format!("{} rg\nBT\n/{} {} Tf\n", col(*color), res, n(*size)));
        if *angle != 0.0 {
            let rad = -*angle * std::f64::consts::PI / 180.0;
            head.push_str(&format!(
                "{} {} {} {} {} {} cm\n1 0 0 1 0 0 Tm\n",
                n(rad.cos()),
                n(rad.sin()),
                n(-rad.sin()),
                n(rad.cos()),
                n(*x),
                n(PAGE_H - y)
            ));
        } else {
            head.push_str(&format!("1 0 0 1 {} {} Tm\n", n(*x), n(PAGE_H - y)));
        }
        if *symbol {
            // the bytes are already in the Symbol encoding, so they are written
            // straight out instead of going through the WinAnsi mapping
            out.extend_from_slice(head.as_bytes());
            out.extend_from_slice(&symbol_string(text));
        } else {
            head.push('(');
            out.extend_from_slice(head.as_bytes());
            out.extend_from_slice(&escape_str(text));
            out.extend_from_slice(b")");
        }
        out.extend_from_slice(b" Tj\nET\nQ\n");
        return;
    }
    let mut s = String::from("q\n");
    push_gs(&mut s, op, gs);
    match op {
        Op::Rect { x, y, w, h, fill, stroke, stroke_width, radius, .. } => {
            if fill.is_none() && stroke.is_none() {
                return;
            }
            if let Some(f) = fill {
                s.push_str(&format!("{} rg\n", col(*f)));
            }
            if let Some(st) = stroke {
                s.push_str(&format!("{} RG\n{} w\n1 J\n1 j\n", col(*st), n(stroke_width.max(0.05))));
            }
            round_rect(&mut s, *x, PAGE_H - (y + h), *w, *h, *radius);
            match (fill.is_some(), stroke.is_some()) {
                (true, true) => s.push_str("B\n"),
                (true, false) => s.push_str("f\n"),
                _ => s.push_str("S\n"),
            }
        }
        Op::Line { x1, y1, x2, y2, color, width, dash, .. } => {
            s.push_str(&format!("{} RG\n{} w\n1 J\n1 j\n", col(*color), n(width.max(0.05))));
            dash_op(dash, &mut s);
            s.push_str(&format!(
                "{} {} m\n{} {} l\nS\n",
                n(*x1),
                n(PAGE_H - y1),
                n(*x2),
                n(PAGE_H - y2)
            ));
        }
        Op::Poly { points, closed, fill, stroke, stroke_width, dash, .. } => {
            if points.is_empty() {
                return;
            }
            if let Some(f) = fill {
                s.push_str(&format!("{} rg\n", col(*f)));
            }
            if let Some(st) = stroke {
                s.push_str(&format!("{} RG\n{} w\n1 J\n1 j\n", col(*st), n(stroke_width.max(0.05))));
            }
            dash_op(dash, &mut s);
            s.push_str(&format!("{} {} m\n", n(points[0].x), n(PAGE_H - points[0].y)));
            for p in points[1..].iter() {
                s.push_str(&format!("{} {} l\n", n(p.x), n(PAGE_H - p.y)));
            }
            if *closed {
                s.push_str("h\n");
            }
            match (fill.is_some(), stroke.is_some()) {
                (true, true) => s.push_str("B\n"),
                (true, false) => s.push_str("f\n"),
                _ => s.push_str("S\n"),
            }
        }
        Op::Image { x, y, w, h, key, .. } => {
            let name = match imgs.get(key) {
                Some(v) => v.clone(),
                None => return,
            };
            s.push_str(&format!(
                "{} 0 0 {} {} {} cm\n/{} Do\n",
                n(*w),
                n(*h),
                n(*x),
                n(PAGE_H - (y + h)),
                name
            ));
        }
        Op::Text { .. } => {}
    }
    s.push_str("Q\n");
    out.extend_from_slice(s.as_bytes());
}

fn emit(l: &Layout) -> Result<Vec<u8>, String> {
    let mut pdf = Pdf::new();
    let catalog = pdf.alloc(Vec::new());
    let pages_id = pdf.alloc(Vec::new());
    let res_id = pdf.alloc(Vec::new());
    let font_ids: Vec<u32> = FONTS.iter().map(|_| pdf.alloc(Vec::new())).collect();
    let mut alphas: Vec<f64> = Vec::new();
    for page in &l.pages {
        for op in &page.ops {
            let a = (op.alpha() * 100.0).round() / 100.0;
            if a > 0.0 && a < 0.999 && !alphas.iter().any(|v| (v - a).abs() < 0.005) {
                alphas.push(a);
            }
        }
    }
    alphas.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let gs_ids: Vec<u32> = alphas.iter().map(|_| pdf.alloc(Vec::new())).collect();
    let mut keys: Vec<String> = Vec::new();
    for page in &l.pages {
        for op in &page.ops {
            if let Op::Image { key, .. } = op {
                if !keys.contains(key) {
                    keys.push(key.clone());
                }
            }
        }
    }
    let mut img_ids: Vec<(u32, Option<u32>)> = Vec::new();
    for k in keys.iter() {
        let smask = match l.embeds.get(k) {
            Some(Embed::Png { channels: 4, .. }) => Some(pdf.alloc(Vec::new())),
            _ => None,
        };
        let id = pdf.alloc(Vec::new());
        img_ids.push((id, smask));
    }
    let page_ids: Vec<u32> = l.pages.iter().map(|_| pdf.alloc(Vec::new())).collect();
    let cont_ids: Vec<u32> = l.pages.iter().map(|_| pdf.alloc(Vec::new())).collect();
    let items: Vec<HeadingInfo> = l.headings.iter().filter(|h| h.level <= 2).cloned().collect();
    let outlines_id = if items.is_empty() { 0 } else { pdf.alloc(Vec::new()) };
    let item_ids: Vec<u32> = items.iter().map(|_| pdf.alloc(Vec::new())).collect();
    let info_id = pdf.alloc(Vec::new());

    for (i, (_, base)) in FONTS.iter().enumerate() {
        // Symbol is a symbolic font with its own built-in encoding; forcing
        // WinAnsi onto it would remap every Greek and operator glyph.
        let body = if *base == "Symbol" {
            format!("<< /Type /Font /Subtype /Type1 /BaseFont /{base} >>")
        } else {
            format!("<< /Type /Font /Subtype /Type1 /BaseFont /{base} /Encoding /WinAnsiEncoding >>")
        };
        pdf.set(font_ids[i], body.into_bytes());
    }
    for (i, a) in alphas.iter().enumerate() {
        let body = format!("<< /Type /ExtGState /ca {} /CA {} >>", n(*a), n(*a));
        pdf.set(gs_ids[i], body.into_bytes());
    }
    for (i, key) in keys.iter().enumerate() {
        let (id, smask) = img_ids[i];
        match l.embeds.get(key) {
            Some(Embed::Png { w, h, channels, pixels }) => {
                let space = if *channels == 1 { "/DeviceGray" } else { "/DeviceRGB" };
                let mut dict = format!(
                    "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace {} /BitsPerComponent 8",
                    w, h, space
                );
                if let Some(s) = smask {
                    dict.push_str(&format!(" /SMask {} 0 R", s));
                }
                let data: Vec<u8> = if *channels == 4 {
                    pixels.iter().step_by(4).copied().collect()
                } else {
                    pixels.clone()
                };
                pdf.set(id, stream_body(&dict, &data));
                if let Some(s) = smask {
                    let alpha: Vec<u8> = pixels.iter().skip(3).step_by(4).copied().collect();
                    let adict = format!(
                        "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceGray /BitsPerComponent 8",
                        w, h
                    );
                    pdf.set(s, stream_body(&adict, &alpha));
                }
            }
            Some(Embed::Jpeg { w, h, comps, bytes }) => {
                let (space, extra) = match comps {
                    1 => ("/DeviceGray", ""),
                    3 => ("/DeviceRGB", ""),
                    4 => ("/DeviceCMYK", " /Decode [1 0 1 0 1 0 1 0]"),
                    _ => ("/DeviceRGB", ""),
                };
                let dict = format!(
                    "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace {} /BitsPerComponent 8 /Filter /DCTDecode{}",
                    w, h, space, extra
                );
                pdf.set(id, stream_body(&dict, bytes));
            }
            _ => {}
        }
    }
    let mut fonts = String::new();
    for (i, (res, _)) in FONTS.iter().enumerate() {
        fonts.push_str(&format!("/{} {} 0 R ", res, font_ids[i]));
    }
    let mut xobjs = String::new();
    for (i, pair) in img_ids.iter().enumerate() {
        xobjs.push_str(&format!("/Im{} {} 0 R ", i + 1, pair.0));
    }
    let mut states = String::new();
    for (i, _) in alphas.iter().enumerate() {
        states.push_str(&format!("/GS{} {} 0 R ", i + 1, gs_ids[i]));
    }
    let res = format!(
        "<< /Font << {} >> /ProcSet [/PDF /Text] /XObject << {} >> /ExtGState << {} >> >>",
        fonts.trim_end(),
        xobjs.trim_end(),
        states.trim_end()
    );
    pdf.set(res_id, res.into_bytes());

    let gs: Vec<(f64, String)> = alphas
        .iter()
        .enumerate()
        .map(|(i, a)| (*a, format!("GS{}", i + 1)))
        .collect();
    let imgs: HashMap<String, String> = keys
        .iter()
        .enumerate()
        .map(|(i, k)| (k.clone(), format!("Im{}", i + 1)))
        .collect();
    for (i, page) in l.pages.iter().enumerate() {
        let mut bytes: Vec<u8> = Vec::new();
        for op in &page.ops {
            write_op(&mut bytes, op, &gs, &imgs);
        }
        pdf.set(cont_ids[i], stream_body("", &bytes));
        let body = format!(
            "<< /Type /Page /Parent {} 0 R /MediaBox [0 0 {} {}] /Resources {} 0 R /Contents {} 0 R >>",
            pages_id,
            n(PAGE_W),
            n(PAGE_H),
            res_id,
            cont_ids[i]
        );
        pdf.set(page_ids[i], body.into_bytes());
    }
    let kids: Vec<String> = page_ids.iter().map(|id| format!("{} 0 R", id)).collect();
    let body = format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), page_ids.len());
    pdf.set(pages_id, body.into_bytes());

    if outlines_id > 0 && !item_ids.is_empty() {
        let last = *item_ids.last().unwrap_or(&0);
        let body = format!(
            "<< /Type /Outlines /First {} 0 R /Last {} 0 R /Count {} >>",
            item_ids[0],
            last,
            item_ids.len()
        );
        pdf.set(outlines_id, body.into_bytes());
        for (i, h) in items.iter().enumerate() {
            let mut body = String::from("<< /Title ");
            body.push_str(&String::from_utf8_lossy(&pdf_string(&h.title)));
            body.push_str(&format!(" /Parent {} 0 R", outlines_id));
            if i > 0 {
                body.push_str(&format!(" /Prev {} 0 R", item_ids[i - 1]));
            }
            if i + 1 < item_ids.len() {
                body.push_str(&format!(" /Next {} 0 R", item_ids[i + 1]));
            }
            let page = page_ids.get(h.page).copied().unwrap_or(page_ids[0]);
            body.push_str(&format!(" /Dest [{} 0 R /XYZ 0 {} null]", page, n(PAGE_H - MARGIN_TOP)));
            body.push_str(" >>");
            pdf.set(item_ids[i], body.into_bytes());
        }
    }

    let author = l.doc.meta.author.join(", ");
    let mut info = String::from("<< /Creator (nano) /Producer (nano)");
    info.push_str(&format!(" /CreationDate ({}) /ModDate ({})", STAMP, STAMP));
    for (key, val) in [
        ("/Title", &l.doc.meta.title),
        ("/Author", &author),
        ("/Subject", &l.doc.meta.description),
    ] {
        if !val.trim().is_empty() {
            info.push_str(&format!(" {} ", key));
            info.push_str(&String::from_utf8_lossy(&pdf_string(val)));
        }
    }
    info.push_str(" >>");
    pdf.set(info_id, info.into_bytes());

    let mut cat = format!("<< /Type /Catalog /Pages {} 0 R", pages_id);
    if outlines_id > 0 {
        cat.push_str(&format!(" /Outlines {} 0 R /PageMode /UseOutlines", outlines_id));
    }
    cat.push_str(" >>");
    pdf.set(catalog, cat.into_bytes());
    Ok(pdf.serialise(catalog, info_id))
}

pub fn render(doc: &Doc) -> Result<Vec<u8>, String> {
    let mut probe = Layout::new(doc, None);
    probe.run()?;
    let toc: Vec<HeadingInfo> = probe.headings.clone();
    let mut layout = Layout::new(doc, Some(&toc));
    layout.run()?;
    emit(&layout)
}

struct PLine {
    text: String,
    color: Color,
}

impl PLine {
    fn new(text: impl Into<String>, color: Color) -> PLine {
        PLine { text: text.into(), color }
    }
}

fn result_lines(r: &CellResult) -> Vec<PLine> {
    let mut out: Vec<PLine> = Vec::new();
    if let Some(e) = &r.error {
        for l in e.lines() {
            out.push(PLine::new(l, BAD_INK));
        }
    }
    if !r.stdout.trim().is_empty() {
        if !out.is_empty() {
            out.push(PLine::new("", MUTED));
        }
        for l in r.stdout.lines() {
            out.push(PLine::new(l, SLATE));
        }
    }
    if let Some(v) = &r.value {
        if !v.trim().is_empty() {
            if !out.is_empty() {
                out.push(PLine::new("", MUTED));
            }
            out.push(PLine::new(v.clone(), INK));
        }
    }
    if out.is_empty() {
        out.push(PLine::new("(no output)", MUTED));
    }
    out
}

impl<'a> Layout<'a> {
    fn panel(
        &mut self,
        x: f64,
        w: f64,
        text_x: f64,
        pad_v: f64,
        lead: f64,
        size: f64,
        lines: &[PLine],
        fill: Color,
        stroke: Option<Color>,
        bar: Option<Color>,
        radius: f64,
        badge: Option<&str>,
    ) {
        if lines.is_empty() {
            return;
        }
        let mut i = 0usize;
        let mut first = true;
        self.need(lead + pad_v * 2.0);
        while i < lines.len() {
            let mut j = i;
            while j < lines.len() && self.y + pad_v * 2.0 + (j - i + 1) as f64 * lead <= self.limit() {
                j += 1;
            }
            if j == i {
                if first {
                    self.new_page();
                    first = false;
                    continue;
                }
                j = i + 1;
            }
            let h = pad_v * 2.0 + (j - i) as f64 * lead;
            let top = self.y;
            self.push(Op::Rect {
                x,
                y: top,
                w,
                h,
                fill: Some(fill),
                stroke,
                stroke_width: if stroke.is_some() { 0.5 } else { 0.0 },
                radius,
                alpha: 1.0,
            });
            if let Some(b) = bar {
                self.push(Op::Rect { x, y: top, w: 3.0, h, fill: Some(b), stroke: None, stroke_width: 0.0, radius: 0.0, alpha: 1.0 });
            }
            for (k, ln) in lines[i..j].iter().enumerate() {
                let line_top = top + pad_v + k as f64 * lead;
                let baseline = line_top + (lead - text_line_height(size)) / 2.0 + text_ascent(size);
                let t = ellipsize(&pdf_text(&ln.text), Font::Mono, Weight::Regular, size, (x + w - 8.0 - text_x).max(20.0));
                let op = self.text_op(Font::Mono, Weight::Regular, false, size, text_x, baseline, &t, ln.color);
                self.push(op);
            }
            if first {
                if let Some(b) = badge {
                    let bw = measure(b, Font::Sans, Weight::Bold, 7.0);
                    let lh = CODE_SIZE;
                    let first_line = ellipsize(&pdf_text(&lines[0].text), Font::Mono, Weight::Regular, lh, (w - 2.0 * 8.0).max(20.0));
                    let used = measure(&first_line, Font::Mono, Weight::Regular, lh);
                    if used + bw + 24.0 < w {
                        let baseline = top + pad_v + (lead - text_line_height(7.0)) / 2.0 + text_ascent(7.0);
                        let op = self.text_op(Font::Sans, Weight::Bold, false, 7.0, x + w - 8.0 - bw, baseline, b, MUTED);
                        self.push(op);
                    }
                }
            }
            self.y = top + h;
            i = j;
            if i < lines.len() {
                self.new_page();
                first = false;
            }
        }
    }

    fn list_item(&mut self, marker: &str, marker_color: Color, inlines: &[Inline], indent: f64) {
        let runs = self.runs_of(inlines);
        self.runs_block(
            &runs,
            self.x0() + indent,
            (self.content_w() - indent).max(20.0),
            LEADING,
            Align::Left,
            Some((marker, marker_color, self.x0())),
        );
        self.advance(3.0);
    }

    fn list(&mut self, ordered: bool, start: u32, items: &[Vec<Inline>]) {
        for (i, item) in items.iter().enumerate() {
            let marker = if ordered { format!("{}.", start as usize + i) } else { "\u{2022}".to_string() };
            let mw = measure(&marker, self.body_font, Weight::Regular, BODY);
            let indent = (mw + 5.0).max(12.0);
            self.list_item(&marker, INK, item, indent);
        }
        self.advance(4.0);
    }

    fn checklist(&mut self, items: &[(bool, Vec<Inline>)]) {
        for (done, item) in items {
            let marker = if *done { "[x]" } else { "[ ]" };
            self.list_item(marker, if *done { GOOD } else { MUTED }, item, 16.0);
        }
        self.advance(4.0);
    }

    fn quote(&mut self, inlines: &[Inline]) {
        let mut runs = self.runs_of(inlines);
        for r in runs.iter_mut() {
            r.font = Font::Serif;
            r.italic = true;
            r.color = INK.mix(&PAPER, 0.15);
            if r.dec == Dec::Code {
                r.font = Font::Mono;
                r.italic = false;
            }
        }
        let indent = 14.0;
        let inner = (self.content_w() - indent).max(20.0);
        let h = self.runs_height(&runs, inner, LEADING) + 10.0;
        self.advance(5.0);
        self.need(h);
        let top = self.y;
        self.push(Op::Rect {
            x: self.x0(),
            y: top,
            w: 2.0,
            h,
            fill: Some(ACCENT),
            stroke: None,
            stroke_width: 0.0,
            radius: 1.0,
            alpha: 1.0,
        });
        self.runs_block(&runs, self.x0() + indent, inner, LEADING, Align::Left, None);
        self.advance(7.0);
    }

    fn rule(&mut self) {
        self.advance(11.0);
        self.need(1.0);
        let op = self.line_op(self.x0(), self.y, self.x0() + self.content_w(), self.y, GRID, 0.6);
        self.push(op);
        self.advance(11.0);
    }

    fn code(&mut self, lang: &str, text: &str, result: Option<&CellResult>) {
        let pad = 8.0;
        let lines: Vec<PLine> = text
            .lines()
            .map(|l| PLine::new(pdf_text(l), CODE_INK))
            .collect();
        let badge = if lang.trim().is_empty() { None } else { Some(lang.trim()) };
        self.advance(5.0);
        if lines.is_empty() {
            self.panel(
                self.x0(),
                self.content_w(),
                self.x0() + pad,
                7.0,
                CODE_LEAD,
                CODE_SIZE,
                &[PLine::new("", CODE_INK)],
                CODE_BG,
                Some(CODE_EDGE),
                None,
                4.0,
                None,
            );
        } else {
            self.panel(self.x0(), self.content_w(), self.x0() + pad, 7.0, CODE_LEAD, CODE_SIZE, &lines, CODE_BG, Some(CODE_EDGE), None, 4.0, badge);
        }
        if let Some(r) = result {
            let rl = result_lines(r);
            self.advance(3.0);
            let bar = if r.error.is_some() { BAD } else { GOOD };
            self.panel(self.x0(), self.content_w(), self.x0() + 12.0, 7.0, CODE_LEAD, CODE_SIZE, &rl, ZEBRA, None, Some(bar), 3.0, None);
        }
        self.advance(9.0);
    }

    fn math(&mut self, display: bool, text: &str) {
        if !display {
            let mut runs = self.runs_of(&[Inline::Math(text.to_string())]);
            for r in runs.iter_mut() {
                r.font = self.body_font;
            }
            self.runs_block(&runs, self.x0(), self.content_w(), LEADING, Align::Left, None);
            self.advance(BODY * 0.55);
            return;
        }
        // display maths is typeset and centred, not shown as source
        let rows: Vec<&str> = text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
        if rows.is_empty() {
            return;
        }
        let size = 12.0;
        let mut blocks = Vec::new();
        for r in &rows {
            blocks.push(crate::math::typeset(r, size, true));
        }
        let gap = size * 0.9;
        let w = blocks.iter().map(|b| b.width).fold(0.0f64, f64::max);
        let h: f64 = blocks.iter().map(|b| b.height).sum::<f64>()
            + blocks.iter().map(|b| b.depth).sum::<f64>()
            + gap * (blocks.len() as f64 - 1.0);
        self.advance(size * 0.5);
        self.need(h + size);
        let top = self.y + h;
        let mut ops: Vec<Op> = Vec::new();
        let mut y = top;
        for b in &blocks {
            y -= b.height;
            let x = self.x0() + (self.content_w() - w) / 2.0;
            math_ops(b, x, y, INK, &mut ops);
            y -= b.depth + gap;
        }
        for op in ops {
            self.push(op);
        }
        self.advance(h + size * 0.6);
    }

    fn note(&mut self, note: &Note) {
        let kind = note.kind.to_lowercase();
        let accent = match kind.as_str() {
            "tip" => Color::rgb(72, 160, 90),
            "warning" | "warn" | "caution" => Color::rgb(230, 162, 60),
            "danger" | "error" => Color::rgb(214, 69, 69),
            _ => Color::rgb(52, 106, 190),
        };
        let title_text = note.title.trim().to_string();
        let mut body: Vec<Inline> = note.body.clone();
        if !title_text.is_empty() {
            if let Some(first) = body.first() {
                let plain = first.plain();
                if plain.trim() == title_text {
                    body.remove(0);
                } else if plain.trim_start().starts_with(&title_text) {
                    let rest = plain.trim_start()[title_text.len()..].trim_start_matches(['\n', ' ']).to_string();
                    body[0] = Inline::Text(rest);
                }
            }
        }
        let label = if note.title.trim().is_empty() {
            kind.to_uppercase()
        } else {
            note.title.trim().to_string()
        };
        let indent = 12.0;
        let inner = (self.content_w() - indent).max(20.0);
        let mut runs = Vec::new();
        push_runs(&mut runs, &body, Style { font: self.body_font, weight: Weight::Regular, italic: false, size: 9.5, color: INK, dec: Dec::Plain, strike: false });
        let body_h = self.runs_height(&runs, inner, 1.4);
        let h = body_h + 16.0 + if label.is_empty() { 0.0 } else { text_line_height(9.0) + 2.0 };
        self.advance(7.0);
        self.need(h);
        let top = self.y;
        self.push(Op::Rect {
            x: self.x0(),
            y: top,
            w: self.content_w(),
            h,
            fill: Some(accent.mix(&PAPER, 0.93)),
            stroke: None,
            stroke_width: 0.0,
            radius: 3.0,
            alpha: 1.0,
        });
        self.push(Op::Rect { x: self.x0(), y: top, w: 3.0, h, fill: Some(accent), stroke: None, stroke_width: 0.0, radius: 0.0, alpha: 1.0 });
        let mut cursor = top + 7.0;
        if !label.is_empty() {
            let baseline = cursor + text_ascent(9.0);
            let op = self.text_op(self.body_font, Weight::Bold, false, 9.0, self.x0() + indent, baseline, &label, accent.mix(&INK, 0.25));
            self.push(op);
            cursor += text_line_height(9.0) + 2.0;
        }
        self.y = cursor;
        self.runs_block(&runs, self.x0() + indent, inner, 1.4, Align::Left, None);
        self.y = top + h;
        self.advance(9.0);
    }

    fn widget(&mut self, w: &Widget) {
        self.advance(4.0);
        if !w.label.trim().is_empty() {
            self.need(text_line_height(9.5));
            let baseline = self.y + text_ascent(9.5);
            let op = self.text_op(self.body_font, Weight::Regular, false, 9.5, self.x0(), baseline, &w.label, MUTED);
            self.push(op);
            self.advance(text_line_height(9.5) + 2.0);
        }
        let kind = w.kind.to_lowercase();
        let x = self.x0();
        let is_check = matches!(kind.as_str(), "checkbox" | "check" | "toggle" | "switch");
        if is_check {
            self.need(18.0);
            let top = self.y;
            self.push(Op::Rect {
                x,
                y: top,
                w: 9.0,
                h: 9.0,
                fill: Some(PAPER),
                stroke: Some(AXIS),
                stroke_width: 0.8,
                radius: 2.0,
                alpha: 1.0,
            });
            if w.checked {
                self.push(Op::Poly {
                    points: vec![Point::new(x + 2.0, top + 4.5), Point::new(x + 3.8, top + 6.6), Point::new(x + 7.0, top + 2.4)],
                    closed: false,
                    fill: None,
                    stroke: Some(ACCENT),
                    stroke_width: 1.2,
                    dash: Vec::new(),
                    alpha: 1.0,
                });
            }
            self.advance(16.0);
        } else if !w.options.is_empty() {
            let mut cx = x;
            let top = self.y;
            self.need(20.0);
            for (i, opt) in w.options.iter().enumerate() {
                let tw = measure(opt, self.body_font, Weight::Regular, 9.0);
                let pw = tw + 14.0;
                let selected = i == 0 && w.value == 0.0;
                self.push(Op::Rect {
                    x: cx,
                    y: top,
                    w: pw,
                    h: 15.0,
                    fill: Some(if selected { ACCENT } else { BOX_BG }),
                    stroke: Some(if selected { ACCENT } else { BOX_EDGE }),
                    stroke_width: 0.6,
                    radius: 7.5,
                    alpha: 1.0,
                });
                let op = self.text_op(
                    self.body_font,
                    Weight::Regular,
                    false,
                    9.0,
                    cx + 7.0,
                    top + 4.0 + text_ascent(9.0),
                    opt,
                    if selected { PAPER } else { SLATE },
                );
                self.push(op);
                cx += pw + 5.0;
                if cx > self.x0() + self.content_w() {
                    break;
                }
            }
            self.advance(20.0);
        } else if w.max > w.min {
            self.need(20.0);
            let track = (self.content_w() * 0.55).min(260.0);
            let top = self.y + 6.0;
            self.push(Op::Rect { x, y: top, w: track, h: 4.0, fill: Some(GRID), stroke: None, stroke_width: 0.0, radius: 2.0, alpha: 1.0 });
            let t = if (w.max - w.min).abs() < 1e-12 { 0.0 } else { ((w.value - w.min) / (w.max - w.min)).clamp(0.0, 1.0) };
            if t > 0.0 {
                self.push(Op::Rect { x, y: top, w: track * t, h: 4.0, fill: Some(ACCENT), stroke: None, stroke_width: 0.0, radius: 2.0, alpha: 1.0 });
            }
            let hx = x + track * t;
            self.push(Op::Rect { x: hx - 4.5, y: top - 2.5, w: 9.0, h: 9.0, fill: Some(PAPER), stroke: Some(ACCENT), stroke_width: 1.0, radius: 4.5, alpha: 1.0 });
            self.advance(16.0);
        } else {
            self.need(14.0);
            self.advance(14.0);
        }
        self.need(14.0);
        let baseline = self.y + text_ascent(9.0);
        let name = self.text_op(Font::Mono, Weight::Regular, false, 9.0, x, baseline, &w.name, SLATE);
        self.push(name);
        let value = if is_check {
            if w.checked { "checked".to_string() } else { "unchecked".to_string() }
        } else if !w.options.is_empty() {
            let idx = w.value.max(0.0) as usize;
            match w.options.get(idx) {
                Some(o) => format!("{} of {}  ({})", idx + 1, w.options.len(), o),
                None => fmt_sig(w.value, 4),
            }
        } else {
            format!(
                "{}   [{} .. {}] step {}",
                fmt_sig(w.value, 4),
                fmt_sig(w.min, 4),
                fmt_sig(w.max, 4),
                fmt_sig(w.step, 4)
            )
        };
        let vw = measure(&value, Font::Mono, Weight::Regular, 9.0);
        let op = self.text_op(Font::Mono, Weight::Regular, false, 9.0, x + self.content_w() - vw, baseline, &value, MUTED);
        self.push(op);
        self.advance(18.0);
    }

    fn raw(&mut self, text: &str) {
        let t = pdf_text(text);
        let wrapped = wrap_text(&t, Font::Mono, Weight::Regular, CODE_SIZE, self.content_w());
        let lines: Vec<PLine> = wrapped.iter().map(|l| PLine::new(l.clone(), CODE_INK)).collect();
        if lines.is_empty() {
            return;
        }
        let lead = CODE_LEAD;
        let total = lines.len() as f64 * lead;
        self.advance(4.0);
        if self.y + total > self.limit() && total < self.limit() - MARGIN_TOP {
            self.new_page();
        }
        for l in &lines {
            self.need(lead);
            let baseline = self.y + (lead - text_line_height(CODE_SIZE)) / 2.0 + text_ascent(CODE_SIZE);
            let op = self.text_op(Font::Mono, Weight::Regular, false, CODE_SIZE, self.x0(), baseline, &l.text, CODE_INK);
            self.push(op);
            self.advance(lead);
        }
        self.advance(8.0);
    }

    fn page_break(&mut self) {
        let empty = self.pages.last().map(|p| p.ops.is_empty()).unwrap_or(true);
        if empty && self.y <= MARGIN_TOP + 0.5 {
            return;
        }
        self.new_page();
    }

    fn toc(&mut self, title: &str) {
        self.advance(10.0);
        if !title.trim().is_empty() {
            self.need(24.0);
            let baseline = self.y + text_ascent(12.5);
            let op = self.text_op(self.body_font, Weight::Bold, false, 12.5, self.x0(), baseline, title, INK);
            self.push(op);
            self.advance(text_line_height(12.5) + 4.0);
        }
        let entries: Vec<(u8, String, usize)> = match self.toc {
            Some(list) => list.iter()
                    .filter(|h| h.level > 1 && h.level <= 3)
                    .map(|h| (h.level, h.title.clone(), h.page + 1))
                    .collect(),
            None => self
                .doc
                .headings()
                .into_iter()
                .filter(|(l, _, _)| *l > 1 && *l <= 3)
                .map(|(l, t, _)| (l, t, 1usize))
                .collect(),
        };
        if entries.is_empty() {
            self.need(16.0);
            let baseline = self.y + text_ascent(9.5);
            let op = self.text_op(self.body_font, Weight::Regular, true, 9.5, self.x0(), baseline, "No sections", MUTED);
            self.push(op);
            self.advance(16.0);
            return;
        }
        for (level, title, page) in entries {
            let indent = ((level as f64) - 1.0).max(0.0) * 12.0;
            let size = if level == 1 { 10.5 } else { 10.0 };
            let weight = if level == 1 { Weight::Bold } else { Weight::Regular };
            let lh = text_line_height(size) + 3.0;
            self.need(lh);
            let baseline = self.y + text_ascent(size);
            let label = ellipsize(&title, self.body_font, weight, size, self.content_w() - indent - 40.0);
            let op = self.text_op(self.body_font, weight, false, size, self.x0() + indent, baseline, &label, INK);
            self.push(op);
            let num = page.to_string();
            let nw = measure(&num, self.body_font, Weight::Regular, size);
            let num_x = self.x0() + self.content_w() - nw;
            let nop = self.text_op(self.body_font, Weight::Regular, false, size, num_x, baseline, &num, INK);
            self.push(nop);
            let lead_start = self.x0() + indent + measure(&label, self.body_font, weight, size) + 3.0;
            if lead_start < num_x - 3.0 {
                self.push(Op::Line {
                    x1: lead_start,
                    y1: baseline - 1.5,
                    x2: num_x - 3.0,
                    y2: baseline - 1.5,
                    color: GRID,
                    width: 0.5,
                    dash: vec![0.5, 2.0],
                    alpha: 1.0,
                });
            }
            self.advance(lh);
        }
        self.advance(10.0);
    }

    fn embed(&mut self, src: &str) -> Result<(String, f64, f64), ()> {
        let s = src.trim();
        if s.is_empty() || s.starts_with("http://") || s.starts_with("https://") || s.starts_with("data:") {
            return Err(());
        }
        let path = self.doc.resolve(s);
        let key = path.to_string_lossy().to_string();
        if !self.embeds.contains_key(&key) {
            let e = match fs::read(&path) {
                Ok(bytes) => {
                    if image::is_png(&bytes) {
                        match image::decode(&bytes) {
                            Ok(r) => Embed::Png { w: r.width, h: r.height, channels: r.channels, pixels: r.data },
                            Err(_) => Embed::Missing,
                        }
                    } else if image::is_jpeg(&bytes) {
                        match image::jpeg_info(&bytes) {
                            Some(i) => Embed::Jpeg { w: i.width, h: i.height, comps: i.components, bytes },
                            None => Embed::Missing,
                        }
                    } else {
                        Embed::Missing
                    }
                }
                Err(_) => Embed::Missing,
            };
            self.embeds.insert(key.clone(), e);
        }
        match self.embeds.get(&key) {
            Some(Embed::Png { w, h, .. }) => Ok((key, *w as f64 * 0.75, *h as f64 * 0.75)),
            Some(Embed::Jpeg { w, h, .. }) => Ok((key, *w as f64 * 0.75, *h as f64 * 0.75)),
            _ => Err(()),
        }
    }

    fn placeholder(&mut self, x: f64, y: f64, w: f64, h: f64, label: &str) {
        self.need(h);
        self.push(Op::Rect {
            x,
            y,
            w,
            h,
            fill: Some(BOX_BG),
            stroke: Some(BOX_EDGE),
            stroke_width: 0.6,
            radius: 4.0,
            alpha: 1.0,
        });
        let t = ellipsize(&pdf_text(label), Font::Sans, Weight::Regular, 8.5, (w - 16.0).max(20.0));
        let tw = measure(&t, Font::Sans, Weight::Regular, 8.5);
        let op = self.text_op(
            Font::Sans,
            Weight::Regular,
            false,
            8.5,
            x + (w - tw) / 2.0,
            y + h / 2.0 + text_ascent(8.5) / 2.0,
            &t,
            MUTED,
        );
        self.push(op);
    }

    fn caption_line(&mut self, caption: &str) {
        self.figure += 1;
        let label = format!("Figure {}. {}", self.figure, caption.trim());
        self.need(15.0);
        let baseline = self.y + text_ascent(9.0);
        self.centre(baseline, &label, self.body_font, Weight::Regular, true, 9.0, MUTED);
        self.advance(16.0);
    }

    fn svg_figure(&mut self, src: &str, alt: &str, caption: &str, width: Option<&String>) -> bool {
        let s = src.trim();
        if s.is_empty() || s.starts_with("http://") || s.starts_with("https://") || s.starts_with("data:") {
            return false;
        }
        let path = self.doc.resolve(s);
        if !path.to_string_lossy().to_ascii_lowercase().ends_with(".svg") {
            return false;
        }
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => return false,
        };
        let svg = match crate::svgpdf::parse(&text) {
            Ok(v) => v,
            Err(_) => return false,
        };
        if !(svg.width > 0.0) || !(svg.height > 0.0) {
            return false;
        }
        let cw = self.content_w();
        let mut w = parse_width(&width.cloned(), cw).min(cw);
        let mut h = (w * svg.height / svg.width).max(1.0);
        let cap_h = if !caption.trim().is_empty() { 18.0 } else { 0.0 };
        let avail = (self.limit() - MARGIN_TOP - 24.0 - cap_h).max(24.0);
        if h > avail {
            let k = avail / h;
            w *= k;
            h = avail;
        }
        self.need(h);
        let x = self.x0() + (cw - w) / 2.0;
        let top = self.y;
        let scale = w / svg.width;
        for item in &svg.items {
            self.scene_op(item, x, top, scale);
        }
        self.y = top + h;
        self.advance(7.0);
        let cap = if !caption.trim().is_empty() { caption.trim().to_string() } else { alt.trim().to_string() };
        if !cap.is_empty() {
            self.caption_line(&cap);
        }
        true
    }

    fn figure_block(&mut self, src: &str, alt: &str, caption: &str, width: Option<&String>) {
        let cw = self.content_w();
        if self.svg_figure(src, alt, caption, width) {
            self.advance(8.0);
            return;
        }
        match self.embed(src) {
            Ok((key, pw, ph)) => {
                let mut w = parse_width(&width.cloned(), cw).min(cw);
                let mut h = if pw > 0.0 { (w * ph / pw).max(1.0) } else { w * 0.6 };
                let avail = self.limit() - MARGIN_TOP - 24.0;
                if h > avail && h > 0.0 {
                    let s = avail / h;
                    w *= s;
                    h = avail;
                }
                self.need(h);
                let x = self.x0() + (cw - w) / 2.0;
                self.push(Op::Image { x, y: self.y, w, h, key, alpha: 1.0 });
                self.advance(h + 7.0);
                let cap = if !caption.trim().is_empty() { caption.trim().to_string() } else { alt.trim().to_string() };
                if !cap.is_empty() {
                    self.caption_line(&cap);
                }
            }
            Err(_) => {
                let w = parse_width(&width.cloned(), cw).min(cw);
                let h = if w > 240.0 { 90.0 } else { w * 0.5 };
                self.need(h);
                let x = self.x0() + (cw - w) / 2.0;
                self.placeholder(x, self.y, w, h, src);
                self.advance(h + 7.0);
                let cap = if !caption.trim().is_empty() { caption.trim().to_string() } else { alt.trim().to_string() };
                if !cap.is_empty() {
                    self.caption_line(&cap);
                }
            }
        }
        self.advance(8.0);
    }

    fn media(&mut self, m: &Media) {
        let cw = self.content_w();
        let w = parse_width(&m.width, cw).min(cw);
        match m.kind {
            MediaKind::Image => {
                let alt = if m.alt.trim().is_empty() { m.src.clone() } else { m.alt.clone() };
                self.advance(4.0);
                self.figure_block(&m.src, &alt, &m.caption, m.width.as_ref());
                return;
            }
            MediaKind::Video | MediaKind::Embed | MediaKind::Iframe => {
                let h = (w / 16.0 * 9.0).min(self.limit() - MARGIN_TOP - 60.0);
                self.advance(6.0);
                self.need(h);
                let x = self.x0() + (cw - w) / 2.0;
                let top = self.y;
                self.push(Op::Rect { x, y: top, w, h, fill: Some(SLATE), stroke: None, stroke_width: 0.0, radius: 6.0, alpha: 1.0 });
                let cx = x + w / 2.0;
                let cy = top + h / 2.0;
                let r = (h * 0.14).min(w * 0.07).max(9.0);
                self.push(Op::Poly {
                    points: vec![Point::new(cx - r * 0.6, cy - r), Point::new(cx + r, cy), Point::new(cx - r * 0.6, cy + r)],
                    closed: true,
                    fill: Some(PAPER),
                    stroke: None,
                    stroke_width: 0.0,
                    dash: Vec::new(),
                    alpha: 1.0,
                });
                self.y = top + h;
                self.advance(5.0);
                let note = match m.kind {
                    MediaKind::Video => "Video cannot be played in a PDF document",
                    _ => "Embedded content cannot be displayed in a PDF document",
                };
                self.need(14.0);
                let baseline = self.y + text_ascent(9.0);
                self.centre(baseline, note, self.body_font, Weight::Regular, true, 9.0, MUTED);
                self.advance(14.0);
            }
            MediaKind::Audio => {
                let h = 56.0;
                self.advance(6.0);
                self.need(h);
                let x = self.x0() + (cw - w) / 2.0;
                let top = self.y;
                self.push(Op::Rect {
                    x,
                    y: top,
                    w,
                    h,
                    fill: Some(BOX_BG),
                    stroke: Some(BOX_EDGE),
                    stroke_width: 0.6,
                    radius: 6.0,
                    alpha: 1.0,
                });
                let bar = (h * 0.5).clamp(6.0, 18.0);
                self.push(Op::Rect {
                    x: x + 12.0,
                    y: top + (h - bar) / 2.0,
                    w: bar,
                    h: bar,
                    fill: Some(ACCENT),
                    stroke: None,
                    stroke_width: 0.0,
                    radius: 2.0,
                    alpha: 1.0,
                });
                self.need(14.0);
                let nb = top + h / 2.0 + text_ascent(9.0) / 2.0;
                self.centre(nb, "Audio track", self.body_font, Weight::Bold, false, 9.0, INK);
                self.y = top + h;
                self.advance(5.0);
                self.need(14.0);
                let baseline = self.y + text_ascent(9.0);
                self.centre(
                    baseline,
                    "Audio cannot be played in a PDF document",
                    self.body_font,
                    Weight::Regular,
                    true,
                    9.0,
                    MUTED,
                );
                self.advance(14.0);
            }
        }
        let url = ellipsize(&pdf_text(&m.src), Font::Mono, Weight::Regular, 8.5, cw);
        self.need(13.0);
        let baseline = self.y + text_ascent(8.5);
        self.centre(baseline, &url, Font::Mono, Weight::Regular, false, 8.5, SLATE);
        self.advance(13.0);
        if !m.caption.trim().is_empty() {
            self.figure += 1;
            let label = format!("Figure {}. {}", self.figure, m.caption.trim());
            self.need(15.0);
            let baseline = self.y + text_ascent(9.0);
            self.centre(baseline, &label, self.body_font, Weight::Regular, true, 9.0, MUTED);
            self.advance(16.0);
        }
        self.advance(8.0);
    }

    fn scene_op(&mut self, item: &Item, ox: f64, oy: f64, s: f64) {
        let tx = |x: f64| ox + x * s;
        let ty = |y: f64| oy + y * s;
        match item {
            Item::Rect { x, y, w, h, fill, stroke, stroke_width, radius } => {
                let alpha = fill.map(|c| c.a).unwrap_or(1.0);
                self.push(Op::Rect {
                    x: tx(*x),
                    y: ty(*y),
                    w: w * s,
                    h: h * s,
                    fill: *fill,
                    stroke: *stroke,
                    stroke_width: (stroke_width * s).max(0.1),
                    radius: radius * s,
                    alpha,
                });
            }
            Item::Line { x1, y1, x2, y2, color, width, dash } => {
                self.push(Op::Line {
                    x1: tx(*x1),
                    y1: ty(*y1),
                    x2: tx(*x2),
                    y2: ty(*y2),
                    color: *color,
                    width: (width * s).max(0.1),
                    dash: dash.iter().map(|d| d * s).collect(),
                    alpha: color.a,
                });
            }
            Item::Poly { points, closed, fill, stroke, stroke_width, dash } => {
                let alpha = fill.map(|c| c.a).or(stroke.map(|c| c.a)).unwrap_or(1.0);
                self.push(Op::Poly {
                    points: points.iter().map(|p| Point::new(tx(p.x), ty(p.y))).collect(),
                    closed: *closed,
                    fill: *fill,
                    stroke: *stroke,
                    stroke_width: (stroke_width * s).max(0.1),
                    dash: dash.iter().map(|d| d * s).collect(),
                    alpha,
                });
            }
            Item::Text { x, y, text, size, color, font, weight, halign, valign, rotate } => {
                let size = size * s;
                let asc = text_ascent(size);
                let desc = size * 0.25;
                // Greek and maths operators have no WinAnsi code, so a label
                // containing any is drawn in runs, switching to the Symbol font
                // for just the glyphs that need it.
                let runs = split_symbol_runs(text, *font, *weight, size);
                let tw: f64 = runs.iter().map(|r| r.1).sum();
                let bx = match halign {
                    HAlign::Left => tx(*x),
                    HAlign::Center => tx(*x) - tw / 2.0,
                    HAlign::Right => tx(*x) - tw,
                };
                let by = match valign {
                    VAlign::Top => ty(*y) + asc,
                    VAlign::Middle => ty(*y) - (asc - desc) / 2.0,
                    VAlign::Bottom => ty(*y) - desc,
                };
                let mut pen = bx;
                for (body, width, is_symbol) in runs {
                    self.push(Op::Text {
                        symbol: is_symbol,
                        res: if is_symbol { "F12" } else { font_res(*font, *weight, false) },
                        size,
                        x: pen,
                        y: by,
                        text: body,
                        color: *color,
                        alpha: color.a,
                        angle: *rotate,
                    });
                    pen += width;
                }
            }
            Item::Image { x, y, w, h, src, alt } => match self.embed(src) {
                Ok((key, _, _)) => {
                    let iw = w * s;
                    let ih = h * s;
                    self.push(Op::Image { x: tx(*x), y: ty(*y), w: iw, h: ih, key, alpha: 1.0 });
                }
                Err(_) => {
                    let iw = w * s;
                    let ih = h * s;
                    let label = if alt.trim().is_empty() { src.clone() } else { alt.clone() };
                    self.placeholder(tx(*x), ty(*y), iw, ih, &label);
                    self.y = ty(*y) + ih;
                }
            },
        }
    }

    fn plot_block(&mut self, pb: &PlotBlock) -> Result<(), String> {
        let scene = plot::render(&pb.spec).map_err(|e| format!("plot: {}", e))?;
        let caption = if !pb.caption.trim().is_empty() { pb.caption.clone() } else { pb.spec.caption.clone() };
        let cw = self.content_w();
        let scale = if scene.width > cw && scene.width > 0.0 { cw / scene.width } else { 1.0 };
        let w = scene.width * scale;
        let mut h = scene.height * scale;
        let cap_h = if caption.trim().is_empty() { 0.0 } else { 18.0 };
        let avail = self.limit() - MARGIN_TOP - cap_h;
        if h > avail && h > 0.0 {
            h = avail;
        }
        self.advance(6.0);
        if self.y + h + cap_h > self.limit() && h + cap_h < self.limit() - MARGIN_TOP {
            self.new_page();
        }
        let top = self.y;
        if let Some(bg) = scene.background {
            self.push(Op::Rect {
                x: self.x0(),
                y: top,
                w,
                h,
                fill: Some(bg),
                stroke: None,
                stroke_width: 0.0,
                radius: 0.0,
                alpha: 1.0,
            });
        }
        for item in &scene.items {
            self.scene_op(item, self.x0(), top, scale);
        }
        self.y = top + h;
        self.advance(7.0);
        if !caption.trim().is_empty() {
            self.caption_line(&caption);
        }
        self.advance(6.0);
        Ok(())
    }

    fn align_of(t: &Table, j: usize) -> Align {
        t.align.get(j).copied().unwrap_or(Align::Left)
    }

    fn table_row(&mut self, row: &[String], widths: &[f64], rh: f64, size: f64, index: usize, aligns: &[Align], zebra: bool) {
        let x = self.x0();
        let top = self.y;
        let cw = self.content_w();
        if zebra && index % 2 == 1 {
            self.push(Op::Rect {
                x,
                y: top,
                w: cw,
                h: rh,
                fill: Some(ZEBRA),
                stroke: None,
                stroke_width: 0.0,
                radius: 0.0,
                alpha: 1.0,
            });
        }
        let baseline = top + (rh - text_line_height(size)) / 2.0 + text_ascent(size);
        let mut cx = x;
        for (j, w) in widths.iter().enumerate() {
            let align = aligns.get(j).copied().unwrap_or(Align::Left);
            let cell = ellipsize(row.get(j).map(|s| s.as_str()).unwrap_or(""), Font::Sans, Weight::Regular, size, (w - 2.0 * PAD).max(10.0));
            let tw = measure(&cell, Font::Sans, Weight::Regular, size);
            let tx = match align {
                Align::Left => cx + PAD,
                Align::Center => cx + (w - tw) / 2.0,
                Align::Right => cx + w - PAD - tw,
            };
            let op = self.text_op(Font::Sans, Weight::Regular, false, size, tx, baseline, &cell, INK);
            self.push(op);
            cx += w;
        }
        self.push(Op::Line {
            x1: x,
            y1: top + rh,
            x2: x + cw,
            y2: top + rh,
            color: GRID,
            width: 0.4,
            dash: Vec::new(),
            alpha: 1.0,
        });
        self.y = top + rh;
    }

    fn table_head(&mut self, head: &[String], widths: &[f64], rh: f64, size: f64, aligns: &[Align]) {
        let x = self.x0();
        let cw = self.content_w();
        let top = self.y;
        self.push(Op::Rect {
            x,
            y: top,
            w: cw,
            h: rh,
            fill: Some(HEAD_FILL),
            stroke: None,
            stroke_width: 0.0,
            radius: 0.0,
            alpha: 1.0,
        });
        self.push(Op::Line { x1: x, y1: top, x2: x + cw, y2: top, color: GRID, width: 0.4, dash: Vec::new(), alpha: 1.0 });
        let baseline = top + (rh - text_line_height(size)) / 2.0 + text_ascent(size);
        let mut cx = x;
        for (j, w) in widths.iter().enumerate() {
            let align = aligns.get(j).copied().unwrap_or(Align::Left);
            let cell = ellipsize(head.get(j).map(|s| s.as_str()).unwrap_or(""), Font::Sans, Weight::Bold, size, (w - 2.0 * PAD).max(10.0));
            let tw = measure(&cell, Font::Sans, Weight::Bold, size);
            let tx = match align {
                Align::Left => cx + PAD,
                Align::Center => cx + (w - tw) / 2.0,
                Align::Right => cx + w - PAD - tw,
            };
            let op = self.text_op(Font::Sans, Weight::Bold, false, size, tx, baseline, &cell, INK);
            self.push(op);
            cx += w;
        }
        self.push(Op::Line {
            x1: x,
            y1: top + rh,
            x2: x + cw,
            y2: top + rh,
            color: AXIS,
            width: 0.6,
            dash: Vec::new(),
            alpha: 1.0,
        });
        self.y = top + rh;
    }

    fn table(&mut self, t: &Table) {
        let size = 9.5;
        let ncol = t
            .columns
            .len()
            .max(t.rows.iter().map(|r| r.len()).max().unwrap_or(0))
            .max(1);
        let head: Vec<String> = (0..ncol).map(|j| pdf_text(t.columns.get(j).map(|s| s.as_str()).unwrap_or(""))).collect();
        let body: Vec<Vec<String>> = t
            .rows
            .iter()
            .map(|r| (0..ncol).map(|j| r.get(j).map(|v| pdf_text(&v.to_display())).unwrap_or_default()).collect())
            .collect();
        let total_rows = body.len();
        let cap = t.max_rows.map(|m| m.min(MAX_TABLE_ROWS)).unwrap_or(MAX_TABLE_ROWS);
        let shown = total_rows.min(cap);
        let cw = self.content_w();
        let mut nat = vec![0.0f64; ncol];
        for (j, w) in nat.iter_mut().enumerate() {
            let mut best = measure(&head[j], Font::Sans, Weight::Bold, size) + 2.0 * PAD;
            for row in body.iter().take(shown) {
                best = best.max(measure(&row[j], Font::Sans, Weight::Regular, size) + 2.0 * PAD);
            }
            *w = best.max(36.0);
        }
        let sum: f64 = nat.iter().sum();
        let mut widths: Vec<f64> = if sum <= cw && sum > 0.0 {
            nat.iter().map(|w| w + (cw - sum) * w / sum).collect()
        } else if sum > 0.0 {
            nat.iter().map(|w| (w * cw / sum).max(18.0)).collect()
        } else {
            vec![cw / ncol as f64; ncol]
        };
        let ws: f64 = widths.iter().sum();
        if ws > 0.0 {
            for w in widths.iter_mut() {
                *w *= cw / ws;
            }
        }
        let aligns: Vec<Align> = (0..ncol).map(|j| Layout::align_of(t, j)).collect();
        if !t.caption.trim().is_empty() {
            self.tables += 1;
            self.advance(8.0);
            self.need(16.0);
            let label = format!("Table {}. {}", self.tables, t.caption.trim());
            let baseline = self.y + text_ascent(9.0);
            self.centre(baseline, &label, self.body_font, Weight::Regular, true, 9.0, MUTED);
            self.advance(text_line_height(9.0) + 5.0);
        }
        self.advance(3.0);
        let rh = size * 1.3 + 6.0;
        self.table_head(&head, &widths, rh, size, &aligns);
        for (i, row) in body.iter().take(shown).enumerate() {
            if self.y + rh > self.limit() {
                self.new_page();
                self.table_head(&head, &widths, rh, size, &aligns);
            }
            self.table_row(row, &widths, rh, size, i, &aligns, t.zebra);
        }
        if total_rows > shown {
            self.need(rh);
            let top = self.y;
            let op = self.line_op(self.x0(), top, self.x0() + cw, top, GRID, 0.4);
            self.push(op);
            let label = format!("... showing {} of {} rows", shown, total_rows);
            self.advance((rh - text_line_height(8.5)) / 2.0 + text_ascent(8.5));
            let baseline = self.y;
            self.centre(baseline, &label, self.body_font, Weight::Regular, true, 8.5, MUTED);
            self.advance(rh);
        }
        self.advance(11.0);
    }

    fn block(&mut self, b: &Block) -> Result<(), String> {
        match b {
            Block::Heading { level, inlines, .. } => self.heading(*level, inlines),
            Block::Para(inlines) => self.para(inlines),
            Block::List { ordered, start, items } => self.list(*ordered, *start, items),
            Block::Checklist { items } => self.checklist(items),
            Block::Quote(inlines) => self.quote(inlines),
            Block::Rule => self.rule(),
            Block::Code { lang, text, result, .. } => self.code(lang, text, result.as_ref()),
            Block::Plot(pb) => self.plot_block(pb)?,
            Block::Table(t) => self.table(t),
            Block::Media(m) => self.media(m),
            Block::Math { display, text } => self.math(*display, text),
            Block::Note(n) => self.note(n),
            Block::Widget(w) => self.widget(w),
            Block::Toc { title } => self.toc(title),
            Block::Raw(s) => self.raw(s),
            Block::Directive(_) => {}
            Block::PageBreak => self.page_break(),
        }
        Ok(())
    }

    fn run(&mut self) -> Result<(), String> {
        let doc = self.doc;
        if !doc.meta.title.trim().is_empty() || !doc.meta.subtitle.trim().is_empty() {
            self.header();
        }
        for (i, b) in doc.blocks.iter().enumerate() {
            if let Err(e) = self.block(b) {
                return Err(format!("block {}: {}", i + 1, e));
            }
        }
        if self.pages.len() > 1 && self.pages.last().map(|p| p.ops.is_empty()).unwrap_or(false) {
            self.pages.pop();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{parse_document, run_cells, Doc};
    use std::path::{Path, PathBuf};

    const SRC: &str = r#"---
title: Nano Report
subtitle: A Tiny Study
author: Ada Lovelace, Bob Stone
date: 2026-01-01
description: A short summary.
theme: serif
---

## Intro {#intro}

A paragraph with **bold**, *em*, `code`, $E = mc^2$ and a [link](https://example.com/a/long/url).

@toc

- first item
- second item

### Details

> quoted line

***

@plot line
x = [0, 1, 2, 3, 4, 5]
y = [1, 4, 9, 16, 25, 36]
title = "Growth"
caption = "Squares"

```nano
let total = 1 + 1
print("sum is " + str(total))
total
```

@table
columns = ["name", "value"]
rows = [["a", 1], ["b", 2]]
align = l,r
caption = "Sample rows"

@note
kind = "warning"
title = "Careful"

Mind the gap.

@widget
kind = "slider"
name = "t"
label = "Gain"
min = 0
max = 1
value = 0.5

@math

\int_0^\infty e^{-x^2} dx

@video
src = "https://example.com/clip.mp4"

@pagebreak

- first item on a fresh page
"#;

    fn doc_of(src: &str, dir: &Path) -> Doc {
        let mut d = parse_document(src, dir);
        run_cells(&mut d);
        d
    }

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("nano-pdf-test").join(name);
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
        if from > hay.len() {
            return None;
        }
        hay[from..]
            .windows(needle.len())
            .position(|w| w == needle)
            .map(|p| p + from)
    }

    fn digits_at(bytes: &[u8], from: usize) -> (usize, usize) {
        let mut i = from;
        while i < bytes.len() && (bytes[i] as char).is_whitespace() {
            i += 1;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        (i, std::str::from_utf8(&bytes[start..i]).unwrap().parse::<usize>().unwrap_or(0))
    }

    fn content_streams(bytes: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let mut at = 0usize;
        while let Some(p) = find(bytes, b"/Length ", at) {
            let (after, len) = digits_at(bytes, p + 8);
            if let Some(s) = find(bytes, b"stream\n", after) {
                let start = s + 7;
                let head = bytes[..after].windows(5).rposition(|w| w == b" obj\n").unwrap_or(0);
                let image = find(&bytes[head..after], b"/Image", 0).is_some();
                if !image && start + len <= bytes.len() {
                    out.push(bytes[start..start + len].to_vec());
                    at = start + len;
                    continue;
                }
            }
            at = p + 8;
        }
        out
    }

    fn check_operators(data: &[u8]) -> usize {
        let mut depth = 0i32;
        let mut i = 0usize;
        let mut tj = 0usize;
        while i < data.len() {
            match data[i] {
                b'\\' => i += 1,
                b'(' => {
                    if depth == 0 && i > 0 && !data[i - 1].is_ascii_whitespace() {
                        panic!("literal string glued to the previous token at byte {}", i);
                    }
                    depth += 1;
                }
                b')' => {
                    depth -= 1;
                    if depth < 0 {
                        panic!("unbalanced parentheses at byte {}", i);
                    }
                    if data[i + 1..].starts_with(b" Tj") {
                        tj += 1;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        assert_eq!(depth, 0, "unterminated string in a content stream");
        tj
    }

    fn validate(bytes: &[u8]) -> usize {
        assert!(bytes.starts_with(b"%PDF-"), "missing header");
        assert!(bytes.ends_with(b"%%EOF\n"), "missing trailer marker");
        let at = find(bytes, b"startxref", bytes.len() / 2).expect("no startxref");
        let (end, start) = digits_at(bytes, at + b"startxref".len());
        assert!(start > 0, "startxref is zero");
        let xr = &bytes[start..];
        assert!(xr.starts_with(b"xref\n"), "startxref does not point at the table");
        let mut pos = 5usize;
        while pos < xr.len() && xr[pos].is_ascii_whitespace() {
            pos += 1;
        }
        while pos < xr.len() && !xr[pos].is_ascii_whitespace() {
            pos += 1;
        }
        while pos < xr.len() && xr[pos].is_ascii_whitespace() {
            pos += 1;
        }
        let count_at = pos;
        while pos < xr.len() && !xr[pos].is_ascii_whitespace() {
            pos += 1;
        }
        let count: usize = std::str::from_utf8(&xr[count_at..pos]).unwrap().parse().unwrap();
        while pos < xr.len() && xr[pos].is_ascii_whitespace() {
            pos += 1;
        }
        for id in 0..count {
            let entry = &xr[pos..pos + 20];
            assert_eq!(entry.len(), 20, "xref entry {} is not 20 bytes", id);
            let off: usize = std::str::from_utf8(&entry[..10]).unwrap().trim().parse().unwrap();
            if id == 0 {
                assert_eq!(off, 0);
                assert_eq!(&entry[17..18], b"f", "the free head entry is malformed");
            } else {
                assert_eq!(&entry[17..18], b"n", "object {} is not in use", id);
                let want = format!("{} 0 obj", id);
                assert_eq!(
                    find(bytes, want.as_bytes(), off),
                    Some(off),
                    "xref entry {} points at {} instead of `{}`",
                    id,
                    String::from_utf8_lossy(&bytes[off..(off + 24).min(bytes.len())]),
                    want
                );
            }
            pos += 20;
        }
        let streams = content_streams(bytes);
        assert!(!streams.is_empty(), "no content streams");
        let mut shown = 0usize;
        for s in &streams {
            shown += check_operators(s);
        }
        let _ = shown;
        let size_at = find(xr, b"/Size", 0).expect("no /Size");
        let (_, size) = digits_at(xr, size_at + 5);
        assert_eq!(size, count, "/Size disagrees with the table");
        let trailer = std::str::from_utf8(&xr[pos..]).unwrap_or("");
        assert!(trailer.contains("/Root 1 0 R"), "trailer: {}", trailer);
        assert!(trailer.contains("/Info"), "trailer has no /Info");
        end
    }

    #[test]
    fn renders_a_well_formed_pdf() {
        let bytes = render(&doc_of(SRC, Path::new("."))).unwrap();
        validate(&bytes);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/Type /Catalog"), "no catalog");
        assert!(text.contains("/Type /Pages"), "no page tree");
        assert!(text.contains("/Type /Page "), "no page object");
        assert!(text.contains("/Count 1") || text.contains("/Count 2"), "no page count");
        assert!(text.contains("/BaseFont /Helvetica"), "no base font");
        assert!(text.contains("/Encoding /WinAnsiEncoding"), "no encoding");
        assert!(text.contains("/ProcSet [/PDF /Text]"), "no procset");
        assert!(text.contains("/Outlines"), "no outline tree");
        assert!(text.contains("/Dest ["), "no outline destinations");
        assert!(text.contains("(nano)"), "no producer");
        assert!(text.contains("D:20240101000000Z"), "timestamp is not fixed");
        assert!(text.contains("/Title "), "no document title");
        assert!(text.contains("Tj"), "no text operators");
        assert!(text.contains("Figure 1. Squares"), "no figure caption");
        assert!(text.contains("Table 1. Sample rows"), "no table caption");
        assert!(text.contains(" rg\n") && text.contains("\nf\n"), "no fill operators");
        assert!(text.contains("q\n") && text.contains("Q\n"), "no graphics state blocks");
    }

    #[test]
    fn is_deterministic() {
        let a = render(&doc_of(SRC, Path::new("."))).unwrap();
        let b = render(&doc_of(SRC, Path::new("."))).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn table_of_contents_reports_page_numbers() {
        let bytes = render(&doc_of(SRC, Path::new("."))).unwrap();
        validate(&bytes);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("Contents"), "no toc heading");
        assert!(text.contains("Intro"), "no toc entry");
        let pages = text.matches("/Type /Page ").count();
        assert!(pages >= 2, "expected several pages, found {}", pages);
        assert!(text.contains("/Dest ["), "no outline destinations");
    }

    #[test]
    fn emits_fonts_and_xobjects() {
        let dir = tmp("assets");
        let png = image::make_png(1, 1, 3, &[200, 40, 90]);
        fs::write(dir.join("dot.png"), &png).unwrap();
        let src = format!(
            "{}\n\n@image\nsrc = \"dot.png\"\nalt = \"A dot\"\ncaption = \"Marker\"\n\n![inline](dot.png)\n",
            SRC
        );
        let bytes = render(&doc_of(&src, &dir)).unwrap();
        validate(&bytes);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/Type /Font"), "no font objects");
        assert!(text.contains("/XObject"), "no xobject resource");
        assert!(text.contains("/Subtype /Image"), "no image xobject");
        assert!(text.contains("/ColorSpace /DeviceRGB"), "no rgb image");
        assert!(text.contains("/Width 1 /Height 1"), "image has the wrong size");
        assert_eq!(text.matches("/Subtype /Image").count(), 1, "the image was not deduplicated");
        assert!(text.contains("Figure 1. Squares"), "no figure caption");
        assert!(text.contains("Marker"), "no image caption");
    }

    #[test]
    fn embeds_rgba_png_with_a_soft_mask() {
        let dir = tmp("alpha");
        let png = image::make_png(2, 2, 4, &[255, 0, 0, 255, 0, 0, 255, 128, 0, 255, 0, 0, 0, 0, 255, 64]);
        fs::write(dir.join("a.png"), &png).unwrap();
        let src = "@image\nsrc = \"a.png\"\nalt = \"alpha\"\n";
        let bytes = render(&doc_of(src, &dir)).unwrap();
        validate(&bytes);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/SMask"), "no soft mask");
        assert_eq!(text.matches("/Subtype /Image").count(), 2, "expected an image and a mask");
    }

    #[test]
    fn embeds_jpeg_verbatim_with_dctdecode() {
        let dir = tmp("jpeg");
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        jpeg.extend_from_slice(b"JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00");
        jpeg.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x40, 0x00, 0x60, 0x03]);
        jpeg.extend_from_slice(&[0xFF, 0xD9]);
        fs::write(dir.join("p.jpg"), &jpeg).unwrap();
        let src = "@image\nsrc = \"p.jpg\"\nalt = \"photo\"\n";
        let bytes = render(&doc_of(src, &dir)).unwrap();
        validate(&bytes);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/Filter /DCTDecode"), "jpeg was not passed through");
        assert!(text.contains("/Width 96 /Height 64"), "wrong jpeg size");
    }

    #[test]
    fn draws_svg_figures_as_vectors() {
        let dir = tmp("svg");
        let chart = r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="60" viewBox="0 0 120 60" font-family="Helvetica, sans-serif">
             <rect x="0" y="0" width="120" height="60" fill="#ffffff"/>
             <line x1="10" y1="50" x2="110" y2="50" stroke="#788090" stroke-width="1"/>
             <polygon points="10,50 40,10 70,50" fill="#346abe" stroke="none"/>
             <circle cx="95" cy="20" r="8" fill="none" stroke="#d64545" stroke-width="1.5"/>
             <path d="M10 58 Q 30 40 50 58" fill="none" stroke="#48a05a" stroke-width="1"/>
             <text x="60" y="30" font-size="9" fill="#1e222a" text-anchor="middle" dominant-baseline="central">vec</text>
           </svg>"##;
        fs::write(dir.join("chart.svg"), chart).unwrap();
        fs::write(dir.join("broken.svg"), "<svg width=\"10\" height=\"10\"><rect x=\"1\"").unwrap();
        let src = "@image\nsrc = \"chart.svg\"\nalt = \"chart\"\ncaption = \"Vector figure\"\n\n@image\nsrc = \"chart.svg\"\nwidth = \"50%\"\n\n@image\nsrc = \"broken.svg\"\nalt = \"broken\"\n\n![inline](chart.svg)\n";
        let bytes = render(&doc_of(src, &dir)).unwrap();
        validate(&bytes);
        let text = String::from_utf8_lossy(&bytes);
        let streams = content_streams(&bytes);
        let mut shown = 0usize;
        for s in &streams {
            shown += check_operators(s);
        }
        assert!(shown > 0, "no text operators were written");
        let ops: String = streams.iter().map(|s| String::from_utf8_lossy(s).to_string()).collect();
        let lines = ops.matches(" l\n").count();
        let curves = ops.matches(" c\n").count();
        assert!(lines > 20, "expected many line segments, found {}", lines);
        assert!(curves > 4, "expected curve operators, found {}", curves);
        assert!(ops.contains("(vec) Tj"), "the svg text was not drawn: {}", &ops[..ops.len().min(400)]);
        assert!(ops.contains(" RG\n") && ops.contains(" rg\n"), "no stroke or fill colours");
        assert!(!text.contains("/Subtype /Image"), "an svg must not be embedded as a raster");
        assert!(text.contains("Vector figure"), "no figure caption");
        assert!(text.contains("broken.svg"), "an unreadable svg did not fall back to a placeholder");
    }

    #[test]
    fn survives_empty_plots_huge_tables_and_missing_files() {
        let mut rows = String::from("[[\"row0\", 0]");
        for i in 1..600 {
            rows.push_str(&format!(", [\"row{}\", {}]", i, i * i));
        }
        rows.push(']');
        let src = format!(
            "@plot line\n\n@table\ncolumns = [\"k\", \"v\"]\nrows = {}\nzebra = true\n\n@image src = \"nowhere.png\" alt = \"gone\"\n\n@video src = \"clip.mp4\"\n\n@audio src = \"note.mp3\"\n\n@embed src = \"https://example.com\"\n\n@widget\nkind = \"checkbox\"\nname = \"flag\"\nchecked = true\n\n@widget\nkind = \"select\"\nname = \"pick\"\noptions = [\"a\", \"b\"]\n\n```nano\nboom(1)\n```\n\n::: raw html text\n\n@raw\n\n<i>markup</i>\n",
            rows
        );
        let bytes = render(&doc_of(&src, Path::new("."))).unwrap();
        validate(&bytes);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("showing 500 of 600 rows"), "no truncation footer");
        assert!(text.contains("nowhere.png"), "no placeholder label");
        assert!(text.contains("Video cannot be played"), "no video note");
        assert!(text.contains("Audio track"), "no audio note");
        assert!(text.contains("Embedded content"), "no embed note");
    }

    #[test]
    fn translucent_scene_fills_become_ext_g_states() {
        let src = "@plot bar\nlabels = [\"a\", \"b\", \"c\"]\ny = [3, 5, 2]\n";
        let bytes = render(&doc_of(src, Path::new("."))).unwrap();
        validate(&bytes);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/ExtGState <<"), "no extgstate resource");
        assert!(text.contains("/ca "), "no alpha channel");
        assert!(text.contains(" gs\n"), "no gs operator in the content stream");
    }

    #[test]
    fn reports_block_errors_without_panicking() {
        let mut d = Doc::default();
        d.blocks.push(crate::doc::Block::Para(vec![Inline::Text("before".into())]));
        let bad = render(&d);
        assert!(bad.is_ok() || bad.is_err());
        let mut d = Doc::default();
        d.blocks.push(crate::doc::Block::Plot(Box::new(crate::doc::PlotBlock {
            spec: crate::plot::PlotSpec::default(),
            caption: String::new(),
            id: String::new(),
        })));
        d.blocks.push(crate::doc::Block::PageBreak);
        d.blocks.push(crate::doc::Block::PageBreak);
        d.blocks.push(crate::doc::Block::Toc { title: "Contents".into() });
        d.blocks.push(crate::doc::Block::Widget(crate::doc::Widget {
            kind: "weird".into(),
            name: "x".into(),
            label: String::new(),
            min: 1.0,
            max: 0.0,
            step: 0.0,
            value: f64::NAN,
            options: Vec::new(),
            checked: false,
        }));
        let bytes = render(&d).unwrap();
        validate(&bytes);
    }

    /// Build a one-page PDF and hand back the bytes, validating as we go.
    fn pdf_of(src: &str) -> String {
        let mut doc = parse_document(src, std::path::Path::new("."));
        run_cells(&mut doc);
        let bytes = render(&doc).expect("pdf render failed");
        validate(&bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// y of every `Tm` in the content stream, in PDF order (y grows upward).
    fn text_baselines(c: &str) -> Vec<f64> {
        let mut out = Vec::new();
        for line in c.lines() {
            if let Some(rest) = line.trim().strip_suffix(" Tm") {
                if let Some(y) = rest.split_whitespace().last() {
                    if let Ok(v) = y.parse::<f64>() {
                        out.push(v);
                    }
                }
            }
        }
        out
    }

    /// y of every `m`/`l` path point in the content stream.
    fn path_ys(c: &str) -> Vec<f64> {
        let mut out = Vec::new();
        for line in c.lines() {
            let t = line.trim();
            if t.ends_with(" m") || t.ends_with(" l") {
                if let Some(y) = t.split_whitespace().nth(1) {
                    if let Ok(v) = y.parse::<f64>() {
                        out.push(v);
                    }
                }
            }
        }
        out
    }

    /// Math is laid out y-up but the op space is y-down; a sign slip here turns
    /// every fraction upside down and hangs radicals below their body.
    #[test]
    fn math_is_not_vertically_mirrored() {
        // \frac{a}{b} = \sqrt{x}  ->  Tm: a, b, =, x ;  path: radical
        let c = content_of("@math\n  \\frac{a}{b} = \\sqrt{x}\n");
        let t = text_baselines(&c);
        let (a, b, eq, x) = (t[0], t[1], t[2], t[3]);
        assert!(a > eq, "fraction numerator {a} must sit above the axis {eq}");
        assert!(b < eq, "fraction denominator {b} must sit below the axis {eq}");

        // the fraction rule is the first path, the radical overbar the highest
        let ys = path_ys(&c);
        let rule = ys[0];
        assert!(a > rule && rule > b, "fraction rule {rule} must sit between {b} and {a}");

        let overbar = ys.iter().cloned().fold(f64::MIN, f64::max);
        assert!(overbar > x, "radical overbar {overbar} must be above its body {x}");
    }

    #[test]
    fn accents_and_scripts_sit_above_the_baseline() {
        let c = content_of("@math\n  \\hat{\\beta} = X^{\\top}\n");
        let t = text_baselines(&c);
        // 0 = beta, 1 = '=', 2 = X, 3 = the exponent
        assert!(t[3] > t[2], "superscript {} must be above the base {}", t[3], t[2]);
        let ys = path_ys(&c);
        let hat = ys.iter().cloned().fold(f64::MIN, f64::max);
        assert!(hat > t[0], "hat {hat} must be above beta {}", t[0]);
    }

    /// The content stream, with the flate compression undone.
    fn content_of(src: &str) -> String {
        let raw = pdf_of(src).into_bytes();
        let mut out = String::new();
        let mut i = 0;
        loop {
            // "stream" also appears inside "endstream", so anchor on the keyword
            let at = match raw[i..].windows(7).position(|w| w == b"stream\n") {
                Some(p) => i + p + 7,
                None => break,
            };
            let end = match raw[at..].windows(9).position(|w| w == b"endstream") {
                Some(p) => at + p,
                None => break,
            };
            let mut body = &raw[at..end];
            while body.last().is_some_and(|c| *c == b'\n' || *c == b'\r') {
                body = &body[..body.len() - 1];
            }
            let text = match crate::image::inflate(body) {
                Ok(d) => String::from_utf8_lossy(&d).into_owned(),
                Err(_) => String::from_utf8_lossy(body).into_owned(),
            };
            out.push_str(&text);
            i = end + 9;
        }
        assert!(!out.is_empty(), "no content stream found");
        out
    }

    fn all_x_positions(c: &str) -> Vec<f64> {
        c.lines()
            .filter(|l| l.ends_with(" Tm"))
            .filter_map(|l| l.split_whitespace().nth(3)?.parse().ok())
            .collect()
    }

    #[test]
    fn inline_math_is_typeset_not_printed() {
        let c = content_of("Value $x^2 + y^2$ here.\n");
        // the source must not survive into the content stream
        assert!(!c.contains("x^2"), "raw LaTeX leaked into the PDF: {c}");
        // Times-Italic carries the variables
        assert!(c.contains("/F6"), "variables should use Times-Italic: {c}");
    }

    #[test]
    fn greek_uses_the_symbol_font_with_its_own_encoding() {
        let c = content_of("$\\alpha$ here.\n");
        assert!(c.contains("/F12"), "Greek needs the Symbol font: {c}");
        // 0x61 is alpha in the Symbol encoding
        assert!(c.contains("Tf\n1 0 0 1 ") && c.contains("(a) Tj"), "alpha not emitted: {c}");
    }

    #[test]
    fn symbol_font_must_not_carry_winansi() {
        let p = pdf_of("$\\alpha$\n");
        assert!(p.contains("/BaseFont /Symbol"), "Symbol font missing");
        // exactly one font may skip /Encoding, and it has to be Symbol
        let basefonts = p.matches("/BaseFont /").count();
        let with_enc = p.matches("/Encoding /WinAnsiEncoding").count();
        assert_eq!(basefonts - with_enc, 1, "only Symbol should skip /Encoding");
        let sym_at = p.find("/BaseFont /Symbol").unwrap();
        let enc_after = p[sym_at..].contains("/Encoding");
        assert!(!enc_after, "Symbol must keep its built-in encoding");
    }

    #[test]
    fn a_fraction_draws_a_rule_between_two_stacks() {
        let c = content_of("$\\frac{1}{2}$\n");
        // one stroked line for the fraction bar
        assert!(c.contains("\nl\n") || c.contains(" l\n"), "no rule drawn: {c}");
    }

    #[test]
    fn display_math_is_centred_and_not_shown_as_source() {
        let c = content_of("$$\n\\frac{a}{b}\n$$\n");
        assert!(!c.contains("frac"), "display math leaked its source: {c}");
        // no code panel is drawn any more
        assert!(!c.contains("/F7"), "display math should not use the mono font: {c}");
    }

    /// A wide formula must stay on the page: an earlier width bug pushed long
    /// formulas off the left edge, where they silently vanished.
    #[test]
    fn long_display_math_stays_inside_the_margins() {
        let c = content_of("$$\n\\int_0^\\infty e^{-x^2}\\,dx = \\frac{\\sqrt{\\pi}}{2}\n$$\n");
        let xs = all_x_positions(&c);
        assert!(!xs.is_empty(), "no glyphs placed: {c}");
        for x in xs {
            assert!(x > 0.0, "glyph drawn off the left edge at x={x}: {c}");
            assert!(x < 560.0, "glyph drawn off the right edge at x={x}: {c}");
        }
    }

    #[test]
    fn symbol_runs_are_split_out_of_plain_labels() {
        let runs = split_symbol_runs("rate \u{3B1}", Font::Sans, Weight::Regular, 10.0);
        assert_eq!(runs.len(), 2, "expected a text run and a Symbol run: {runs:?}");
        assert!(!runs[0].2, "the word stays in the text font");
        assert!(runs[1].2, "Greek switches to Symbol");
        // alpha is 0x61 in the Symbol encoding
        assert_eq!(runs[1].0, "a");
        // a label with nothing special needs exactly one run
        assert_eq!(split_symbol_runs("plain", Font::Sans, Weight::Regular, 10.0).len(), 1);
        assert_eq!(split_symbol_runs("", Font::Sans, Weight::Regular, 10.0).len(), 0);
    }

    #[test]
    fn greek_in_a_chart_label_reaches_the_symbol_font() {
        let c = content_of("@plot line\n  x = [0, 1]\n  y = [1, 2]\n  title = \"rate $\\alpha$\"\n");
        assert!(c.contains("/F12"), "Greek in a chart title should use Symbol: {c}");
    }

    #[test]
    fn dollar_display_blocks_become_math_blocks() {
        let doc = parse_document("$$\nx + y\n$$\n", std::path::Path::new("."));
        assert!(
            doc.blocks.iter().any(|b| matches!(b, Block::Math { display: true, .. })),
            "$$ ... $$ should parse as display math"
        );
        // a single-line form works too
        let doc = parse_document("$$x = 1$$\n", std::path::Path::new("."));
        assert!(
            doc.blocks.iter().any(|b| matches!(b, Block::Math { display: true, .. })),
            "single-line $$ ... $$ should parse as display math"
        );
    }

    #[test]
    fn inline_dollars_are_left_alone() {
        let doc = parse_document("cost is $5 and $6\n", std::path::Path::new("."));
        assert!(
            !doc.blocks.iter().any(|b| matches!(b, Block::Math { .. })),
            "bare $ amounts are not display math"
        );
    }

    #[test]
    fn pdf_text_folds_to_winansi() {
        assert_eq!(pdf_text("plain ascii"), "plain ascii");
        assert_eq!(pdf_text("caf\u{e9} \u{2014} r\u{e9}sum\u{e9}"), "cafe \u{0097} resume");
        assert_eq!(pdf_text("\u{2022} bullet"), "\u{0095} bullet");
        assert_eq!(pdf_text("a\tb\nc"), "a b c");
        assert_eq!(pdf_text("\u{4e2d}\u{6587}"), "??");
        assert_eq!(pdf_text("\u{2192}"), "->");
        assert!(measure("caf\u{e9}", Font::Sans, Weight::Regular, 10.0) > 0.0);
    }

    #[test]
    fn fmt_sig_rounds_to_four_significant_digits() {
        assert_eq!(fmt_sig(0.0, 4), "0");
        assert_eq!(fmt_sig(0.5, 4), "0.5");
        assert_eq!(fmt_sig(1.23456, 4), "1.235");
        assert_eq!(fmt_sig(12345.6, 4), "12350");
        assert_eq!(fmt_sig(0.000123456, 4), "0.0001235");
    }

    #[test]
    fn parse_width_accepts_percent_px_and_bare_numbers() {
        assert_eq!(parse_width(&None, 480.0), 480.0);
        assert_eq!(parse_width(&Some("50%".into()), 480.0), 240.0);
        assert_eq!(parse_width(&Some("300px".into()), 480.0), 300.0);
        assert_eq!(parse_width(&Some("120".into()), 480.0), 120.0);
        assert_eq!(parse_width(&Some("nonsense".into()), 480.0), 480.0);
    }

    #[test]
    fn page_geometry_matches_a4() {
        assert!((PAGE_W - 595.276).abs() < 0.001);
        assert!((PAGE_H - 841.89).abs() < 0.001);
        assert!((PAGE_W - 2.0 * MARGIN - 483.276).abs() < 0.001);
    }
}
