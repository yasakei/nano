use crate::doc::ArgSet;
use crate::interp::Interp;
use crate::render::*;
use crate::value::{fmt_num, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotKind {
    Line,
    Scatter,
    Area,
    Step,
    Bar,
    HBar,
    Hist,
    Box,
    Heatmap,
    Errorbar,
    Function,
    Stem,
    Density,
}

impl PlotKind {
    pub fn parse(s: &str) -> PlotKind {
        match s.trim().to_lowercase().as_str() {
            "line" | "lines" | "plot" => PlotKind::Line,
            "scatter" | "points" | "dots" | "point" => PlotKind::Scatter,
            "area" | "fill" => PlotKind::Area,
            "step" | "stairs" => PlotKind::Step,
            "bar" | "bars" | "column" | "columns" => PlotKind::Bar,
            "hbar" | "bars_h" => PlotKind::HBar,
            "hist" | "histogram" => PlotKind::Hist,
            "box" | "boxplot" | "boxes" => PlotKind::Box,
            "heatmap" | "matrix" | "imshow" => PlotKind::Heatmap,
            "errorbar" | "errors" | "err" => PlotKind::Errorbar,
            "fn" | "function" | "f" => PlotKind::Function,
            "stem" => PlotKind::Stem,
            "density" | "kde" => PlotKind::Density,
            _ => PlotKind::Line,
        }
    }

    pub fn is_categorical(&self) -> bool {
        matches!(self, PlotKind::Bar | PlotKind::HBar)
    }
}

#[derive(Clone, Debug)]
pub struct Series {
    pub label: String,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub lo: Vec<f64>,
    pub hi: Vec<f64>,
    pub color: Option<Color>,
    pub values: Vec<f64>,
    pub bins: Vec<(f64, f64)>,
    pub stats: Vec<[f64; 5]>,
    pub cells: Vec<f64>,
    pub rows: usize,
    pub cols: usize,
    pub weight: f64,
    pub group: String,
}

impl Series {
    pub fn empty() -> Series {
        Series {
            label: String::new(),
            x: Vec::new(),
            y: Vec::new(),
            lo: Vec::new(),
            hi: Vec::new(),
            color: None,
            values: Vec::new(),
            bins: Vec::new(),
            stats: Vec::new(),
            cells: Vec::new(),
            rows: 0,
            cols: 0,
            weight: 1.0,
            group: String::new(),
        }
    }

    pub fn color(&self, i: usize) -> Color {
        self.color.unwrap_or_else(|| series_color(i))
    }
}

#[derive(Clone, Debug)]
pub struct PlotSpec {
    pub kind: PlotKind,
    pub series: Vec<Series>,
    pub categories: Vec<String>,
    pub title: String,
    pub xlabel: String,
    pub ylabel: String,
    pub width: f64,
    pub height: f64,
    pub legend: bool,
    pub grid: bool,
    pub log_x: bool,
    pub log_y: bool,
    pub smooth: bool,
    pub style: String,
    pub fill: f64,
    pub bins: usize,
    pub xmin: Option<f64>,
    pub xmax: Option<f64>,
    pub ymin: Option<f64>,
    pub ymax: Option<f64>,
    pub ci: Option<f64>,
    pub caption: String,
    pub interactive: Option<String>,
    pub formula: Option<String>,
    pub var: String,
    pub domain: (f64, f64),
    pub accent: Option<Color>,
    pub point_size: f64,
    pub line_width: f64,
    pub font: Font,
    pub y2: Vec<f64>,
    pub y2label: String,
    pub show_points: Option<bool>,
    pub annots: Vec<Annot>,
}

#[derive(Clone, Debug)]
pub struct Annot {
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub series: usize,
}

impl Default for PlotSpec {
    fn default() -> Self {
        PlotSpec {
            kind: PlotKind::Line,
            series: Vec::new(),
            categories: Vec::new(),
            title: String::new(),
            xlabel: String::new(),
            ylabel: String::new(),
            width: 660.0,
            height: 400.0,
            legend: true,
            grid: true,
            log_x: false,
            log_y: false,
            smooth: false,
            style: String::new(),
            fill: 0.18,
            bins: 0,
            xmin: None,
            xmax: None,
            ymin: None,
            ymax: None,
            ci: None,
            caption: String::new(),
            interactive: None,
            formula: None,
            var: "t".into(),
            domain: (0.0, 1.0),
            accent: None,
            point_size: 3.2,
            line_width: 1.8,
            font: Font::Sans,
            y2: Vec::new(),
            y2label: String::new(),
            show_points: None,
            annots: Vec::new(),
        }
    }
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn eval(interp: &mut Interp, src: &str, errs: &mut Vec<String>) -> Value {
    match crate::parser::parse_expr_str(src) {
        Ok(e) => match interp.eval(&e) {
            Ok(v) => v,
            Err(msg) => {
                errs.push(format!("`{}` ({})", src.trim(), msg));
                Value::Null
            }
        },
        Err(msg) => {
            errs.push(format!("`{}` ({})", src.trim(), msg));
            Value::Null
        }
    }
}

fn nums(v: &Value) -> Vec<f64> {
    match v {
        Value::List(l) => l.borrow().iter().filter_map(|x| x.num()).collect(),
        Value::Range(..) => v.as_list().unwrap_or_default().iter().filter_map(|x| x.num()).collect(),
        Value::Map(m) => m.borrow().iter().filter_map(|(_, v)| v.num()).collect(),
        Value::Str(s) => s
            .split_whitespace()
            .filter_map(|t| t.parse::<f64>().ok())
            .collect(),
        Value::Num(n) => vec![*n],
        _ => Vec::new(),
    }
}

fn strs(v: &Value) -> Vec<String> {
    v.items().iter().map(|x| x.to_display()).collect()
}

fn is_matrix(v: &Value) -> bool {
    match v.as_list() {
        Some(items) => !items.is_empty() && items.iter().all(|i| matches!(i, Value::List(_))),
        None => false,
    }
}

impl PlotSpec {
    /// `errs` collects every argument that failed to evaluate. A plot whose data
    /// is missing must be reported, not drawn as an empty set of axes.
    pub fn from_args(
        kind: &str,
        args: &ArgSet,
        interp: &mut Interp,
        errs: &mut Vec<String>,
    ) -> PlotSpec {
        let mut spec = PlotSpec { kind: PlotKind::parse(kind), ..Default::default() };
        let get_raw = |k: &str| args.named.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
        let get = |k: &str, i: &mut Interp, e: &mut Vec<String>| get_raw(k).map(|v| eval(i, &v, e));
        let pos = |i: usize, ip: &mut Interp, e: &mut Vec<String>| {
            args.positional.get(i).map(|s| eval(ip, s, e))
        };

        // chart labels are drawn as plain strings in every backend, so any TeX
        // in them is reduced to readable text rather than shown as source
        spec.title = get_raw("title")
            .map(|s| crate::math::plain(s.trim_matches('"')))
            .unwrap_or_default();
        spec.xlabel = get_raw("xlabel")
            .or_else(|| get_raw("x_label"))
            .map(|s| crate::math::plain(s.trim_matches('"')))
            .unwrap_or_default();
        spec.ylabel = get_raw("ylabel")
            .or_else(|| get_raw("y_label"))
            .map(|s| crate::math::plain(s.trim_matches('"')))
            .unwrap_or_default();
        spec.legend = get_raw("legend").map(|s| s != "false" && s != "none" && s != "0").unwrap_or_else(|| {
            get_raw("legend").is_none() || !matches!(get_raw("legend").unwrap().as_str(), "false" | "none" | "0")
        });
        if let Some(v) = get_raw("grid") {
            spec.grid = !(v == "false" || v == "none" || v == "0");
        }
        if let Some(v) = get_raw("log_y").or_else(|| get_raw("logy")) {
            spec.log_y = !(v == "false" || v == "0");
        }
        if let Some(v) = get_raw("log_x").or_else(|| get_raw("logx")) {
            spec.log_x = !(v == "false" || v == "0");
        }
        if let Some(v) = get_raw("smooth") {
            spec.smooth = !(v == "false" || v == "0");
        }
        spec.style = get_raw("style").map(|s| s.trim_matches('"').to_string()).unwrap_or_default();
        if let Some(v) = get("width", interp, errs).and_then(|v| v.num()) {
            spec.width = v;
        }
        if let Some(v) = get("height", interp, errs).and_then(|v| v.num()) {
            spec.height = v;
        }
        if let Some(v) = get("bins", interp, errs).and_then(|v| v.num()) {
            spec.bins = v as usize;
        }
        if let Some(v) = get("fill", interp, errs).and_then(|v| v.num()) {
            spec.fill = v;
        }
        if let Some(v) = get("ci", interp, errs).and_then(|v| v.num()) {
            spec.ci = Some(v);
        }
        spec.xmin = get("xmin", interp, errs).and_then(|v| v.num());
        spec.xmax = get("xmax", interp, errs).and_then(|v| v.num());
        spec.ymin = get("ymin", interp, errs).and_then(|v| v.num());
        spec.ymax = get("ymax", interp, errs).and_then(|v| v.num());
        spec.accent = get_raw("color").map(|c| parse_color(&c, series_color(0)));
        if let Some(v) = get("points", interp, errs) {
            spec.show_points = Some(v.truthy());
        }
        if let Some(v) = get("point_size", interp, errs).and_then(|v| v.num()) {
            spec.point_size = v;
        }
        if let Some(v) = get("linewidth", interp, errs).and_then(|v| v.num()) {
            spec.line_width = v;
        }
        spec.formula = get_raw("f")
            .or_else(|| get_raw("formula"))
            .or_else(|| get_raw("fn"))
            .map(|s| s.trim_matches('"').to_string());
        spec.var = get_raw("x").map(|s| s.trim_matches('"').to_string()).unwrap_or_else(|| "t".into());
        if let Some(d) = get("domain", interp, errs) {
            let v = nums(&d);
            if v.len() >= 2 {
                spec.domain = (v[0], v[1]);
            }
        }
        spec.interactive = get_raw("interactive").map(|s| s.trim_matches('"').to_string());
        // `x` is only data when it resolves; on an interactive plot a bare
        // identifier names the widget, which only exists in the browser.
        let x_val = {
            let mark = errs.len();
            let v = get("x", interp, errs);
            if errs.len() > mark {
                if spec.interactive.is_some() {
                    if let Some(raw) = get_raw("x") {
                        let name = raw.trim().trim_matches('"').to_string();
                        if is_identifier(&name) {
                            spec.var = name;
                            errs.truncate(mark);
                            None
                        } else {
                            v
                        }
                    } else {
                        v
                    }
                } else {
                    v
                }
            } else {
                v
            }
        };
        if let Some(d) = &x_val {
            if spec.formula.is_some() && get_raw("x").is_some() && !is_matrix(d) {
                let v = nums(d);
                if v.len() == 2 {
                    spec.domain = (v[0], v[1]);
                    spec.var = get_raw("x").unwrap().trim_matches('"').to_string();
                }
            }
        }
        if let Some(v) = get_raw("var") {
            spec.var = v.trim_matches('"').to_string();
        }
        let labels: Vec<String> = get("labels", interp, errs)
            .map(|v| strs(&v).into_iter().map(|l| crate::math::plain(&l)).collect())
            .unwrap_or_default();
        let colors = get_raw("colors")
            .map(|s| split_list(&s).iter().map(|c| parse_color(c, series_color(0))).collect::<Vec<_>>())
            .unwrap_or_default();

        let x_val = x_val.or_else(|| pos(1, interp, errs));
        let y_val = get("y", interp, errs)
            .or_else(|| pos(2, interp, errs))
            .or_else(|| pos(1, interp, errs));
        let y_val = y_val.filter(|v| !matches!(v, Value::Null));
        let x_val = x_val.filter(|v| !matches!(v, Value::Null));

        let kind = spec.kind;
        if let Some(f) = spec.formula.clone() {
            let (a, b) = spec.domain;
            let xs: Vec<f64> = (0..241).map(|i| a + (b - a) * i as f64 / 240.0).collect();
            let mut s = Series::empty();
            s.label = labels.first().cloned().unwrap_or_else(|| "f(x)".into());
            s.x = xs.clone();
            s.y = xs.iter().map(|x| eval_formula(&f, &spec.var, *x).unwrap_or(f64::NAN)).collect();
            s.color = spec.accent;
            spec.series.push(s);
            spec.kind = PlotKind::Function;
            return finalize(spec);
        }

        match kind {
            PlotKind::Hist | PlotKind::Density => {
                let values = y_val
                    .as_ref()
                    .map(nums)
                    .or_else(|| x_val.as_ref().map(nums))
                    .unwrap_or_default();
                let mut s = Series::empty();
                s.values = values.clone();
                s.label = labels.first().cloned().unwrap_or_else(|| "count".into());
                s.color = spec.accent;
                spec.series.push(s);
                return finalize(spec);
            }
            PlotKind::Heatmap => {
                let m = y_val.as_ref().or(x_val.as_ref()).map(|v| matrix(v)).unwrap_or_default();
                let (rows, cols) = (m.len(), m.first().map(|r| r.len()).unwrap_or(0));
                let mut s = Series::empty();
                s.cells = m.into_iter().flatten().collect();
                s.rows = rows;
                s.cols = cols;
                s.label = labels.first().cloned().unwrap_or_default();
                spec.series.push(s);
                return finalize(spec);
            }
            PlotKind::Box => {
                let groups = group_values(y_val.as_ref(), &spec);
                for (i, (name, vals)) in groups.into_iter().enumerate() {
                    let mut s = Series::empty();
                    s.label = labels.get(i).cloned().unwrap_or(name);
                    s.values = vals;
                    s.color = spec.accent.or(colors.get(i).copied());
                    spec.series.push(s);
                }
                return finalize(spec);
            }
            PlotKind::Bar | PlotKind::HBar => {
                let cats = labels.clone();
                let groups = group_values(y_val.as_ref(), &spec);
                if groups.len() == 1 {
                    let (name, vals) = groups.into_iter().next().unwrap();
                    let n = vals.len();
                    for (i, v) in vals.iter().enumerate() {
                        let mut s = Series::empty();
                        s.label = cats.get(i).cloned().unwrap_or_else(|| format!("{}", i + 1));
                        s.values = vec![*v];
                        s.weight = 1.0;
                        s.color = spec.accent.or(colors.get(i).copied());
                        let _ = &name;
                        spec.series.push(s);
                    }
                    let _ = n;
                } else {
                    for (i, (name, vals)) in groups.into_iter().enumerate() {
                        let mut s = Series::empty();
                        s.label = labels.get(i).cloned().unwrap_or(name);
                        s.values = vals;
                        s.color = spec.accent.or(colors.get(i).copied());
                        spec.series.push(s);
                    }
                }
                spec.categories = cats;
                return finalize(spec);
            }
            _ => {}
        }

        let xs: Option<Vec<f64>> = x_val.as_ref().map(nums);
        let groups = group_values(y_val.as_ref(), &spec);
        let n = groups.first().map(|(_, v)| v.len()).unwrap_or(0);
        let nseries = groups.len();
        for (i, (name, vals)) in groups.into_iter().enumerate() {
            let mut s = Series::empty();
            s.label = labels.get(i).cloned().unwrap_or(if nseries == 1 { String::new() } else { name });
            s.y = vals;
            s.x = xs.clone().unwrap_or_else(|| (0..n).map(|k| k as f64).collect());
            s.color = spec.accent.or(colors.get(i).copied());
            spec.series.push(s);
        }
        if let Some(e) = y_val.as_ref().and_then(|v| v.get_member("err").ok()) {
            let err = nums(&e);
            if !err.is_empty() {
                if let Some(first) = spec.series.first_mut() {
                    for (i, y) in first.y.iter().enumerate() {
                        if let Some(d) = err.get(i) {
                            first.lo.push(y - d);
                            first.hi.push(y + d);
                        }
                    }
                }
            }
        }
        finalize(spec)
    }
}

fn split_list(s: &str) -> Vec<String> {
    let t = s.trim().trim_start_matches('[').trim_end_matches(']');
    t.split(',').map(|x| x.trim().trim_matches('"').to_string()).collect()
}

fn matrix(v: &Value) -> Vec<Vec<f64>> {
    v.as_list()
        .unwrap_or_default()
        .iter()
        .map(|row| nums(row))
        .collect()
}

fn group_values(v: Option<&Value>, spec: &PlotSpec) -> Vec<(String, Vec<f64>)> {
    let v = match v {
        Some(v) => v,
        None => return Vec::new(),
    };
    if is_matrix(v) {
        return v
            .as_list()
            .unwrap_or_default()
            .iter()
            .enumerate()
            .map(|(i, row)| (format!("s{}", i + 1), nums(row)))
            .collect();
    }
    if let Value::Map(m) = v {
        let pairs: Vec<(String, Vec<f64>)> = m
            .borrow()
            .iter()
            .map(|(k, val)| (k.clone(), nums(val)))
            .collect();
        if !pairs.is_empty() {
            return pairs;
        }
    }
    if spec.kind.is_categorical() {
        if let Value::List(l) = v {
            let rows: Vec<Value> = l.borrow().clone();
            let mut names: Vec<String> = Vec::new();
            let mut cols: Vec<Vec<f64>> = Vec::new();
            for row in rows {
                match row {
                    Value::Map(m) => {
                        for (k, val) in m.borrow().iter() {
                            if let Some(pos) = names.iter().position(|n| n == k) {
                                cols[pos].push(val.num().unwrap_or(f64::NAN));
                            } else {
                                names.push(k.clone());
                                cols.push(vec![val.num().unwrap_or(f64::NAN)]);
                            }
                        }
                    }
                    Value::List(pair) if pair.borrow().len() == 2 => {
                        let key = pair.borrow()[0].to_display();
                        let num = pair.borrow()[1].num().unwrap_or(f64::NAN);
                        if let Some(pos) = names.iter().position(|n| *n == key) {
                            cols[pos].push(num);
                        } else {
                            names.push(key);
                            cols.push(vec![num]);
                        }
                    }
                    other => {
                        names.push(other.to_display());
                        cols.push(vec![other.num().unwrap_or(f64::NAN)]);
                    }
                }
            }
            if names.len() > 1 {
                return names.into_iter().zip(cols).collect();
            }
        }
    }
    vec![(String::new(), nums(v))]
}

fn finalize(mut spec: PlotSpec) -> PlotSpec {
    if spec.series.is_empty() {
        spec.series.push(Series::empty());
    }
    if spec.legend && spec.series.len() < 2 && !spec.title.is_empty() {
        spec.legend = false;
    }
    if spec.series.iter().all(|s| s.label.is_empty()) {
        spec.legend = false;
    }
    if spec.kind == PlotKind::Line && spec.style.is_empty() {
        spec.style = match spec.show_points {
            Some(true) => "both".into(),
            Some(false) => "line".into(),
            None => {
                let n = spec.series.first().map(|s| s.y.len()).unwrap_or(0);
                if n > 0 && n <= 40 && spec.series.len() == 1 {
                    "both".into()
                } else {
                    "line".into()
                }
            }
        };
    }
    if spec.kind == PlotKind::Scatter && spec.style.is_empty() {
        spec.style = "dot".into();
    }
    spec
}

pub fn eval_formula(src: &str, var: &str, x: f64) -> Option<f64> {
    let expr = crate::parser::parse_expr_str(src).ok()?;
    let mut interp = Interp::new();
    let bound = subst(&expr, var, x);
    interp.eval(&bound).ok().and_then(|v| v.num())
}

fn subst(e: &crate::ast::Expr, var: &str, x: f64) -> crate::ast::Expr {
    use crate::ast::Arg;
    use crate::ast::Expr as E;
    match e {
        E::Ident(name) if name == var || name == "x" => E::Num(x),
        E::Unary(op, a) => E::Unary(op, Box::new(subst(a, var, x))),
        E::Binary(op, a, b) => E::Binary(op, Box::new(subst(a, var, x)), Box::new(subst(b, var, x))),
        E::Call(f, args) => E::Call(
            Box::new(subst(f, var, x)),
            args.iter()
                .map(|a| match a {
                    Arg::Pos(e) => Arg::Pos(subst(e, var, x)),
                    Arg::Named(n, e) => Arg::Named(n.clone(), subst(e, var, x)),
                })
                .collect(),
        ),
        E::Ternary(c, a, b) => E::Ternary(
            Box::new(subst(c, var, x)),
            Box::new(subst(a, var, x)),
            Box::new(subst(b, var, x)),
        ),
        E::List(items) => E::List(items.iter().map(|i| subst(i, var, x)).collect()),
        other => other.clone(),
    }
}

pub fn render(spec: &PlotSpec) -> Result<Scene, String> {
    let mut frame = Frame::new(spec);
    match spec.kind {
        PlotKind::Line | PlotKind::Function | PlotKind::Step | PlotKind::Stem
        | PlotKind::Scatter | PlotKind::Area | PlotKind::Errorbar => draw_xy(&mut frame),
        PlotKind::Bar => draw_bars(&mut frame),
        PlotKind::HBar => draw_hbars(&mut frame),
        PlotKind::Hist => draw_hist(&mut frame),
        PlotKind::Density => draw_density(&mut frame),
        PlotKind::Box => draw_box(&mut frame),
        PlotKind::Heatmap => draw_heatmap(&mut frame),
    }
    Ok(frame.finish())
}

pub struct Scale {
    lo: f64,
    hi: f64,
    px0: f64,
    px1: f64,
    log: bool,
}

impl Scale {
    fn map(&self, v: f64) -> f64 {
        if self.log {
            let v = if v > 0.0 { v.log10() } else { f64::NAN };
            let lo = if self.lo > 0.0 { self.lo.log10() } else { 0.0 };
            let hi = if self.hi > 0.0 { self.hi.log10() } else { 1.0 };
            if (hi - lo).abs() < 1e-12 {
                return self.px0;
            }
            self.px0 + (v - lo) / (hi - lo) * (self.px1 - self.px0)
        } else {
            if (self.hi - self.lo).abs() < 1e-12 {
                return self.px0;
            }
            self.px0 + (v - self.lo) / (self.hi - self.lo) * (self.px1 - self.px0)
        }
    }
}

pub struct Frame {
    pub spec: PlotSpec,
    pub scene: Scene,
    pub plot: (f64, f64, f64, f64),
    pub xs: Scale,
    pub ys: Scale,
    pub cat_slots: Vec<(f64, f64)>,
    pub legend: Vec<(String, Color, bool)>,
    pub pad: (f64, f64, f64, f64),
}

impl Frame {
    pub fn new(spec: &PlotSpec) -> Frame {
        let scene = Scene::new(spec.width, spec.height);
        Frame {
            spec: spec.clone(),
            scene,
            plot: (0.0, 0.0, spec.width, spec.height),
            xs: Scale { lo: 0.0, hi: 1.0, px0: 0.0, px1: 1.0, log: false },
            ys: Scale { lo: 0.0, hi: 1.0, px0: 0.0, px1: 1.0, log: false },
            cat_slots: Vec::new(),
            legend: Vec::new(),
            pad: (0.0, 0.0, 0.0, 0.0),
        }
    }

    fn collect_values(&self) -> (Vec<f64>, Vec<f64>) {
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        for s in &self.spec.series {
            for (i, y) in s.y.iter().enumerate() {
                if y.is_finite() {
                    ys.push(*y);
                    if let Some(x) = s.x.get(i) {
                        xs.push(*x);
                    }
                }
            }
            for v in &s.values {
                if v.is_finite() {
                    if !matches!(self.spec.kind, PlotKind::Bar | PlotKind::HBar) {
                        xs.push(*v);
                    }
                    ys.push(*v);
                }
            }
        }
        (xs, ys)
    }

    fn axis_labels(&self, spec: &str) -> (f64, f64, f64) {
        let (xmin, xmax) = match spec {
            "x" => {
                if self.spec.xmin.is_some() || self.spec.xmax.is_some() {
                    (self.spec.xmin.unwrap(), self.spec.xmax.unwrap_or(f64::MAX))
                } else {
                    (0.0, 1.0)
                }
            }
            _ => {
                if self.spec.ymin.is_some() || self.spec.ymax.is_some() {
                    (self.spec.ymin.unwrap(), self.spec.ymax.unwrap_or(f64::MAX))
                } else {
                    (0.0, 1.0)
                }
            }
        };
        (xmin, xmax, 1.0)
    }

    fn compute_bounds(&self) -> (f64, f64, f64, f64) {
        let (mut xs, mut ys) = self.collect_values();
        if matches!(self.spec.kind, PlotKind::Bar | PlotKind::HBar) {
            for s in &self.spec.series {
                for v in &s.values {
                    xs.push(*v);
                }
            }
        }
        if xs.is_empty() {
            xs.push(0.0);
        }
        if ys.is_empty() {
            ys.push(0.0);
        }
        let (mut xlo, mut xhi) = min_max(&xs);
        let (mut ylo, mut yhi) = min_max(&ys);
        if self.spec.xmin.is_some() {
            xlo = self.spec.xmin.unwrap();
        }
        if self.spec.xmax.is_some() {
            xhi = self.spec.xmax.unwrap();
        }
        if self.spec.ymin.is_some() {
            ylo = self.spec.ymin.unwrap();
        }
        if self.spec.ymax.is_some() {
            yhi = self.spec.ymax.unwrap();
        }
        // a histogram counts and a density estimates a height, so their y axis
        // spans counts and curve height, not the range of the data
        let vals = all_values(&self.spec);
        match self.spec.kind {
            PlotKind::Hist if !vals.is_empty() => {
                let nb = if self.spec.bins > 0 { self.spec.bins } else { sturges(vals.len()) };
                let (_, _, counts) = hist_bins(&vals, nb);
                let maxc = counts.iter().copied().max().unwrap_or(1) as f64;
                if self.spec.ymin.is_none() {
                    ylo = 0.0;
                }
                if self.spec.ymax.is_none() {
                    yhi = (maxc * 1.06).max(1.0);
                }
            }
            PlotKind::Density if vals.len() >= 2 => {
                let (a, b, _, ys) = kde(&vals);
                if self.spec.xmin.is_none() {
                    xlo = a;
                }
                if self.spec.xmax.is_none() {
                    xhi = b;
                }
                if self.spec.ymin.is_none() {
                    ylo = 0.0;
                }
                if self.spec.ymax.is_none() {
                    yhi = ys.iter().copied().fold(0.0f64, f64::max) * 1.06;
                }
            }
            _ => {}
        }
        if self.spec.log_x {
            xlo = xlo.max(1e-12);
            xhi = xhi.max(xlo * 10.0);
        }
        if self.spec.log_y {
            ylo = ylo.max(1e-12);
            yhi = yhi.max(ylo * 10.0);
        }
        if self.spec.kind == PlotKind::HBar {
            // a horizontal bar keeps a real value range on x, with a zero baseline
            let all_nonneg = self.spec.series.iter().flat_map(|s| s.values.iter()).all(|v| *v >= 0.0);
            if all_nonneg && self.spec.xmin.is_none() {
                xlo = 0.0;
            }
        } else if self.spec.kind.is_categorical() {
            // for every other categorical kind the x axis is the category axis
            xlo = 0.0;
            xhi = 1.0;
            ylo = 0.0;
            if yhi == 0.0 {
                yhi = 1.0;
            }
        }
        if matches!(self.spec.kind, PlotKind::Line | PlotKind::Function | PlotKind::Step) {
            let all_nonneg = ys.iter().all(|v| *v >= 0.0);
            if all_nonneg && self.spec.ymin.is_none() {
                ylo = 0.0;
            }
        }
        (xlo, xhi, ylo, yhi)
    }

    fn layout(&mut self) {
        let font = self.spec.font;
        let tick_size = 10.0;
        let probe = 9.0;
        let (xlo, xhi, ylo, yhi) = self.raw_bounds();
        let mut left = 16.0f64;
        if self.spec.kind == PlotKind::HBar {
            // the left gutter holds category names, not numbers
            let per_category = self.spec.series.iter().all(|s| s.values.len() <= 1);
            let n = if per_category {
                self.spec.series.len()
            } else {
                self.spec.series.iter().map(|s| s.values.len()).max().unwrap_or(0)
            };
            for i in 0..n {
                let label = self
                    .spec
                    .categories
                    .get(i)
                    .cloned()
                    .or_else(|| self.spec.series.get(i).map(|s| s.label.clone()))
                    .unwrap_or_else(|| (i + 1).to_string());
                left = left.max(text_width(&label, font, Weight::Regular, 10.0) + 13.0);
            }
        } else {
            let ydec = decimals_for(ticks(ylo, yhi, self.spec.log_y, 6));
            for t in ticks(ylo, yhi, self.spec.log_y, 6) {
                let label = if self.spec.log_y {
                    fmt_pow10(t)
                } else {
                    fmt_num((t * 10f64.powi(ydec)).round() / 10f64.powi(ydec))
                };
                left = left.max(text_width(&label, font, Weight::Regular, tick_size) + 13.0);
            }
        }
        if !self.spec.ylabel.is_empty() {
            left = left.max(46.0);
        }
        let right = 14.0f64;
        let mut bottom: f64 = 12.0;
        bottom += if self.spec.kind.is_categorical() {
            16.0
        } else {
            let xt = ticks(xlo, xhi, self.spec.log_x, 8);
            let xdec = decimals_for(xt.clone());
            let mut w = 0.0f64;
            for t in &xt {
                let label = if self.spec.log_x {
                    fmt_pow10(*t)
                } else {
                    fmt_num((t * 10f64.powi(xdec)).round() / 10f64.powi(xdec))
                };
                w = w.max(text_width(&label, font, Weight::Regular, tick_size));
            }
            let w2 = self
                .spec
                .categories
                .iter()
                .take(24)
                .map(|c| text_width(c, font, Weight::Regular, probe))
                .fold(0.0f64, f64::max);
            w.max(w2.min(140.0)) * 0.5 + 14.0
        };
        if !self.spec.xlabel.is_empty() {
            bottom += 18.0;
        }
        let top = if self.spec.title.is_empty() {
            10.0
        } else {
            30.0
        };
        let w = self.spec.width.max(120.0);
        let h = self.spec.height.max(100.0);
        self.plot = (left, top, (w - left - right).max(40.0), (h - top - bottom).max(40.0));
        self.pad = (left, top, right, bottom);
    }

    fn raw_bounds(&self) -> (f64, f64, f64, f64) {
        self.compute_bounds()
    }

    fn legend_width(&self) -> f64 {
        if !self.spec.legend {
            return 0.0;
        }
        let mut max = 0.0f64;
        for s in &self.spec.series {
            if !s.label.is_empty() {
                max = max.max(text_width(&s.label, self.spec.font, Weight::Regular, 10.0));
            }
        }
        max + 30.0
    }

    fn draw_frame(&mut self) {
        self.layout();
        let (xlo, xhi, ylo, yhi) = self.compute_bounds();
        let (px, py, pw, ph) = self.plot;
        self.xs = Scale { lo: xlo, hi: xhi, px0: px, px1: px + pw, log: self.spec.log_x };
        self.ys = Scale { lo: ylo, hi: yhi, px0: py + ph, px1: py, log: self.spec.log_y };

        if self.spec.grid {
            // a horizontal bar's value axis is x, so its grid lines are vertical
            let vgrid = |f: &mut Frame, t: f64| {
                let x = f.xs.map(t);
                if x >= px - 0.5 && x <= px + pw + 0.5 {
                    f.scene.dashed(x, py, x, py + ph, GRID, 0.7, vec![2.0, 3.0]);
                }
            };
            if self.spec.kind == PlotKind::HBar {
                for t in ticks(xlo, xhi, self.spec.log_x, 6) {
                    vgrid(self, t);
                }
            } else {
                for t in ticks(ylo, yhi, self.spec.log_y, 6) {
                    let y = self.ys.map(t);
                    if y >= py - 0.5 && y <= py + ph + 0.5 {
                        self.scene.dashed(px, y, px + pw, y, GRID, 0.7, vec![2.0, 3.0]);
                    }
                }
                if !self.spec.kind.is_categorical() {
                    for t in ticks(xlo, xhi, self.spec.log_x, 8) {
                        vgrid(self, t);
                    }
                }
            }
        }

        self.scene.line(px, py + ph, px + pw, py + ph, AXIS, 1.0);
        self.scene.line(px, py, px, py + ph, AXIS, 1.0);

        let tick_size = 10.0;
        let hbar = self.spec.kind == PlotKind::HBar;
        let ydec = decimals_for(ticks(ylo, yhi, self.spec.log_y, 6));
        if !hbar {
            for t in ticks(ylo, yhi, self.spec.log_y, 6) {
                let y = self.ys.map(t);
                if y < py - 0.5 || y > py + ph + 0.5 {
                    continue;
                }
                self.scene.dashed(px - 3.0, y, px, y, AXIS, 0.8, Vec::new());
                let label = if self.spec.log_y { fmt_pow10(t) } else { fmt_num((t * 10f64.powi(ydec)).round() / 10f64.powi(ydec)) };
                self.scene.text_at(px - 7.0, y, label, tick_size, MUTED, self.spec.font, Weight::Regular, HAlign::Right, VAlign::Middle);
            }
        }

        if hbar {
            // categories down the left, values along the bottom
            let slots = self.category_slots();
            for (i, (cy, _)) in slots.iter().enumerate() {
                let label = self
                    .spec
                    .categories
                    .get(i)
                    .cloned()
                    .or_else(|| self.spec.series.get(i).map(|s| s.label.clone()))
                    .unwrap_or_else(|| (i + 1).to_string());
                let label = ellipsize(&label, self.spec.font, Weight::Regular, 10.0, ph.max(60.0));
                self.scene.text_at(px - 7.0, *cy, label, 10.0, MUTED, self.spec.font, Weight::Regular, HAlign::Right, VAlign::Middle);
            }
            let xt = ticks(xlo, xhi, self.spec.log_x, 8);
            let xdec = decimals_for(xt.clone());
            for t in xt {
                let x = self.xs.map(t);
                if x < px - 0.5 || x > px + pw + 0.5 {
                    continue;
                }
                self.scene.dashed(x, py + ph, x, py + ph + 3.0, AXIS, 0.8, Vec::new());
                let label = if self.spec.log_x {
                    fmt_pow10(t)
                } else {
                    fmt_num((t * 10f64.powi(xdec)).round() / 10f64.powi(xdec))
                };
                self.scene.text_at(x, py + ph + 6.0, label, tick_size, MUTED, self.spec.font, Weight::Regular, HAlign::Center, VAlign::Top);
            }
        } else if self.spec.kind.is_categorical() {
            let slots = self.category_slots();
            for (i, (cx, _)) in slots.iter().enumerate() {
                let label = self
                    .spec
                    .categories
                    .get(i)
                    .cloned()
                    .or_else(|| self.spec.series.get(i).map(|s| s.label.clone()))
                    .unwrap_or_else(|| (i + 1).to_string());
                let label = ellipsize(&label, self.spec.font, Weight::Regular, 10.0, pw.max(60.0));
                self.scene.text_at(*cx, py + ph + 6.0, label, 10.0, MUTED, self.spec.font, Weight::Regular, HAlign::Center, VAlign::Top);
            }
        } else {
            let xt = ticks(xlo, xhi, self.spec.log_x, 8);
            let xdec = decimals_for(xt.clone());
            for t in xt {
                let x = self.xs.map(t);
                if x < px - 0.5 || x > px + pw + 0.5 {
                    continue;
                }
                self.scene.dashed(x, py + ph, x, py + ph + 3.0, AXIS, 0.8, Vec::new());
                let label = if self.spec.log_x {
                    fmt_pow10(t)
                } else {
                    fmt_num((t * 10f64.powi(xdec)).round() / 10f64.powi(xdec))
                };
                self.scene.text_at(x, py + ph + 6.0, label, tick_size, MUTED, self.spec.font, Weight::Regular, HAlign::Center, VAlign::Top);
            }
        }

        if !self.spec.title.is_empty() {
            self.scene.text_at(px, 12.0, self.spec.title.clone(), 14.0, INK, self.spec.font, Weight::Bold, HAlign::Left, VAlign::Top);
        }
        if !self.spec.xlabel.is_empty() {
            self.scene.text_at(px + pw / 2.0, self.spec.height - 14.0, self.spec.xlabel.clone(), 11.0, MUTED, self.spec.font, Weight::Regular, HAlign::Center, VAlign::Top);
        }
        if !self.spec.ylabel.is_empty() {
            self.scene.push(Item::Text {
                x: 13.0,
                y: py + ph / 2.0,
                text: self.spec.ylabel.clone(),
                size: 11.0,
                color: MUTED,
                font: self.spec.font,
                weight: Weight::Regular,
                halign: HAlign::Center,
                valign: VAlign::Middle,
                rotate: -90.0,
            });
        }
        self.draw_legend();
    }

    fn legend_entries(&self) -> Vec<(String, Color, bool)> {
        self.spec
            .series
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.label.is_empty())
            .map(|(i, s)| (s.label.clone(), s.color(i), !matches!(s.values.is_empty() && false, true)))
            .collect()
    }

    fn draw_legend(&mut self) {
        if !self.spec.legend {
            return;
        }
        let entries = self.legend_entries();
        if entries.is_empty() {
            return;
        }
        self.legend = entries.clone();
        let size = 10.0;
        let row_h = 14.0;
        let mut total = 0.0;
        for (label, _, _) in &entries {
            total += 16.0 + text_width(label, self.spec.font, Weight::Regular, size) + 12.0;
        }
        let (px, py, pw, ph) = self.plot;
        let avail_top = self.spec.width - px - 10.0;
        if total <= avail_top && py >= 20.0 {
            let mut x = self.spec.width - 10.0 - total;
            let y = 8.0;
            for (label, color, _) in &entries {
                self.scene.line(x, y + 5.0, x + 12.0, y + 5.0, *color, 2.0);
                self.scene.rect(x + 4.0, y + 2.5, 4.0, 4.0, Some(*color), None);
                self.scene.text_at(x + 17.0, y, label.clone(), size, INK, self.spec.font, Weight::Regular, HAlign::Left, VAlign::Top);
                x += 16.0 + text_width(label, self.spec.font, Weight::Regular, size) + 12.0;
            }
            let _ = (pw, ph);
            return;
        }
        let max_w = entries
            .iter()
            .map(|(l, _, _)| text_width(l, self.spec.font, Weight::Regular, size) + 22.0)
            .fold(0.0f64, f64::max);
        let bw = (max_w + 12.0).min(pw * 0.5);
        let bh = entries.len() as f64 * row_h + 8.0;
        let bx = px + pw - bw - 8.0;
        let by = py + 8.0;
        self.scene.rect(bx, by, bw, bh, Some(PAPER.with_alpha(0.9)), Some(GRID));
        for (i, (label, color, _)) in entries.iter().enumerate() {
            let y = by + 6.0 + i as f64 * row_h;
            self.scene.line(bx + 7.0, y + 5.0, bx + 19.0, y + 5.0, *color, 2.0);
            self.scene.rect(bx + 11.0, y + 2.5, 4.0, 4.0, Some(*color), None);
            self.scene.text_at(bx + 24.0, y, label.clone(), size, INK, self.spec.font, Weight::Regular, HAlign::Left, VAlign::Top);
        }
    }

    fn category_slots(&mut self) -> Vec<(f64, f64)> {
        if !self.cat_slots.is_empty() {
            return self.cat_slots.clone();
        }
        let (px, py, pw, ph) = self.plot;
        // one series per category, or one value per category when series are grouped
        let per_category = self.spec.series.iter().all(|s| s.values.len() <= 1);
        let n = if per_category {
            self.spec.series.len()
        } else {
            self.spec.series.iter().map(|s| s.values.len()).max().unwrap_or(0)
        };
        if n == 0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        if self.spec.kind == PlotKind::HBar {
            // categories run down the y axis
            let step = ph / n as f64;
            for i in 0..n {
                out.push((py + step * (i as f64 + 0.5), step));
            }
        } else {
            let step = pw / n as f64;
            for i in 0..n {
                out.push((px + step * (i as f64 + 0.5), step));
            }
        }
        self.cat_slots = out.clone();
        out
    }

    pub fn finish(self) -> Scene {
        self.scene
    }
}

fn draw_xy(frame: &mut Frame) {
    frame.draw_frame();
    let style = frame.spec.style.clone();
    let show_line = matches!(style.as_str(), "" | "line" | "both" | "area" | "step" | "stem");
    let show_pts = matches!(style.as_str(), "both" | "dot" | "points" | "area")
        || (style.is_empty() && frame.spec.show_points == Some(true));
    let filled = matches!(frame.spec.kind, PlotKind::Area) || style == "area";
    for (si, s) in frame.spec.series.clone().iter().enumerate() {
        let color = s.color(si);
        let pts: Vec<Point> = s
            .x
            .iter()
            .zip(s.y.iter())
            .filter(|(_, y)| y.is_finite())
            .map(|(x, y)| Point::new(frame.xs.map(*x), frame.ys.map(*y)))
            .collect();
        if pts.is_empty() {
            continue;
        }
        if frame.spec.kind == PlotKind::Errorbar || !s.lo.is_empty() {
            draw_errorbars(frame, s, si);
        }
        if filled && pts.len() > 1 {
            let base = frame.ys.map(frame.ys.lo.max(0.0).min(frame.ys.hi));
            let mut poly = vec![Point::new(pts[0].x, base)];
            poly.extend(pts.iter().cloned());
            poly.push(Point::new(pts[pts.len() - 1].x, base));
            frame.scene.filled_poly(poly, color.with_alpha(frame.spec.fill), None, 0.0);
        }
        if show_line && pts.len() > 1 {
            let mut path = pts.clone();
            if frame.spec.kind == PlotKind::Step {
                path = stepify(&pts);
            }
            if frame.spec.smooth {
                path = smooth(&pts);
            }
            frame.scene.poly(path, color, frame.spec.line_width, false);
        }
        if show_pts || frame.spec.kind == PlotKind::Scatter {
            let r = frame.spec.point_size;
            for p in &pts {
                if frame.spec.kind == PlotKind::Scatter {
                    frame.scene.rect(p.x - r / 1.6, p.y - r / 1.6, r * 1.25, r * 1.25, Some(color.with_alpha(0.85)), None);
                } else if frame.spec.kind == PlotKind::Stem {
                    frame.scene.line(p.x, frame.ys.map(0.0), p.x, p.y, color, 1.2);
                    frame.scene.rect(p.x - r / 2.0, p.y - r / 2.0, r, r, Some(color), None);
                } else {
                    frame.scene.rect(p.x - r / 2.0, p.y - r / 2.0, r, r, Some(PAPER), Some(color));
                }
            }
        }
    }
    for a in frame.spec.annots.clone() {
        let x = frame.xs.map(a.x);
        let y = frame.ys.map(a.y);
        frame.scene.line(x, y, x, y - 16.0, MUTED, 0.8);
        frame.scene.text_at(x, y - 18.0, a.text.clone(), 10.0, INK, frame.spec.font, Weight::Regular, HAlign::Center, VAlign::Bottom);
    }
}

fn draw_errorbars(frame: &mut Frame, s: &Series, si: usize) {
    let color = s.color(si);
    for i in 0..s.y.len() {
        let (x, y) = match (s.x.get(i), s.y.get(i)) {
            (Some(x), Some(y)) => (frame.xs.map(*x), frame.ys.map(*y)),
            _ => continue,
        };
        let lo = s.lo.get(i).map(|v| frame.ys.map(*v)).unwrap_or(y);
        let hi = s.hi.get(i).map(|v| frame.ys.map(*v)).unwrap_or(y);
        frame.scene.line(x, lo, x, hi, color, 1.0);
        frame.scene.line(x - 3.0, lo, x + 3.0, lo, color, 1.0);
        frame.scene.line(x - 3.0, hi, x + 3.0, hi, color, 1.0);
    }
}

fn stepify(pts: &[Point]) -> Vec<Point> {
    let mut out = Vec::new();
    for i in 0..pts.len() {
        if i > 0 {
            out.push(Point::new(pts[i].x, pts[i - 1].y));
        }
        out.push(pts[i]);
    }
    out
}

fn smooth(pts: &[Point]) -> Vec<Point> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut out = Vec::with_capacity(pts.len());
    out.push(pts[0]);
    for i in 0..pts.len() - 1 {
        let p0 = if i == 0 { pts[0] } else { pts[i - 1] };
        let p1 = pts[i];
        let p2 = pts[i + 1];
        let p3 = if i + 2 < pts.len() { pts[i + 2] } else { p2 };
        for k in 0..6 {
            let t = k as f64 / 6.0;
            let x = catmull(p0.x, p1.x, p2.x, p3.x, t);
            let y = catmull(p0.y, p1.y, p2.y, p3.y, t);
            out.push(Point::new(x, y));
        }
    }
    out.push(pts[pts.len() - 1]);
    out
}

