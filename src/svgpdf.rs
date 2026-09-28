use crate::render::{parse_color, Color, Font, HAlign, Item, Point, VAlign, Weight};

pub struct Svg {
    pub width: f64,
    pub height: f64,
    pub items: Vec<Item>,
}

const MAX_DEPTH: usize = 120;
const QUAD_STEPS: usize = 8;
const CURVE_STEPS: usize = 16;
const ARC_STEPS: usize = 8;
const KAPPA: f64 = 0.5522847498307933;

#[derive(Clone, Copy, Debug)]
struct Mat {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Mat {
    fn new() -> Mat {
        Mat { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 }
    }

    fn then(&self, m: Mat) -> Mat {
        Mat {
            a: self.a * m.a + self.c * m.b,
            b: self.b * m.a + self.d * m.b,
            c: self.a * m.c + self.c * m.d,
            d: self.b * m.c + self.d * m.d,
            e: self.a * m.e + self.c * m.f + self.e,
            f: self.b * m.e + self.d * m.f + self.f,
        }
    }

    fn xf(&self, x: f64, y: f64) -> Point {
        Point::new(self.a * x + self.c * y + self.e, self.b * x + self.d * y + self.f)
    }

    fn scale(&self) -> f64 {
        let s = (self.a * self.a + self.b * self.b).sqrt();
        if s.is_finite() && s > 1e-6 { s } else { 1.0 }
    }

    fn angle(&self) -> f64 {
        let s = self.scale();
        if s > 1e-6 { self.b.atan2(self.a).to_degrees() } else { 0.0 }
    }
}

#[derive(Clone)]
struct Style {
    fill: Option<Color>,
    stroke: Option<Color>,
    sw: f64,
    dash: Vec<f64>,
    linecap: String,
    opacity: f64,
    fill_op: f64,
    stroke_op: f64,
    size: f64,
    family: String,
    weight: Weight,
    italic: bool,
    anchor: HAlign,
    baseline: VAlign,
    underline: bool,
}

impl Default for Style {
    fn default() -> Style {
        Style {
            fill: Some(Color::rgb(0, 0, 0)),
            stroke: None,
            sw: 1.0,
            dash: Vec::new(),
            linecap: "butt".to_string(),
            opacity: 1.0,
            fill_op: 1.0,
            stroke_op: 1.0,
            size: 16.0,
            family: "sans-serif".to_string(),
            weight: Weight::Regular,
            italic: false,
            anchor: HAlign::Left,
            baseline: VAlign::Bottom,
            underline: false,
        }
    }
}

impl Style {
    fn set(&mut self, key: &str, raw: &str) {
        let v = raw.trim();
        if v.is_empty() {
            return;
        }
        match key {
            "fill" => self.fill = paint(v),
            "stroke" => self.stroke = paint(v),
            "stroke-width" => {
                if let Some(x) = parse_len(v) {
                    if x.is_finite() && x >= 0.0 {
                        self.sw = x;
                    }
                }
            }
            "stroke-dasharray" => {
                self.dash = if v.eq_ignore_ascii_case("none") {
                    Vec::new()
                } else {
                    let mut d = numbers(v);
                    d.retain(|x| x.is_finite() && *x >= 0.0);
                    d
                };
            }
            "stroke-linecap" => self.linecap = v.to_ascii_lowercase(),
            "opacity" => set_op(&mut self.opacity, v),
            "fill-opacity" => set_op(&mut self.fill_op, v),
            "stroke-opacity" => set_op(&mut self.stroke_op, v),
            "font-size" => {
                if let Some(x) = parse_len(v) {
                    if x.is_finite() && x > 0.0 {
                        self.size = x;
                    }
                }
            }
            "font-family" => self.family = v.to_string(),
            "font-weight" => {
                let w = v.to_ascii_lowercase();
                self.weight = if w == "bold" || w == "bolder" || num(&w).map(|n| n >= 600.0).unwrap_or(false) {
                    Weight::Bold
                } else {
                    Weight::Regular
                };
            }
            "font-style" => self.italic = v.to_ascii_lowercase().contains("italic"),
            "text-anchor" => {
                self.anchor = match v {
                    "middle" => HAlign::Center,
                    "end" => HAlign::Right,
                    _ => HAlign::Left,
                }
            }
            "dominant-baseline" | "alignment-baseline" => {
                self.baseline = match v {
                    "hanging" | "text-before-edge" | "top" | "before-edge" => VAlign::Top,
                    "central" | "middle" | "mathematical" => VAlign::Middle,
                    _ => VAlign::Bottom,
                }
            }
            "text-decoration" | "text-decoration-line" => self.underline = v.to_ascii_lowercase().contains("underline"),
            "color" => self.fill = paint(v),
            _ => {}
        }
    }

    fn fill(&self) -> Option<Color> {
        tint(self.fill, self.opacity * self.fill_op)
    }

    fn stroke(&self) -> Option<Color> {
        tint(self.stroke, self.opacity * self.stroke_op)
    }

    fn font(&self) -> Font {
        let f = self.family.to_ascii_lowercase();
        if f.contains("courier") || f.contains("mono") || f.contains("consolas") || f.contains("menlo") {
            Font::Mono
        } else if f.contains("times") || f.contains("georgia") || (f.contains("serif") && !f.contains("sans-serif")) {
            Font::Serif
        } else {
            Font::Sans
        }
    }
}

fn set_op(slot: &mut f64, v: &str) {
    if let Some(x) = num(v) {
        *slot = x.clamp(0.0, 1.0);
    }
}

fn num(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    match t.parse::<f64>() {
        Ok(v) if v.is_finite() => Some(v),
        _ => None,
    }
}

fn unit_factor(u: &str) -> Option<f64> {
    match u {
        "" | "px" => Some(1.0),
        "pt" => Some(96.0 / 72.0),
        "pc" => Some(16.0),
        "in" => Some(96.0),
        "mm" => Some(96.0 / 25.4),
        "cm" => Some(96.0 / 2.54),
        _ => None,
    }
}

fn parse_len(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let (v, next) = scan_number(t.as_bytes(), 0);
    let v = v?;
    let f = unit_factor(t[next..].trim().to_ascii_lowercase().as_str())?;
    Some(v * f)
}

fn parse_viewbox(s: &str) -> Option<(f64, f64, f64, f64)> {
    let mut v = numbers(s);
    v.retain(|x| x.is_finite());
    if v.len() < 4 {
        return None;
    }
    Some((v[0], v[1], v[2], v[3]))
}

fn numbers(s: &str) -> Vec<f64> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == b',' || (b[i] as char).is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let (v, next) = scan_number(b, i);
        if let Some(v) = v {
            out.push(v);
        }
        i = if next > i { next } else { i + 1 };
    }
    out
}

fn scan_number(b: &[u8], from: usize) -> (Option<f64>, usize) {
    let mut i = from;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let mut digits = 0usize;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        digits += 1;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return (None, from);
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let save = i;
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let d = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == d {
            i = save;
        }
    }
    let text = std::str::from_utf8(&b[from..i]).unwrap_or("");
    let v = text.parse::<f64>().ok().filter(|v| v.is_finite());
    (v, i)
}

fn channel(s: &str) -> Option<f64> {
    let t = s.trim();
    if let Some(p) = t.strip_suffix('%') {
        return num(p).map(|v| v * 2.55);
    }
    num(t)
}

fn paint(v: &str) -> Option<Color> {
    let s = v.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("none") || s.eq_ignore_ascii_case("transparent") {
        return None;
    }
    if let Some(hex) = s.strip_prefix('#') {
        let h = hex.trim();
        let good = h.len() == 3 || h.len() == 4 || h.len() == 6 || h.len() == 8;
        if good && h.chars().all(|c| c.is_ascii_hexdigit()) {
            let (r, g, b, a) = if h.len() <= 4 {
                let d = |i: usize| u8::from_str_radix(&h[i..i + 1], 16).unwrap_or(0);
                let r = d(0);
                let g = d(1);
                let b = d(2);
                let a = if h.len() == 4 { f64::from(d(3)) * 17.0 / 255.0 } else { 1.0 };
                (r * 17, g * 17, b * 17, a)
            } else {
                let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0);
                let a = if h.len() == 8 { f64::from(p(6)) / 255.0 } else { 1.0 };
                (p(0), p(2), p(4), a)
            };
            return Some(Color { r, g, b, a });
        }
    }
    let lower = s.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("rgba(").or_else(|| lower.strip_prefix("rgb(")) {
        if let Some(body) = rest.strip_suffix(')') {
            let parts: Vec<&str> = body.split(',').collect();
            if parts.len() >= 3 {
                let r = channel(parts[0]).unwrap_or(0.0);
                let g = channel(parts[1]).unwrap_or(0.0);
                let b = channel(parts[2]).unwrap_or(0.0);
                let mut a = 1.0;
                if let Some(p) = parts.get(3) {
                    let p = p.trim();
                    a = if let Some(pc) = p.strip_suffix('%') {
                        num(pc).unwrap_or(100.0) / 100.0
                    } else {
                        num(p).unwrap_or(1.0)
                    };
                    if a > 1.0 {
                        a /= 255.0;
                    }
                }
                return Some(Color {
                    r: r.clamp(0.0, 255.0).round() as u8,
                    g: g.clamp(0.0, 255.0).round() as u8,
                    b: b.clamp(0.0, 255.0).round() as u8,
                    a: a.clamp(0.0, 1.0),
                });
            }
        }
    }
    if let Some(c) = extra_named(&lower) {
        return Some(c);
    }
    if lower.starts_with('#') || lower.starts_with("rgb") {
        return None;
    }
    Some(parse_color(s, Color::rgb(0, 0, 0)))
}

