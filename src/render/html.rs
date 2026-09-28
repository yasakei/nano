use std::cell::{Cell, RefCell};
use std::fs;
use std::path::PathBuf;

use crate::doc::{
    slugify, Align, Block, CellResult, Doc, Inline, Media, MediaKind, Note, PlotBlock, Table, Widget,
};
use crate::plot::PlotSpec;
use crate::render::svg;
use crate::value::{fmt_num, Value};

const TEX_JS: &str = include_str!("tex.js");
const RUNTIME_JS: &str = include_str!("js_runtime.js");

pub fn render(doc: &Doc) -> String {
    let theme = theme_name(&doc.meta.theme);
    let ctx = Ctx::default();
    let title = doc.meta.title.trim().to_string();
    let heading_title = if title.is_empty() { doc.name.trim().to_string() } else { title.clone() };

    let mut body = String::new();
    body.push_str("<main id=\"content\">\n");
    if !title.is_empty() {
        body.push_str(&title_header(doc));
    }
    for b in &doc.blocks {
        body.push_str(&block(doc, b, &ctx));
    }
    body.push_str("</main>\n");

    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n\
         <meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{}</title>\n\
         <meta name=\"description\" content=\"{}\">\n\
         <meta name=\"author\" content=\"{}\">\n\
         <meta name=\"generator\" content=\"nano\">\n\
         <style>\n{}\n</style>\n\
         </head>\n<body class=\"theme-{}\">\n{}\
         <script>\n{}\n{}\n\
         window.NanoTex = window.NanoTex || globalThis.NanoTex;\n\
         window.Nano = window.Nano || globalThis.Nano;\n\
         </script>\n\
         </body>\n</html>\n",
        svg::escape(&heading_title),
        svg::escape(doc.meta.description.trim()),
        svg::escape(&doc.meta.author.join(", ")),
        STYLESHEET,
        theme,
        body,
        TEX_JS.trim_end(),
        RUNTIME_JS.trim_end()
    )
}

fn theme_name(raw: &str) -> String {
    match raw.trim().to_lowercase().as_str() {
        "serif" | "dark" | "minimal" => raw.trim().to_lowercase(),
        _ => "default".to_string(),
    }
}

#[derive(Default)]
struct Ctx {
    cells: Cell<usize>,
    figures: Cell<usize>,
    tables: Cell<usize>,
    widgets: Cell<usize>,
    assets: RefCell<Vec<(PathBuf, String)>>,
}

fn next(c: &Cell<usize>) -> usize {
    c.set(c.get() + 1);
    c.get()
}

fn topbar(title: &str) -> String {
    format!(
        "<header class=\"topbar\" id=\"top\"><div class=\"topbar-inner\">\
         <span class=\"brand\">{}</span><a class=\"to-top\" href=\"#top\">↑ top</a>\
         </div></header>\n",
        svg::escape(title)
    )
}

fn title_header(doc: &Doc) -> String {
    let m = &doc.meta;
    let mut out = String::from("<header class=\"doc-head\">\n<h1 class=\"doc-title\">");
    out.push_str(&svg::escape(m.title.trim()));
    out.push_str("</h1>\n");
    let sub = m.subtitle.trim();
    if !sub.is_empty() {
        out.push_str(&format!("<p class=\"doc-sub\">{}</p>\n", svg::escape(sub)));
    }
    let names: Vec<&str> = m.author.iter().map(|a| a.trim()).filter(|a| !a.is_empty()).collect();
    let date = m.date.trim();
    if !names.is_empty() || !date.is_empty() {
        let mut line: Vec<String> = Vec::new();
        if !names.is_empty() {
            line.push(names.join(", "));
        }
        if !date.is_empty() {
            line.push(date.to_string());
        }
        out.push_str(&format!("<p class=\"byline\">{}</p>\n", svg::escape(&line.join(" · "))));
    }
    let desc = m.description.trim();
    if !desc.is_empty() {
        out.push_str(&format!("<p class=\"lead\">{}</p>\n", svg::escape(desc)));
    }
    let kw: Vec<&str> = m.keywords.iter().map(|k| k.trim()).filter(|k| !k.is_empty()).collect();
    if !kw.is_empty() {
        out.push_str(&format!(
            "<p class=\"keywords\"><span class=\"kw-label\">Keywords</span>{}</p>\n",
            svg::escape(&kw.join(" · "))
        ));
    }
    out.push_str("</header>\n");
    out
}

fn block(doc: &Doc, b: &Block, ctx: &Ctx) -> String {
    match b {
        Block::Heading { level, inlines, id } => heading(*level, inlines, id, doc, ctx),
        Block::Para(items) => format!("<p>{}</p>\n", inlines_in(items, doc, ctx)),
        Block::List { ordered, start, items } => list(*ordered, *start, items, doc, ctx),
        Block::Checklist { items } => checklist(items, doc, ctx),
        Block::Quote(items) => format!("<blockquote class=\"quote\">{}</blockquote>\n", inlines_in(items, doc, ctx)),
        Block::Rule => "<hr>\n".to_string(),
        Block::Code { lang, text, runnable, result } => code(lang, text, *runnable, result.as_ref(), ctx),
        Block::Plot(p) => plot(p, ctx),
        Block::Table(t) => table(t, ctx),
        Block::Media(m) => media(doc, m, ctx),
        Block::Math { display, text } => math(*display, text, true),
        Block::Note(n) => note(n, doc, ctx),
        Block::Widget(w) => widget(w, ctx),
        Block::Toc { title } => toc(doc, title),
        Block::Raw(s) => format!("{}\n", s.trim_end()),
        Block::Directive(_) => "".to_string(),
        Block::PageBreak => "<div class=\"pagebreak\"></div>\n".to_string(),
    }
}

fn heading(level: u8, items: &[Inline], id: &str, doc: &Doc, ctx: &Ctx) -> String {
    let lvl = level.clamp(1, 6);
    let text: String = items.iter().map(|i| i.plain()).collect();
    let anchor_id = if id.trim().is_empty() { slugify(&text) } else { id.trim().to_string() };
    format!(
        "<h{lvl} id=\"{id}\">{body}<a class=\"anchor\" href=\"#{id}\">#</a></h{lvl}>\n",
        lvl = lvl,
        id = svg::escape(&anchor_id),
        body = inlines_in(items, doc, ctx)
    )
}

fn list(ordered: bool, start: u32, items: &[Vec<Inline>], doc: &Doc, ctx: &Ctx) -> String {
    let mut out = String::new();
    if ordered {
        if start == 1 {
            out.push_str("<ol>\n");
        } else {
            out.push_str(&format!("<ol start=\"{}\">\n", start));
        }
    } else {
        out.push_str("<ul>\n");
    }
    for item in items {
        out.push_str(&format!("<li>{}</li>\n", inlines_in(item, doc, ctx)));
    }
    out.push_str(if ordered { "</ol>\n" } else { "</ul>\n" });
    out
}

fn checklist(items: &[(bool, Vec<Inline>)], doc: &Doc, ctx: &Ctx) -> String {
    let mut out = String::from("<ul class=\"checklist\">\n");
    for (done, item) in items {
        out.push_str(&format!(
            "<li class=\"{}\"><span class=\"box\" aria-hidden=\"true\">{}</span>{}</li>\n",
            if *done { "done" } else { "todo" },
            if *done { "✓" } else { "" },
            inlines_in(item, doc, ctx)
        ));
    }
    out.push_str("</ul>\n");
    out
}

