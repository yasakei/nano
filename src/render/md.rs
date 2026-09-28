use std::fs;

use crate::doc::{
    Align, Block, CellResult, Doc, Inline, Media, MediaKind, Note, PlotBlock, Table, Widget,
};
use crate::render::svg;
use crate::value::{fmt_num, Value};

pub fn render(doc: &Doc) -> String {
    let mut ctx = Ctx { plots: 0, assets: 0 };
    let mut chunks: Vec<String> = Vec::new();
    if let Some(head) = header(doc) {
        chunks.push(head);
    }
    for b in &doc.blocks {
        if let Some(c) = block(doc, b, &mut ctx) {
            chunks.push(c);
        }
    }
    if chunks.is_empty() {
        return String::new();
    }
    let mut out = chunks.join("\n\n");
    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }
    format!("{}\n", out.trim())
}

struct Ctx {
    plots: usize,
    assets: usize,
}

fn header(doc: &Doc) -> Option<String> {
    let m = &doc.meta;
    if m.title.trim().is_empty() {
        return None;
    }
    let mut out = format!("# {}", m.title.trim());
    let sub = m.subtitle.trim();
    if !sub.is_empty() {
        out.push_str("\n\n*");
        out.push_str(sub);
        out.push('*');
    }
    let names: Vec<String> = m.author.iter().filter(|a| !a.trim().is_empty()).map(|a| a.trim().to_string()).collect();
    if !names.is_empty() {
        out.push_str("\n\n*");
        out.push_str(&names.join(", "));
        out.push('*');
    }
    let date = m.date.trim();
    if !date.is_empty() {
        out.push_str("\n\n");
        out.push_str(date);
    }
    let desc = m.description.trim();
    if !desc.is_empty() {
        out.push_str("\n\n");
        let quoted: Vec<String> = desc.lines().map(quote_line).collect();
        out.push_str(&quoted.join("\n"));
    }
    let kw: Vec<String> = m.keywords.iter().filter(|k| !k.trim().is_empty()).map(|k| k.trim().to_string()).collect();
    if !kw.is_empty() {
        out.push_str("\n\n**Keywords:** ");
        out.push_str(&kw.join(", "));
    }
    Some(out)
}

fn block(doc: &Doc, b: &Block, ctx: &mut Ctx) -> Option<String> {
    let out = match b {
        Block::Heading { level, inlines, id } => heading(*level, inlines, id, doc),
        Block::Para(items) => inlines(items, doc),
        Block::List { ordered, start, items } => list(*ordered, *start, items, doc),
        Block::Checklist { items } => checklist(items, doc),
        Block::Quote(items) => quote(items, doc),
        Block::Rule => "---".to_string(),
        Block::Code { lang, text, runnable, result } => code(lang, text, *runnable, result.as_ref()),
        Block::Plot(p) => plot(doc, p, ctx),
        Block::Table(t) => table(t),
        Block::Media(m) => media(doc, m, ctx),
        Block::Math { display, text } => math(*display, text),
        Block::Note(n) => note(n, doc),
        Block::Widget(w) => widget(w),
        Block::Toc { title } => toc(doc, title),
        Block::Raw(s) => s.clone(),
        Block::Directive(_) => "".to_string(),
        Block::PageBreak => "---".to_string(),
    };
    if out.trim().is_empty() {
        None
    } else {
        Some(out.trim_end().to_string())
    }
}

fn heading(level: u8, items: &[Inline], id: &str, doc: &Doc) -> String {
    let hashes = "#".repeat(level.clamp(1, 6) as usize);
    let mut out = format!("{} {}", hashes, inlines(items, doc));
    if !id.trim().is_empty() {
        out.push_str(&format!(" <a id=\"{}\"></a>", id.trim()));
    }
    out
}

fn toc(doc: &Doc, title: &str) -> String {
    let label = if title.trim().is_empty() { "Contents" } else { title.trim() };
    let mut out = format!("## {}", label);
    let hs: Vec<(u8, String, String)> =
        doc.headings().into_iter().filter(|(l, _, _)| *l > 1).collect();
    if hs.is_empty() {
        out.push_str("\n\n_(no headings)_");
        return out;
    }
    let base = hs.iter().map(|(l, _, _)| *l).min().unwrap_or(2);
    out.push_str("\n\n");
    for (lvl, text, id) in hs {
        let indent = "  ".repeat(lvl.saturating_sub(base) as usize);
        let label = if text.trim().is_empty() { id.clone() } else { text.trim().to_string() };
        let target = if id.trim().is_empty() { crate::doc::slugify(&label) } else { id.trim().to_string() };
        out.push_str(&format!("{}- [{}](#{})", indent, label, target));
        out.push('\n');
    }
    out.trim_end().to_string()
}