fn extra_named(s: &str) -> Option<Color> {
    Some(match s {
        "silver" => Color::rgb(192, 192, 192),
        "yellow" => Color::rgb(226, 190, 62),
        "lime" => Color::rgb(150, 200, 60),
        "olive" => Color::rgb(128, 128, 40),
        "maroon" => Color::rgb(128, 40, 56),
        "aqua" => Color::rgb(60, 190, 190),
        "fuchsia" | "magenta" => Color::rgb(198, 70, 180),
        "darkgray" | "darkgrey" => Color::rgb(96, 96, 96),
        "lightblue" => Color::rgb(150, 190, 230),
        "lightgreen" => Color::rgb(140, 200, 130),
        "whitesmoke" => Color::rgb(245, 245, 245),
        _ => return Color::named(s),
    })
}

fn tint(c: Option<Color>, op: f64) -> Option<Color> {
    c.map(|c| {
        let a = (c.a * op).clamp(0.0, 1.0);
        c.with_alpha(a)
    })
}

fn r2(v: f64) -> f64 {
    if v.is_finite() {
        (v * 100.0).round() / 100.0
    } else {
        0.0
    }
}

struct Node {
    name: String,
    attrs: Vec<(String, String)>,
    children: Vec<Ev>,
}

impl Node {
    fn get(&self, key: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
}

enum Ev {
    Node(Node),
    Text(String),
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn ws(&mut self) {
        while self.i < self.s.len() && (self.s[self.i] as char).is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn starts(&self, p: &[u8]) -> bool {
        self.s[self.i..].starts_with(p)
    }

    fn at(&mut self, p: &[u8]) -> Result<(), String> {
        if self.starts(p) {
            self.i += p.len();
            Ok(())
        } else {
            Err(format!("svg: expected `{}` at byte {}", String::from_utf8_lossy(p), self.i))
        }
    }

    fn find(&mut self, p: &[u8]) -> Result<String, String> {
        let from = self.i;
        let hay = &self.s[from..];
        match hay.windows(p.len()).position(|w| w == p) {
            Some(at) => {
                let raw = std::str::from_utf8(&hay[..at]).map_err(|_| "svg: invalid utf-8".to_string())?;
                self.i = from + at + p.len();
                Ok(raw.to_string())
            }
            None => Err(format!("svg: unterminated `{}`", String::from_utf8_lossy(p))),
        }
    }

    fn junk(&mut self) -> Result<bool, String> {
        loop {
            self.ws();
            if self.starts(b"<!--") {
                self.i += 4;
                self.find(b"-->")?;
            } else if self.starts(b"<?") {
                self.i += 2;
                self.find(b"?>")?;
            } else if self.starts(b"<!") {
                self.i += 2;
                self.find(b">")?;
            } else {
                return Ok(false);
            }
        }
    }

    fn name(&mut self) -> Result<String, String> {
        let start = self.i;
        while self.i < self.s.len() {
            let c = self.s[self.i];
            if c.is_ascii_whitespace() || c == b'>' || c == b'/' || c == b'=' || c == b'<' {
                break;
            }
            self.i += 1;
        }
        if self.i == start {
            return Err(format!("svg: expected a tag name at byte {}", start));
        }
        let raw = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| "svg: invalid utf-8".to_string())?;
        Ok(raw.to_ascii_lowercase())
    }

    fn document(&mut self) -> Result<Vec<Ev>, String> {
        let mut out = Vec::new();
        loop {
            if self.junk()? {
                continue;
            }
            if self.i >= self.s.len() {
                return Ok(out);
            }
            if self.s[self.i] != b'<' {
                return Err(format!("svg: stray text at byte {}", self.i));
            }
            if self.starts(b"</") {
                return Err("svg: an end tag has no start tag".to_string());
            }
            out.push(self.node(0)?);
        }
    }

    fn node(&mut self, depth: usize) -> Result<Ev, String> {
        if depth > MAX_DEPTH {
            return Err("svg: elements are nested too deeply".to_string());
        }
        self.i += 1;
        let name = self.name()?;
        let mut attrs: Vec<(String, String)> = Vec::new();
        loop {
            self.ws();
            if self.i >= self.s.len() {
                return Err(format!("svg: <{}> is not terminated", name));
            }
            if self.s[self.i] == b'>' {
                self.i += 1;
                let children = self.children(&name, depth + 1)?;
                return Ok(Ev::Node(Node { name, attrs, children }));
            }
            if self.s[self.i] == b'/' {
                self.i += 1;
                self.at(b">")?;
                return Ok(Ev::Node(Node { name, attrs, children: Vec::new() }));
            }
            let key = self.name()?;
            self.ws();
            self.at(b"=")?;
            self.ws();
            if self.i >= self.s.len() {
                return Err("svg: attribute has no value".to_string());
            }
            let q = self.s[self.i];
            if q != b'"' && q != b'\'' {
                return Err("svg: attribute value is not quoted".to_string());
            }
            self.i += 1;
            let start = self.i;
            while self.i < self.s.len() && self.s[self.i] != q {
                self.i += 1;
            }
            if self.i >= self.s.len() {
                return Err(format!("svg: value of `{}` is not terminated", key));
            }
            let raw = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| "svg: invalid utf-8".to_string())?;
            self.i += 1;
            if !attrs.iter().any(|(k, _)| *k == key) {
                attrs.push((key, unescape(raw)));
            }
        }
    }

    fn children(&mut self, root: &str, depth: usize) -> Result<Vec<Ev>, String> {
        let mut out = Vec::new();
        loop {
            self.ws();
            if self.i >= self.s.len() {
                return Err(format!("svg: <{}> is never closed", root));
            }
            if self.starts(b"<![CDATA[") {
                self.i += 9;
                out.push(Ev::Text(self.find(b"]]>")?));
                continue;
            }
            if self.starts(b"</") {
                self.i += 2;
                let name = self.name()?;
                self.ws();
                self.at(b">")?;
                if name != root {
                    return Err(format!("svg: <{}> is closed by </{}>", root, name));
                }
                return Ok(out);
            }
            if self.junk()? {
                continue;
            }
            if self.s[self.i] == b'<' {
                out.push(self.node(depth)?);
                continue;
            }
            let start = self.i;
            while self.i < self.s.len() && self.s[self.i] != b'<' {
                self.i += 1;
            }
            let raw = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| "svg: invalid utf-8".to_string())?;
            out.push(Ev::Text(unescape(raw)));
        }
    }
}

fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    while i < b.len() {
        if b[i] != b'&' {
            let c = s[i..].chars().next().unwrap_or(' ');
            out.push(c);
            i += c.len_utf8();
            continue;
        }
        let rest = &s[i..];
        let end = match rest.find(';') {
            Some(e) if e <= 12 => i + e,
            _ => {
                out.push('&');
                i += 1;
                continue;
            }
        };
        let name = &s[i + 1..end];
        let rep = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some('\u{00A0}'),
            _ => {
                if let Some(h) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
                    u32::from_str_radix(h, 16).ok().and_then(char::from_u32)
                } else {
                    name.strip_prefix('#').and_then(|d| d.parse::<u32>().ok()).and_then(char::from_u32)
                }
            }
        };
        match rep {
            Some(c) => {
                out.push(c);
                i = end + 1;
            }
            None => {
                out.push('&');
                i += 1;
            }
        }
    }
    out
}

fn decls(style: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for part in style.split(';') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        match p.find(':') {
            Some(k) => {
                let key = p[..k].trim().to_ascii_lowercase();
                let val = p[k + 1..].trim();
                if !key.is_empty() && !val.is_empty() {
                    out.push((key, val.to_string()));
                }
            }
            None => {}
        }
    }
    out
}