fn toc(doc: &Doc, title: &str) -> String {
    let label = if title.trim().is_empty() { "Contents" } else { title.trim() };
    let hs: Vec<(u8, String, String)> =
        doc.headings().into_iter().filter(|(l, _, _)| *l > 1).collect();
    let mut out = format!("<nav class=\"toc\"><h2>{}</h2>\n", svg::escape(label));
    if hs.is_empty() {
        out.push_str("<p class=\"toc-empty\">No sections yet.</p>\n</nav>\n");
        return out;
    }
    let base = hs.iter().map(|(l, _, _)| *l).min().unwrap_or(2);
    let mut depth = 0usize;
    for (lvl, text, id) in hs {
        let want = (lvl.saturating_sub(base) as usize) + 1;
        while depth < want {
            out.push_str("<ul>\n");
            depth += 1;
        }
        while depth > want {
            out.push_str("</ul>\n");
            depth -= 1;
        }
        let name = if text.trim().is_empty() { id.clone() } else { text.trim().to_string() };
        let target = if id.trim().is_empty() { slugify(&name) } else { id.trim().to_string() };
        out.push_str(&format!("<li><a href=\"#{}\">{}</a></li>\n", svg::escape(&target), svg::escape(&name)));
    }
    while depth > 0 {
        out.push_str("</ul>\n");
        depth -= 1;
    }
    out.push_str("</nav>\n");
    out
}

fn code(lang: &str, text: &str, runnable: bool, result: Option<&CellResult>, ctx: &Ctx) -> String {
    let n = next(&ctx.cells);
    let id = format!("cell-{}", n);
    let mut out = format!("<div class=\"cell\" id=\"{}\">\n", id);
    out.push_str(&format!(
        "<div class=\"cell-head\"><span class=\"lang\">{}</span>\
         <button class=\"copy\" type=\"button\" data-copy-target=\"{}\">copy</button></div>\n",
        svg::escape(if lang.trim().is_empty() { "nano" } else { lang.trim() }),
        id
    ));
    out.push_str(&format!("<pre><code>{}</code></pre>\n", svg::escape(text.trim_end())));
    if let Some(r) = result {
        if runnable {
            let good = r.error.is_none();
            out.push_str(&format!(
                "<div class=\"result {}\">\n",
                if good { "good" } else { "bad" }
            ));
            if !r.stdout.trim_end().is_empty() {
                out.push_str(&format!("<div class=\"out\">{}</div>\n", svg::escape(r.stdout.trim_end())));
            }
            if let Some(v) = &r.value {
                if !v.trim_end().is_empty() {
                    out.push_str(&format!("<div class=\"value\">{}</div>\n", svg::escape(v.trim_end())));
                }
            }
            if let Some(e) = &r.error {
                if !e.trim_end().is_empty() {
                    out.push_str(&format!("<div class=\"err\">{}</div>\n", svg::escape(e.trim_end())));
                }
            }
            out.push_str("</div>\n");
        }
    }
    out.push_str("</div>\n");
    out
}

fn plot(p: &PlotBlock, ctx: &Ctx) -> String {
    let n = next(&ctx.figures);
    let caption = if !p.caption.trim().is_empty() {
        p.caption.trim().to_string()
    } else {
        p.spec.caption.trim().to_string()
    };
    let scene = match crate::plot::render(&p.spec) {
        Ok(s) => s,
        Err(e) => return format!("<p class=\"plot-error\">plot error: {}</p>\n", svg::escape(&e)),
    };
    let title = if !p.spec.title.trim().is_empty() {
        p.spec.title.trim().to_string()
    } else if !caption.is_empty() {
        caption.clone()
    } else {
        format!("Figure {}", n)
    };
    let mut markup = svg::scene_to_svg(&scene);
    if markup.starts_with("<svg ") {
        markup = markup.replacen(
            "<svg ",
            "<svg role=\"img\" style=\"width:100%;height:auto;display:block\" ",
            1,
        );
    }
    if let Some(i) = markup.find('>') {
        markup.insert_str(i + 1, &format!("<title>{}</title>", svg::escape(&title)));
    }
    let mut out = format!(
        "<figure class=\"plot\" id=\"fig-{}\">\n<div class=\"plot-wrap live-canvas-wrap\">\n{}",
        n, markup
    );
    if p.spec.interactive.is_some() && p.spec.formula.is_some() {
        out.push_str(&live_canvas(&p.spec, n));
    }
    out.push_str("</div>\n");
    if !caption.is_empty() {
        out.push_str(&format!(
            "<figcaption>Figure {}. {}</figcaption>\n",
            n,
            svg::escape(&caption)
        ));
    }
    out.push_str("</figure>\n");
    out
}

fn num_attr(name: &str, v: f64) -> String {
    format!(" data-{}=\"{}\"", name, fmt_num(v))
}

fn live_canvas(spec: &PlotSpec, n: usize) -> String {
    let id = format!("live-{}", n);
    let widget_var = if spec.var.trim().is_empty() { "t".to_string() } else { spec.var.trim().to_string() };
    let axis = if widget_var == "t" || widget_var == "x" { "x".to_string() } else { widget_var.clone() };
    let formula = spec.formula.as_deref().unwrap_or("").trim().to_string();
    let (x0, x1) = spec.domain;
    let labels: Vec<&str> = spec.series.iter().map(|s| s.label.trim()).filter(|l| !l.is_empty()).collect();

    let mut style = spec.style.trim().to_lowercase();
    if style.is_empty() {
        style = "line".to_string();
    }
    if style == "line" && (spec.show_points == Some(true) || spec.style == "both") {
        style = "both".to_string();
    }

    let mut attrs = String::new();
    attrs.push_str(&format!(" data-var=\"{}\"", svg::escape(&widget_var)));
    attrs.push_str(&format!(" data-formula=\"{}\"", svg::escape(&formula)));
    attrs.push_str(&format!(" data-var-name=\"{}\"", svg::escape(&axis)));
    attrs.push_str(&num_attr("xmin", x0));
    attrs.push_str(&num_attr("xmax", x1));
    if let Some(v) = spec.ymin {
        attrs.push_str(&num_attr("ymin", v));
    }
    if let Some(v) = spec.ymax {
        attrs.push_str(&num_attr("ymax", v));
    }
    attrs.push_str(" data-points=\"240\"");
    if let Some(c) = spec.accent {
        attrs.push_str(&format!(" data-color=\"{}\"", c.to_css()));
    }
    attrs.push_str(&format!(" data-style=\"{}\"", svg::escape(&style)));
    attrs.push_str(&format!(" data-fill=\"{}\"", if spec.fill > 0.0 { "1" } else { "0" }));
    if !labels.is_empty() {
        attrs.push_str(&format!(" data-labels=\"{}\"", svg::escape(&labels.join(";"))));
    }
    if !spec.xlabel.trim().is_empty() {
        attrs.push_str(&format!(" data-xlabel=\"{}\"", svg::escape(spec.xlabel.trim())));
    }
    if !spec.ylabel.trim().is_empty() {
        attrs.push_str(&format!(" data-ylabel=\"{}\"", svg::escape(spec.ylabel.trim())));
    }

    format!(
        "<canvas class=\"live\" id=\"{}\" width=\"{}\" height=\"{}\"{}></canvas>\n",
        id,
        fmt_num(spec.width),
        fmt_num(spec.height),
        attrs
    )
}

