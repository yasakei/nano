use std::cell::RefCell;
use std::rc::Rc;

use crate::interp::Scope;
use crate::interp::index;
use crate::value::*;

thread_local! {
    static SINK: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static RNG: RefCell<u64> = const { RefCell::new(0x2545F4914F6CDD1D) };
}

pub fn emit(s: String) {
    SINK.with(|k| k.borrow_mut().push(s));
}

pub fn take_output() -> Vec<String> {
    SINK.with(|k| std::mem::take(&mut *k.borrow_mut()))
}

pub fn arg_index(fname: &str, arg: &str) -> Option<usize> {
    let table: &[(&str, &[&str])] = &[
        ("print", &["sep", "end"]),
        ("round", &["digits"]),
        ("sort", &["by", "desc"]),
        ("top", &["n", "key"]),
        ("linspace", &["n", "step"]),
        ("read", &["path"]),
        ("replace", &["all"]),
        ("format", &["spec"]),
        ("map", &[]),
    ];
    for (name, args) in table {
        if *name == fname {
            return args.iter().position(|a| *a == arg);
        }
    }
    None
}

pub fn globals() -> Scope {
    let mut g: Scope = Scope::new();
    fn reg(
        g: &mut Scope,
        name: &'static str,
        arity: usize,
        f: impl Fn(&[Value]) -> Result<Value, String> + 'static,
    ) {
        g.insert(name.to_string(), native(name, arity, f));
    }
    macro_rules! reg {
        ($name:expr, $arity:expr, $f:expr) => {
            reg(&mut g, $name, $arity, $f)
        };
    }

    g.insert("true".into(), Value::Bool(true));
    g.insert("false".into(), Value::Bool(false));
    g.insert("null".into(), Value::Null);

    reg!("print", 1, |a| {
        let line: Vec<String> = a.iter().map(|v| v.to_display()).collect();
        emit(line.join(" "));
        Ok(Value::Null)
    });
    reg!("type", 1, |a| Ok(Value::str(arg(a, 0).type_name())));
    reg!("str", 1, |a| Ok(Value::str(arg(a, 0).to_display())));
    reg!("num", 1, |a| Ok(Value::Num(arg(a, 0).num().unwrap_or(f64::NAN))));
    reg!("len", 1, |a| Ok(Value::Num(length(&arg(a, 0))? as f64)));
    reg!("is_null", 1, |a| Ok(Value::Bool(matches!(arg(a, 0), Value::Null))));
    reg!("is_num", 1, |a| Ok(Value::Bool(matches!(arg(a, 0), Value::Num(_)))));
    reg!("is_str", 1, |a| Ok(Value::Bool(matches!(arg(a, 0), Value::Str(_)))));
    reg!("is_list", 1, |a| Ok(Value::Bool(matches!(arg(a, 0), Value::List(_)))));
    reg!("is_map", 1, |a| Ok(Value::Bool(matches!(arg(a, 0), Value::Map(_)))));
    reg!("is_fn", 1, |a| {
        Ok(Value::Bool(matches!(
            arg(a, 0),
            Value::Fn(_) | Value::Native(_)
        )))
    });

    reg!("abs", 1, |a| Ok(Value::Num(arg(a, 0).num().ok_or("abs: not a number")?.abs())));
    reg!("sign", 1, |a| {
        Ok(Value::Num(match arg(a, 0).num().ok_or("sign: not a number")? {
            n if n > 0.0 => 1.0,
            n if n < 0.0 => -1.0,
            _ => 0.0,
        }))
    });
    reg!("sqrt", 1, |a| Ok(Value::Num(arg(a, 0).num().ok_or("sqrt: not a number")?.sqrt())));
    reg!("floor", 1, |a| Ok(Value::Num(arg(a, 0).num().ok_or("floor: not a number")?.floor())));
    reg!("ceil", 1, |a| Ok(Value::Num(arg(a, 0).num().ok_or("ceil: not a number")?.ceil())));
    reg!("round", 2, |a| {
        let n = arg(a, 0).num().ok_or("round: not a number")?;
        let d = a.get(1).and_then(|v| v.num()).unwrap_or(0.0) as i32;
        let m = 10f64.powi(d);
        Ok(Value::Num((n * m).round() / m))
    });
    reg!("pow", 2, |a| Ok(Value::Num(
        arg(a, 0).num().ok_or("pow: not a number")?
            .powf(arg(a, 1).num().ok_or("pow: not a number")?),
    )));
    reg!("min", 2, |a| {
        if a.len() == 1 {
            let items = arg(a, 0).as_list().unwrap_or_default();
            return Ok(numeric_extreme(&items, true).ok_or("min: empty")?);
        }
        Ok(Value::Num(
            arg(a, 0).num().ok_or("min: not a number")?.min(arg(a, 1).num().ok_or("min: not a number")?),
        ))
    });
    reg!("max", 2, |a| {
        if a.len() == 1 {
            let items = arg(a, 0).as_list().unwrap_or_default();
            return Ok(numeric_extreme(&items, false).ok_or("max: empty")?);
        }
        Ok(Value::Num(
            arg(a, 0).num().ok_or("max: not a number")?.max(arg(a, 1).num().ok_or("max: not a number")?),
        ))
    });
    reg!("clamp", 3, |a| {
        let (x, lo, hi) = (num(a, 0)?, num(a, 1)?, num(a, 2)?);
        Ok(Value::Num(x.max(lo).min(hi)))
    });
    reg!("sum", 1, |a| Ok(Value::Num(
        arg(a, 0).as_list().unwrap_or_default().iter().filter_map(|v| v.num()).sum(),
    )));
    reg!("prod", 1, |a| Ok(Value::Num(
        arg(a, 0).as_list().unwrap_or_default().iter().filter_map(|v| v.num()).product(),
    )));
    reg!("mean", 1, |a| {
        let ns = numbers(&arg(a, 0))?;
        if ns.is_empty() {
            return Ok(Value::Null);
        }
        Ok(Value::Num(ns.iter().sum::<f64>() / ns.len() as f64))
    });
    reg!("median", 1, |a| Ok(sorted_numbers(&arg(a, 0)).map(stats_median).unwrap_or(Value::Null)));
    reg!("mode", 1, |a| {
        let mut counts: Vec<(f64, usize)> = Vec::new();
        for n in numbers(&arg(a, 0))? {
            match counts.iter_mut().find(|(v, _)| *v == n) {
                Some(c) => c.1 += 1,
                None => counts.push((n, 1)),
            }
        }
        match counts.into_iter().max_by_key(|(_, c)| *c) {
            Some((v, _)) => Ok(Value::Num(v)),
            None => Ok(Value::Null),
        }
    });
    reg!("std", 2, |a| Ok(stat_var(a).map(|(var, _)| Value::Num(var.sqrt())).unwrap_or(Value::Null)));
    reg!("var", 1, |a| Ok(stat_var(a).map(|(v, _)| Value::Num(v)).unwrap_or(Value::Null)));
    reg!("sem", 1, |a| Ok(stat_var(a)
        .map(|(v, n)| Value::Num((v / n as f64).sqrt()))
        .unwrap_or(Value::Null)));
    reg!("quantile", 2, |a| {
        let mut ns = numbers(&arg(a, 0))?;
        ns.sort_by(|x, y| x.partial_cmp(y).unwrap());
        let q = num(a, 1)?;
        Ok(Value::Num(stats_quantile(&ns, q)))
    });
    reg!("cumsum", 1, |a| {
        let mut t = 0.0;
        Ok(Value::list(
            numbers(&arg(a, 0))?
                .into_iter()
                .map(|n| {
                    t += n;
                    Value::Num(t)
                })
                .collect(),
        ))
    });
    reg!("diff", 1, |a| {
        let ns = numbers(&arg(a, 0))?;
        Ok(Value::list(
            ns.windows(2).map(|w| Value::Num(w[1] - w[0])).collect(),
        ))
    });
    reg!("corr", 2, |a| {
        let (x, y) = (numbers(&arg(a, 0))?, numbers(&arg(a, 1))?);
        if x.len() != y.len() || x.len() < 2 {
            return Ok(Value::Null);
        }
        let (mx, my) = (mean_of(&x), mean_of(&y));
        let mut num_ = 0.0;
        let mut dx = 0.0;
        let mut dy = 0.0;
        for i in 0..x.len() {
            num_ += (x[i] - mx) * (y[i] - my);
            dx += (x[i] - mx).powi(2);
            dy += (y[i] - my).powi(2);
        }
        Ok(Value::Num(num_ / (dx * dy).sqrt()))
    });
    reg!("linspace", 2, |a| {
        let lo = num(a, 0)?;
        let hi = num(a, 1)?;
        let n = a.get(2).and_then(|v| v.num()).unwrap_or(50.0).max(2.0) as usize;
        Ok(Value::list(
            (0..n)
                .map(|i| Value::Num(lo + (hi - lo) * i as f64 / (n - 1) as f64))
                .collect(),
        ))
    });
    reg!("arange", 2, |a| {
        let lo = num(a, 0)?;
        let hi = num(a, 1)?;
        let step = a.get(2).and_then(|v| v.num()).unwrap_or(1.0);
        Ok(Value::Range(lo, hi, step))
    });
    reg!("range", 3, |a| {
        let lo = a.first().and_then(|v| v.num()).unwrap_or(0.0);
        let hi = a.get(1).and_then(|v| v.num()).unwrap_or(0.0);
        let step = a.get(2).and_then(|v| v.num()).unwrap_or(1.0);
        Ok(Value::Range(lo, hi, step))
    });

    reg!("exp", 1, |a| Ok(Value::Num(num(a, 0)?.exp())));
    reg!("log", 2, |a| {
        let x = num(a, 0)?;
        Ok(Value::Num(match a.get(1) {
            Some(b) if b.num().is_some() => x.log(b.num().unwrap()),
            _ => x.ln(),
        }))
    });
    reg!("log2", 1, |a| Ok(Value::Num(num(a, 0)?.log2())));
    reg!("log10", 1, |a| Ok(Value::Num(num(a, 0)?.log10())));
    reg!("sin", 1, |a| Ok(Value::Num(num(a, 0)?.sin())));
    reg!("cos", 1, |a| Ok(Value::Num(num(a, 0)?.cos())));
    reg!("tan", 1, |a| Ok(Value::Num(num(a, 0)?.tan())));
    reg!("asin", 1, |a| Ok(Value::Num(num(a, 0)?.asin())));
    reg!("acos", 1, |a| Ok(Value::Num(num(a, 0)?.acos())));
    reg!("atan", 1, |a| Ok(Value::Num(num(a, 0)?.atan())));
    reg!("atan2", 2, |a| Ok(Value::Num(num(a, 0)?.atan2(num(a, 1)?))));
    reg!("sinh", 1, |a| Ok(Value::Num(num(a, 0)?.sinh())));
    reg!("cosh", 1, |a| Ok(Value::Num(num(a, 0)?.cosh())));
    reg!("tanh", 1, |a| Ok(Value::Num(num(a, 0)?.tanh())));
    reg!("hypot", 2, |a| Ok(Value::Num(num(a, 0)?.hypot(num(a, 1)?))));
    reg!("gcd", 2, |a| {
        let mut x = num(a, 0)?.abs() as i64;
        let mut y = num(a, 1)?.abs() as i64;
        while y != 0 {
            let t = y;
            y = x % y;
            x = t;
        }
        Ok(Value::Num(x as f64))
    });

    reg!("seed", 1, |a| {
        let s = num(a, 0)? as u64;
        RNG.with(|r| *r.borrow_mut() = s.wrapping_mul(6364136223846793005).wrapping_add(1));
        Ok(Value::Null)
    });
    reg!("random", 0, |_| {
        Ok(Value::Num(next_f64()))
    });
    reg!("randint", 2, |a| {
        let lo = num(a, 0)?;
        let hi = num(a, 1)?;
        Ok(Value::Num(lo + (hi - lo + 1.0) * next_f64().floor().min(hi - lo)))
    });
    reg!("randn", 1, |a| {
        let n = a.first().and_then(|v| v.num()).unwrap_or(1.0) as usize;
        Ok(Value::list((0..n).map(|_| Value::Num(gauss())).collect()))
    });
    reg!("shuffle", 1, |a| {
        let mut items = arg(a, 0).as_list().unwrap_or_default();
        for i in (1..items.len()).rev() {
            let j = (next_f64() * (i + 1) as f64) as usize;
            items.swap(i, j.min(i));
        }
        Ok(Value::list(items))
    });
    reg!("choice", 1, |a| {
        let items = arg(a, 0).as_list().unwrap_or_default();
        if items.is_empty() {
            return Ok(Value::Null);
        }
        Ok(items[(next_f64() * items.len() as f64) as usize % items.len()].clone())
    });

    reg!("map", 2, |a| {
        let (list, f) = list_and_fn(a);
        let items = list.as_list().unwrap_or_default();
        let mut out = Vec::with_capacity(items.len());
        for it in items {
            out.push(f.call(vec![it])?);
        }
        Ok(Value::list(out))
    });
    reg!("filter", 2, |a| {
        let (list, f) = list_and_fn(a);
        let items = list.as_list().unwrap_or_default();
        let mut out = Vec::new();
        for it in items {
            if f.call(vec![it.clone()])?.truthy() {
                out.push(it);
            }
        }
        Ok(Value::list(out))
    });
    reg!("reduce", 2, |a| {
        let (list, f) = list_and_fn(a);
        let items = list.as_list().unwrap_or_default();
        let mut iter = items.into_iter();
        let mut acc = match a.get(2) {
            Some(init) => init.clone(),
            None => match iter.next() {
                Some(v) => v,
                None => return Ok(Value::Null),
            },
        };
        for it in iter {
            acc = f.call(vec![acc, it])?;
        }
        Ok(acc)
    });
    reg!("each", 2, |a| {
        let (list, f) = list_and_fn(a);
        for it in list.as_list().unwrap_or_default() {
            f.call(vec![it])?;
        }
        Ok(Value::Null)
    });
    reg!("sort", 2, |a| {
        let (list, key) = list_and_fn(a);
        let mut items = list.as_list().unwrap_or_default();
        let desc = a.get(2).map(|v| v.truthy()).unwrap_or(false);
        crate::builtins::sort_values(&mut items, &key, desc);
        Ok(Value::list(items))
    });
    reg!("sorted", 2, |a| {
        let (list, key) = list_and_fn(a);
        let mut items = list.as_list().unwrap_or_default();
        let desc = a.get(2).map(|v| v.truthy()).unwrap_or(false);
        crate::builtins::sort_values(&mut items, &key, desc);
        Ok(Value::list(items))
    });
    reg!("reverse", 1, |a| {
        let mut items = arg(a, 0).as_list().unwrap_or_default();
        items.reverse();
        Ok(Value::list(items))
    });
    reg!("flatten", 1, |a| {
        let mut out = Vec::new();
        flatten_into(&arg(a, 0), &mut out);
        Ok(Value::list(out))
    });
    reg!("unique", 1, |a| {
        let mut out: Vec<Value> = Vec::new();
        for v in arg(a, 0).as_list().unwrap_or_default() {
            if !out.iter().any(|o| o.eq(&v)) {
                out.push(v);
            }
        }
        Ok(Value::list(out))
    });
    reg!("zip", 2, |a| {
        let (x, y) = (arg(a, 0).as_list().unwrap_or_default(), arg(a, 1).as_list().unwrap_or_default());
        Ok(Value::list(
            x.iter()
                .zip(y.iter())
                .map(|(p, q)| Value::list(vec![p.clone(), q.clone()]))
                .collect(),
        ))
    });
    reg!("enumerate", 1, |a| {
        Ok(Value::list(
            arg(a, 0)
                .as_list()
                .unwrap_or_default()
                .into_iter()
                .enumerate()
                .map(|(i, v)| Value::list(vec![Value::Num(i as f64), v]))
                .collect(),
        ))
    });
    reg!("concat", 2, |a| {
        let mut out = arg(a, 0).as_list().unwrap_or_default();
        out.extend(arg(a, 1).as_list().unwrap_or_default());
        Ok(Value::list(out))
    });
    reg!("slice", 3, |a| {
        let items = arg(a, 0).as_list().unwrap_or_default();
        let n = items.len() as i64;
        let from = norm_index(a.get(1).and_then(|v| v.num()).unwrap_or(0.0), n);
        let to = norm_index(a.get(2).and_then(|v| v.num()).unwrap_or(n as f64), n);
        Ok(Value::list(if from <= to { items[from as usize..to as usize].to_vec() } else { Vec::new() }))
    });
    reg!("first", 1, |a| Ok(arg(a, 0).as_list().unwrap_or_default().first().cloned().unwrap_or(Value::Null)));
    reg!("last", 1, |a| Ok(arg(a, 0).as_list().unwrap_or_default().last().cloned().unwrap_or(Value::Null)));
    reg!("take", 2, |a| {
        let items = arg(a, 0).as_list().unwrap_or_default();
        let n = a.get(1).and_then(|v| v.num()).unwrap_or(0.0).max(0.0) as usize;
        Ok(Value::list(items.into_iter().take(n).collect()))
    });
    reg!("drop", 2, |a| {
        let items = arg(a, 0).as_list().unwrap_or_default();
        let n = a.get(1).and_then(|v| v.num()).unwrap_or(0.0).max(0.0) as usize;
        Ok(Value::list(items.into_iter().skip(n).collect()))
    });
    reg!("contains", 2, |a| {
        Ok(Value::Bool(
            arg(a, 0).as_list().unwrap_or_default().iter().any(|v| v.eq(&arg(a, 1))),
        ))
    });
    reg!("count", 2, |a| {
        Ok(Value::Num(
            arg(a, 0).as_list().unwrap_or_default().iter().filter(|v| v.eq(&arg(a, 1))).count() as f64,
        ))
    });
    reg!("any", 2, |a| {
        Ok(Value::Bool(
            arg(a, 0).as_list().unwrap_or_default().iter().any(|v| f2(&arg(a, 1), v)),
        ))
    });
    reg!("all", 2, |a| {
        Ok(Value::Bool(
            arg(a, 0).as_list().unwrap_or_default().iter().all(|v| f2(&arg(a, 1), v)),
        ))
    });
    reg!("group_by", 2, |a| {
        let (list, f) = list_and_fn(a);
        let mut out: Vec<(String, Vec<Value>)> = Vec::new();
        for v in list.as_list().unwrap_or_default() {
            let k = f.call(vec![v.clone()])?.to_display();
            match out.iter_mut().find(|(kk, _)| *kk == k) {
                Some(g) => g.1.push(v),
                None => out.push((k, vec![v])),
            }
        }
        Ok(Value::map(
            out.into_iter().map(|(k, v)| (k, Value::list(v))).collect(),
        ))
    });
    reg!("join", 2, |a| {
        let sep = a.get(1).and_then(|v| v.as_str()).unwrap_or_default();
        let parts: Vec<String> = arg(a, 0).items().iter().map(|v| v.to_display()).collect();
        Ok(Value::str(parts.join(&sep)))
    });
    reg!("split", 2, |a| {
        let s = arg(a, 0).as_str().unwrap_or_default();
        let sep = a.get(1).and_then(|v| v.as_str()).unwrap_or_else(|| " ".to_string());
        Ok(Value::list(split_str(&s, &sep).into_iter().map(Value::str).collect()))
    });
    reg!("lines", 1, |a| {
        let s = arg(a, 0).to_display();
        Ok(Value::list(
            s.lines().map(|l| Value::str(l.to_string())).collect(),
        ))
    });
    reg!("replace", 3, |a| {
        let s = arg(a, 0).to_display();
        let from = arg(a, 1).to_display();
        let to = arg(a, 2).to_display();
        Ok(Value::str(s.replace(&from, &to)))
    });
    reg!("upper", 1, |a| Ok(Value::str(arg(a, 0).to_display().to_uppercase())));
    reg!("lower", 1, |a| Ok(Value::str(arg(a, 0).to_display().to_lowercase())));
    reg!("trim", 1, |a| Ok(Value::str(arg(a, 0).to_display().trim().to_string())));
    reg!("starts_with", 2, |a| {
        Ok(Value::Bool(arg(a, 0).to_display().starts_with(&arg(a, 1).to_display())))
    });
    reg!("ends_with", 2, |a| {
        Ok(Value::Bool(arg(a, 0).to_display().ends_with(&arg(a, 1).to_display())))
    });
    reg!("find", 2, |a| {
        let s = arg(a, 0).to_display();
        let needle = arg(a, 1).to_display();
        Ok(Value::Num(s.find(&needle).map(|b| s[..b].chars().count() as f64).unwrap_or(-1.0)))
    });
    reg!("pad", 3, |a| {
        let s = arg(a, 0).to_display();
        let n = a.get(1).and_then(|v| v.num()).unwrap_or(0.0) as usize;
        let c = a.get(2).and_then(|v| v.as_str()).unwrap_or_else(|| " ".to_string());
        let c = c.chars().next().map(|x| x.to_string()).unwrap_or_else(|| " ".to_string());
        let cur = s.chars().count();
        if cur >= n {
            Ok(Value::str(s))
        } else {
            let total = n - cur;
            let left = total / 2;
            let right = total - left;
            Ok(Value::str(c.repeat(left) + &s + &c.repeat(right)))
        }
    });
    reg!("format", 2, |a| {
        let spec = a.get(1).and_then(|v| v.as_str()).unwrap_or_default();
        Ok(Value::str(arg(a, 0).format(&spec)))
    });
    reg!("title", 1, |a| {
        let s = arg(a, 0).to_display();
        let mut out = String::new();
        let mut cap = true;
        for c in s.chars() {
            if c.is_alphanumeric() {
                if cap {
                    out.extend(c.to_uppercase());
                    cap = false;
                } else {
                    out.push(c);
                }
            } else {
                out.push(c);
                cap = true;
            }
        }
        Ok(Value::str(out))
    });
    reg!("slug", 1, |a| {
        let s = arg(a, 0).to_display().to_lowercase();
        let mut out = String::new();
        for c in s.chars() {
            if c.is_alphanumeric() {
                out.push(c);
            } else if !out.ends_with('-') {
                out.push('-');
            }
        }
        Ok(Value::str(out.trim_matches('-').to_string()))
    });
    reg!("keys", 1, |a| match arg(a, 0) {
        Value::Map(m) => Ok(Value::list(
            m.borrow().iter().map(|(k, _)| Value::str(k.clone())).collect(),
        )),
        other => Ok(Value::list(
            other.as_list().unwrap_or_default().iter().map(|v| Value::str(v.to_display())).collect(),
        )),
    });
    reg!("values", 1, |a| match arg(a, 0) {
        Value::Map(m) => Ok(Value::list(m.borrow().iter().map(|(_, v)| v.clone()).collect())),
        other => Ok(Value::list(other.as_list().unwrap_or_default())),
    });
    reg!("has", 2, |a| match (arg(a, 0), arg(a, 1)) {
        (Value::Map(m), k) => {
            let key = k.as_str().unwrap_or_else(|| k.to_plain());
            Ok(Value::Bool(m.borrow().iter().any(|(kk, _)| *kk == key)))
        }
        (other, k) => Ok(Value::Bool(other.as_list().unwrap_or_default().iter().any(|v| v.eq(&k)))),
    });
    reg!("get", 3, |a| match index(&arg(a, 0), &arg(a, 1)) {
        Ok(v) => Ok(v),
        Err(_) => Ok(a.get(2).cloned().unwrap_or(Value::Null)),
    });
    reg!("index_of", 2, |a| {
        let items = arg(a, 0).as_list().unwrap_or_default();
        Ok(Value::Num(
            items
                .iter()
                .position(|v| v.eq(&arg(a, 1)))
                .map(|i| i as f64)
                .unwrap_or(-1.0),
        ))
    });
    reg!("index_where", 2, |a| {
        let items = arg(a, 0).as_list().unwrap_or_default();
        let f = arg(a, 1).clone();
        Ok(Value::Num(
            items
                .iter()
                .position(|v| f2(&f, v))
                .map(|i| i as f64)
                .unwrap_or(-1.0),
        ))
    });
    reg!("find", 2, |a| {
        let items = arg(a, 0).as_list().unwrap_or_default();
        let f = arg(a, 1).clone();
        Ok(items
            .into_iter()
            .find(|v| f2(&f, v))
            .unwrap_or(Value::Null))
    });
    reg!("argmax", 1, |a| {
        let ns = numbers(&arg(a, 0))?;
        if ns.is_empty() {
            return Ok(Value::Null);
        }
        let mut best = 0;
        for i in 1..ns.len() {
            if ns[i] > ns[best] {
                best = i;
            }
        }
        Ok(Value::Num(best as f64))
    });
    reg!("argmin", 1, |a| {
        let ns = numbers(&arg(a, 0))?;
        if ns.is_empty() {
            return Ok(Value::Null);
        }
        let mut best = 0;
        for i in 1..ns.len() {
            if ns[i] < ns[best] {
                best = i;
            }
        }
        Ok(Value::Num(best as f64))
    });
    reg!("interp", 3, |a| {
        let x = num(a, 0)?;
        let xs = numbers(&arg(a, 1))?;
        let ys = numbers(&arg(a, 2))?;
        if xs.is_empty() || ys.is_empty() {
            return Ok(Value::Null);
        }
        if x <= xs[0] {
            return Ok(Value::Num(ys[0]));
        }
        for i in 1..xs.len().min(ys.len()) {
            if x <= xs[i] {
                let span = xs[i] - xs[i - 1];
                if span.abs() < 1e-12 {
                    return Ok(Value::Num(ys[i]));
                }
                let t = (x - xs[i - 1]) / span;
                return Ok(Value::Num(ys[i - 1] + t * (ys[i] - ys[i - 1])));
            }
        }
        Ok(Value::Num(ys[ys.len().min(xs.len()) - 1]))
    });
    reg!("dot", 2, |a| {
        let x = numbers(&arg(a, 0))?;
        let y = numbers(&arg(a, 1))?;
        Ok(Value::Num(
            x.iter().zip(y.iter()).map(|(p, q)| p * q).sum::<f64>(),
        ))
    });
    reg!("norm", 1, |a| {
        let ns = numbers(&arg(a, 0))?;
        Ok(Value::Num(ns.iter().map(|v| v * v).sum::<f64>().sqrt()))
    });
    reg!("zip_with", 3, |a| {
        let (list, f) = list_and_fn(a);
        let ys = arg(a, 2).as_list().unwrap_or_default();
        let mut out = Vec::new();
        for (i, x) in list.as_list().unwrap_or_default().into_iter().enumerate() {
            out.push(f.call(vec![x, arg(a, 1).clone(), ys.get(i).cloned().unwrap_or(Value::Null)])?);
        }
        Ok(Value::list(out))
    });
    reg!("cumprod", 1, |a| {
        let mut t = 1.0;
        Ok(Value::list(
            numbers(&arg(a, 0))?
                .into_iter()
                .map(|n| {
                    t *= n;
                    Value::Num(t)
                })
                .collect(),
        ))
    });
    reg!("pad_start", 3, |a| {
        let s = arg(a, 0).to_display();
        let n = a.get(1).and_then(|v| v.num()).unwrap_or(0.0) as usize;
        let c = a.get(2).and_then(|v| v.as_str()).unwrap_or_else(|| "0".into());
        let c = c.chars().next().map(|x| x.to_string()).unwrap_or_else(|| "0".into());
        let cur = s.chars().count();
        Ok(if cur >= n {
            Value::str(s)
        } else {
            Value::str(c.repeat(n - cur) + &s)
        })
    });
    reg!("set", 3, |a| {
        if let Value::Map(m) = arg(a, 0) {
            map_set(&m, &arg(a, 1).to_plain(), arg(a, 2).clone());
        }
        Ok(arg(a, 0).clone())
    });
    reg!("merge", 2, |a| {
        let mut out = match arg(a, 0) {
            Value::Map(m) => m.borrow().clone(),
            _ => Vec::new(),
        };
        if let Value::Map(m) = arg(a, 1) {
            for (k, v) in m.borrow().iter() {
                if let Some(slot) = out.iter_mut().find(|(kk, _)| kk == k) {
                    slot.1 = v.clone();
                } else {
                    out.push((k.clone(), v.clone()));
                }
            }
        }
        Ok(Value::map(out))
    });
    reg!("pairs", 1, |a| match arg(a, 0) {
        Value::Map(m) => Ok(Value::list(
            m.borrow()
                .iter()
                .map(|(k, v)| Value::list(vec![Value::str(k.clone()), v.clone()]))
                .collect(),
        )),
        _ => Ok(Value::Null),
    });

    g
}