fn apply_attrs(st: &mut Style, attrs: &[(String, String)]) -> bool {
    for (k, v) in attrs {
        match k.as_str() {
            "display" => {
                if v.trim().eq_ignore_ascii_case("none") {
                    return false;
                }
            }
            "visibility" => {
                let t = v.trim().to_ascii_lowercase();
                if t == "hidden" || t == "collapse" {
                    return false;
                }
            }
            _ => {}
        }
    }
    for (k, v) in attrs {
        if k != "style" {
            st.set(k, v);
        }
    }
    for (k, v) in attrs {
        if k == "style" {
            for (pk, pv) in decls(v) {
                st.set(&pk, &pv);
            }
        }
    }
    true
}

fn push_call(cur: &mut String, out: &mut Vec<(String, String)>) {
    let t = cur.trim().to_string();
    cur.clear();
    if t.is_empty() {
        return;
    }
    match t.find('(') {
        Some(k) => out.push((t[..k].trim().to_ascii_lowercase(), t[k + 1..].trim_end_matches(')').to_string())),
        None => out.push((t, String::new())),
    }
}

fn split_calls(s: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    for c in s.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth -= 1;
                cur.push(c);
            }
            c if depth <= 0 && (c == ',' || c.is_whitespace()) => push_call(&mut cur, &mut out),
            _ => cur.push(c),
        }
    }
    push_call(&mut cur, &mut out);
    out
}

fn transform_one(name: &str, args: &str) -> Option<Mat> {
    let mut v = numbers(args);
    v.retain(|x| x.is_finite());
    let at = |i: usize| v.get(i).copied();
    let need = |n: usize| if v.len() >= n { Some(()) } else { None };
    match name {
        "translate" => {
            need(1)?;
            let a = at(0)?;
            let b = at(1).unwrap_or(0.0);
            Some(Mat { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: a, f: b })
        }
        "scale" => {
            need(1)?;
            let a = at(0)?;
            let b = at(1).unwrap_or(a);
            Some(Mat { a, b: 0.0, c: 0.0, d: b, e: 0.0, f: 0.0 })
        }
        "rotate" => {
            need(1)?;
            let a = at(0)?;
            let r = a.to_radians();
            let rot = Mat { a: r.cos(), b: r.sin(), c: -r.sin(), d: r.cos(), e: 0.0, f: 0.0 };
            match (at(1), at(2)) {
                (Some(cx), Some(cy)) => Some(
                    Mat { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: cx, f: cy }
                        .then(rot)
                        .then(Mat { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: -cx, f: -cy }),
                ),
                _ => Some(rot),
            }
        }
        "skewx" => {
            need(1)?;
            Some(Mat { a: 1.0, b: 0.0, c: at(0)?.tan(), d: 1.0, e: 0.0, f: 0.0 })
        }
        "skewy" => {
            need(1)?;
            Some(Mat { a: 1.0, b: at(0)?.tan(), c: 0.0, d: 1.0, e: 0.0, f: 0.0 })
        }
        "matrix" => {
            need(6)?;
            Some(Mat { a: at(0)?, b: at(1)?, c: at(2)?, d: at(3)?, e: at(4)?, f: at(5)? })
        }
        _ => None,
    }
}

fn compose(base: Mat, list: &str) -> Mat {
    let mut m = base;
    for (name, args) in split_calls(list) {
        if let Some(t) = transform_one(&name, &args) {
            m = m.then(t);
        }
    }
    m
}

struct Ctx {
    m: Mat,
    st: Style,
}

pub fn parse(src: &str) -> Result<Svg, String> {
    if src.is_empty() {
        return Err("svg: the document is empty".to_string());
    }
    let mut p = Parser { s: src.as_bytes(), i: 0 };
    let top = p.document()?;
    let root = top
        .iter()
        .find_map(|e| match e {
            Ev::Node(n) if n.name == "svg" => Some(n),
            _ => None,
        })
        .ok_or_else(|| "svg: no <svg> element".to_string())?;
    let vb = root.get("viewbox").and_then(parse_viewbox).filter(|(_, _, w, h)| *w > 0.0 && *h > 0.0);
    let dw = root.get("width").and_then(parse_len).filter(|v| *v > 0.0);
    let dh = root.get("height").and_then(parse_len).filter(|v| *v > 0.0);
    let (width, height, base) = match (vb, dw, dh) {
        (Some((x, y, vw, vh)), Some(w), Some(h)) => (
            w,
            h,
            Mat { a: w / vw, b: 0.0, c: 0.0, d: h / vh, e: -x * w / vw, f: -y * h / vh },
        ),
        (Some((x, y, vw, vh)), _, _) => (vw, vh, Mat { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: -x, f: -y }),
        (None, Some(w), Some(h)) => (w, h, Mat::new()),
        _ => return Err("svg: the root <svg> has no usable size".to_string()),
    };
    let mut inherited = Style::default();
    if !apply_attrs(&mut inherited, &root.attrs) {
        inherited = Style::default();
    }
    let mut items: Vec<Item> = Vec::new();
    for ch in &root.children {
        if let Ev::Node(k) = ch {
            walk(k, &Ctx { m: base, st: inherited.clone() }, &mut items)?;
        }
    }
    Ok(Svg { width, height, items })
}

fn skip_subtree(name: &str) -> bool {
    matches!(
        name,
        "defs" | "symbol" | "marker" | "clippath" | "mask" | "pattern" | "filter" | "style" | "title" | "desc" | "metadata"
            | "script" | "use" | "image" | "foreignobject" | "lineargradient" | "radialgradient" | "animate" | "textpath" | "cursor"
            | "font" | "font-face" | "color-profile" | "switch"
    )
}

fn walk(node: &Node, ctx: &Ctx, out: &mut Vec<Item>) -> Result<(), String> {
    if skip_subtree(&node.name) {
        return Ok(());
    }
    let mut st = ctx.st.clone();
    if !apply_attrs(&mut st, &node.attrs) {
        return Ok(());
    }
    let mut m = ctx.m;
    if let Some(t) = node.get("transform") {
        m = compose(m, t);
    }
    match node.name.as_str() {
        "rect" => rect(node, &m, &st, out),
        "line" => line(node, &m, &st, out),
        "polyline" | "polygon" => return points_shape(node, &m, &st, out),
        "circle" | "ellipse" => oval(node, &m, &st, out),
        "path" => return path(node, &m, &st, out),
        "text" => text(node, &m, &st, out),
        _ => {
            for ch in &node.children {
                if let Ev::Node(k) = ch {
                    walk(k, &Ctx { m, st: st.clone() }, out)?;
                }
            }
        }
    }
    Ok(())
}

fn attr_f(node: &Node, key: &str, d: f64) -> f64 {
    node.get(key).and_then(parse_len).filter(|v| v.is_finite()).unwrap_or(d)
}

fn rect(node: &Node, m: &Mat, st: &Style, out: &mut Vec<Item>) {
    let x = attr_f(node, "x", 0.0);
    let y = attr_f(node, "y", 0.0);
    let w = attr_f(node, "width", 0.0).max(0.0);
    let h = attr_f(node, "height", 0.0).max(0.0);
    let s = m.scale();
    let rx = attr_f(node, "rx", 0.0).max(attr_f(node, "ry", 0.0)).max(0.0);
    let skew = m.b.abs() > 1e-6 || m.c.abs() > 1e-6 || (m.d - m.a).abs() > 1e-6;
    if skew {
        let corners = [m.xf(x, y), m.xf(x + w, y), m.xf(x + w, y + h), m.xf(x, y + h)];
        out.push(Item::Poly {
            points: corners.iter().map(|p| Point::new(r2(p.x), r2(p.y))).collect(),
            closed: true,
            fill: st.fill(),
            stroke: st.stroke(),
            stroke_width: r2(st.sw * s),
            dash: st.dash.iter().map(|d| r2(d * s)).collect(),
        });
        return;
    }
    let p = m.xf(x, y);
    let q = m.xf(x + w, y + h);
    out.push(Item::Rect {
        x: r2(p.x),
        y: r2(p.y),
        w: r2((q.x - p.x).abs()),
        h: r2((q.y - p.y).abs()),
        fill: st.fill(),
        stroke: st.stroke(),
        stroke_width: r2(st.sw * s),
        radius: r2((rx * s).min((w * s).abs() / 2.0).min((h * s).abs() / 2.0)),
    });
}

fn line(node: &Node, m: &Mat, st: &Style, out: &mut Vec<Item>) {
    let s = m.scale();
    let a = m.xf(attr_f(node, "x1", 0.0), attr_f(node, "y1", 0.0));
    let b = m.xf(attr_f(node, "x2", 0.0), attr_f(node, "y2", 0.0));
    if let Some(c) = st.stroke() {
        out.push(Item::Line {
            x1: r2(a.x),
            y1: r2(a.y),
            x2: r2(b.x),
            y2: r2(b.y),
            color: c,
            width: r2((st.sw * s).max(0.0)),
            dash: st.dash.iter().map(|d| r2(d * s)).collect(),
        });
    }
}

