pub mod html;
pub mod md;
pub mod pdf;
pub mod svg;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f64,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 1.0 }
    }

    pub fn hex(s: &str) -> Color {
        let s = s.trim().trim_start_matches('#');
        let s = if s.len() == 3 {
            format!("{}{}{}{}{}{}", &s[0..1], &s[0..1], &s[1..2], &s[1..2], &s[2..3], &s[2..3])
        } else {
            s.to_string()
        };
        let parse = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).unwrap_or(0);
        if s.len() >= 6 {
            Color::rgb(parse(0), parse(2), parse(4))
        } else {
            Color::rgb(0, 0, 0)
        }
    }

    pub fn named(name: &str) -> Option<Color> {
        Some(match name.to_lowercase().as_str() {
            "black" => Color::rgb(0, 0, 0),
            "white" => Color::rgb(255, 255, 255),
            "gray" | "grey" => Color::rgb(130, 130, 130),
            "lightgray" | "lightgrey" => Color::rgb(200, 200, 200),
            "red" => Color::rgb(214, 69, 69),
            "blue" => Color::rgb(52, 106, 190),
            "green" => Color::rgb(72, 160, 90),
            "orange" => Color::rgb(224, 138, 48),
            "purple" => Color::rgb(126, 87, 168),
            "teal" => Color::rgb(38, 148, 148),
            "pink" => Color::rgb(214, 105, 140),
            "brown" => Color::rgb(150, 100, 60),
            "cyan" => Color::rgb(50, 175, 190),
            "gold" => Color::rgb(212, 175, 55),
            "navy" => Color::rgb(28, 48, 96),
            "crimson" => Color::rgb(190, 30, 60),
            "slate" => Color::rgb(90, 100, 118),
            _ => return None,
        })
    }

    pub fn to_css(&self) -> String {
        if self.a >= 0.999 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("rgba({},{},{},{})", self.r, self.g, self.b, (self.a * 1000.0).round() / 1000.0)
        }
    }

    pub fn with_alpha(&self, a: f64) -> Color {
        Color { a, ..*self }
    }

    pub fn mix(&self, other: &Color, t: f64) -> Color {
        let t = t.clamp(0.0, 1.0);
        Color {
            r: (self.r as f64 + (other.r as f64 - self.r as f64) * t).round() as u8,
            g: (self.g as f64 + (other.g as f64 - self.g as f64) * t).round() as u8,
            b: (self.b as f64 + (other.b as f64 - self.b as f64) * t).round() as u8,
            a: self.a + (other.a - self.a) * t,
        }
    }

    pub fn shade(&self, t: f64) -> Color {
        let target = if t > 0.0 { Color::rgb(255, 255, 255) } else { Color::rgb(0, 0, 0) };
        self.mix(&target, t.abs())
    }
}

pub const INK: Color = Color::rgb(30, 34, 42);
pub const MUTED: Color = Color::rgb(122, 130, 146);
pub const GRID: Color = Color::rgb(228, 231, 238);
pub const AXIS: Color = Color::rgb(120, 128, 144);
pub const PAPER: Color = Color::rgb(255, 255, 255);

pub const PALETTE: [Color; 10] = [
    Color::rgb(52, 106, 190),
    Color::rgb(224, 105, 62),
    Color::rgb(72, 160, 90),
    Color::rgb(140, 92, 196),
    Color::rgb(38, 148, 148),
    Color::rgb(214, 69, 110),
    Color::rgb(224, 160, 40),
    Color::rgb(90, 100, 118),
    Color::rgb(120, 170, 60),
    Color::rgb(190, 100, 60),
];

pub fn series_color(i: usize) -> Color {
    if i < PALETTE.len() {
        PALETTE[i]
    } else {
        PALETTE[i % PALETTE.len()].mix(&Color::rgb(120, 120, 120), (i / PALETTE.len()) as f64 * 0.12)
    }
}

pub fn parse_color(spec: &str, fallback: Color) -> Color {
    let s = spec.trim();
    if let Some(c) = Color::named(s) {
        return c;
    }
    if s.starts_with('#') || s.len() == 6 || s.len() == 3 {
        return Color::hex(s);
    }
    if let Some(rest) = s.strip_prefix("rgb(").and_then(|r| r.strip_suffix(')')) {
        let parts: Vec<&str> = rest.split(',').map(|p| p.trim()).collect();
        if parts.len() >= 3 {
            return Color::rgb(
                parts[0].parse().unwrap_or(0),
                parts[1].parse().unwrap_or(0),
                parts[2].parse().unwrap_or(0),
            );
        }
    }
    fallback
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    Sans,
    Serif,
    Mono,
}