fn list(ordered: bool, start: u32, items: &[Vec<Inline>], doc: &Doc) -> String {
    let mut out = String::new();
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let marker = if ordered { format!("{}.", start as usize + i) } else { "-".to_string() };
        let text = inlines(item, doc);
        let text = if text.trim().is_empty() { marker.clone() } else { format!("{} {}", marker, text) };
        out.push_str(&text);
    }
    out
}

fn checklist(items: &[(bool, Vec<Inline>)], doc: &Doc) -> String {
    let mut out = String::new();
    for (i, (done, item)) in items.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format!("- [{}] {}", if *done { "x" } else { " " }, inlines(item, doc)));
    }
    out
}

fn quote(items: &[Inline], doc: &Doc) -> String {
    let text = inlines(items, doc);
    let mut out = String::new();
    for (i, l) in text.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&quote_line(l));
    }
    out
}

fn quote_line(l: &str) -> String {
    if l.trim().is_empty() {
        ">".to_string()
    } else {
        format!("> {}", l)
    }
}

fn code(lang: &str, text: &str, runnable: bool, result: Option<&CellResult>) -> String {
    let mut out = String::from("```");
    out.push_str(lang.trim());
    out.push('\n');
    if !text.trim_end().is_empty() {
        out.push_str(text.trim_end());
        out.push('\n');
    }
    out.push_str("```");
    let r = match result {
        Some(r) if runnable => r,
        _ => return out,
    };
    let stdout = r.stdout.trim_end();
    if !stdout.is_empty() {
        out.push_str("\n\n```text\n");
        out.push_str(stdout);
        out.push_str("\n```");
    }
    if let Some(v) = &r.value {
        let v = v.trim_end();
        if !v.is_empty() {
            out.push_str("\n\n```text\n");
            out.push_str(v);
            out.push_str("\n```");
        }
    }
    if let Some(e) = &r.error {
        out.push_str("\n\n> **error:**");
        for l in e.trim_end().lines() {
            out.push_str("\n> ");
            out.push_str(l);
        }
    }
    out
}

fn plot(doc: &Doc, p: &PlotBlock, ctx: &mut Ctx) -> String {
    let caption = first_non_empty(&[p.caption.as_str(), p.spec.caption.as_str()]).to_string();
    let alt = if !caption.is_empty() {
        caption.clone()
    } else if !p.spec.title.trim().is_empty() {
        p.spec.title.trim().to_string()
    } else {
        "Figure".to_string()
    };
    match crate::plot::render(&p.spec) {
        Ok(scene) => {
            ctx.plots += 1;
            let name = format!("plot-{}.svg", ctx.plots);
            let dir = doc.base_dir.join("assets");
            let _ = fs::create_dir_all(&dir);
            let _ = fs::write(dir.join(&name), svg::scene_to_svg(&scene));
            let mut out = format!("![{}](assets/{})", escape_label(&alt), name);
            if !caption.is_empty() {
                out.push_str("\n\n*");
                out.push_str(&caption);
                out.push('*');
            }
            out
        }
        Err(e) => format!("> **plot error:** {}", e),
    }
}

fn table(t: &Table) -> String {
    let widest = t.rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut cols: Vec<String> = t.columns.iter().map(|c| c.trim().to_string()).collect();
    while cols.len() < widest {
        cols.push((cols.len() + 1).to_string());
    }
    if cols.is_empty() {
        return "_(empty table)_".to_string();
    }
    let n = cols.len();
    let mut out = String::new();
    row(&mut out, &cols);
    let sep: Vec<String> = (0..n)
        .map(|i| match t.align.get(i).copied().unwrap_or(Align::Left) {
            Align::Left => ":---".to_string(),
            Align::Center => ":---:".to_string(),
            Align::Right => "---:".to_string(),
        })
        .collect();
    row(&mut out, &sep);
    let limit = t.max_rows.unwrap_or(usize::MAX);
    let shown = t.rows.len().min(limit);
    for r in t.rows.iter().take(shown) {
        let mut cells: Vec<String> = r.iter().map(cell_value).collect();
        while cells.len() < n {
            cells.push(String::new());
        }
        cells.truncate(n);
        row(&mut out, &cells);
    }
    if t.rows.len() > shown {
        let mut cells = vec!["…".to_string(); n];
        let note = format!("_{}_ more rows", t.rows.len() - shown);
        if n >= 2 {
            cells[1] = note;
        } else {
            cells[0] = note;
        }
        row(&mut out, &cells);
    }
    let caption = t.caption.trim();
    if !caption.is_empty() {
        out.push('\n');
        out.push('*');
        out.push_str(caption);
        out.push('*');
    }
    out
}