fn table(t: &Table, ctx: &Ctx) -> String {
    let widest = t.rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut cols: Vec<String> = t.columns.iter().map(|c| c.trim().to_string()).collect();
    while cols.len() < widest {
        cols.push((cols.len() + 1).to_string());
    }
    if cols.is_empty() {
        return "<p class=\"toc-empty\">(empty table)</p>\n".to_string();
    }
    let n = cols.len();
    let limit = t.max_rows.unwrap_or(usize::MAX);
    let shown = t.rows.len().min(limit);

    let mut out = String::from("<figure class=\"table-figure\">\n<div class=\"table-wrap\">\n<table");
    if t.zebra {
        out.push_str(" class=\"zebra\"");
    }
    out.push_str(">\n<thead>\n<tr>");
    for (i, c) in cols.iter().enumerate() {
        out.push_str(&format!(
            "<th style=\"text-align:{}\">{}</th>",
            align_css(t.align.get(i).copied().unwrap_or(Align::Left)),
            svg::escape(c)
        ));
    }
    out.push_str("</tr>\n</thead>\n<tbody>\n");
    for r in t.rows.iter().take(shown) {
        let mut cells: Vec<String> = r.iter().map(cell_html).collect();
        cells.resize(n, String::new());
        out.push_str("<tr>");
        for c in cells.iter().take(n) {
            out.push_str(&format!("<td>{}</td>", c));
        }
        out.push_str("</tr>\n");
    }
    if t.rows.len() > shown {
        out.push_str(&format!(
            "<tr class=\"more\"><td colspan=\"{}\"><em>{} more rows</em></td></tr>\n",
            n,
            t.rows.len() - shown
        ));
    }
    out.push_str("</tbody>\n</table>\n</div>\n");
    let caption = t.caption.trim();
    if !caption.is_empty() {
        out.push_str(&format!(
            "<figcaption>Table {}. {}</figcaption>\n",
            next(&ctx.tables),
            svg::escape(caption)
        ));
    }
    out.push_str("</figure>\n");
    out
}

fn cell_html(v: &Value) -> String {
    svg::escape(&v.to_display().replace(['\n', '\r'], " "))
}

fn align_css(a: Align) -> &'static str {
    match a {
        Align::Left => "left",
        Align::Center => "center",
        Align::Right => "right",
    }
}

fn media(doc: &Doc, m: &Media, ctx: &Ctx) -> String {
    let n = next(&ctx.figures);
    let src = m.src.trim();
    let alt = if m.alt.trim().is_empty() { src.to_string() } else { m.alt.trim().to_string() };
    let mut cls = String::from("media");
    match m.float.as_deref().map(|f| f.trim().to_lowercase()).unwrap_or_default().as_str() {
        "left" => cls.push_str(" float-left"),
        "right" => cls.push_str(" float-right"),
        _ => {}
    }
    let width = width_style(m.width.as_deref());
    let fig_style = if width.is_empty() { String::new() } else { format!(" style=\"{}\"", width) };

    let body = match m.kind {
        MediaKind::Image => match asset(doc, src, ctx) {
            Asset::Url(u) => format!(
                "<img src=\"{}\" alt=\"{}\" loading=\"lazy\"{}>",
                svg::escape(&u),
                svg::escape(&alt),
                style_attr(&width)
            ),
            Asset::Inline(body) => format!("<span class=\"svg-inline\">{}</span>", body),
            Asset::Missing => format!("<div class=\"missing\">image not found: {}</div>", svg::escape(src)),
        },
        MediaKind::Video => {
            let url = src.to_string();
            let mut a = String::new();
            if m.autoplay {
                a.push_str(" autoplay");
            }
            if m.loop_media {
                a.push_str(" loop");
            }
            if m.muted {
                a.push_str(" muted");
            }
            if m.controls {
                a.push_str(" controls");
            }
            if let Some(p) = m.poster.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
                let poster = match asset(doc, p, ctx) {
                    Asset::Url(u) => Some(u),
                    _ => Some(p.to_string()),
                };
                if let Some(u) = poster {
                    a.push_str(&format!(" poster=\"{}\"", svg::escape(&u)));
                }
            }
            let ty = media_type(src, "video");
            format!(
                "<video{} playsinline><source src=\"{}\"{}>Your browser cannot play this video. \
                 <a href=\"{}\">Download it</a>.</video>",
                a,
                svg::escape(&url),
                ty,
                svg::escape(&url)
            )
        }
        MediaKind::Audio => {
            let url = src.to_string();
            let ty = media_type(src, "audio");
            format!(
                "<div class=\"audio-card\"><span class=\"audio-icon\" aria-hidden=\"true\">♪</span>\
                 <div class=\"audio-meta\"><span class=\"audio-name\">{}</span>\
                 <audio{} preload=\"none\"><source src=\"{}\"{}></audio></div></div>",
                svg::escape(&alt),
                if m.controls { " controls" } else { "" },
                svg::escape(&url),
                ty
            )
        }
        MediaKind::Embed | MediaKind::Iframe => format!(
            "<iframe src=\"{}\" allowfullscreen loading=\"lazy\" referrerpolicy=\"no-referrer\" \
             style=\"aspect-ratio:16/9;width:100%;border:0\" title=\"{}\"></iframe>",
            svg::escape(src),
            svg::escape(&alt)
        ),
    };

    let mut out = format!("<figure class=\"{}\"{}>\n", cls, fig_style);
    out.push_str(&body);
    out.push('\n');
    let caption = m.caption.trim();
    if !caption.is_empty() {
        out.push_str(&format!("<figcaption>Figure {}. {}</figcaption>\n", n, svg::escape(caption)));
    }
    out.push_str("</figure>\n");
    out
}

fn media_type(src: &str, kind: &str) -> String {
    let ext = src.rsplit('.').next().unwrap_or("").trim().to_lowercase();
    let mime = match ext.as_str() {
        "mp4" | "m4v" => "mp4",
        "webm" => "webm",
        "ogv" => "ogg",
        "ogg" => "ogg",
        "mov" => "quicktime",
        "mp3" => "mpeg",
        "wav" => "wav",
        "m4a" => "mp4",
        "flac" => "flac",
        "aac" => "aac",
        _ => return String::new(),
    };
    format!(" type=\"{}/{};\"", kind, mime).replace(";\"", "\"")
}

fn style_attr(style: &str) -> String {
    if style.is_empty() {
        String::new()
    } else {
        format!(" style=\"{}\"", style)
    }
}

fn width_style(w: Option<&str>) -> String {
    let t = match w {
        Some(v) => v.trim(),
        None => return String::new(),
    };
    if t.is_empty() {
        return String::new();
    }
    if let Some(pct) = t.strip_suffix('%') {
        return match pct.trim().parse::<f64>() {
            Ok(v) if v > 0.0 && v <= 100.0 => format!("max-width:{}%", fmt_num(v)),
            _ => String::new(),
        };
    }
    for unit in ["px", "rem", "em", "ch", "vw", "pt", "in", "cm", "mm"] {
        if let Some(v) = t.strip_suffix(unit) {
            if let Ok(v) = v.trim().parse::<f64>() {
                if v > 0.0 {
                    return format!("max-width:{}{}", fmt_num(v), unit);
                }
            }
            return String::new();
        }
    }
    match t.parse::<f64>() {
        Ok(v) if v > 0.0 => format!("max-width:{}px", fmt_num(v)),
        _ => String::new(),
    }
}

fn math(display: bool, text: &str, block: bool) -> String {
    let t = text.trim();
    if t.is_empty() {
        return String::new();
    }
    if display || t.contains('\n') {
        format!(
            "<div class=\"math-display\" data-tex=\"{}\">{}</div>\n",
            svg::escape(t),
            svg::escape(t)
        )
    } else {
        let span = format!("<span class=\"math\" data-tex=\"{}\">{}</span>", svg::escape(t), svg::escape(t));
        if block {
            format!("{}\n", span)
        } else {
            span
        }
    }
}