impl Font {
    pub fn parse(s: &str) -> Font {
        match s.to_lowercase().as_str() {
            "serif" | "times" => Font::Serif,
            "mono" | "monospace" | "code" | "courier" => Font::Mono,
            _ => Font::Sans,
        }
    }

    pub fn css(&self) -> &'static str {
        match self {
            Font::Sans => "Helvetica, Arial, 'Segoe UI', 'Liberation Sans', sans-serif",
            Font::Serif => "Georgia, 'Times New Roman', 'Liberation Serif', serif",
            Font::Mono => "'SF Mono', 'JetBrains Mono', Menlo, Consolas, 'Liberation Mono', monospace",
        }
    }

    pub fn pdf_name(&self, weight: Weight, italic: bool) -> &'static str {
        match (self, weight, italic) {
            (Font::Sans, Weight::Bold, false) => "Helvetica-Bold",
            (Font::Sans, _, true) => "Helvetica-Oblique",
            (Font::Sans, _, false) => "Helvetica",
            (Font::Serif, Weight::Bold, false) => "Times-Bold",
            (Font::Serif, _, true) => "Times-Italic",
            (Font::Serif, _, false) => "Times-Roman",
            (Font::Mono, Weight::Bold, false) => "Courier-BoldOblique",
            (Font::Mono, _, true) => "Courier-Oblique",
            (Font::Mono, _, false) => "Courier",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    Regular,
    Bold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VAlign {
    Top,
    Middle,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Point {
        Point { x, y }
    }
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub width: f64,
    pub height: f64,
    pub background: Option<Color>,
    pub items: Vec<Item>,
    pub title: String,
}

impl Scene {
    pub fn new(width: f64, height: f64) -> Scene {
        Scene { width, height, background: Some(PAPER), items: Vec::new(), title: String::new() }
    }

    pub fn blank(width: f64, height: f64) -> Scene {
        Scene { width, height, background: None, items: Vec::new(), title: String::new() }
    }

    pub fn push(&mut self, item: Item) {
        self.items.push(item);
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[derive(Clone, Debug)]
pub enum Item {
    Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        fill: Option<Color>,
        stroke: Option<Color>,
        stroke_width: f64,
        radius: f64,
    },
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        color: Color,
        width: f64,
        dash: Vec<f64>,
    },
    Poly {
        points: Vec<Point>,
        closed: bool,
        fill: Option<Color>,
        stroke: Option<Color>,
        stroke_width: f64,
        dash: Vec<f64>,
    },
    Text {
        x: f64,
        y: f64,
        text: String,
        size: f64,
        color: Color,
        font: Font,
        weight: Weight,
        halign: HAlign,
        valign: VAlign,
        rotate: f64,
    },
    Image {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        src: String,
        alt: String,
    },
}

impl Scene {
    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, fill: Option<Color>, stroke: Option<Color>) {
        self.push(Item::Rect { x, y, w, h, fill, stroke, stroke_width: 1.0, radius: 0.0 });
    }

    pub fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, color: Color, width: f64) {
        self.push(Item::Line { x1, y1, x2, y2, color, width, dash: Vec::new() });
    }

    pub fn dashed(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, color: Color, width: f64, dash: Vec<f64>) {
        self.push(Item::Line { x1, y1, x2, y2, color, width, dash });
    }

    pub fn text(&mut self, x: f64, y: f64, text: impl Into<String>, size: f64, color: Color) {
        self.push(Item::Text {
            x,
            y,
            text: text.into(),
            size,
            color,
            font: Font::Sans,
            weight: Weight::Regular,
            halign: HAlign::Left,
            valign: VAlign::Top,
            rotate: 0.0,
        });
    }

    pub fn text_at(
        &mut self,
        x: f64,
        y: f64,
        text: impl Into<String>,
        size: f64,
        color: Color,
        font: Font,
        weight: Weight,
        halign: HAlign,
        valign: VAlign,
    ) {
        self.push(Item::Text {
            x,
            y,
            text: text.into(),
            size,
            color,
            font,
            weight,
            halign,
            valign,
            rotate: 0.0,
        });
    }

    pub fn poly(&mut self, points: Vec<Point>, color: Color, width: f64, closed: bool) {
        self.push(Item::Poly {
            points,
            closed,
            fill: None,
            stroke: Some(color),
            stroke_width: width,
            dash: Vec::new(),
        });
    }

    pub fn filled_poly(&mut self, points: Vec<Point>, fill: Color, stroke: Option<Color>, width: f64) {
        self.push(Item::Poly {
            points,
            closed: true,
            fill: Some(fill),
            stroke,
            stroke_width: width,
            dash: Vec::new(),
        });
    }
}