fn row(out: &mut String, cells: &[String]) {
    out.push_str("| ");
    out.push_str(&cells.join(" | "));
    out.push_str(" |\n");
}

fn cell_value(v: &Value) -> String {
    let text = v.to_display();
    text.replace('\n', " ").replace('\r', " ").replace('|', "\\|")
}

fn media(doc: &Doc, m: &Media, ctx: &mut Ctx) -> String {
    let src = m.src.trim();
    let label = first_non_empty(&[m.caption.as_str(), src]).to_string();
    let mut out = match m.kind {
        MediaKind::Image => {
            let link = copy_asset(doc, src, &mut ctx.assets);
            let alt = first_non_empty(&[m.alt.as_str(), m.caption.as_str(), src]).to_string();
            format!("![{}]({})", escape_label(&alt), link)
        }
        MediaKind::Video => format!("[Video: {}]({})", escape_label(&label), src),
        MediaKind::Audio => format!("[Audio: {}]({})", escape_label(&label), src),
        MediaKind::Embed | MediaKind::Iframe => format!("[Embed: {}]({})", escape_label(src), src),
    };
    let caption = m.caption.trim();
    if !caption.is_empty() {
        out.push_str("\n\n*");
        out.push_str(caption);
        out.push('*');
    }
    out
}

fn math(display: bool, text: &str) -> String {
    let t = text.trim();
    if t.is_empty() {
        return String::new();
    }
    if display || t.contains('\n') {
        format!("$$\n{}\n$$", t)
    } else {
        format!("${}$", t)
    }
}

fn note(n: &Note, doc: &Doc) -> String {
    let kind = match n.kind.trim().to_lowercase().as_str() {
        "tip" => "TIP",
        "warning" | "warn" | "caution2" => "WARNING",
        "danger" | "caution" | "error" => "CAUTION",
        "important" => "IMPORTANT",
        _ => "NOTE",
    };
    let mut out = format!("> [!{}]", kind);
    let title = n.title.trim();
    if !title.is_empty() {
        out.push_str("\n> **");
        out.push_str(title);
        out.push_str("**");
    }
    let body = inlines(&n.body, doc);
    let mut skip_title = !title.is_empty();
    for l in body.lines() {
        if skip_title {
            skip_title = false;
            if l.trim() == title {
                continue;
            }
        }
        out.push('\n');
        out.push_str(&quote_line(l));
    }
    out
}

fn widget(w: &Widget) -> String {
    let kind = if w.kind.trim().is_empty() { "control".to_string() } else { w.kind.trim().to_lowercase() };
    let label = if w.label.trim().is_empty() {
        default_label(&kind).to_string()
    } else {
        w.label.trim().to_string()
    };
    let name = if w.name.trim().is_empty() { "value" } else { w.name.trim() };
    let mut detail = kind.clone();
    if matches!(kind.as_str(), "slider" | "range" | "number" | "input" | "scale") {
        detail = format!("{}, {} – {}", detail, fmt_num(w.min), fmt_num(w.max));
        if w.step > 0.0 && w.step != 1.0 {
            detail = format!("{}, step {}", detail, fmt_num(w.step));
        }
    }
    if is_check(&kind) {
        if w.checked {
            detail = format!("{}, checked", detail);
        }
    }
    if !w.options.is_empty() {
        detail = format!("{}: {}", detail, w.options.join(", "));
    }
    format!("**{}** `{}` = {} ({})", label, name, widget_value(w, &kind), detail)
}

