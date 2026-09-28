use crate::render::*;

pub fn scene_to_svg(scene: &Scene) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\" font-family=\"{}\">\n",
        num(scene.width),
        num(scene.height),
        num(scene.width),
        num(scene.height),
        Font::Sans.css()
    ));
    if let Some(bg) = scene.background {
        s.push_str(&format!("<rect width=\"{}\" height=\"{}\" fill=\"{}\"/>\n", num(scene.width), num(scene.height), bg.to_css()));
    }
    for item in &scene.items {
        push_item(&mut s, item);
    }
    s.push_str("</svg>\n");
    s
}

fn num(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r == r.trunc() {
        format!("{}", r as i64)
    } else {
        format!("{}", r)
    }
}

fn dash_attr(d: &[f64]) -> String {
    if d.is_empty() {
        String::new()
    } else {
        format!(" stroke-dasharray=\"{}\"", d.iter().map(|v| num(*v)).collect::<Vec<_>>().join(","))
    }
}

fn push_item(s: &mut String, item: &Item) {
    match item {
        Item::Rect { x, y, w, h, fill, stroke, stroke_width, radius } => {
            s.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>",
                num(*x),
                num(*y),
                num(*w),
                num(*h),
                num(*radius),
                fill.map(|c| c.to_css()).unwrap_or_else(|| "none".into()),
                stroke.map(|c| c.to_css()).unwrap_or_else(|| "none".into()),
                num(*stroke_width)
            ));
        }
        Item::Line { x1, y1, x2, y2, color, width, dash } => {
            s.push_str(&format!(
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\"{}/>",
                num(*x1),
                num(*y1),
                num(*x2),
                num(*y2),
                color.to_css(),
                num(*width),
                dash_attr(dash)
            ));
        }
        Item::Poly { points, closed, fill, stroke, stroke_width, dash } => {
            let pts: Vec<String> = points.iter().map(|p| format!("{},{}", num(p.x), num(p.y))).collect();
            s.push_str(&format!(
                "<{} points=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"round\" stroke-linecap=\"round\"{}/>",
                if *closed { "polygon" } else { "polyline" },
                pts.join(" "),
                fill.map(|c| c.to_css()).unwrap_or_else(|| "none".into()),
                stroke.map(|c| c.to_css()).unwrap_or_else(|| "none".into()),
                num(*stroke_width),
                dash_attr(dash)
            ));
        }
        Item::Text { x, y, text, size, color, font, weight, halign, valign, rotate } => {
            let anchor = match halign {
                HAlign::Left => "start",
                HAlign::Center => "middle",
                HAlign::Right => "end",
            };
            let base = match valign {
                VAlign::Top => "hanging",
                VAlign::Middle => "central",
                VAlign::Bottom => "auto",
            };
            let w = match weight {
                Weight::Regular => "normal",
                Weight::Bold => "bold",
            };
            let rot = if *rotate != 0.0 {
                format!(" transform=\"rotate({} {} {})\"", num(*rotate), num(*x), num(*y))
            } else {
                String::new()
            };
            s.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" font-size=\"{}\" fill=\"{}\" font-family=\"{}\" font-weight=\"{}\" text-anchor=\"{}\" dominant-baseline=\"{}\"{}>{}</text>",
                num(*x),
                num(*y),
                num(*size),
                color.to_css(),
                font.css(),
                w,
                anchor,
                base,
                rot,
                escape(text)
            ));
        }
        Item::Image { x, y, w, h, src, alt } => {
            s.push_str(&format!(
                "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" href=\"{}\" preserveAspectRatio=\"xMidYMid meet\"><title>{}</title></image>",
                num(*x),
                num(*y),
                num(*w),
                num(*h),
                escape(src),
                escape(alt)
            ));
        }
    }
    s.push('\n');
}

pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