pub fn text_width(text: &str, font: Font, weight: Weight, size: f64) -> f64 {
    let table = metrics(font, weight);
    let mut w = 0.0;
    for c in text.chars() {
        w += char_width(c, table, font);
    }
    w * size / 1000.0
}

pub fn char_width(c: char, table: &[u16; 95], font: Font) -> f64 {
    let code = c as u32;
    if (32..=126).contains(&code) {
        table[(code - 32) as usize] as f64
    } else {
        match font {
            Font::Mono => 600.0,
            Font::Serif => 500.0,
            Font::Sans => 556.0,
        }
    }
}

fn metrics(font: Font, weight: Weight) -> &'static [u16; 95] {
    match (font, weight) {
        (Font::Sans, Weight::Regular) => &HELVETICA,
        (Font::Sans, Weight::Bold) => &HELVETICA_BOLD,
        (Font::Serif, Weight::Regular) => &TIMES,
        (Font::Serif, Weight::Bold) => &TIMES_BOLD,
        (Font::Mono, _) => &COURIER,
    }
}

pub fn text_ascent(size: f64) -> f64 {
    size * 0.75
}

pub fn text_line_height(size: f64) -> f64 {
    size * 1.22
}

pub fn wrap_text(text: &str, font: Font, weight: Weight, size: f64, max_width: f64) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        if para.trim().is_empty() {
            out.push(String::new());
            continue;
        }
        let mut line = String::new();
        for word in para.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_string()
            } else {
                format!("{} {}", line, word)
            };
            if text_width(&candidate, font, weight, size) <= max_width || line.is_empty() {
                line = candidate;
            } else {
                out.push(std::mem::take(&mut line));
                line = word.to_string();
            }
        }
        if !line.is_empty() {
            out.push(line);
        }
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

pub fn ellipsize(text: &str, font: Font, weight: Weight, size: f64, max_width: f64) -> String {
    if text_width(text, font, weight, size) <= max_width {
        return text.to_string();
    }
    let mut s = text.to_string();
    while !s.is_empty() && text_width(&format!("{}...", s), font, weight, size) > max_width {
        s.pop();
    }
    if s.is_empty() {
        "...".to_string()
    } else {
        format!("{}...", s)
    }
}

#[rustfmt::skip]
static HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278,
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556,
    1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778,
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556,
    333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556,
    556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];

#[rustfmt::skip]
static HELVETICA_BOLD: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278,
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611,
    975, 722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833, 722, 778,
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 333, 278, 333, 584, 556,
    333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611,
    611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
];

#[rustfmt::skip]
static TIMES: [u16; 95] = [
    250, 333, 408, 500, 500, 833, 778, 180, 333, 333, 500, 564, 250, 333, 250, 278,
    500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 278, 278, 564, 564, 564, 444,
    921, 722, 667, 667, 722, 611, 556, 722, 722, 333, 389, 722, 611, 889, 722, 722,
    556, 722, 667, 556, 611, 722, 722, 944, 722, 722, 611, 333, 278, 333, 469, 500,
    333, 444, 500, 444, 500, 444, 333, 500, 500, 278, 278, 500, 278, 778, 500, 500,
    500, 500, 333, 389, 278, 500, 500, 722, 500, 500, 444, 480, 200, 480, 541,
];

#[rustfmt::skip]
static TIMES_BOLD: [u16; 95] = [
    250, 333, 555, 500, 500, 1000, 833, 278, 333, 333, 500, 570, 250, 333, 250, 278,
    500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 333, 333, 570, 570, 570, 500,
    930, 722, 667, 722, 722, 667, 611, 778, 778, 389, 500, 778, 667, 944, 722, 778,
    611, 778, 722, 556, 667, 722, 722, 1000, 722, 722, 667, 333, 278, 333, 581, 500,
    333, 500, 556, 444, 556, 444, 333, 500, 556, 278, 333, 556, 278, 833, 556, 500,
    556, 556, 444, 389, 333, 556, 500, 722, 500, 500, 444, 394, 220, 394, 520,
];

static COURIER: [u16; 95] = [600; 95];