fn widget_value(w: &Widget, kind: &str) -> String {
    if is_check(kind) {
        return if w.checked { "true".to_string() } else { "false".to_string() };
    }
    if matches!(kind, "select" | "dropdown" | "choice" | "options" | "list") {
        let i = w.value.round();
        if i >= 0.0 && (i as usize) < w.options.len() {
            return w.options[i as usize].clone();
        }
    }
    fmt_num(w.value)
}

fn is_check(kind: &str) -> bool {
    matches!(kind, "checkbox" | "check" | "toggle" | "switch" | "bool" | "boolean")
}

fn default_label(kind: &str) -> &'static str {
    match kind {
        "select" | "dropdown" | "choice" | "options" | "list" => "Option",
        "checkbox" | "check" | "toggle" | "switch" | "bool" | "boolean" => "Toggle",
        "text" | "input" | "field" | "string" => "Input",
        "color" | "colour" => "Color",
        "button" => "Button",
        "date" => "Date",
        _ => "Parameter",
    }
}

pub fn copy_asset(doc: &Doc, src: &str, counter: &mut usize) -> String {
    let rel = src.trim();
    if rel.is_empty() {
        return src.to_string();
    }
    if rel.starts_with("http://") || rel.starts_with("https://") || rel.starts_with("data:") {
        return rel.to_string();
    }
    let path = doc.resolve(rel);
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp") {
        return rel.to_string();
    }
    if !path.is_file() {
        return rel.to_string();
    }
    let mut name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    *counter += 1;
    if name.is_empty() {
        name = format!("asset-{}.{}", counter, ext);
    }
    let dir = doc.base_dir.join("assets");
    if fs::create_dir_all(&dir).is_err() {
        return rel.to_string();
    }
    let target = dir.join(&name);
    if target != path && fs::copy(&path, &target).is_err() {
        return rel.to_string();
    }
    format!("assets/{}", name)
}

pub fn inlines(items: &[Inline], doc: &Doc) -> String {
    let mut ctx = InlineCtx { doc, assets: 0, at_start: true };
    let mut out = String::new();
    for i in items {
        inline(i, &mut out, &mut ctx);
    }
    out.trim_end().to_string()
}

struct InlineCtx<'a> {
    doc: &'a Doc,
    assets: usize,
    at_start: bool,
}

fn nested(items: &[Inline], out: &mut String, ctx: &mut InlineCtx) {
    for i in items {
        inline(i, out, ctx);
    }
    ctx.at_start = false;
}

fn inline(item: &Inline, out: &mut String, ctx: &mut InlineCtx) {
    match item {
        Inline::Text(t) => text(t, out, ctx),
        Inline::Interp(t) => text(t, out, ctx),
        Inline::Code(c) => {
            if c.contains('`') {
                out.push_str("``");
                out.push_str(c);
                out.push_str("``");
            } else {
                out.push('`');
                out.push_str(c);
                out.push('`');
            }
            ctx.at_start = false;
        }
        Inline::Strong(v) => {
            out.push_str("**");
            nested(v, out, ctx);
            out.push_str("**");
            ctx.at_start = false;
        }
        Inline::Em(v) => {
            out.push('*');
            nested(v, out, ctx);
            out.push('*');
            ctx.at_start = false;
        }
        Inline::Strike(v) => {
            out.push_str("~~");
            nested(v, out, ctx);
            out.push_str("~~");
            ctx.at_start = false;
        }
        Inline::Link { text: label, href } => {
            out.push('[');
            nested(label, out, ctx);
            out.push_str("](");
            out.push_str(href.trim());
            out.push(')');
            ctx.at_start = false;
        }
        Inline::Image { alt, src } => {
            let link = copy_asset(ctx.doc, src, &mut ctx.assets);
            out.push_str("![");
            out.push_str(&escape_label(alt));
            out.push_str("](");
            out.push_str(&link);
            out.push(')');
            ctx.at_start = false;
        }
        Inline::Math(m) => {
            let t = m.trim();
            if t.is_empty() {
                return;
            }
            if t.contains('\n') {
                out.push_str("$$\n");
                out.push_str(t);
                out.push_str("\n$$");
            } else {
                out.push('$');
                out.push_str(t);
                out.push('$');
            }
            ctx.at_start = false;
        }
        Inline::Break => {
            out.push_str("  \n");
            ctx.at_start = true;
        }
        Inline::Ref(r) => {
            out.push('[');
            out.push_str(r.trim());
            out.push(']');
            ctx.at_start = false;
        }
    }
}