fn points_shape(node: &Node, m: &Mat, st: &Style, out: &mut Vec<Item>) -> Result<(), String> {
    let closed = node.name == "polygon";
    let raw = numbers(node.get("points").unwrap_or(""));
    if raw.len() % 2 != 0 {
        return Err(format!("svg: <{}> has an odd number of coordinates", node.name));
    }
    let mut pts: Vec<Point> = Vec::with_capacity(raw.len() / 2);
    let mut i = 0usize;
    while i + 1 < raw.len() {
        pts.push(m.xf(raw[i], raw[i + 1]));
        i += 2;
    }
    if pts.is_empty() {
        return Ok(());
    }
    let fill = if closed { st.fill() } else { None };
    out.push(Item::Poly {
        points: pts.iter().map(|p| Point::new(r2(p.x), r2(p.y))).collect(),
        closed,
        fill,
        stroke: st.stroke(),
        stroke_width: r2(st.sw * m.scale()),
        dash: st.dash.iter().map(|d| r2(d * m.scale())).collect(),
    });
    Ok(())
}

fn sample_cubic(p0: Point, c1: Point, c2: Point, p1: Point, steps: usize, pts: &mut Vec<Point>) {
    for k in 1..=steps {
        let t = k as f64 / steps as f64;
        let u = 1.0 - t;
        let w0 = u * u * u;
        let w1 = 3.0 * u * u * t;
        let w2 = 3.0 * u * t * t;
        let w3 = t * t * t;
        pts.push(Point::new(
            w0 * p0.x + w1 * c1.x + w2 * c2.x + w3 * p1.x,
            w0 * p0.y + w1 * c1.y + w2 * c2.y + w3 * p1.y,
        ));
    }
}

fn ellipse_points(cx: f64, cy: f64, rx: f64, ry: f64) -> Vec<Point> {
    let kx = rx * KAPPA;
    let ky = ry * KAPPA;
    let mut pts = vec![Point::new(cx + rx, cy)];
    let p0 = Point::new(cx + rx, cy);
    sample_cubic(p0, Point::new(cx + rx, cy + ky), Point::new(cx + kx, cy + ry), Point::new(cx, cy + ry), QUAD_STEPS, &mut pts);
    sample_cubic(Point::new(cx, cy + ry), Point::new(cx - kx, cy + ry), Point::new(cx - rx, cy + ky), Point::new(cx - rx, cy), QUAD_STEPS, &mut pts);
    sample_cubic(Point::new(cx - rx, cy), Point::new(cx - rx, cy - ky), Point::new(cx - kx, cy - ry), Point::new(cx, cy - ry), QUAD_STEPS, &mut pts);
    sample_cubic(Point::new(cx, cy - ry), Point::new(cx + kx, cy - ry), Point::new(cx + rx, cy - ky), Point::new(cx + rx, cy), QUAD_STEPS, &mut pts);
    pts
}

fn oval(node: &Node, m: &Mat, st: &Style, out: &mut Vec<Item>) {
    let cx = attr_f(node, "cx", 0.0);
    let cy = attr_f(node, "cy", 0.0);
    let (rx, ry) = if node.name == "circle" {
        let r = attr_f(node, "r", 0.0).abs();
        (r, r)
    } else {
        (attr_f(node, "rx", 0.0).abs(), attr_f(node, "ry", 0.0).abs())
    };
    if rx <= 0.0 || ry <= 0.0 {
        return;
    }
    let s = m.scale();
    let mut pts = ellipse_points(cx, cy, rx, ry);
    for p in pts.iter_mut() {
        *p = m.xf(p.x, p.y);
    }
    out.push(Item::Poly {
        points: pts.iter().map(|p| Point::new(r2(p.x), r2(p.y))).collect(),
        closed: true,
        fill: st.fill(),
        stroke: st.stroke(),
        stroke_width: r2(st.sw * s),
        dash: st.dash.iter().map(|d| r2(d * s)).collect(),
    });
}

struct PathLex<'a> {
    s: &'a str,
    b: &'a [u8],
    i: usize,
}

impl<'a> PathLex<'a> {
    fn skip(&mut self) {
        while self.i < self.b.len() && (self.b[self.i] == b',' || (self.b[self.i] as char).is_ascii_whitespace()) {
            self.i += 1;
        }
    }

    fn word(&mut self) -> Option<char> {
        self.skip();
        if self.i < self.b.len() && self.b[self.i].is_ascii_alphabetic() {
            let c = self.b[self.i] as char;
            self.i += 1;
            Some(c)
        } else {
            None
        }
    }

    fn number(&mut self) -> Option<f64> {
        self.skip();
        let start = self.i;
        if self.i < self.b.len() && (self.b[self.i] == b'+' || self.b[self.i] == b'-') {
            self.i += 1;
        }
        while self.i < self.b.len() && self.b[self.i].is_ascii_digit() {
            self.i += 1;
        }
        if self.i < self.b.len() && self.b[self.i] == b'.' {
            self.i += 1;
            while self.i < self.b.len() && self.b[self.i].is_ascii_digit() {
                self.i += 1;
            }
        }
        if self.i == start {
            return None;
        }
        match self.s[start..self.i].parse::<f64>() {
            Ok(v) if v.is_finite() => Some(v),
            _ => {
                self.i = start + 1;
                None
            }
        }
    }

    fn flag(&mut self) -> Option<bool> {
        self.skip();
        if self.i < self.b.len() && (self.b[self.i] == b'0' || self.b[self.i] == b'1') {
            let v = self.b[self.i] == b'1';
            self.i += 1;
            Some(v)
        } else {
            None
        }
    }

    fn done(&mut self) -> bool {
        self.skip();
        self.i >= self.b.len()
    }

    fn next_is_word(&mut self) -> bool {
        self.skip();
        self.i < self.b.len() && self.b[self.i].is_ascii_alphabetic()
    }
}

struct Sub {
    pts: Vec<Point>,
    closed: bool,
}