fn catmull(p0: f64, p1: f64, p2: f64, p3: f64, t: f64) -> f64 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * (2.0 * p1 + (-p0 + p2) * t + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2 + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

fn draw_bars(frame: &mut Frame) {
    frame.draw_frame();
    let pw = frame.plot.2;
    let slots = frame.category_slots();
    let n = slots.len().max(1);
    let gap_ratio = 0.25;
    let inner = pw / n as f64 * (1.0 - gap_ratio);
    let each = inner / frame.spec.series.len().max(1) as f64;
    for (si, s) in frame.spec.series.clone().iter().enumerate() {
        let color = s.color(si);
        for (i, v) in s.values.iter().enumerate() {
            if !v.is_finite() {
                continue;
            }
            let (cx, _) = slots[i.min(slots.len().saturating_sub(1))];
            let x = cx - inner / 2.0 + each * si as f64;
            let yv = frame.ys.map(*v);
            let y0 = frame.ys.map(0.0);
            let (top, h) = if yv <= y0 { (yv, y0 - yv) } else { (y0, yv - y0) };
            let h = h.max(0.5);
            frame.scene.rect(x + each * gap_ratio / 2.0, top, each * (1.0 - gap_ratio), h, Some(color), None);
        }
    }
}

fn draw_hbars(frame: &mut Frame) {
    frame.draw_frame();
    let slots = frame.category_slots();
    if slots.is_empty() {
        return;
    }
    let gap = 0.25;
    let per_category = frame.spec.series.iter().all(|s| s.values.len() <= 1);
    let nseries = frame.spec.series.len().max(1) as f64;
    for (si, s) in frame.spec.series.clone().iter().enumerate() {
        let color = s.color(si);
        // a category gets the whole slot; grouped series share one slot
        let (mut cy, thick) = if per_category {
            slots[si.min(slots.len() - 1)]
        } else {
            (0.0, slots[0].1 * (1.0 - gap))
        };
        let each = thick / nseries;
        for (i, v) in s.values.iter().enumerate() {
            if !v.is_finite() {
                continue;
            }
            if !per_category {
                cy = slots[i.min(slots.len().saturating_sub(1))].0 - thick / 2.0 + each * si as f64;
            }
            let h = each * (1.0 - gap);
            let xv = frame.xs.map(*v);
            let x0 = frame.xs.map(0.0);
            let (left, w) = if xv >= x0 { (x0, xv - x0) } else { (xv, x0 - xv) };
            frame.scene.rect(left, cy - h / 2.0, w.max(0.5), h, Some(color), None);
            if w < 40.0 {
                frame.scene.text_at(left + w + 5.0, cy, fmt_num(*v), 9.5, MUTED, frame.spec.font, Weight::Regular, HAlign::Left, VAlign::Middle);
            }
        }
    }
}

/// Bin `vals` into `nb` equal buckets, returning the data range and the counts.
fn hist_bins(vals: &[f64], nb: usize) -> (f64, f64, Vec<usize>) {
    let (lo, hi) = min_max(vals);
    let nb = nb.max(1);
    let w = ((hi - lo) / nb as f64).max(1e-9);
    let mut counts = vec![0usize; nb];
    for v in vals {
        let k = (((*v - lo) / w).floor() as isize).clamp(0, nb as isize - 1) as usize;
        counts[k] += 1;
    }
    (lo, hi, counts)
}

/// Gaussian kernel density over a slightly padded range, as (a, b, xs, ys).
fn kde(vals: &[f64]) -> (f64, f64, Vec<f64>, Vec<f64>) {
    let n: f64 = 200.0;
    let (lo, hi) = min_max(vals);
    let pad = (hi - lo) * 0.15;
    let a = lo - pad;
    let b = hi + pad;
    let bw = (0.9 * (variance(vals).sqrt()) * n.powf(-0.2)).max(1e-9);
    let xs: Vec<f64> = (0..=200).map(|i| a + (b - a) * i as f64 / 200.0).collect();
    let ys: Vec<f64> = xs
        .iter()
        .map(|x| {
            vals.iter()
                .map(|v| (-((x - v).powi(2)) / (2.0 * bw * bw)).exp())
                .sum::<f64>()
                / (vals.len() as f64 * bw * (2.0 * std::f64::consts::PI).sqrt())
        })
        .collect();
    (a, b, xs, ys)
}

/// Every finite value a series carries, whichever field it was stored in.
fn all_values(spec: &PlotSpec) -> Vec<f64> {
    spec.series
        .iter()
        .flat_map(|s| s.values.iter().chain(s.y.iter()))
        .copied()
        .filter(|v| v.is_finite())
        .collect()
}

fn draw_hist(frame: &mut Frame) {
    frame.draw_frame();
    let (px, _, pw, _) = frame.plot;
    let s = frame.spec.series.first().cloned().unwrap_or_else(Series::empty);
    let vals = s.values.clone();
    if vals.is_empty() {
        return;
    }
    let color = s.color(0);
    let nb = if frame.spec.bins > 0 { frame.spec.bins } else { sturges(vals.len()) };
    let (lo, hi, counts) = hist_bins(&vals, nb);
    let w = ((hi - lo) / nb as f64).max(1e-9);
    let maxc = counts.iter().copied().max().unwrap_or(1) as f64;
    let bw = pw / nb as f64;
    for (i, c) in counts.iter().enumerate() {
        let h = if maxc > 0.0 { *c as f64 / maxc * frame.plot.3 * 0.9 } else { 0.0 };
        let x = px + bw * i as f64;
        frame.scene.rect(x + 0.5, frame.plot.1 + frame.plot.3 - h, bw - 1.0, h, Some(color.with_alpha(0.9)), Some(color.shade(-0.15)));
    }
    for i in 0..nb {
        if nb > 12 && i % 2 == 1 {
            continue;
        }
        let x = px + bw * (i as f64 + 0.5);
        frame.scene.text_at(x, frame.plot.1 + frame.plot.3 + 6.0, fmt_num(lo + w * (i as f64 + 0.5)), 9.5, MUTED, frame.spec.font, Weight::Regular, HAlign::Center, VAlign::Top);
    }
}

fn draw_density(frame: &mut Frame) {
    frame.draw_frame();
    let s = frame.spec.series.first().cloned().unwrap_or_else(Series::empty);
    let vals = s.values.clone();
    if vals.len() < 2 {
        return;
    }
    let (_, _, xs, ys) = kde(&vals);
    let pts: Vec<Point> = xs
        .iter()
        .zip(ys.iter())
        .map(|(x, y)| Point::new(frame.xs.map(*x), frame.ys.map(*y)))
        .collect();
    let color = s.color(0);
    frame.scene.filled_poly(pts.clone(), color.with_alpha(0.2), None, 0.0);
    frame.scene.poly(pts, color, frame.spec.line_width, false);
}

fn draw_box(frame: &mut Frame) {
    frame.draw_frame();
    let series = frame.spec.series.clone();
    let (px, py, pw, ph) = frame.plot;
    let n = series.len().max(1);
    let slot = pw / n as f64;
    let bw = (slot * 0.5).clamp(24.0, 120.0);
    for (i, s) in series.iter().enumerate() {
        let vals = &s.values;
        if vals.is_empty() {
            continue;
        }
        let color = s.color(i);
        let x = px + slot * (i as f64 + 0.5);
        let stats = five_number(vals);
        let q1 = frame.ys.map(stats[0]);
        let q3 = frame.ys.map(stats[1]);
        let med = frame.ys.map(stats[2]);
        let wlo = frame.ys.map(stats[3]);
        let whi = frame.ys.map(stats[4]);
        frame.scene.line(x - bw / 2.0, q1, x + bw / 2.0, q1, color, 1.4);
        frame.scene.line(x - bw / 2.0, q3, x + bw / 2.0, q3, color, 1.4);
        frame.scene.line(x - bw / 2.0, q1, x - bw / 2.0, q3, color, 1.4);
        frame.scene.line(x + bw / 2.0, q1, x + bw / 2.0, q3, color, 1.4);
        frame.scene.line(x, wlo, x, q1, color, 1.0);
        frame.scene.line(x, q3, x, whi, color, 1.0);
        frame.scene.line(x - bw / 4.0, wlo, x + bw / 4.0, wlo, color, 1.0);
        frame.scene.line(x - bw / 4.0, whi, x + bw / 4.0, whi, color, 1.0);
        frame.scene.rect(x - bw / 2.0, q3, bw, (q1 - q3).abs(), Some(color.with_alpha(0.18)), Some(color));
        frame.scene.line(x - bw / 2.0, med, x + bw / 2.0, med, color, 2.0);
        for q in [wlo, whi, q1, q3] {
            frame.scene.rect(x - 1.5, q - 1.5, 3.0, 3.0, Some(color), None);
        }
        let _ = (py, ph);
    }
}

fn draw_heatmap(frame: &mut Frame) {
    frame.draw_frame();
    let s = frame.spec.series.first().cloned().unwrap_or_else(Series::empty);
    if s.rows == 0 || s.cols == 0 {
        return;
    }
    let (px, py, pw, ph) = frame.plot;
    let lo = s.cells.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = s.cells.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let cw = pw / s.cols as f64;
    let ch = ph / s.rows as f64;
    for r in 0..s.rows {
        for c in 0..s.cols {
            let v = s.cells[r * s.cols + c];
            let t = if (hi - lo).abs() < 1e-12 { 0.5 } else { (v - lo) / (hi - lo) };
            let color = heat_color(t);
            frame.scene.rect(px + cw * c as f64, py + ch * r as f64, cw, ch, Some(color), None);
        }
    }
    for i in 0..=s.cols {
        let x = px + cw * i as f64;
        frame.scene.line(x, py, x, py + ph, PAPER.with_alpha(0.35), 0.4);
    }
    for i in 0..=s.rows {
        let y = py + ch * i as f64;
        frame.scene.line(px, y, px + pw, y, PAPER.with_alpha(0.35), 0.4);
    }
    frame.scene.rect(px, py, pw, ph, None, Some(GRID));
}

pub fn heat_color(t: f64) -> Color {
    let t = t.clamp(0.0, 1.0);
    let stops = [
        (0.0, Color::rgb(247, 251, 255)),
        (0.35, Color::rgb(158, 202, 240)),
        (0.7, Color::rgb(66, 134, 200)),
        (1.0, Color::rgb(8, 48, 96)),
    ];
    for i in 0..stops.len() - 1 {
        let (t0, c0) = stops[i];
        let (t1, c1) = stops[i + 1];
        if t >= t0 && t <= t1 {
            return c0.mix(&c1, (t - t0) / (t1 - t0));
        }
    }
    stops[stops.len() - 1].1
}

fn variance(v: &[f64]) -> f64 {
    if v.len() < 2 {
        return 0.0;
    }
    let m = v.iter().sum::<f64>() / v.len() as f64;
    v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (v.len() - 1) as f64
}

fn five_number(v: &[f64]) -> [f64; 5] {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    [
        crate::builtins::stats_quantile(&s, 0.25),
        crate::builtins::stats_quantile(&s, 0.75),
        crate::builtins::stats_quantile(&s, 0.5),
        s[0],
        s[s.len() - 1],
    ]
}

fn sturges(n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    let k = (2.0 * n as f64).ln() / std::f64::consts::LN_2 + 1.0;
    k.round().clamp(4.0, 60.0) as usize
}

fn min_max(v: &[f64]) -> (f64, f64) {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for x in v {
        if x.is_finite() {
            lo = lo.min(*x);
            hi = hi.max(*x);
        }
    }
    if !lo.is_finite() {
        return (0.0, 1.0);
    }
    if (hi - lo).abs() < 1e-12 {
        let pad = if hi == 0.0 { 1.0 } else { hi.abs() * 0.1 };
        return (lo - pad, hi + pad);
    }
    let pad = (hi - lo) * 0.06;
    (lo - pad, hi + pad)
}

pub fn ticks(lo: f64, hi: f64, log: bool, target: usize) -> Vec<f64> {
    if log {
        let mut out = Vec::new();
        let l0 = lo.max(1e-12).log10().floor() as i32;
        let l1 = hi.max(1e-12).log10().ceil() as i32;
        let mut e = l0;
        while e <= l1 {
            if e >= l0 {
                out.push(10f64.powi(e));
            }
            e += 1;
        }
        if out.len() > 12 {
            out.retain(|v| v.log10().fract() == 0.0 || v.log10().fract().abs() - 0.5 < 1e-9);
        }
        return out;
    }
    if !lo.is_finite() || !hi.is_finite() || hi <= lo {
        return vec![lo];
    }
    let span = nice_num(hi - lo, false);
    let step = nice_num(span / target.max(2) as f64, true);
    let start = (lo / step).ceil() * step;
    let mut out = Vec::new();
    let mut v = start;
    let mut guard = 0;
    while v <= hi + step * 1e-9 && guard < 200 {
        out.push(round_to(v, step));
        v += step;
        guard += 1;
    }
    out
}

fn decimals_for(ts: Vec<f64>) -> i32 {
    let mut d = 0;
    for t in ts {
        let s = fmt_num(t);
        if let Some(pos) = s.find('.') {
            d = d.max((s.len() - pos - 1) as i32);
        }
    }
    d.min(6)
}

fn round_to(v: f64, step: f64) -> f64 {
    if step <= 0.0 {
        return v;
    }
    let dec = (-step.log10().floor()).max(0.0) as i32 + 1;
    (v * 10f64.powi(dec)).round() / 10f64.powi(dec)
}

fn nice_num(range: f64, round: bool) -> f64 {
    if range <= 0.0 {
        return 1.0;
    }
    let exp = range.log10().floor();
    let f = range / 10f64.powf(exp);
    let nf = if round {
        if f < 1.5 {
            1.0
        } else if f < 3.0 {
            2.0
        } else if f < 7.0 {
            5.0
        } else {
            10.0
        }
    } else if f <= 1.0 {
        1.0
    } else if f <= 2.0 {
        2.0
    } else if f <= 5.0 {
        5.0
    } else {
        10.0
    };
    nf * 10f64.powf(exp)
}

fn fmt_pow10(v: f64) -> String {
    let e = v.log10().round() as i32;
    if e >= -2 && e <= 3 {
        fmt_num(v)
    } else {
        format!("1e{}", e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(kind: &str, args: &crate::doc::ArgSet) -> PlotSpec {
        let mut ip = Interp::new();
        let mut errs = Vec::new();
        PlotSpec::from_args(kind, args, &mut ip, &mut errs)
    }

    /// Every argument that fails to evaluate is reported, so that a plot with
    /// missing data becomes a visible error instead of an empty set of axes.
    #[test]
    fn failed_arguments_are_reported() {
        let set = crate::doc::ArgSet {
            named: vec![("y".into(), "undefined_series".into())],
            positional: vec!["bar".into()],
        };
        let mut ip = Interp::new();
        let mut errs = Vec::new();
        let _ = PlotSpec::from_args("bar", &set, &mut ip, &mut errs);
        assert_eq!(errs.len(), 1, "{:?}", errs);
        assert!(errs[0].contains("undefined_series"), "{:?}", errs);
        assert!(errs[0].contains("undefined variable"), "{:?}", errs);
    }

    #[test]
    fn valid_arguments_produce_no_errors() {
        let set = crate::doc::ArgSet {
            named: vec![
                ("y".into(), "[1, 2, 3]".into()),
                ("title".into(), r#""A title""#.into()),
                ("width".into(), "400".into()),
                ("labels".into(), r#"["a", "b", "c"]"#.into()),
            ],
            positional: vec!["bar".into()],
        };
        let mut ip = Interp::new();
        let mut errs = Vec::new();
        let spec = PlotSpec::from_args("bar", &set, &mut ip, &mut errs);
        assert!(errs.is_empty(), "{:?}", errs);
        assert_eq!(spec.title, "A title");
        assert!(!spec.series.is_empty());
    }

    fn args(named: &[(&str, &str)], positional: &[&str]) -> crate::doc::ArgSet {
        crate::doc::parse_args(
            &named
                .iter()
                .map(|(k, v)| format!("{} = {}", k, v))
                .chain(positional.iter().map(|s| s.to_string()))
                .collect::<Vec<String>>(),
        )
    }

    #[test]
    fn renders_every_chart_kind() {
        let cases: Vec<(&str, Vec<(&str, &str)>, Vec<&str>)> = vec![
            ("line", vec![("x", "[0,1,2,3]"), ("y", "[1,4,2,5]")], vec![]),
            ("scatter", vec![("x", "[1,2,3,4,5]"), ("y", "[2,1,4,3,5]")], vec![]),
            ("area", vec![("x", "[0,1,2]"), ("y", "[1,2,1]")], vec![]),
            ("bar", vec![("labels", "[\"a\",\"b\",\"c\"]"), ("y", "[3,5,2]")], vec![]),
            ("hbar", vec![("labels", "[\"a\",\"b\"]"), ("y", "[3,5]")], vec![]),
            ("hist", vec![("y", "[1,1,2,2,2,3,4,4,5]")], vec![]),
            ("box", vec![("y", "[1,2,3,4,5,6,7,8]")], vec![]),
            ("heatmap", vec![("y", "[[1,2],[3,4]]")], vec![]),
            ("errorbar", vec![("x", "[1,2,3]"), ("y", "[1,2,3]")], vec![]),
            ("density", vec![("y", "[1,1,2,2,2,3,4,4,5]")], vec![]),
            ("function", vec![("f", "\"sin(x)\""), ("domain", "[0, 6.28]")], vec![]),
        ];
        for (kind, named, positional) in cases {
            let s = spec(kind, &args(&named, &positional));
            let scene = render(&s).unwrap_or_else(|e| panic!("{} failed: {}", kind, e));
            assert!(scene.width > 0.0 && scene.height > 0.0, "{}", kind);
            assert!(scene.items.len() > 3, "{} produced only {} items", kind, scene.items.len());
        }
    }

    /// On an interactive plot `x = t` names the widget, and a widget only
    /// exists in the browser, so there is no `t` in scope at build time.
    #[test]
    fn interactive_x_may_be_only_a_widget_name() {
        let set = crate::doc::ArgSet {
            named: vec![
                ("f".into(), "\"exp(-t * x)\"".into()),
                ("x".into(), "t".into()),
                ("domain".into(), "[0, 5]".into()),
                ("interactive".into(), "t".into()),
            ],
            positional: vec!["line".into()],
        };
        let mut ip = Interp::new();
        let mut errs = Vec::new();
        let s = PlotSpec::from_args("line", &set, &mut ip, &mut errs);
        assert_eq!(errs, Vec::<String>::new(), "{errs:?}");
        assert_eq!(s.var, "t");
        assert_eq!(s.interactive.as_deref(), Some("t"));
    }

    #[test]
    fn empty_plot_still_renders() {
        let s = spec("line", &args(&[], &[]));
        let scene = render(&s).unwrap();
        assert!(scene.items.len() > 3);
    }

    /// The bars of a horizontal chart have to fit the frame they are drawn in.
    #[test]
    fn horizontal_bars_stay_inside_the_plot() {
        let s = spec("hbar", &args(&[("labels", "[\"a\",\"b\"]"), ("y", "[2,1]")], &[]));
        let scene = render(&s).unwrap();
        let (w, h) = (s.width, s.height);
        let bars: Vec<(f64, f64)> = scene
            .items
            .iter()
            .filter_map(|it| match it {
                Item::Rect { x, y, w, h, .. } if *w > 40.0 && *h > 4.0 => Some((*x, *w)),
                _ => None,
            })
            .collect();
        assert_eq!(bars.len(), 2, "expected one bar per category: {bars:?}");
        for (x, bw) in &bars {
            assert!(*x >= -0.5, "a bar starts left of the frame at {x}");
            assert!(x + bw <= w + 0.5, "a bar of width {bw} at {x} runs past the {w}pt frame");
        }
        // the bar for 2 must be twice the bar for 1
        let (long, short) = if bars[0].1 > bars[1].1 { (bars[0].1, bars[1].1) } else { (bars[1].1, bars[0].1) };
        assert!((long / short - 2.0).abs() < 0.01, "{long} is not twice {short}");
        let _ = h;
    }

    /// Categories belong beside the bars, and values along the bottom axis.
    #[test]
    fn horizontal_bars_put_categories_beside_the_bars() {
        let s = spec("hbar", &args(&[("labels", "[\"alpha\",\"beta\"]"), ("y", "[2,1]")], &[]));
        let scene = render(&s).unwrap();
        let mut beside = Vec::new();
        for it in &scene.items {
            if let Item::Text { text, halign, valign, .. } = it {
                // a category label sits on the y axis, so it is right aligned and
                // centred against a bar rather than centred under the chart
                if (text == "alpha" || text == "beta") && matches!(valign, VAlign::Middle) {
                    beside.push((*halign, text.clone()));
                }
            }
        }
        assert_eq!(beside.len(), 2, "both category labels are drawn: {beside:?}");
        for (halign, _) in &beside {
            assert!(matches!(halign, HAlign::Right), "a category label is not beside its bar: {beside:?}");
        }
    }

    /// A density curve that leaves the frame is clipped away and looks broken.
    #[test]
    fn density_curve_stays_inside_the_plot() {
        let s = spec("density", &args(&[("y", "[1,1,1,2,2,3]")], &[]));
        let scene = render(&s).unwrap();
        let mut lo = f64::MAX;
        let mut hi = f64::MIN;
        for it in &scene.items {
            if let Item::Poly { points, .. } = it {
                for p in points {
                    lo = lo.min(p.x);
                    hi = hi.max(p.x);
                }
            }
        }
        assert!(lo < hi, "the density curve was drawn");
        assert!(lo >= -0.5, "the density curve starts at {lo}, left of the frame");
        assert!(hi <= s.width + 0.5, "the density curve ends at {hi}, past the {}pt frame", s.width);
    }

    /// A histogram counts, and a density estimates a height: neither y axis is
    /// the range of the data it was given.
    #[test]
    fn histogram_and_density_y_axes_are_not_the_data_range() {
        for kind in ["hist", "density"] {
            let s = spec(kind, &args(&[("y", "[1,1,1,2,2,3]")], &[]));
            let scene = render(&s).unwrap();
            // the tick labels along the bottom axis are the x range, the ones
            // down the side are the y range; take the widest y tick
            let mut top = 0.0f64;
            for it in &scene.items {
                if let Item::Text { text, halign, .. } = it {
                    if !matches!(halign, HAlign::Right) {
                        continue;
                    }
                    if let Ok(v) = text.parse::<f64>() {
                        top = top.max(v);
                    }
                }
            }
            assert!(top > 0.0, "{kind} drew no value axis");
            assert!(top <= 3.5, "the {kind} y axis reaches {top}, which is the data range, not a count or a height");
        }
    }
}