fn text(src: &str, out: &mut String, ctx: &mut InlineCtx) {
    if src.is_empty() {
        return;
    }
    for (i, line) in src.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
            ctx.at_start = true;
        }
        if ctx.at_start {
            let rest = line.trim_start();
            let pad = &line[..line.len() - rest.len()];
            out.push_str(pad);
            match rest.chars().next() {
                Some('#') | Some('>') | Some('-') | Some('|') => {
                    out.push('\\');
                    out.push_str(rest);
                }
                _ => out.push_str(line),
            }
        } else {
            out.push_str(line);
        }
        ctx.at_start = false;
    }
}

fn escape_label(s: &str) -> String {
    s.replace('\\', "\\\\").replace('[', "\\[").replace(']', "\\]")
}

fn first_non_empty<'a>(parts: &[&'a str]) -> &'a str {
    for p in parts {
        if !p.trim().is_empty() {
            return p.trim();
        }
    }
    ""
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
keywords: alpha, beta
---

## Intro {#intro}

A paragraph with **bold**, *em*, `code` and a [link](https://example.com).

@toc

- first item
- second item

### Details

3. third
4. fourth

- [x] shipped
- [ ] pending

> quoted line

***

```nano
let total = 1 + 1
print("sum")
total
```

@table
columns = ["name", "value"]
rows = [["a", 1], ["b", 2]]
align = l,r
caption = "Sample rows"

@note
kind = "tip"
title = "Heads up"

Be careful here.

@widget
kind = "slider"
name = "t"
label = "Gain"
min = 0
max = 1
value = 0.5

@math

\int_0^\infty e^{-x^2} dx

@pagebreak
"#;

    fn doc_of(src: &str, dir: &Path) -> Doc {
        let mut d = parse_document(src, dir);
        run_cells(&mut d);
        d
    }

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("nano-md-test").join(name);
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn renders_full_document() {
        let md = render(&doc_of(SRC, Path::new(".")));
        assert!(md.contains("# Nano Report"), "{}", md);
        assert!(md.contains("*A Tiny Study*"), "{}", md);
        assert!(md.contains("*Ada Lovelace, Bob Stone*"), "{}", md);
        assert!(md.contains("2026-01-01"), "{}", md);
        assert!(md.contains("> A short summary."), "{}", md);
        assert!(md.contains("**Keywords:** alpha, beta"), "{}", md);
        assert!(md.contains("## Contents"), "{}", md);
        assert!(md.contains("- [Intro](#intro)"), "{}", md);
        assert!(md.contains("  - [Details](#details)"), "{}", md);
        assert!(md.contains("## Intro <a id=\"intro\"></a>"), "{}", md);
        assert!(md.contains("**bold**"), "{}", md);
        assert!(md.contains("`code`"), "{}", md);
        assert!(md.contains("[link](https://example.com)"), "{}", md);
        assert!(md.contains("- first item"), "{}", md);
        assert!(md.contains("3. third"), "{}", md);
        assert!(md.contains("4. fourth"), "{}", md);
        assert!(md.contains("- [x] shipped"), "{}", md);
        assert!(md.contains("> quoted line"), "{}", md);
        assert!(md.contains("\n---\n"), "{}", md);
        assert!(md.contains("```nano\nlet total = 1 + 1"), "{}", md);
        assert!(md.contains("```text\nsum\n```"), "{}", md);
        assert!(md.contains("| name | value |"), "{}", md);
        assert!(md.contains("| :--- | ---: |"), "{}", md);
        assert!(md.contains("| a | 1 |"), "{}", md);
        assert!(md.contains("*Sample rows*"), "{}", md);
        assert!(md.contains("> [!TIP]"), "{}", md);
        assert!(md.contains("> **Heads up**"), "{}", md);
        assert!(md.contains("**Gain** `t` = 0.5 (slider, 0 – 1)"), "{}", md);
        assert!(md.contains("$$\n\\int_0^\\infty e^{-x^2} dx\n$$"), "{}", md);
        assert!(md.ends_with('\n'), "{}", md);
    }

    #[test]
    fn code_error_becomes_blockquote() {
        let src = "```nano\nlet x = missing_fn()\n```\n";
        let md = render(&doc_of(src, Path::new(".")));
        assert!(md.contains("> **error:**"), "{}", md);
        assert!(md.contains("\n> "), "{}", md);
    }

    #[test]
    fn table_without_columns_is_numbered_and_truncated() {
        let mut d = Doc { base_dir: PathBuf::from("."), ..Default::default() };
        d.blocks.push(Block::Table(Table {
            columns: Vec::new(),
            rows: vec![
                vec![Value::str("a"), Value::Num(1.0)],
                vec![Value::str("b"), Value::Num(2.0)],
                vec![Value::str("c"), Value::Num(3.0)],
            ],
            align: vec![Align::Center],
            caption: String::new(),
            zebra: true,
            max_rows: Some(2),
        }));
        let md = render(&d);
        assert!(md.contains("| 1 | 2 |"), "{}", md);
        assert!(md.contains("| :---: | :--- |"), "{}", md);
        assert!(!md.contains("| c | 3 |"), "{}", md);
        assert!(md.contains("_1_ more rows"), "{}", md);
    }

    #[test]
    fn checklist_renders_both_states() {
        let mut d = Doc { base_dir: PathBuf::from("."), ..Default::default() };
        d.blocks.push(Block::Checklist {
            items: vec![
                (true, vec![Inline::Text("done".into())]),
                (false, vec![Inline::Text("todo".into())]),
            ],
        });
        let md = render(&d);
        assert!(md.contains("- [x] done"), "{}", md);
        assert!(md.contains("- [ ] todo"), "{}", md);
    }

    #[test]
    fn escapes_line_leading_markdown() {
        let mut d = Doc { base_dir: PathBuf::from("."), ..Default::default() };
        d.blocks.push(Block::Para(vec![Inline::Text("# not a heading".into())]));
        d.blocks.push(Block::Para(vec![Inline::Text("- not a list".into())]));
        d.blocks.push(Block::Para(vec![Inline::Text("> not a quote".into())]));
        d.blocks.push(Block::Para(vec![Inline::Text("a-b stays".into())]));
        let md = render(&d);
        assert!(md.contains("\\# not a heading"), "{}", md);
        assert!(md.contains("\\- not a list"), "{}", md);
        assert!(md.contains("\\> not a quote"), "{}", md);
        assert!(md.contains("a-b stays"), "{}", md);
    }

    #[test]
    fn plot_writes_svg_into_assets() {
        let dir = tmp("plot");
        let src = "@plot line\nx = [1,2,3]\ny = [2,4,3]\ntitle = \"Trend\"\ncaption = \"Growth\"\n\n@plot bar\nlabels = [\"a\",\"b\"]\ny = [1,2]\n\n";
        let md = render(&doc_of(src, &dir));
        assert!(md.contains("![Growth](assets/plot-1.svg)"), "{}", md);
        assert!(md.contains("*Growth*"), "{}", md);
        assert!(md.contains("![Figure](assets/plot-2.svg)"), "{}", md);
        assert!(dir.join("assets/plot-1.svg").is_file());
        assert!(dir.join("assets/plot-2.svg").is_file());
        let svg = fs::read_to_string(dir.join("assets/plot-1.svg")).unwrap();
        assert!(svg.starts_with("<svg"), "{}", svg);
    }

    #[test]
    fn images_are_copied_next_to_output() {
        let dir = tmp("media");
        fs::write(dir.join("pic.png"), b"\x89PNG\r\n\x1a\n").unwrap();
        let src = "@image\nsrc = \"pic.png\"\nalt = \"A picture\"\ncaption = \"Nice\"\n\n@video\nsrc = \"clip.mp4\"\ncaption = \"Clip\"\n\n";
        let md = render(&doc_of(src, &dir));
        assert!(md.contains("![A picture](assets/pic.png)"), "{}", md);
        assert!(md.contains("*Nice*"), "{}", md);
        assert!(md.contains("[Video: Clip](clip.mp4)"), "{}", md);
        assert!(dir.join("assets/pic.png").is_file());
    }

    #[test]
    fn remote_assets_are_left_alone() {
        let d = Doc { base_dir: PathBuf::from("."), ..Default::default() };
        let mut n = 0;
        assert_eq!(copy_asset(&d, "https://example.com/a.png", &mut n), "https://example.com/a.png");
        assert_eq!(copy_asset(&d, "missing.png", &mut n), "missing.png");
        assert_eq!(n, 0);
        let out = inlines(&[Inline::Image { alt: "x".into(), src: "https://e/i.png".into() }], &d);
        assert_eq!(out, "![x](https://e/i.png)");
    }
}