fn flatten(d: &str) -> Result<Vec<Sub>, String> {
    let mut lx = PathLex { s: d, b: d.as_bytes(), i: 0 };
    let mut subs: Vec<Sub> = Vec::new();
    let mut cur = Sub { pts: Vec::new(), closed: false };
    let mut at = Point::new(0.0, 0.0);
    let mut origin = Point::new(0.0, 0.0);
    let mut last_c: Option<Point> = None;
    let mut last_q: Option<Point> = None;
    let mut cmd = ' ';
    let mut started = false;
    loop {
        if let Some(c) = lx.word() {
            cmd = c;
        } else if cmd == ' ' || lx.done() {
            break;
        }
        let rel = cmd.is_ascii_lowercase();
        let up = cmd.to_ascii_uppercase();
        if !started && up != 'M' {
            return Err(format!("svg: the path starts with `{}` instead of a moveto", cmd));
        }
        started = true;
        match up {
            'Z' => {
                if !cur.pts.is_empty() {
                    cur.closed = true;
                    subs.push(std::mem::replace(&mut cur, Sub { pts: Vec::new(), closed: false }));
                }
                at = origin;
                last_c = None;
                last_q = None;
                cmd = 'Z';
                if lx.next_is_word() {
                    continue;
                }
                break;
            }
            'M' => {
                let (x, y) = pair(&mut lx)?;
                at = if rel { Point::new(at.x + x, at.y + y) } else { Point::new(x, y) };
                if !cur.pts.is_empty() {
                    subs.push(std::mem::replace(&mut cur, Sub { pts: Vec::new(), closed: false }));
                }
                origin = at;
                cur.pts.push(at);
                last_c = None;
                last_q = None;
                cmd = if rel { 'l' } else { 'L' };
            }
            'L' => {
                let (x, y) = pair(&mut lx)?;
                at = if rel { Point::new(at.x + x, at.y + y) } else { Point::new(x, y) };
                push_pt(&mut cur, at);
                last_c = None;
                last_q = None;
            }
            'H' => {
                let x = lx.number().ok_or_else(|| "svg: H needs a length".to_string())?;
                at = Point::new(if rel { at.x + x } else { x }, at.y);
                push_pt(&mut cur, at);
                last_c = None;
                last_q = None;
            }
            'V' => {
                let y = lx.number().ok_or_else(|| "svg: V needs a length".to_string())?;
                at = Point::new(at.x, if rel { at.y + y } else { y });
                push_pt(&mut cur, at);
                last_c = None;
                last_q = None;
            }
            'C' => {
                let (x1, y1) = pair(&mut lx)?;
                let (x2, y2) = pair(&mut lx)?;
                let (x, y) = pair(&mut lx)?;
                let c1 = if rel { Point::new(at.x + x1, at.y + y1) } else { Point::new(x1, y1) };
                let c2 = if rel { Point::new(at.x + x2, at.y + y2) } else { Point::new(x2, y2) };
                let p = if rel { Point::new(at.x + x, at.y + y) } else { Point::new(x, y) };
                sample_cubic(at, c1, c2, p, CURVE_STEPS, &mut cur.pts);
                at = p;
                last_c = Some(c2);
                last_q = None;
            }
            'S' => {
                let (x2, y2) = pair(&mut lx)?;
                let (x, y) = pair(&mut lx)?;
                let c1 = reflect(at, last_c);
                let c2 = if rel { Point::new(at.x + x2, at.y + y2) } else { Point::new(x2, y2) };
                let p = if rel { Point::new(at.x + x, at.y + y) } else { Point::new(x, y) };
                sample_cubic(at, c1, c2, p, CURVE_STEPS, &mut cur.pts);
                at = p;
                last_c = Some(c2);
                last_q = None;
            }
            'Q' => {
                let (x1, y1) = pair(&mut lx)?;
                let (x, y) = pair(&mut lx)?;
                let q = if rel { Point::new(at.x + x1, at.y + y1) } else { Point::new(x1, y1) };
                let p = if rel { Point::new(at.x + x, at.y + y) } else { Point::new(x, y) };
                let c1 = Point::new(at.x + 2.0 / 3.0 * (q.x - at.x), at.y + 2.0 / 3.0 * (q.y - at.y));
                let c2 = Point::new(p.x + 2.0 / 3.0 * (q.x - p.x), p.y + 2.0 / 3.0 * (q.y - p.y));
                sample_cubic(at, c1, c2, p, CURVE_STEPS, &mut cur.pts);
                at = p;
                last_q = Some(q);
                last_c = None;
            }
            'T' => {
                let (x, y) = pair(&mut lx)?;
                let q = reflect(at, last_q);
                let p = if rel { Point::new(at.x + x, at.y + y) } else { Point::new(x, y) };
                let c1 = Point::new(at.x + 2.0 / 3.0 * (q.x - at.x), at.y + 2.0 / 3.0 * (q.y - at.y));
                let c2 = Point::new(p.x + 2.0 / 3.0 * (q.x - p.x), p.y + 2.0 / 3.0 * (q.y - p.y));
                sample_cubic(at, c1, c2, p, CURVE_STEPS, &mut cur.pts);
                at = p;
                last_q = Some(q);
                last_c = None;
            }
            'A' => {
                let rx = lx.number().ok_or_else(|| "svg: A needs rx".to_string())?;
                let ry = lx.number().ok_or_else(|| "svg: A needs ry".to_string())?;
                let rot = lx.number().ok_or_else(|| "svg: A needs a rotation".to_string())?;
                let large = lx.flag().ok_or_else(|| "svg: A needs the large-arc flag".to_string())?;
                let sweep = lx.flag().ok_or_else(|| "svg: A needs the sweep flag".to_string())?;
                let (x, y) = pair(&mut lx)?;
                let p = if rel { Point::new(at.x + x, at.y + y) } else { Point::new(x, y) };
                arc_points(at, p, rx, ry, rot, large, sweep, &mut cur.pts);
                at = p;
                last_c = None;
                last_q = None;
            }
            _ => return Err(format!("svg: `{}` is not a path command", cmd)),
        }
        if lx.done() {
            break;
        }
    }
    if !cur.pts.is_empty() {
        subs.push(cur);
    }
    if subs.is_empty() {
        return Err("svg: the path has no points".to_string());
    }
    Ok(subs)
}

fn pair(lx: &mut PathLex) -> Result<(f64, f64), String> {
    let x = lx.number().ok_or_else(|| "svg: a coordinate is missing".to_string())?;
    let y = lx.number().ok_or_else(|| "svg: a coordinate is missing".to_string())?;
    Ok((x, y))
}

fn push_pt(cur: &mut Sub, p: Point) {
    if let Some(last) = cur.pts.last() {
        if (last.x - p.x).abs() < 1e-12 && (last.y - p.y).abs() < 1e-12 {
            return;
        }
    }
    cur.pts.push(p);
}

fn reflect(at: Point, prev: Option<Point>) -> Point {
    match prev {
        Some(p) => Point::new(2.0 * at.x - p.x, 2.0 * at.y - p.y),
        None => at,
    }
}