fn is_callable(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::Fn(_)) | Some(Value::Native(_)))
}

fn list_and_fn(a: &[Value]) -> (Value, Value) {
    if is_callable(a.first()) && !is_callable(a.get(1)) {
        (arg(a, 1).clone(), arg(a, 0).clone())
    } else {
        (arg(a, 0).clone(), arg(a, 1).clone())
    }
}

fn f2(f: &Value, v: &Value) -> bool {
    match f {
        Value::Null => v.truthy(),
        other => match other.call(vec![v.clone()]) {
            Ok(r) => r.truthy(),
            Err(_) => false,
        },
    }
}

fn arg<'a>(a: &'a [Value], i: usize) -> &'a Value {
    a.get(i).unwrap_or(&Value::Null)
}

fn num(a: &[Value], i: usize) -> Result<f64, String> {
    a.get(i)
        .and_then(|v| v.num())
        .ok_or_else(|| format!("argument {} must be a number", i + 1))
}

fn length(v: &Value) -> Result<usize, String> {
    match v {
        Value::List(l) => Ok(l.borrow().len()),
        Value::Map(m) => Ok(m.borrow().len()),
        Value::Str(s) => Ok(s.chars().count()),
        Value::Range(a, b, s) => Ok(range_items(*a, *b, *s).len()),
        other => Ok(other.items().len()),
    }
}