fn note(n: &Note, doc: &Doc, ctx: &Ctx) -> String {
    let kind = note_kind(&n.kind);
    let title = n.title.trim();
    let mut body = n.body.clone();
    if !title.is_empty() {
        if let Some(first) = body.first() {
            let plain = first.plain();
            if plain.trim() == title {
                body.remove(0);
            } else if plain.trim_start().starts_with(title) {
                let rest = plain.trim_start()[title.len()..]
                    .trim_start_matches(['\n', ' ', '\t'])
                    .to_string();
                body[0] = Inline::Text(rest);
            }
        }
    }
    let label = if title.is_empty() { kind.to_uppercase() } else { title.to_string() };
    let mut out = format!("<aside class=\"note note-{}\">\n", kind);
    out.push_str(&format!("<div class=\"note-label\">{}</div>\n", svg::escape(&label)));
    if !title.is_empty() {
        out.push_str(&format!("<p class=\"note-title\"><strong>{}</strong></p>\n", svg::escape(title)));
    }
    if !body.is_empty() {
        out.push_str(&format!("<p class=\"note-body\">{}</p>\n", inlines_in(&body, doc, ctx)));
    }
    out.push_str("</aside>\n");
    out
}

fn note_kind(raw: &str) -> &'static str {
    match raw.trim().to_lowercase().as_str() {
        "info" | "information" => "info",
        "tip" | "hint" => "tip",
        "warn" | "warning" | "caution" | "important" | "attention" => "warning",
        "danger" | "error" | "critical" => "danger",
        _ => "note",
    }
}

fn widget(w: &Widget, ctx: &Ctx) -> String {
    let n = next(&ctx.widgets);
    let id = format!("w-{}", n);
    let name = if w.name.trim().is_empty() { "value".to_string() } else { w.name.trim().to_string() };
    let kind = w.kind.trim().to_lowercase();
    let kind = if kind.is_empty() { "control".to_string() } else { kind };

    let mut out = format!("<div class=\"widget widget-{}\" id=\"{}\">\n", svg::escape(&kind), id);
    let label = w.label.trim();
    if !label.is_empty() {
        out.push_str(&format!(
            "<label for=\"{}\">{}</label>\n",
            id,
            svg::escape(label)
        ));
    }

    let mut shown = fmt_num(w.value);
    if is_check(&kind) {
        out.push_str(&format!(
            "<input type=\"checkbox\" id=\"{id}\" data-var=\"{name}\"{checked}>\n",
            id = id,
            name = svg::escape(&name),
            checked = if w.checked { " checked" } else { "" }
        ));
        shown = if w.checked { "true".to_string() } else { "false".to_string() };
    } else if is_select(&kind) || !w.options.is_empty() {
        if w.options.is_empty() {
            out.push_str(&format!(
                "<input type=\"number\" id=\"{}\" data-var=\"{}\" min=\"{}\" max=\"{}\" step=\"{}\" value=\"{}\">\n",
                id,
                svg::escape(&name),
                fmt_num(w.min),
                fmt_num(w.max),
                fmt_num(w.step),
                fmt_num(w.value)
            ));
        } else {
            out.push_str(&format!("<select id=\"{}\" data-var=\"{}\">", id, svg::escape(&name)));
            for (i, opt) in w.options.iter().enumerate() {
                let sel = w.value.round() as i64 == i as i64 && w.value >= 0.0;
                out.push_str(&format!(
                    "<option value=\"{}\"{}>{}</option>",
                    svg::escape(opt.trim()),
                    if sel { " selected" } else { "" },
                    svg::escape(opt.trim())
                ));
            }
            out.push_str("</select>\n");
            if w.value >= 0.0 && (w.value.round() as usize) < w.options.len() {
                shown = w.options[w.value.round() as usize].trim().to_string();
            }
        }
    } else if is_text(&kind) {
        out.push_str(&format!(
            "<input type=\"text\" id=\"{}\" data-var=\"{}\" value=\"{}\">\n",
            id,
            svg::escape(&name),
            svg::escape(&shown)
        ));
    } else {
        out.push_str(&format!(
            "<input type=\"range\" id=\"{}\" data-var=\"{}\" min=\"{}\" max=\"{}\" step=\"{}\" value=\"{}\">\n",
            id,
            svg::escape(&name),
            fmt_num(w.min),
            fmt_num(w.max),
            fmt_num(w.step),
            fmt_num(w.value)
        ));
    }
    out.push_str(&format!(
        "<span class=\"value\" data-var-readout=\"{}\">{}</span>\n</div>\n",
        svg::escape(&name),
        svg::escape(&shown)
    ));
    out
}

fn is_check(kind: &str) -> bool {
    matches!(kind, "checkbox" | "check" | "toggle" | "switch" | "bool" | "boolean")
}

fn is_select(kind: &str) -> bool {
    matches!(kind, "select" | "dropdown" | "choice" | "options" | "list" | "menu")
}

fn is_text(kind: &str) -> bool {
    matches!(kind, "text" | "input" | "field" | "string")
}

enum Asset {
    Url(String),
    Inline(String),
    Missing,
}

fn asset(doc: &Doc, src: &str, ctx: &Ctx) -> Asset {
    let rel = src.trim();
    if rel.is_empty() {
        return Asset::Missing;
    }
    if rel.starts_with("http://") || rel.starts_with("https://") || rel.starts_with("data:") {
        return Asset::Url(rel.to_string());
    }
    let path = doc.resolve(rel);
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg") {
        return Asset::Url(rel.to_string());
    }
    if !path.is_file() {
        return Asset::Missing;
    }
    let key = path.clone();
    let cached = ctx.assets.borrow().iter().find(|(p, _)| *p == key).map(|(_, v)| v.clone());
    let value = match cached {
        Some(v) => v,
        None => {
            let bytes = match fs::read(&path) {
                Ok(b) => b,
                Err(_) => return Asset::Missing,
            };
            let v = if ext == "svg" {
                String::from_utf8_lossy(&bytes).to_string()
            } else {
                let mime = if ext == "jpg" { "jpeg" } else { ext.as_str() };
                format!("data:image/{};base64,{}", mime, b64(&bytes))
            };
            ctx.assets.borrow_mut().push((key, v.clone()));
            v
        }
    };
    if ext == "svg" {
        Asset::Inline(value)
    } else {
        Asset::Url(value)
    }
}