fn arc_points(from: Point, to: Point, rx: f64, ry: f64, rot: f64, large: bool, sweep: bool, out: &mut Vec<Point>) {
    if (from.x - to.x).abs() < 1e-12 && (from.y - to.y).abs() < 1e-12 {
        return;
    }
    let mut rx = rx.abs();
    let mut ry = ry.abs();
    if rx < 1e-12 || ry < 1e-12 {
        out.push(to);
        return;
    }
    let phi = rot.to_radians();
    let (sp, cp) = (phi.sin(), phi.cos());
    let dx = (from.x - to.x) / 2.0;
    let dy = (from.y - to.y) / 2.0;
    let x1 = cp * dx + sp * dy;
    let y1 = -sp * dx + cp * dy;
    let lam = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lam > 1.0 {
        let k = lam.sqrt();
        rx *= k;
        ry *= k;
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut co = if den.abs() < 1e-12 { 0.0 } else { (num / den).max(0.0).sqrt() };
    if large == sweep {
        co = -co;
    }
    let cxp = co * rx * y1 / ry;
    let cyp = -co * ry * x1 / rx;
    let cx = cp * cxp - sp * cyp + (from.x + to.x) / 2.0;
    let cy = sp * cxp + cp * cyp + (from.y + to.y) / 2.0;
    let ux = (x1 - cxp) / rx;
    let uy = (y1 - cyp) / ry;
    let vx = (-x1 - cxp) / rx;
    let vy = (-y1 - cyp) / ry;
    let theta = angle_between(1.0, 0.0, ux, uy);
    let mut delta = angle_between(ux, uy, vx, vy);
    if !sweep && delta > 0.0 {
        delta -= std::f64::consts::TAU;
    } else if sweep && delta < 0.0 {
        delta += std::f64::consts::TAU;
    }
    let steps = ((delta.abs() / std::f64::consts::FRAC_PI_2).ceil() as usize).clamp(1, 8);
    let seg = delta / steps as f64;
    let k = 4.0 / 3.0 * (seg / 4.0).tan();
    let mut th = theta;
    let mut p0 = from;
    for _ in 0..steps {
        let th2 = th + seg;
        let p1 = arc_point(cx, cy, rx, ry, cp, sp, th2);
        let c1 = arc_point(cx, cy, rx, ry, cp, sp, th + k * (th2 - th));
        let c2 = arc_point(cx, cy, rx, ry, cp, sp, th2 - k * (th2 - th));
        sample_cubic(p0, c1, c2, p1, ARC_STEPS, out);
        p0 = p1;
        th = th2;
    }
}

fn arc_point(cx: f64, cy: f64, rx: f64, ry: f64, cp: f64, sp: f64, th: f64) -> Point {
    let x = rx * th.cos();
    let y = ry * th.sin();
    Point::new(cp * x - sp * y + cx, sp * x + cp * y + cy)
}

fn angle_between(ux: f64, uy: f64, vx: f64, vy: f64) -> f64 {
    let dot = ux * vx + uy * vy;
    let len = ((ux * ux + uy * uy) * (vx * vx + vy * vy)).sqrt();
    if len < 1e-12 {
        return 0.0;
    }
    let mut a = (dot / len).clamp(-1.0, 1.0).acos();
    if ux * vy - uy * vx < 0.0 {
        a = -a;
    }
    a
}

fn path(node: &Node, m: &Mat, st: &Style, out: &mut Vec<Item>) -> Result<(), String> {
    let d = node.get("d").unwrap_or("").trim();
    if d.is_empty() {
        return Err("svg: <path> has no `d` attribute".to_string());
    }
    let subs = flatten(d)?;
    let s = m.scale();
    let fill = st.fill();
    let stroke = st.stroke();
    let sw = r2(st.sw * s);
    let dash: Vec<f64> = st.dash.iter().map(|d| r2(d * s)).collect();
    let mut count = 0usize;
    for sub in subs {
        if sub.pts.len() < 2 {
            continue;
        }
        let f = if sub.closed || stroke.is_none() { fill } else { None };
        out.push(Item::Poly {
            points: sub.pts.iter().map(|p| { let q = m.xf(p.x, p.y); Point::new(r2(q.x), r2(q.y)) }).collect(),
            closed: sub.closed,
            fill: f,
            stroke,
            stroke_width: sw,
            dash: dash.clone(),
        });
        count += 1;
    }
    if count == 0 {
        return Err("svg: the path draws nothing".to_string());
    }
    Ok(())
}

struct Run {
    text: String,
    st: Style,
    x: Option<f64>,
    y: Option<f64>,
}

fn text_runs(node: &Node, st: &Style, x: Option<f64>, y: Option<f64>, out: &mut Vec<Run>, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    for ch in &node.children {
        match ch {
            Ev::Text(t) => {
                let s = t.split_whitespace().collect::<Vec<_>>().join(" ");
                if !s.is_empty() {
                    out.push(Run { text: s, st: st.clone(), x, y });
                }
            }
            Ev::Node(k) => {
                if k.name != "tspan" {
                    continue;
                }
                let mut s2 = st.clone();
                if !apply_attrs(&mut s2, &k.attrs) {
                    continue;
                }
                let nx = k.get("x").and_then(parse_len).filter(|v| v.is_finite()).or(x);
                let ny = k.get("y").and_then(parse_len).filter(|v| v.is_finite()).or(y);
                text_runs(k, &s2, nx, ny, out, depth + 1);
            }
        }
    }
}

fn text(node: &Node, m: &Mat, st: &Style, out: &mut Vec<Item>) {
    let mut runs: Vec<Run> = Vec::new();
    text_runs(node, st, None, None, &mut runs, 0);
    if runs.is_empty() {
        return;
    }
    let bx = node.get("x").and_then(parse_len).filter(|v| v.is_finite());
    let by = node.get("y").and_then(parse_len).filter(|v| v.is_finite());
    let scale = m.scale();
    let angle = m.angle();
    let mut cursor: Option<f64> = None;
    for run in runs {
        let ux = run.x.or(cursor).or(bx).unwrap_or(0.0);
        let uy = run.y.or(by).unwrap_or(0.0);
        let size = run.st.size * scale;
        let p = m.xf(ux, uy);
        let color = match run.st.fill() {
            Some(c) => c,
            None => continue,
        };
        out.push(Item::Text {
            x: r2(p.x),
            y: r2(p.y),
            text: run.text.clone(),
            size: r2(size),
            color,
            font: run.st.font(),
            weight: run.st.weight,
            halign: run.st.anchor,
            valign: run.st.baseline,
            rotate: r2(angle),
        });
        if run.st.underline {
            let w = crate::render::text_width(&run.text, run.st.font(), run.st.weight, run.st.size);
            let uy2 = uy + run.st.size * 0.12;
            let a = m.xf(ux, uy2);
            let b = m.xf(ux + w, uy2);
            out.push(Item::Line {
                x1: r2(a.x),
                y1: r2(a.y),
                x2: r2(b.x),
                y2: r2(b.y),
                color,
                width: r2((run.st.sw * scale).max(0.4)),
                dash: Vec::new(),
            });
        }
        cursor = Some(ux + crate::render::text_width(&run.text, run.st.font(), run.st.weight, run.st.size));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item_counts(src: &str) -> Vec<usize> {
        let s = parse(src).expect("parse");
        let mut out: Vec<usize> = Vec::new();
        for i in &s.items {
            let n = match i {
                Item::Poly { points, .. } => points.len(),
                _ => 1,
            };
            out.push(n);
        }
        out
    }

    #[test]
    fn shapes_become_items() {
        let src = r##"<svg width="200" height="100" xmlns="http://www.w3.org/2000/svg">
            <rect x="1" y="2" width="30" height="40" fill="#ff0000" stroke="black" stroke-width="2"/>
            <line x1="0" y1="0" x2="10" y2="10" stroke="blue" stroke-width="1.5" stroke-dasharray="4,2"/>
            <polyline points="0,0 10,0 10,10" fill="red" stroke="green" stroke-width="1"/>
            <polygon points="1,1 2,2 3,1" fill="#0f0" stroke="none"/>
            <circle cx="50" cy="50" r="20" fill="blue"/>
            <ellipse cx="10" cy="20" rx="30" ry="15" fill="none" stroke="teal" stroke-width="3"/>
        </svg>"##;
        let s = parse(src).unwrap();
        assert_eq!(s.width, 200.0);
        assert_eq!(s.height, 100.0);
        assert_eq!(s.items.len(), 6);
        match &s.items[0] {
            Item::Rect { x, y, w, h, fill, stroke, stroke_width, .. } => {
                assert_eq!((*x, *y, *w, *h), (1.0, 2.0, 30.0, 40.0));
                assert_eq!(fill.unwrap().to_css(), "#ff0000");
                assert_eq!(stroke.unwrap().to_css(), "#000000");
                assert_eq!(*stroke_width, 2.0);
            }
            other => panic!("expected a rect, got {:?}", other),
        }
        match &s.items[1] {
            Item::Line { x1, y1, x2, y2, color, width, dash } => {
                assert_eq!((*x1, *y1, *x2, *y2), (0.0, 0.0, 10.0, 10.0));
                assert_eq!(color.to_css(), "#346abe");
                assert_eq!(*width, 1.5);
                assert_eq!(dash, &vec![4.0, 2.0]);
            }
            other => panic!("expected a line, got {:?}", other),
        }
        match &s.items[2] {
            Item::Poly { points, closed, fill, stroke, .. } => {
                assert!(!closed);
                assert!(fill.is_none());
                assert_eq!(points.len(), 3);
                assert_eq!(points[2], Point::new(10.0, 10.0));
                assert_eq!(stroke.unwrap().to_css(), "#48a05a");
            }
            other => panic!("expected a polyline, got {:?}", other),
        }
        match &s.items[3] {
            Item::Poly { points, closed, fill, stroke, .. } => {
                assert!(closed);
                assert_eq!(points.len(), 3);
                assert_eq!(fill.unwrap().to_css(), "#00ff00");
                assert!(stroke.is_none());
            }
            other => panic!("expected a polygon, got {:?}", other),
        }
        match &s.items[4] {
            Item::Poly { points, closed, fill, .. } => {
                assert!(closed);
                assert_eq!(points.len(), 1 + 4 * QUAD_STEPS);
                assert_eq!(fill.unwrap().to_css(), "#346abe");
                assert!((points[0].x - 70.0).abs() < 0.3 && (points[0].y - 50.0).abs() < 0.01);
                let far = points.iter().map(|p| (p.x - 50.0).powi(2) + (p.y - 50.0).powi(2)).fold(0.0f64, f64::max);
                assert!((far.sqrt() - 20.0).abs() < 0.2, "circle radius drifted: {}", far.sqrt());
            }
            other => panic!("expected a circle, got {:?}", other),
        }
        match &s.items[5] {
            Item::Poly { points, fill, stroke, stroke_width, .. } => {
                assert!(fill.is_none());
                assert_eq!(stroke.unwrap().to_css(), "#269494");
                assert_eq!(*stroke_width, 3.0);
                assert_eq!(points.len(), 1 + 4 * QUAD_STEPS);
                let minx = points.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
                let maxx = points.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
                assert!((minx - (-20.0)).abs() < 0.3 && (maxx - 40.0).abs() < 0.3);
            }
            other => panic!("expected an ellipse, got {:?}", other),
        }
    }

    #[test]
    fn path_moves_lines_curves_and_closes() {
        let src = r##"<svg width="100" height="100"><path d="M 0 0 L 10 0 C 20 0 20 10 10 10 Z" fill="blue" stroke="black" stroke-width="1"/></svg>"##;
        let s = parse(src).unwrap();
        assert_eq!(s.items.len(), 1);
        match &s.items[0] {
            Item::Poly { points, closed, fill, stroke, .. } => {
                assert!(closed);
                assert_eq!(points[0], Point::new(0.0, 0.0));
                assert_eq!(points[1], Point::new(10.0, 0.0));
                assert_eq!(points.len(), 2 + CURVE_STEPS);
                assert_eq!(points[points.len() - 1], Point::new(10.0, 10.0));
                assert_eq!(fill.unwrap().to_css(), "#346abe");
                assert!(stroke.is_some());
            }
            other => panic!("expected a poly, got {:?}", other),
        }
    }

    #[test]
    fn quadratics_and_smooth_curves_reflect() {
        let src = r##"<svg width="60" height="60">
            <path d="M5 5 Q 15 5 15 15 T 25 25" fill="none" stroke="black"/>
            <path d="M0 0 C 5 0 10 0 15 0 S 25 0 30 0" fill="none" stroke="black"/>
        </svg>"##;
        let s = parse(src).unwrap();
        assert_eq!(s.items.len(), 2);
        match &s.items[0] {
            Item::Poly { points, .. } => {
                assert_eq!(points.len(), 1 + 2 * CURVE_STEPS);
                assert_eq!(points[points.len() - 1], Point::new(25.0, 25.0));
            }
            other => panic!("expected a poly, got {:?}", other),
        }
    }

    #[test]
    fn arcs_are_sampled_into_cubics() {
        let src = r##"<svg width="40" height="40"><path d="M 5 20 A 15 15 0 1 0 35 20" fill="none" stroke="black"/></svg>"##;
        let s = parse(src).unwrap();
        assert_eq!(s.items.len(), 1);
        match &s.items[0] {
            Item::Poly { points, closed, .. } => {
                assert!(!closed);
                assert_eq!(points[0], Point::new(5.0, 20.0));
                assert_eq!(points[points.len() - 1], Point::new(35.0, 20.0));
                assert!(points.len() >= 12, "a half circle needs samples, got {}", points.len());
                let maxy = points.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
                assert!(maxy > 20.0, "the arc did not sweep downwards: {}", maxy);
            }
            other => panic!("expected a poly, got {:?}", other),
        }
    }

    #[test]
    fn several_subpaths_become_separate_items() {
        let s = parse(
            r##"<svg width="40" height="40"><path d="M0 0 H10 V10 Z M20 20 h5 v5 z" fill="red" stroke="black"/></svg>"##,
        )
        .unwrap();
        assert_eq!(s.items.len(), 2);
        match &s.items[0] {
            Item::Poly { points, closed, .. } => {
                assert!(closed);
                assert_eq!(points.len(), 3);
                assert_eq!(points[2], Point::new(10.0, 10.0));
            }
            other => panic!("expected a poly, got {:?}", other),
        }
        match &s.items[1] {
            Item::Poly { points, closed, .. } => {
                assert!(closed);
                assert_eq!(points.len(), 3);
                assert_eq!(points[0], Point::new(20.0, 20.0));
                assert_eq!(points[2], Point::new(25.0, 25.0));
            }
            other => panic!("expected a poly, got {:?}", other),
        }
    }

    #[test]
    fn transforms_compose_into_the_emitted_coordinates() {
        let src = r##"<svg width="200" height="200">
            <g transform="translate(10,20)">
              <rect x="1" y="2" width="3" height="4" fill="black"/>
              <g transform="scale(2)">
                <line x1="0" y1="0" x2="5" y2="0" stroke="black" stroke-width="2"/>
              </g>
              <g transform="rotate(90)">
                <rect x="1" y="0" width="2" height="2" fill="black"/>
              </g>
              <g transform="matrix(1,0,0,1,5,5)">
                <rect x="1" y="1" width="1" height="1" fill="black"/>
              </g>
            </g>
        </svg>"##;
        let s = parse(src).unwrap();
        assert_eq!(s.items.len(), 4);
        match &s.items[0] {
            Item::Rect { x, y, w, h, .. } => {
                assert_eq!((*x, *y), (11.0, 22.0));
                assert_eq!((*w, *h), (3.0, 4.0));
            }
            other => panic!("expected a rect, got {:?}", other),
        }
        match &s.items[1] {
            Item::Line { x1, y1, x2, y2, width, .. } => {
                assert_eq!((*x1, *y1, *x2, *y2), (10.0, 20.0, 20.0, 20.0));
                assert_eq!(*width, 4.0);
            }
            other => panic!("expected a line, got {:?}", other),
        }
        match &s.items[2] {
            Item::Poly { points, closed, .. } => {
                assert!(closed, "a rotated rect must close");
                assert_eq!(points.len(), 4);
                assert_eq!(points[0], Point::new(10.0, 21.0));
                assert_eq!(points[1], Point::new(10.0, 23.0));
                assert_eq!(points[2], Point::new(8.0, 23.0));
                assert_eq!(points[3], Point::new(8.0, 21.0));
            }
            other => panic!("expected a rotated rect as a poly, got {:?}", other),
        }
        match &s.items[3] {
            Item::Rect { x, y, .. } => assert_eq!((*x, *y), (16.0, 26.0)),
            other => panic!("expected a rect, got {:?}", other),
        }
    }

    #[test]
    fn viewbox_supplies_the_size() {
        let s = parse(r##"<svg viewBox="0 0 120 60"><rect x="10" y="10" width="5" height="5" fill="black"/></svg>"##).unwrap();
        assert_eq!(s.width, 120.0);
        assert_eq!(s.height, 60.0);
        match &s.items[0] {
            Item::Rect { x, y, .. } => assert_eq!((*x, *y), (10.0, 10.0)),
            other => panic!("expected a rect, got {:?}", other),
        }
        let s = parse(r##"<svg viewBox="-5 -5 100 50" width="200" height="100"><rect x="-5" y="-5" width="1" height="1" fill="black"/></svg>"##).unwrap();
        assert_eq!((s.width, s.height), (200.0, 100.0));
        match &s.items[0] {
            Item::Rect { x, y, w, .. } => {
                assert_eq!((*x, *y), (0.0, 0.0));
                assert_eq!(*w, 2.0);
            }
            other => panic!("expected a rect, got {:?}", other),
        }
    }

    #[test]
    fn colours_follow_the_subset() {
        let s = parse(
            r##"<svg width="10" height="10">
                 <rect x="0" y="0" width="1" height="1" fill="none" stroke="red"/>
                 <rect x="0" y="0" width="1" height="1" fill="#0f8"/>
                 <rect x="0" y="0" width="1" height="1" fill="rgb(10,20,30)" stroke="rgba(1,2,3,0.5)"/>
                 <rect x="0" y="0" width="1" height="1" fill="silver"/>
                 <rect x="0" y="0" width="1" height="1" fill="#10203080" opacity="0.5"/>
               </svg>"##,
        )
        .unwrap();
        assert_eq!(s.items.len(), 5);
        match &s.items[0] {
            Item::Rect { fill, stroke, .. } => {
                assert!(fill.is_none());
                assert_eq!(stroke.unwrap().to_css(), "#d64545");
            }
            other => panic!("expected a rect, got {:?}", other),
        }
        match &s.items[1] {
            Item::Rect { fill, .. } => assert_eq!(fill.unwrap().to_css(), "#00ff88"),
            other => panic!("expected a rect, got {:?}", other),
        }
        match &s.items[2] {
            Item::Rect { fill, stroke, .. } => {
                assert_eq!(fill.unwrap().to_css(), "#0a141e");
                assert!((stroke.unwrap().a - 0.5).abs() < 1e-9);
            }
            other => panic!("expected a rect, got {:?}", other),
        }
        match &s.items[3] {
            Item::Rect { fill, .. } => assert_eq!(fill.unwrap().to_css(), "#c0c0c0"),
            other => panic!("expected a rect, got {:?}", other),
        }
        match &s.items[4] {
            Item::Rect { fill, .. } => {
                let c = fill.unwrap();
                assert_eq!((c.r, c.g, c.b), (0x10, 0x20, 0x30));
                assert!((c.a - 0.5 * 128.0 / 255.0).abs() < 1e-6);
            }
            other => panic!("expected a rect, got {:?}", other),
        }
    }

    #[test]
    fn style_beats_presentation_attributes() {
        let s = parse(
            r##"<svg width="10" height="10" fill="red">
                 <rect x="0" y="0" width="1" height="1" style="fill:blue;stroke:#00ff00" stroke="black"/>
                 <g fill="purple" fill-opacity="0.5"><rect x="0" y="0" width="1" height="1"/></g>
                 <text x="0" y="0">&unknown; entity</text>
                 <g display="none"><rect x="0" y="0" width="1" height="1" fill="black"/></g>
                 <circle cx="a" cy="b" r="c" fill="black"/>
               </svg>"##,
        )
        .unwrap();
        assert_eq!(s.items.len(), 3);
        match &s.items[0] {
            Item::Rect { fill, stroke, .. } => {
                assert_eq!(fill.unwrap().to_css(), "#346abe");
                assert_eq!(stroke.unwrap().to_css(), "#00ff00");
            }
            other => panic!("expected a rect, got {:?}", other),
        }
        match &s.items[1] {
            Item::Rect { fill, .. } => {
                let c = fill.unwrap();
                assert_eq!(c.to_css(), "rgba(126,87,168,0.5)");
            }
            other => panic!("expected a rect, got {:?}", other),
        }
        match &s.items[2] {
            Item::Text { text, .. } => assert_eq!(text, "&unknown; entity"),
            other => panic!("expected text, got {:?}", other),
        }
    }

    #[test]
    fn text_maps_onto_the_display_list() {
        let s = parse(
            r##"<svg width="200" height="60" font-size="12">
                 <text x="10" y="20" fill="#123456" font-family="Times New Roman, serif" font-weight="bold" text-anchor="middle" dominant-baseline="central">Hello</text>
                 <text x="10" y="40" font-family="Courier, monospace">mono <tspan x="50" font-weight="700" fill="red">run</tspan> tail</text>
                 <text x="10" y="55" text-anchor="end" text-decoration="underline">link</text>
               </svg>"##,
        )
        .unwrap();
        let texts: Vec<&Item> = s.items.iter().filter(|i| matches!(i, Item::Text { .. })).collect();
        assert_eq!(texts.len(), 5, "one item per text node and per tspan");
        match texts[0] {
            Item::Text { x, y, text, size, color, font, weight, halign, valign, rotate } => {
                assert_eq!((*x, *y), (10.0, 20.0));
                assert_eq!(text, "Hello");
                assert_eq!(*size, 12.0);
                assert_eq!(color.to_css(), "#123456");
                assert_eq!(*font, Font::Serif);
                assert_eq!(*weight, Weight::Bold);
                assert_eq!(*halign, HAlign::Center);
                assert_eq!(*valign, VAlign::Middle);
                assert_eq!(*rotate, 0.0);
            }
            other => panic!("expected text, got {:?}", other),
        }
        match texts[1] {
            Item::Text { x, font, .. } => {
                assert_eq!(*x, 10.0);
                assert_eq!(*font, Font::Mono);
            }
            other => panic!("expected text, got {:?}", other),
        }
        match texts[2] {
            Item::Text { x, text, weight, color, .. } => {
                assert_eq!(*x, 50.0);
                assert_eq!(text, "run");
                assert_eq!(*weight, Weight::Bold);
                assert_eq!(color.to_css(), "#d64545");
            }
            other => panic!("expected text, got {:?}", other),
        }
        match texts[3] {
            Item::Text { x, text, .. } => {
                assert_eq!(text, "tail");
                assert!(*x > 50.0 && *x < 110.0, "the trailing run did not advance: {}", x);
            }
            other => panic!("expected text, got {:?}", other),
        }
        match texts[4] {
            Item::Text { halign, text, valign, .. } => {
                assert_eq!(*halign, HAlign::Right);
                assert_eq!(*valign, VAlign::Bottom);
                assert_eq!(text, "link");
            }
            other => panic!("expected text, got {:?}", other),
        }
        assert!(s.items.iter().any(|i| matches!(i, Item::Line { .. })), "underline was not drawn");
    }

    #[test]
    fn rotated_text_keeps_its_angle() {
        let s = parse(
            r##"<svg width="100" height="100"><text x="13" y="50" font-size="11" text-anchor="middle" dominant-baseline="central" transform="rotate(-90 13 50)">axis</text></svg>"##,
        )
        .unwrap();
        match &s.items[0] {
            Item::Text { x, y, rotate, .. } => {
                assert_eq!((*x, *y), (13.0, 50.0));
                assert!((rotate - -90.0).abs() < 0.01, "angle was {}", rotate);
            }
            other => panic!("expected text, got {:?}", other),
        }
    }

    #[test]
    fn use_style_and_titles_are_ignored_without_failing() {
        let s = parse(
            r##"<?xml version="1.0" encoding="UTF-8"?>
               <!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">
               <svg width="100" height="100" xmlns="http://www.w3.org/2000/svg">
                 <title>a title</title>
                 <desc>a description</desc>
                 <style>.c { fill: red; }</style>
                 <defs><rect id="r" width="9" height="9" fill="black"/></defs>
                 <use href="#r" x="1" y="1"/>
                 <g><!-- comment --><rect x="2" y="2" width="3" height="3" fill="black"/></g>
               </svg>"##,
        )
        .unwrap();
        assert_eq!(s.items.len(), 1);
        match &s.items[0] {
            Item::Rect { x, y, w, h, .. } => assert_eq!((*x, *y, *w, *h), (2.0, 2.0, 3.0, 3.0)),
            other => panic!("expected a rect, got {:?}", other),
        }
        assert!(parse("<svg width='10' height='10'><circle cx='a' r='c'/><rect width='bad' height='x' fill='black'/></svg>").is_ok());
    }

    #[test]
    fn entities_and_cdata_are_decoded() {
        let s = parse(
            r##"<svg width="10" height="10"><text x="0" y="0">a &amp; b &#65; &#x42; <![CDATA[c]]></text></svg>"##,
        )
        .unwrap();
        let joined: String = s
            .items
            .iter()
            .map(|i| match i {
                Item::Text { text, .. } => format!("{} ", text),
                other => panic!("expected text, got {:?}", other),
            })
            .collect();
        assert_eq!(joined, "a & b A B c ");
    }

    #[test]
    fn units_nesting_and_hidden_text() {
        let s = parse(
            r##"<svg width="100pt" height="50pt">
                 <g transform="translate(5,5)" opacity="0.5">
                   <g transform="scale(2)">
                     <rect width="10" height="10" fill="red" fill-opacity="0.5"/>
                     <text x="0" y="20" font-size="4" fill="none">invisible</text>
                   </g>
                 </g>
               </svg>"##,
        )
        .unwrap();
        assert!((s.width - 133.3333).abs() < 0.01, "pt was not converted: {}", s.width);
        assert!((s.height - 66.6666).abs() < 0.01, "pt was not converted: {}", s.height);
        assert_eq!(s.items.len(), 1);
        match &s.items[0] {
            Item::Rect { x, y, w, h, fill, .. } => {
                assert_eq!((*x, *y, *w, *h), (5.0, 5.0, 20.0, 20.0));
                let c = fill.unwrap();
                assert_eq!((c.r, c.g, c.b), (214, 69, 69));
                assert!((c.a - 0.25).abs() < 1e-9, "group and fill opacity did not multiply: {}", c.a);
            }
            other => panic!("expected a rect, got {:?}", other),
        }
    }

    #[test]
    fn malformed_input_is_reported_and_never_panics() {
        let broken = [
            "",
            "   ",
            "not xml at all",
            "<svg",
            "<svg width='10' height='10'",
            "<svg width='10' height='10'><rect x='1'</svg>",
            "<svg width=10 height='10'></svg>",
            "<svg width='10' height='10'><rect width='1'></g></svg>",
            "<svg></rect></svg>",
            "<svg width='10' height='10'><path d='L 5 5'/></svg>",
            "<svg width='10' height='10'><path d=''/></svg>",
            "<svg viewBox='0 0 10 10'><path d='M 0 0 X 4 4'/></svg>",
            "<svg viewBox='1 2'/>",
            "<svg width='0' height='0'></svg>",
            "<svg width='10' height='10'><text x='0' y='0'>unclosed",
            "<svg width='10' height='10'><polyline points='1,2 3' fill='black'/></svg>",
            "<svg width='10' height='10'><path d='M 0 0 A 5 5 0 9 0 10 10'/></svg>",
            "<svg width='10' height='10'><path d='M 0 0 Q 1'/></svg>",
        ];
        for src in broken {
            let r = std::panic::catch_unwind(|| parse(src));
            assert!(r.is_ok(), "parse panicked on {:?}", src);
            assert!(r.unwrap().is_err(), "expected an error for {:?}", src);
        }
    }

    #[test]
    fn deep_nesting_is_rejected_rather_than_overflowing() {
        let mut src = String::from("<svg width='10' height='10'>");
        for _ in 0..MAX_DEPTH + 10 {
            src.push_str("<g>");
        }
        for _ in 0..MAX_DEPTH + 10 {
            src.push_str("</g>");
        }
        src.push_str("</svg>");
        assert!(parse(&src).is_err());
    }

    #[test]
    fn item_counts_are_stable_for_the_plot_subset() {
        let src = r##"<svg xmlns="http://www.w3.org/2000/svg" width="620" height="320" viewBox="0 0 620 320" font-family="Helvetica, Arial, sans-serif">
            <rect width="620" height="320" fill="#ffffff"/>
            <line x1="46" y1="270.44" x2="606" y2="270.44" stroke="#e4e7ee" stroke-width="0.7" stroke-dasharray="2,3"/>
            <polygon points="76,270.44 143,120 210,270.44" fill="#346abe" stroke="none"/>
            <polyline points="76,240 143,90 210,250" fill="none" stroke="#346abe" stroke-width="1.8"/>
            <text x="13" y="150.22" font-size="11" fill="#7a8292" font-family="Helvetica, Arial, 'Liberation Sans', sans-serif" text-anchor="middle" dominant-baseline="central" transform="rotate(-90 13 150.22)">amplitude</text>
        </svg>"##;
        let counts = item_counts(src);
        assert_eq!(counts, vec![1, 1, 3, 3, 1]);
        let s = parse(src).unwrap();
        let sans = s
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Text { font, .. } => Some(*font),
                _ => None,
            })
            .next()
            .unwrap();
        assert_eq!(sans, Font::Sans);
    }

    #[test]
    fn hidden_and_zero_sized_shapes_are_skipped_or_safe() {
        let s = parse(
            r##"<svg width="10" height="10">
                 <ellipse cx="1" cy="1" rx="0" ry="4" fill="black"/>
                 <circle cx="1" cy="1" r="0" fill="black"/>
                 <rect x="0" y="0" width="0" height="0" fill="black"/>
                 <line x1="0" y1="0" x2="0" y2="0" fill="black"/>
                 <line x1="0" y1="0" x2="4" y2="4" stroke="black"/>
               </svg>"##,
        )
        .unwrap();
        assert_eq!(s.items.len(), 2);
    }
}