fn numbers(v: &Value) -> Result<Vec<f64>, String> {
    let items = match v {
        Value::List(_) | Value::Range(..) => v.as_list().unwrap_or_default(),
        Value::Null => Vec::new(),
        other => vec![other.clone()],
    };
    let mut out = Vec::with_capacity(items.len());
    for it in items {
        match it.num() {
            Some(n) => out.push(n),
            None => return Err(format!("expected a list of numbers, found {}", it.type_name())),
        }
    }
    Ok(out)
}

fn sorted_numbers(v: &Value) -> Option<Vec<f64>> {
    let mut ns = numbers(v).ok()?;
    ns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(ns)
}

fn numeric_extreme(items: &[Value], min: bool) -> Option<Value> {
    let mut best: Option<(f64, Value)> = None;
    for it in items {
        if let Some(n) = it.num() {
            if best.as_ref().map_or(true, |(b, _)| if min { n < *b } else { n > *b }) {
                best = Some((n, it.clone()));
            }
        }
    }
    best.map(|(_, v)| v)
}

fn mean_of(ns: &[f64]) -> f64 {
    ns.iter().sum::<f64>() / ns.len() as f64
}

fn stat_var(a: &[Value]) -> Option<(f64, usize)> {
    let ns = numbers(arg(a, 0)).ok()?;
    if ns.len() < 2 {
        return None;
    }
    let m = mean_of(&ns);
    let v = ns.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (ns.len() - 1) as f64;
    Some((v, ns.len()))
}