fn b64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(A[((n >> 18) & 63) as usize] as char);
        out.push(A[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(A[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(A[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn js_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn inlines(items: &[Inline], doc: &Doc) -> String {
    let ctx = Ctx::default();
    inlines_in(items, doc, &ctx)
}

fn inlines_in(items: &[Inline], doc: &Doc, ctx: &Ctx) -> String {
    let mut out = String::new();
    for i in items {
        inline(i, doc, ctx, &mut out);
    }
    out
}

fn nested(items: &[Inline], doc: &Doc, ctx: &Ctx) -> String {
    inlines_in(items, doc, ctx)
}

fn inline(item: &Inline, doc: &Doc, ctx: &Ctx, out: &mut String) {
    match item {
        Inline::Text(t) | Inline::Interp(t) => out.push_str(&svg::escape(t)),
        Inline::Code(c) => {
            out.push_str("<code>");
            out.push_str(&svg::escape(c));
            out.push_str("</code>");
        }
        Inline::Strong(v) => {
            out.push_str("<strong>");
            out.push_str(&nested(v, doc, ctx));
            out.push_str("</strong>");
        }
        Inline::Em(v) => {
            out.push_str("<em>");
            out.push_str(&nested(v, doc, ctx));
            out.push_str("</em>");
        }
        Inline::Strike(v) => {
            out.push_str("<del>");
            out.push_str(&nested(v, doc, ctx));
            out.push_str("</del>");
        }
        Inline::Link { text, href } => {
            out.push_str(&format!(
                "<a href=\"{}\">{}</a>",
                svg::escape(href.trim()),
                nested(text, doc, ctx)
            ));
        }
        Inline::Image { alt, src } => match asset(doc, src, ctx) {
            Asset::Url(u) => {
                out.push_str(&format!(
                    "<img class=\"inline-img\" src=\"{}\" alt=\"{}\" loading=\"lazy\">",
                    svg::escape(&u),
                    svg::escape(alt)
                ));
            }
            Asset::Inline(body) => {
                out.push_str(&format!("<span class=\"svg-inline\">{}</span>", body));
            }
            Asset::Missing => {
                out.push_str(&format!(
                    "<span class=\"missing\">{}</span>",
                    svg::escape(if alt.trim().is_empty() { src } else { alt })
                ));
            }
        },
        Inline::Math(m) => out.push_str(&math(false, m, false)),
        Inline::Break => out.push_str("<br>\n"),
        Inline::Ref(r) => {
            out.push_str("<span class=\"ref\">[");
            out.push_str(&svg::escape(r.trim()));
            out.push_str("]</span>");
        }
    }
}

const STYLESHEET: &str = r##":root {
  --ink: #1e222a;
  --muted: #666e7e;
  --grid: #e4e7ee;
  --code-edge: #e4e7ee;
  --accent: #346aba;
  --paper: #ffffff;
  --panel: #f6f7f9;
  --border: #e2e6ee;
  --code-bg: #f7f8fa;
  --shadow: 0 1px 2px rgba(16,24,40,.05), 0 10px 28px rgba(16,24,40,.06);
  --font-body: "Times New Roman", Times, "Liberation Serif", Georgia, serif;
  --font-head: var(--font-body);
  --font-mono: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace;
  --font-math: "Latin Modern Math", "Cambria Math", Georgia, "Times New Roman", serif;
  --font-sans-math: "Helvetica Neue", Helvetica, Arial, "Segoe UI", sans-serif;
  --size: 19px;
  --leading: 1.45;
  --measure: 46rem;
  /* vertical rhythm taken from the PDF layout: a paragraph leaves .55 of a
     line, a figure .8, a panel 1.2, a new section 1.9 */
  --s1: .55rem;
  --s2: .8rem;
  --s3: 1.2rem;
  --s4: 1.9rem;
  --good: #3a8f5c;
  --good-ink: #2f7d4f;
  --bad: #d64545;
  --bad-ink: #b3312f;
}

body.theme-serif {
  --font-body: Georgia, "Iowan Old Style", "Palatino Linotype", "Times New Roman", "Liberation Serif", serif;
  --font-head: var(--font-body);
  --leading: 1.55;
  --measure: 44rem;
  --accent: #8a5a2b;
}

body.theme-dark {
  --ink: #e6e8ee;
  --muted: #98a1b1;
  --grid: #262c37;
  --accent: #7fb2f0;
  --paper: #0f1115;
  --panel: #171b23;
  --border: #262c37;
  --code-bg: #141821;
  --good-ink: #6cc38d;
  --bad-ink: #f08a86;
  --shadow: 0 1px 2px rgba(0,0,0,.5), 0 14px 34px rgba(0,0,0,.45);
}

body.theme-minimal {
  --ink: #16181d;
  --muted: #6c7280;
  --grid: transparent;
  --accent: #16181d;
  --border: transparent;
  --panel: transparent;
  --code-bg: transparent;
  --shadow: none;
  --size: 17px;
  --leading: 1.6;
}

*, *::before, *::after { box-sizing: border-box; }

html { font-size: var(--size); -webkit-text-size-adjust: 100%; }

body {
  margin: 0;
  background: var(--paper);
  color: var(--ink);
  font-family: var(--font-body);
  font-size: 1rem;
  line-height: var(--leading);
  text-rendering: optimizeLegibility;
}

main {
  max-width: var(--measure);
  margin: 0 auto;
  padding: var(--s4) 1.5rem 5rem;
}

p { margin: 0 0 var(--s1); }
p:last-child { margin-bottom: 0; }

a { color: var(--accent); text-decoration: none; border-bottom: 1px solid color-mix(in srgb, var(--accent) 32%, transparent); }
a:hover { border-bottom-color: var(--accent); }

code {
  font-family: var(--font-mono);
  font-size: .86em;
  background: var(--panel);
  border: 0;
  border-radius: 0;
  padding: .06em .26em;
}

del { color: var(--muted); }

hr {
  border: 0;
  border-top: 1px solid var(--grid);
  margin: var(--s3) 0;
}

h1, h2, h3, h4, h5, h6 {
  font-family: var(--font-head);
  font-weight: 650;
  line-height: 1.22;
  letter-spacing: -.012em;
  scroll-margin-top: 4rem;
  margin: 0 0 .7em;
}

h1 { font-size: 2.1rem; }
h1, h2, h3, h4, h5, h6 { margin-bottom: var(--s1); font-weight: 700; letter-spacing: 0; }
h1 { font-size: 1.43rem; margin-top: var(--s4); }
h2 { font-size: 1.19rem; margin-top: var(--s4); }
h3, h4 { font-size: 1.05rem; margin-top: var(--s3); }
h5 { font-size: 1rem; margin-top: var(--s2); }
h6 { font-size: .9rem; margin-top: var(--s2); color: var(--muted); }

.anchor {
  margin-left: .45em;
  color: var(--muted);
  border: 0;
  font-weight: 400;
  opacity: 0;
  transition: opacity .12s ease;
}

h1:hover .anchor, h2:hover .anchor, h3:hover .anchor,
h4:hover .anchor, h5:hover .anchor, h6:hover .anchor { opacity: .55; }
.anchor:hover { opacity: 1 !important; color: var(--accent); }

ul, ol { margin: 0 0 var(--s1); padding-left: 1.45em; }
li { margin: .28em 0; }
li::marker { color: var(--accent); }
ol li::marker { font-weight: 650; font-variant-numeric: tabular-nums; }

.ref { color: var(--muted); font-family: var(--font-mono); font-size: .86em; }

.pagebreak { break-after: page; page-break-after: always; height: 0; margin: 0; border-top: 1px dashed var(--grid); }

.topbar {
  position: sticky;
  top: 0;
  z-index: 30;
  background: color-mix(in srgb, var(--paper) 86%, transparent);
  -webkit-backdrop-filter: saturate(1.5) blur(10px);
  backdrop-filter: saturate(1.5) blur(10px);
  border-bottom: 1px solid var(--border);
}

.topbar-inner {
  max-width: var(--measure);
  margin: 0 auto;
  padding: .5rem 1.5rem;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
}

.brand {
  font-family: var(--font-head);
  font-size: .84rem;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.to-top { font-size: .76rem; color: var(--muted); border: 0; white-space: nowrap; }
.to-top:hover { color: var(--accent); }

.doc-head { margin: 0 0 var(--s3); text-align: center; }
.doc-title { margin: 0 0 .2em; font-size: 2.1rem; font-weight: 700; }
.doc-sub { font-family: var(--font-head); font-size: 1.24rem; color: var(--muted); margin: 0 0 .5em; }
.byline { margin: 0 0 .2em; }
.lead {
  font-size: 1.05rem;
  color: var(--muted);
  line-height: 1.5;
  /* set as an abstract: narrower than the measure so it reads as a block
     rather than a stretched line */
  max-width: 32rem;
  margin: var(--s2) auto 0;
  text-align: center;
}
.keywords { font-size: .86rem; color: var(--muted); margin: var(--s1) auto 0; max-width: 32rem; }
.kw-label {
  font-size: 1em;
  font-weight: 700;
  margin-right: .4em;
}

.toc { margin: var(--s3) 0; }
.toc h2 { margin: 0 0 .3rem; font-size: 1.05rem; }
.toc ul { list-style: none; margin: 0; padding: 0; }
.toc ul ul { padding-left: 1.15em; }
.toc li { margin: 0 0 .15em; }
.toc a { color: var(--ink); border: 0; }
.toc a:hover { color: var(--accent); }
.toc-empty { color: var(--muted); font-size: .92rem; margin: 0; }

.quote {
  margin: var(--s2) 0;
  padding: 0 0 0 1rem;
  border-left: 3px solid var(--grid);
  color: var(--muted);
  font-style: italic;
}
.quote code { font-style: normal; }

.checklist { list-style: none; margin: 0 0 1.1em; padding: 0; }
.checklist li { display: flex; align-items: baseline; gap: .55em; margin: .3em 0; }
.checklist .box {
  flex: 0 0 auto;
  display: inline-block;
  width: 1.02em;
  height: 1.02em;
  margin-bottom: -.14em;
  border: 1.5px solid var(--border);
  border-radius: 4px;
  text-align: center;
  font-size: .82em;
  line-height: 1;
  color: var(--paper);
}
.checklist li.done .box { background: var(--accent); border-color: var(--accent); }
.checklist li.done { color: var(--muted); }

.cell {
  margin: 0 0 var(--s2);
  border: 1px solid var(--code-edge);
  border-radius: 0;
  background: var(--code-bg);
  overflow: hidden;
}
.cell-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: .6rem;
  padding: .15rem .4rem;
  border-bottom: 1px solid var(--border);
}
.cell-head .lang {
  font-family: var(--font-mono);
  font-size: .7rem;
  color: var(--muted);
  background: none;
  border: 0;
  padding: 0;
}
.copy {
  font: inherit;
  font-size: .72rem;
  line-height: 1.5;
  color: var(--muted);
  background: none;
  border: 0;
  border-radius: 0;
  padding: .1rem .3rem;
  cursor: pointer;
}
.copy:hover { color: var(--accent); }
.cell pre { margin: 0; padding: .45rem .55rem; overflow-x: auto; }
.cell pre code {
  display: block;
  font-family: var(--font-mono);
  font-size: .81rem;
  line-height: 1.62;
  background: none;
  border: 0;
  border-radius: 0;
  padding: 0;
  color: var(--ink);
  white-space: pre;
}
.cell .result {
  padding: .7rem 1rem;
  background: var(--paper);
  border-top: 1px solid var(--border);
  border-left: 3px solid transparent;
}
.cell .result.good { border-left-color: var(--good); }
.cell .result.bad { border-left-color: var(--bad); }
.cell .out {
  font-family: var(--font-mono);
  font-size: .82rem;
  line-height: 1.55;
  white-space: pre-wrap;
  margin: 0 0 .35em;
}
.cell .result > .value {
  font-family: var(--font-mono);
  font-size: .82rem;
  line-height: 1.55;
  color: var(--accent);
  background: none;
  border: 0;
  padding: 0;
  margin: 0;
  white-space: pre-wrap;
}
.cell .result > .err {
  font-family: var(--font-mono);
  font-size: .82rem;
  line-height: 1.55;
  color: var(--bad-ink);
  white-space: pre-wrap;
  margin: 0;
}

figure { margin: var(--s3) 0; }
figcaption {
  margin-top: .4rem;
  font-size: .86rem;
  font-style: italic;
  line-height: 1.45;
  color: var(--muted);
  text-align: center;
}
figure.plot svg { width: 100%; height: auto; display: block; }
.plot-wrap, .live-canvas-wrap { position: relative; }
.live-canvas-wrap canvas.live, .plot-wrap canvas.live {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
}
.plot-error { color: var(--bad-ink); font-family: var(--font-mono); font-size: .86rem; }

.table-wrap {
  overflow-x: auto;
  border-top: 1px solid var(--grid);
  border-bottom: 1px solid var(--grid);
  background: var(--paper);
}
table { border-collapse: collapse; width: 100%; font-size: .9rem; }
th, td { padding: .48rem .72rem; border-bottom: 0.5px solid var(--grid); text-align: left; vertical-align: top; }
thead th {
  font-weight: 650;
  background: var(--panel);
  border-bottom: 1px solid var(--border);
  white-space: nowrap;
}
tbody tr:last-child td { border-bottom: 0; }
table.zebra tbody tr:nth-child(even) { background: var(--panel); }
tr.more td { text-align: center; color: var(--muted); font-size: .84rem; }
tr.more em { font-style: italic; }

.note {
  margin: var(--s3) 0;
  padding: .5rem .8rem;
  border-left: 3px solid var(--note, var(--accent));
  border-radius: 0 3px 3px 0;
  background: var(--panel);
  background: color-mix(in srgb, var(--note, var(--accent)) 7%, var(--paper));
  font-size: .9rem;
  line-height: 1.4;
}
.note-info { --note: #346aba; }
.note-tip { --note: #48a05a; }
.note-warning { --note: #e0a028; }
.note-danger { --note: #d64545; }
.note-note { --note: #5a6476; }
.note-label {
  font-size: .86rem;
  font-weight: 700;
  color: var(--note, var(--accent));
  margin-bottom: .1rem;
}
.note-title { margin: 0 0 .25rem; }
.note-title strong { color: var(--note, var(--accent)); }
.note-body { margin: 0; }

.widget {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: .7rem;
  margin: var(--s2) 0;
  padding: .35rem 0;
  border: 0;
  border-top: 1px solid var(--grid);
  border-bottom: 1px solid var(--grid);
  background: none;
}
.widget label { font-size: .85rem; font-weight: 600; color: var(--muted); min-width: 5.5rem; }
.widget input[type="range"] { flex: 1 1 12rem; min-width: 8rem; height: 1.3rem; accent-color: var(--accent); }
.widget input[type="checkbox"] { width: 1.12rem; height: 1.12rem; accent-color: var(--accent); }
.widget select, .widget input[type="number"], .widget input[type="text"] {
  font: inherit;
  font-size: .88rem;
  color: var(--ink);
  background: var(--paper);
  border: 1px solid var(--border);
  border-radius: 7px;
  padding: .26rem .5rem;
}
.widget .value {
  margin-left: auto;
  font-family: var(--font-mono);
  font-size: .84rem;
  color: var(--accent);
  background: none;
  border: 0;
  padding: .1rem .2rem;
  min-width: 3.6rem;
  text-align: center;
}

img { max-width: 100%; height: auto; border-radius: 8px; }
.inline-img { display: inline-block; vertical-align: middle; }
.svg-inline { display: inline-block; max-width: 100%; line-height: 0; }
.svg-inline svg { max-width: 100%; height: auto; }
.missing {
  display: inline-block;
  font-family: var(--font-mono);
  font-size: .78rem;
  color: var(--muted);
  border: 1px dashed var(--border);
  border-radius: 6px;
  padding: .3rem .5rem;
}

figure.media { margin: var(--s3) 0; }
figure.media.float-left { float: left; margin: .4rem 1.6rem 1rem 0; max-width: 50%; }
figure.media.float-right { float: right; margin: .4rem 0 1rem 1.6rem; max-width: 50%; }
figure.media img { display: block; }
figure.media video, figure.media iframe {
  display: block;
  width: 100%;
  border: 0;
  border-radius: 0;
  background: #000;
}
figure.media iframe { aspect-ratio: 16 / 9; }
.audio-card {
  display: flex;
  align-items: center;
  gap: .9rem;
  padding: .4rem 0;
  border: 0;
  background: none;
}
.audio-icon {
  flex: 0 0 auto;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 1.5rem;
  height: 1.5rem;
  border-radius: 0;
  background: none;
  color: var(--accent);
  font-size: 1.05rem;
}
.audio-meta { flex: 1 1 auto; min-width: 0; }
.audio-name { display: block; font-size: .82rem; color: var(--muted); margin-bottom: .2rem; }
.audio-card audio { width: 100%; }

.math { white-space: nowrap; }
.math-display {
  display: block;
  margin: var(--s2) 0;
  padding: 0;
  overflow-x: auto;
  text-align: center;
}

.tex {
  font-family: var(--font-math);
  font-style: normal;
  line-height: 1.4;
}
.tex-display { font-size: 1.06em; }
.tex .mi { font-style: italic; }
.tex .mi.rm, .tex .mi.bf, .tex .mi.sf, .tex .mi.tt { font-style: normal; }
.tex .mi.sf { font-family: var(--font-sans-math); }
.tex .cal { font-family: var(--font-math); font-style: italic; }
.tex .mi.tt, .tex .mtext.tt { font-family: var(--font-mono); }
.tex .mn, .tex .mo, .tex .opname, .tex .op, .tex .bb, .tex .mtext, .tex .delim, .tex .space, .tex .lim-over, .tex .lim-under { font-style: normal; }
.tex .bb { font-weight: 400; }
.tex .mo.rel { padding: 0 .28em; }
.tex .mo.bin { padding: 0 .18em; }
.tex .mo.un { padding: 0 .04em; }
.tex .mo.punct { padding-right: .18em; }
.tex .opname { padding-right: .15em; }
.tex .op { font-size: 1.35em; line-height: 1; }
.tex .opbig { display: inline-flex; flex-direction: column; align-items: center; vertical-align: middle; line-height: 1.08; margin: 0 .12em; }
.tex .lim-over, .tex .lim-under { display: block; text-align: center; font-size: .7em; line-height: 1.15; }
.tex .scripts, .tex .mscripts { display: inline-block; white-space: nowrap; }
.tex .mscripts > .scripts { display: inline-block; }
.tex sub, .tex sup, .tex .sub, .tex .sup { font-size: .72em; line-height: 0; }
.tex .acc, .tex .acc-over, .tex .acc-under, .tex .acc-hat, .tex .acc-bar,
.tex .acc-vec, .tex .acc-dot, .tex .acc-ddot, .tex .acc-tilde, .tex .acc-check,
.tex .acc-breve, .tex .acc-acute, .tex .acc-grave, .tex .acc-mathring,
.tex .acc-widetilde, .tex .acc-widehat, .tex .acc-overrightarrow, .tex .acc-overleftarrow { position: relative; display: inline-block; }
.tex .accm { position: absolute; left: 50%; top: 0; transform: translateX(-50%); font-style: normal; line-height: 1; }
.tex .frac .num, .tex .frac .den { display: block; padding: 0 .2em; }
.tex .frac .bar { display: block; border-top: 1px solid currentColor; }
.tex .binom .num { padding-bottom: .18em; }
.tex .binom .den { padding-top: .18em; }
.tex .tfrac .num, .tex .tfrac .den { font-size: .88em; }
.tex .sqrt { display: inline-block; white-space: nowrap; margin: 0 .1em; }
.tex .sqrt .root { display: inline-block; border-top: 1px solid currentColor; padding: 0 .1em 0 0; }
.tex .sqrt .rad { font-size: 1.5em; line-height: 1; }
.tex .sqrt .idx { font-size: .6em; margin: 0 -.05em -.2em 0; }
.tex .delimpair, .tex .delim-in { display: inline-block; vertical-align: middle; white-space: nowrap; }
.tex .delim.big { line-height: .9; }
.tex .delim.big1 { font-size: 1.2em; }
.tex .delim.big2 { font-size: 1.5em; }
.tex .delim.big3 { font-size: 1.9em; }
.tex .delim.big4 { font-size: 2.3em; }
.tex .delim.big5 { font-size: 2.7em; }
.tex .delim { font-style: normal; }
.tex .not { position: relative; display: inline-block; }
.tex .notslash { position: absolute; left: -.08em; top: -.12em; font-size: .85em; line-height: 1; }
.tex .space, .tex .space.sp-thin, .tex .space.sp-med, .tex .space.sp-thick,
.tex .space.sp-neg, .tex .space.sp-norm, .tex .space.sp-quad, .tex .space.sp-qquad,
.tex .space.sp-hskip { display: inline-block; }
.tex .space.sp-nbsp { white-space: pre; }
.tex .space.sp-row { display: block; height: 0; }
.tex .prime { font-style: normal; }
.tex .pr { padding: 0 .04em; }
.tex .unk {
  color: #b23;
  font-family: var(--font-mono);
  font-size: .9em;
  border-bottom: 1px dotted currentColor;
}
.tex .hline { display: block; border-top: 1px solid var(--muted); }
.tex .eqtag { float: right; color: var(--muted); font-size: .85em; }
.tex .tex-env, .tex .tex-align { display: block; margin: .7em 0; width: 100%; }
.tex .env-equation, .tex .env-align, .tex .env-gather { text-align: center; }
.tex .al-row { display: flex; align-items: center; gap: .9em; width: 100%; }
.tex .al-center { text-align: center; }
.tex .al-r { flex: 1 1 0; text-align: right; }
.tex .al-l { flex: 1 1 0; text-align: left; }
.tex .matrix, .tex .matrix-matrix, .tex .matrix-smallmatrix, .tex .matrix-pmatrix,
.tex .matrix-bmatrix, .tex .matrix-Bmatrix, .tex .matrix-vmatrix, .tex .matrix-Vmatrix,
.tex .matrix-cases, .tex .matrix-dcases, .tex .matrix-array { display: inline-block; vertical-align: middle; margin: 0 .15em; }
.tex .mbrace, .tex .mbrace-l, .tex .mbrace-r { display: inline-block; vertical-align: middle; width: .3em; }
.tex .mbrace-l { border-right: 0; }
.tex .mbrace-r { border-left: 0; }
.tex .mgrid { display: inline-grid; justify-items: center; align-items: center; vertical-align: middle; }
.tex .mrow-grid { display: contents; }
.tex .mcell { padding: .12em .4em; }
.tex .mrow, .tex .grp, .tex .accb, .tex .radx { display: inline-block; }
.tex .colored { color: inherit; }
.tex .overset, .tex .underset { display: inline-block; text-align: center; vertical-align: middle; }

@page { margin: 16mm 14mm; }

@media print {
  html { font-size: 11.5pt; }
  .topbar, .copy, .anchor, canvas.live { display: none !important; }
  figure, table, .cell, .note, .widget, .quote, .toc { break-inside: avoid; }
  a { border-bottom: 0; }
  a[href^="http"]::after { content: " (" attr(href) ")"; font-size: .8em; color: var(--muted); word-break: break-all; }
  body.theme-default, body.theme-serif, body.theme-dark, body.theme-minimal {
    --ink: #1e222a;
    --muted: #5f6675;
    --grid: #d9dee7;
    --accent: #346aba;
    --paper: #ffffff;
    --panel: #f6f7f9;
    --border: #d9dee7;
    --code-bg: #f7f8fa;
    --good-ink: #2f7d4f;
    --bad-ink: #b3312f;
    --shadow: none;
  }
  html, body { background: #ffffff; }
}
"##;

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
keywords: alpha, beta
---

Intro paragraph with **bold**, *em*, `code`, a [link](https://example.com)
and inline math $a^2 + b^2 = c^2$ plus a tiny ![dot](pic.png).

## Intro {#intro}

Another paragraph with ~~struck~~ text and a [[1]] reference.

@toc

- first item
- second item

### Details

- [x] shipped
- [ ] pending

> quoted line

***

```nano
let total = 1 + 1
print("sum")
total
```

```text
not runnable
```

@table
columns = ["name", "value"]
rows = [["a", 1], ["b", 2], ["c", 3], ["d", 4]]
align = l,r
max_rows = 2
caption = "Sample rows"

@plot line
x = [0,1,2,3]
y = [1,4,2,5]
title = "Trend"
caption = "Growth"

@plot fn
f = "sin(x*t)"
domain = [0, 6.28]
interactive = "t"
title = "Live"
caption = "Damped"

@widget
kind = "slider"
name = "t"
label = "Gain"
min = 0
max = 1
step = 0.01
value = 0.5

@widget
kind = "checkbox"
name = "show"
label = "Show grid"
checked = true

@widget
kind = "select"
name = "mode"
label = "Mode"
options = ["fast", "slow"]

@note
kind = "tip"
title = "Heads up"

Be careful here.

@math

\int_0^\infty e^{-x^2} dx

@image
src = "pic.png"
alt = "A picture"
caption = "Nice"

@image
src = "vec.svg"
alt = "Vector art"
width = 60%
float = right
caption = "Inlined"

@video
src = "clip.mp4"
caption = "Clip"

@audio
src = "track.mp3"
caption = "Track"

@embed
src = "https://example.com/embed"
caption = "Embed"

@pagebreak

```nano
let oops = missing_fn()
```
"#;

    fn doc_of(src: &str, dir: &Path) -> Doc {
        let mut d = parse_document(src, dir);
        run_cells(&mut d);
        d
    }

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("nano-html-test").join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_dir() -> PathBuf {
        let dir = tmp("sample");
        fs::write(dir.join("pic.png"), b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR").unwrap();
        fs::write(dir.join("vec.svg"), b"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 4 4\"><rect width=\"4\" height=\"4\"/></svg>").unwrap();
        dir
    }

    #[test]
    fn renders_full_document() {
        let dir = sample_dir();
        let html = render(&doc_of(SRC, &dir));
        for needle in [
            "<!doctype html",
            "<html lang=\"en\">",
            "Nano Report",
            "A Tiny Study",
            "Ada Lovelace, Bob Stone",
            "id=\"intro\"",
            "class=\"anchor\"",
            "class=\"toc\"",
            "class=\"checklist\"",
            "class=\"quote\"",
            "<hr>",
            "class=\"cell\"",
            "data-copy-target=\"cell-1\"",
            "class=\"out\"",
            "class=\"err\"",
            "<figure",
            "<svg",
            "class=\"math-display\"",
            "data-tex=\"",
            "class=\"note note-tip\"",
            "data-var=\"t\"",
            "data-var-readout=\"t\"",
            "<canvas class=\"live\"",
            "window.NanoTex",
            "window.Nano",
            "<table",
            "Table 1. Sample rows",
            "2 more rows",
            "<video controls playsinline",
            "type=\"video/mp4\"",
            "<audio controls",
            "class=\"audio-card\"",
            "<iframe",
            "allowfullscreen",
            "referrerpolicy=\"no-referrer\"",
            "<select",
            "<option value=",
            "class=\"svg-inline\"",
            "data:image/png;base64,",
            "class=\"pagebreak\"",
            "</html>",
        ] {
            assert!(html.contains(needle), "missing {} in html", needle);
        }
        assert!(html.contains("Figure 1. Growth"), "{}", &html[..400]);
        assert!(html.contains("id=\"live-2\""), "second plot canvas missing");
        assert!(html.contains("data-var-name=\"x\""), "canvas axis name missing");
        assert!(
            html.contains("data-formula=") && html.contains("data-style="),
            "live plot options must live on the canvas as data attributes"
        );
        assert_eq!(
            html.matches("<script>").count(),
            1,
            "the runtime must be the only script tag: an inline script before the\n\
             bundle would throw ReferenceError: Nano is not defined and kill\n\
             math typesetting, widgets and every live plot"
        );
        assert!(!html.contains("src=\"pic.png\""), "local png was not embedded");
        assert!(!html.contains("src=\"vec.svg\""), "local svg was not embedded");
    }

    #[test]
    fn embeds_both_scripts_and_stays_offline() {
        let html = render(&doc_of("# Title\n\nhello\n", Path::new(".")));
        assert!(html.contains(include_str!("tex.js").trim_end()), "tex.js not embedded");
        assert!(html.contains(include_str!("js_runtime.js").trim_end()), "js_runtime.js not embedded");
        assert!(
            html.len() > include_str!("tex.js").len() + include_str!("js_runtime.js").len(),
            "scripts missing from output"
        );
        assert_eq!(html.matches("<script>").count(), 1, "expected a single script tag");
        for bad in ["http://cdn", "https://cdn", "//fonts.googleapis.com", "<link rel=\"stylesheet\"", "src=\"http://"] {
            assert!(!html.contains(bad), "external reference {} found", bad);
        }
    }

    #[test]
    fn renders_every_theme() {
        for theme in ["default", "serif", "dark", "minimal", "wobbly"] {
            let src = format!("---\ntitle: T\ntheme: {}\n---\n\n## S\n\ntext\n", theme);
            let html = render(&doc_of(&src, Path::new(".")));
            let want = if theme == "wobbly" { "default" } else { theme };
            assert!(html.contains(&format!("class=\"theme-{}\"", want)), "theme {} missing", want);
            assert!(html.contains("--font-mono"), "stylesheet missing for {}", want);
            // the page is set like the PDF, so no theme gets a sticky web bar
            assert!(!html.contains("class=\"topbar\""), "top bar should be absent for {}", want);
        }
    }

    #[test]
    fn inline_math_and_errors_are_escaped() {
        let d = Doc { base_dir: PathBuf::from("."), ..Default::default() };
        let out = inlines(
            &[
                Inline::Math("a < b \\& c".into()),
                Inline::Strong(vec![Inline::Text("x".into())]),
            ],
            &d,
        );
        assert_eq!(out, "<span class=\"math\" data-tex=\"a &lt; b \\&amp; c\">a &lt; b \\&amp; c</span><strong>x</strong>");
    }

    #[test]
    fn error_cell_gets_a_red_border() {
        let d = doc_of("```nano\nlet oops = missing_fn()\n```\n", Path::new("."));
        let html = render(&d);
        assert!(html.contains("<div class=\"result bad\">"), "{}", html);
    }

    #[test]
    fn base64_encoder_matches_reference() {
        assert_eq!(b64(b""), "");
        assert_eq!(b64(b"f"), "Zg==");
        assert_eq!(b64(b"fo"), "Zm8=");
        assert_eq!(b64(b"foo"), "Zm9v");
        assert_eq!(b64(b"foobar"), "Zm9vYmFy");
        assert_eq!(b64(&[0u8, 255, 128]), "AP+A");
    }

    #[test]
    fn width_parsing_covers_units_and_bare_numbers() {
        assert_eq!(width_style(None), "");
        assert_eq!(width_style(Some("")), "");
        assert_eq!(width_style(Some("nonsense")), "");
        assert_eq!(width_style(Some("60%")), "max-width:60%");
        assert_eq!(width_style(Some("300px")), "max-width:300px");
        assert_eq!(width_style(Some("120")), "max-width:120px");
        assert_eq!(width_style(Some("12rem")), "max-width:12rem");
    }
}