pub fn stats_median(ns: Vec<f64>) -> Value {
    if ns.is_empty() {
        return Value::Null;
    }
    let n = ns.len();
    Value::Num(if n % 2 == 1 {
        ns[n / 2]
    } else {
        (ns[n / 2 - 1] + ns[n / 2]) / 2.0
    })
}

pub fn stats_quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let pos = q.clamp(0.0, 1.0) * (sorted.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    if lo == hi {
        return sorted[lo];
    }
    sorted[lo] + (sorted[hi] - sorted[lo]) * (pos - lo as f64)
}

fn flatten_into(v: &Value, out: &mut Vec<Value>) {
    match v {
        Value::List(l) => {
            for it in l.borrow().iter() {
                flatten_into(it, out);
            }
        }
        other => out.push(other.clone()),
    }
}

pub fn sort_values(items: &mut [Value], key: &Value, desc: bool) {
    match key {
        Value::Null => {
            items.sort_by(|a, b| a.cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        }
        Value::Str(path) => {
            let path: Vec<&str> = path.as_str().split('.').collect();
            items.sort_by(|a, b| {
                let av = lookup_path(a, &path);
                let bv = lookup_path(b, &path);
                av.cmp(&bv).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        other => {
            let key = other.clone();
            items.sort_by(|a, b| {
                let av = key.call(vec![a.clone()]).unwrap_or(Value::Null);
                let bv = key.call(vec![b.clone()]).unwrap_or(Value::Null);
                av.cmp(&bv).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }
    if desc {
        items.reverse();
    }
}

fn lookup_path(v: &Value, path: &[&str]) -> Value {
    let mut cur = v.clone();
    for p in path {
        match cur.get_member(p) {
            Ok(next) => cur = next,
            Err(_) => return Value::Null,
        }
    }
    cur
}

pub fn map_set(m: &Rc<RefCell<Dict>>, key: &str, v: Value) {
    let mut map = m.borrow_mut();
    match map.iter_mut().find(|(k, _)| k == key) {
        Some(slot) => slot.1 = v,
        None => map.push((key.to_string(), v)),
    }
}

pub fn split_str(s: &str, sep: &str) -> Vec<String> {
    if sep.is_empty() {
        return s.chars().map(|c| c.to_string()).collect();
    }
    s.split(sep).map(|x| x.to_string()).collect()
}

fn norm_index(i: f64, n: i64) -> i64 {
    let k = i as i64;
    if k < 0 {
        (k + n).max(0)
    } else {
        k.min(n)
    }
}

fn next_f64() -> f64 {
    RNG.with(|r| {
        let mut s = *r.borrow();
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        *r.borrow_mut() = s;
        ((s >> 11) as f64) / ((1u64 << 53) as f64)
    })
}

fn gauss() -> f64 {
    let mut u1 = next_f64();
    if u1 < 1e-12 {
        u1 = 1e-12;
    }
    let u2 = next_f64();
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}

pub fn member(base: &Value, name: &str) -> Result<Value, String> {
    match base {
        Value::Str(_) => Ok(match name {
            "upper" => native("upper", 1, |a| Ok(Value::str(arg(a, 0).to_display().to_uppercase()))),
            "lower" => native("lower", 1, |a| Ok(Value::str(arg(a, 0).to_display().to_lowercase()))),
            "trim" => native("trim", 1, |a| Ok(Value::str(arg(a, 0).to_display().trim().to_string()))),
            "len" => native("len", 1, |a| Ok(Value::Num(arg(a, 0).to_display().chars().count() as f64))),
            "split" => native("split", 2, |a| {
                let sep = a.get(1).and_then(|v| v.as_str()).unwrap_or_default();
                Ok(Value::list(
                    split_str(&arg(a, 0).to_display(), &sep).into_iter().map(Value::str).collect(),
                ))
            }),
            "lines" => native("lines", 1, |a| Ok(Value::list(
                arg(a, 0).to_display().lines().map(|l| Value::str(l.to_string())).collect(),
            ))),
            "replace" => native("replace", 3, |a| Ok(Value::str(
                arg(a, 0).to_display().replace(&arg(a, 1).to_display(), &arg(a, 2).to_display()),
            ))),
            "starts_with" => native("starts_with", 2, |a| Ok(Value::Bool(
                arg(a, 0).to_display().starts_with(&arg(a, 1).to_display())
            ))),
            "ends_with" => native("ends_with", 2, |a| Ok(Value::Bool(
                arg(a, 0).to_display().ends_with(&arg(a, 1).to_display())
            ))),
            "contains" => native("contains", 2, |a| Ok(Value::Bool(
                arg(a, 0).to_display().contains(&arg(a, 1).to_display())
            ))),
            "slice" => native("slice", 3, |a| {
                let s: Vec<char> = arg(a, 0).to_display().chars().collect();
                let n = s.len() as i64;
                let from = norm_index(a.get(1).and_then(|v| v.num()).unwrap_or(0.0), n) as usize;
                let to = norm_index(a.get(2).and_then(|v| v.num()).unwrap_or(n as f64), n) as usize;
                Ok(Value::str(s[from.min(n as usize)..to.max(from).min(n as usize)].iter().collect::<String>()))
            }),
            "pad" => native("pad", 3, |a| {
                let s = arg(a, 0).to_display();
                let n = a.get(1).and_then(|v| v.num()).unwrap_or(0.0) as usize;
                let c = a.get(2).and_then(|v| v.as_str()).unwrap_or_else(|| " ".into());
                let c = c.chars().next().map(|x| x.to_string()).unwrap_or_else(|| " ".into());
                let cur = s.chars().count();
                if cur >= n {
                    return Ok(Value::str(s));
                }
                let t = n - cur;
                Ok(Value::str(c.repeat(t / 2) + &s + &c.repeat(t - t / 2)))
            }),
            "to_num" => native("to_num", 1, |a| Ok(Value::Num(arg(a, 0).num().unwrap_or(f64::NAN)))),
            other => return Err(format!("string has no method `{}`", other)),
        }),
        Value::List(_) | Value::Range(..) => Ok(match name {
            "len" => native("len", 1, |a| Ok(Value::Num(length(&arg(a, 0))? as f64))),
            "sum" => native("sum", 1, |a| Ok(Value::Num(
                arg(a, 0).as_list().unwrap_or_default().iter().filter_map(|v| v.num()).sum(),
            ))),
            "mean" => native("mean", 1, |a| {
                let ns = numbers(&arg(a, 0))?;
                if ns.is_empty() {
                    return Ok(Value::Null);
                }
                Ok(Value::Num(mean_of(&ns)))
            }),
            "min" => native("min", 1, |a| Ok(
                numeric_extreme(&arg(a, 0).as_list().unwrap_or_default(), true).ok_or("empty")?,
            )),
            "max" => native("max", 1, |a| Ok(
                numeric_extreme(&arg(a, 0).as_list().unwrap_or_default(), false).ok_or("empty")?,
            )),
            "sort" => native("sort", 1, |a| {
                let mut items = arg(a, 0).as_list().unwrap_or_default();
                sort_values(&mut items, &Value::Null, false);
                Ok(Value::list(items))
            }),
            "reverse" => native("reverse", 1, |a| {
                let mut items = arg(a, 0).as_list().unwrap_or_default();
                items.reverse();
                Ok(Value::list(items))
            }),
            "unique" => native("unique", 1, |a| {
                let mut out: Vec<Value> = Vec::new();
                for v in arg(a, 0).as_list().unwrap_or_default() {
                    if !out.iter().any(|o| o.eq(&v)) {
                        out.push(v);
                    }
                }
                Ok(Value::list(out))
            }),
            "map" => native("map", 2, |a| {
                let f = arg(a, 1).clone();
                let mut out = Vec::new();
                for it in arg(a, 0).as_list().unwrap_or_default() {
                    out.push(f.call(vec![it])?);
                }
                Ok(Value::list(out))
            }),
            "filter" => native("filter", 2, |a| {
                let f = arg(a, 1).clone();
                let mut out = Vec::new();
                for it in arg(a, 0).as_list().unwrap_or_default() {
                    if f.call(vec![it.clone()])?.truthy() {
                        out.push(it);
                    }
                }
                Ok(Value::list(out))
            }),
            "join" => native("join", 2, |a| {
                let sep = a.get(1).and_then(|v| v.as_str()).unwrap_or_default();
                Ok(Value::str(
                    arg(a, 0).items().iter().map(|v| v.to_display()).collect::<Vec<_>>().join(&sep),
                ))
            }),
            "each" | "for_each" => native("each", 2, |a| {
                let f = arg(a, 1).clone();
                for it in arg(a, 0).as_list().unwrap_or_default() {
                    f.call(vec![it])?;
                }
                Ok(Value::Null)
            }),
            "reduce" => native("reduce", 3, |a| {
                let (list, f) = list_and_fn(a);
                let items = list.as_list().unwrap_or_default();
                let mut iter = items.into_iter();
                let mut acc = match a.get(2) {
                    Some(init) => init.clone(),
                    None => match iter.next() {
                        Some(v) => v,
                        None => return Ok(Value::Null),
                    },
                };
                for it in iter {
                    acc = f.call(vec![acc, it])?;
                }
                Ok(acc)
            }),
            "contains" => native("contains", 2, |a| Ok(Value::Bool(
                arg(a, 0).as_list().unwrap_or_default().iter().any(|v| v.eq(&arg(a, 1))),
            ))),
            "first" => native("first", 1, |a| Ok(arg(a, 0).as_list().unwrap_or_default().first().cloned().unwrap_or(Value::Null))),
            "last" => native("last", 1, |a| Ok(arg(a, 0).as_list().unwrap_or_default().last().cloned().unwrap_or(Value::Null))),
            "flatten" => native("flatten", 1, |a| {
                let mut out = Vec::new();
                flatten_into(&arg(a, 0), &mut out);
                Ok(Value::list(out))
            }),
            "slice" => native("slice", 3, |a| {
                let items = arg(a, 0).as_list().unwrap_or_default();
                let n = items.len() as i64;
                let from = norm_index(a.get(1).and_then(|v| v.num()).unwrap_or(0.0), n) as usize;
                let to = norm_index(a.get(2).and_then(|v| v.num()).unwrap_or(n as f64), n) as usize;
                Ok(Value::list(items[from.min(items.len())..to.max(from).min(items.len())].to_vec()))
            }),
            "push" | "append" | "add_item" => native("push", 2, |a| {
                if let Value::List(l) = &arg(a, 0) {
                    l.borrow_mut().push(arg(a, 1).clone());
                }
                Ok(arg(a, 0).clone())
            }),
            "pop" => native("pop", 1, |a| {
                if let Value::List(l) = &arg(a, 0) {
                    return Ok(l.borrow_mut().pop().unwrap_or(Value::Null));
                }
                Ok(Value::Null)
            }),
            "insert" => native("insert", 3, |a| {
                if let Value::List(l) = &arg(a, 0) {
                    let n = l.borrow().len() as i64;
                    let k = a.get(1).and_then(|v| v.num()).unwrap_or(0.0) as i64;
                    let k = if k < 0 { (k + n).max(0) } else { k.min(n) };
                    l.borrow_mut().insert(k as usize, arg(a, 2).clone());
                }
                Ok(arg(a, 0).clone())
            }),
            "extend" => native("extend", 2, |a| {
                if let Value::List(l) = &arg(a, 0) {
                    let extra = arg(a, 1).as_list().unwrap_or_default();
                    l.borrow_mut().extend(extra);
                }
                Ok(arg(a, 0).clone())
            }),
            "remove" => native("remove", 2, |a| {
                if let Value::List(l) = &arg(a, 0) {
                    let n = l.borrow().len() as i64;
                    let k = a.get(1).and_then(|v| v.num()).unwrap_or(0.0) as i64;
                    let k = if k < 0 { k + n } else { k };
                    if k >= 0 && (k as usize) < n as usize {
                        l.borrow_mut().remove(k as usize);
                    }
                }
                Ok(arg(a, 0).clone())
            }),
            "sort_by" => native("sort_by", 2, |a| {
                let mut items = arg(a, 0).as_list().unwrap_or_default();
                sort_values(&mut items, &arg(a, 1), false);
                Ok(Value::list(items))
            }),
            other => return Err(format!("list has no method `{}`", other)),
        }),
        Value::Map(_) => Ok(match name {
            "keys" => native("keys", 1, |a| match arg(a, 0) {
                Value::Map(mm) => Ok(Value::list(mm.borrow().iter().map(|(k, _)| Value::str(k.clone())).collect())),
                _ => Ok(Value::Null),
            }),
            "values" => native("values", 1, |a| match arg(a, 0) {
                Value::Map(mm) => Ok(Value::list(mm.borrow().iter().map(|(_, v)| v.clone()).collect())),
                _ => Ok(Value::Null),
            }),
            "len" => native("len", 1, |a| Ok(Value::Num(length(&arg(a, 0))? as f64))),
            "has" => native("has", 2, |a| Ok(Value::Bool(
                matches!(arg(a, 0), Value::Map(ref mm) if mm.borrow().iter().any(|(k, _)| *k == arg(a, 1).to_plain()))
            ))),
            "get" => native("get", 3, |a| match index(&arg(a, 0), &arg(a, 1)) {
                Ok(v) => Ok(v),
                Err(_) => Ok(a.get(2).cloned().unwrap_or(Value::Null)),
            }),
            other => return Err(format!("map has no method `{}`", other)),
        }),
        Value::Num(_) => Ok(match name {
            "to_str" => native("to_str", 1, |a| Ok(Value::str(arg(a, 0).to_display()))),
            other => return Err(format!("number has no method `{}`", other)),
        }),
        other => Err(format!("{} has no method `{}`", other.type_name(), name)),
    }
}

pub fn apply_spec(v: &Value, spec: &str) -> Result<String, String> {
    if spec.is_empty() {
        return Ok(v.to_display());
    }
    let chars: Vec<char> = spec.chars().collect();
    let mut i = 0;
    let mut fill = ' ';
    let mut align = '\0';
    if chars.len() >= 2 && matches!(chars[1], '<' | '>' | '^' | '=') {
        fill = chars[0];
        align = chars[1];
        i = 2;
    } else if !chars.is_empty() && matches!(chars[0], '<' | '>' | '^' | '=') {
        align = chars[0];
        i = 1;
    }
    let mut sign = false;
    while i < chars.len() && matches!(chars[i], '+' | '-' | ' ') {
        sign = chars[i] == '+';
        i += 1;
    }
    let mut comma = false;
    if i < chars.len() && chars[i] == ',' {
        comma = true;
        i += 1;
    }
    let mut width = 0usize;
    while i < chars.len() && chars[i].is_ascii_digit() {
        width = width * 10 + chars[i].to_digit(10).unwrap() as usize;
        i += 1;
    }
    let mut precision: Option<usize> = None;
    if i < chars.len() && chars[i] == '.' {
        i += 1;
        let mut p = 0usize;
        while i < chars.len() && chars[i].is_ascii_digit() {
            p = p * 10 + chars[i].to_digit(10).unwrap() as usize;
            i += 1;
        }
        precision = Some(p);
    }
    let ty = chars.get(i).copied().unwrap_or('\0');
    let percent = ty == '%';

    let mut body = match ty {
        'd' => match v.num() {
            Some(n) => format!("{}", n as i64),
            None => v.to_display(),
        },
        'f' | 'F' => match v.num() {
            Some(n) => format!("{:.*}", precision.unwrap_or(6), n),
            None => v.to_display(),
        },
        'e' | 'E' => match v.num() {
            Some(n) => {
                let s = format!("{:.*e}", precision.unwrap_or(6), n);
                if ty == 'E' {
                    s.to_uppercase()
                } else {
                    s
                }
            }
            None => v.to_display(),
        },
        'g' | 'G' => match v.num() {
            Some(n) => {
                let p = precision.unwrap_or(6);
                let s = format!("{:.*}", p, n);
                let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
                if ty == 'G' { s.to_uppercase() } else { s }
            }
            None => v.to_display(),
        },
        'x' => match v.num() {
            Some(n) => format!("{:x}", n as i64),
            None => v.to_display(),
        },
        's' => v.to_display(),
        _ => v.to_display(),
    };
    if sign {
        if let Some(n) = v.num() {
            if n >= 0.0 && !body.starts_with('-') {
                body = format!("+{}", body);
            }
        }
    }
    if percent {
        if let Some(n) = v.num() {
            let p = precision.unwrap_or(0);
            body = format!("{:.*}%", p, n * 100.0);
        }
    }
    if comma {
        let (int_part, rest) = match body.find('.') {
            Some(k) => (body[..k].to_string(), body[k..].to_string()),
            None => (body.clone(), String::new()),
        };
        let mut digits: Vec<char> = int_part.chars().rev().collect();
        let mut out = String::new();
        let neg = !digits.is_empty() && (digits.last() == Some(&'-') || digits.last() == Some(&'+'));
        if neg {
            digits.pop();
        }
        while digits.len() % 3 != 0 {
            digits.push('0');
        }
        while !digits.is_empty() {
            let take = digits.len().min(3);
            for c in digits.split_off(digits.len() - take).iter().rev() {
                out.push(*c);
            }
            if !digits.is_empty() {
                out.push(',');
            }
        }
        body = format!("{}{}{}", if neg { "-" } else { "" }, out, rest);
    }
    if body.chars().count() < width {
        let pad = width - body.chars().count();
        let a = if align == '\0' {
            if v.num().map_or(false, |n| n < 0.0) { '<' } else { '>' }
        } else {
            align
        };
        let (l, r) = match a {
            '<' => (0, pad),
            '>' => (pad, 0),
            '^' => (pad / 2, pad - pad / 2),
            _ => (pad / 2, pad - pad / 2),
        };
        let f: String = std::iter::repeat(fill).take(l).collect();
        let g: String = std::iter::repeat(fill).take(r).collect();
        body = format!("{}{}{}", f, body, g);
    }
    Ok(body)
}

