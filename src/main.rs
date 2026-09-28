#![allow(dead_code)]

mod ast;
mod builtins;
mod doc;
mod image;
mod interp;
mod lexer;
mod math;
mod parser;
mod plot;
mod render;
mod svgpdf;
mod value;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        print_help();
        return ExitCode::SUCCESS;
    }
    if wants_version(&args) {
        println!("{}", version_line());
        return ExitCode::SUCCESS;
    }
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("nano: {}", e);
            ExitCode::FAILURE
        }
    }
}

/// `nano --version`, `nano -V`, and the flag anywhere in the line, so that
/// `nano build paper.nano --version` also works.
fn wants_version(args: &[String]) -> bool {
    args.iter().any(|a| a == "--version" || a == "-V")
}

/// The single line printed by `--version`, and asserted against in the CLI tests.
fn version_line() -> String {
    format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
}

fn print_help() {
    println!(
        r#"nano — a research document language

USAGE
  nano build <file.nano> [options]
  nano check <file.nano>
  nano run <file.nano>            evaluate and print output only

OPTIONS
  -o, --out <path>     output file, or prefix when --format is used
  -f, --format <fmt>   md | html | pdf | all        (default: all)
      --title <text>   override document title
      --theme <name>   default | serif | dark | minimal
      --open           print the path of the HTML output
  -q, --quiet          only report errors
  -V, --version        print the version and exit

EXAMPLES
  nano build paper.nano
  nano build paper.nano -f pdf -o paper.pdf
  nano build notes.nano -f html -o notes.html
  nano --version
"#
    );
}

fn run(args: &[String]) -> Result<(), String> {
    if wants_version(args) {
        println!("{}", version_line());
        return Ok(());
    }
    let cmd = args[0].as_str();
    let rest = &args[1..];
    let file = rest
        .iter()
        .find(|a| !a.starts_with('-') && (a.ends_with(".nano") || a.ends_with(".n")))
        .cloned()
        .or_else(|| {
            rest.iter()
                .position(|a| a == "-o" || a == "--out" || a == "-f" || a == "--format")
                .and_then(|i| rest.get(i + 1).cloned())
        })
        .ok_or("no input file given (try: nano build file.nano)")?;
    if !Path::new(&file).exists() {
        return Err(format!("cannot find `{}`", file));
    }
    let path = PathBuf::from(&file);
    let mut format = "all".to_string();
    let mut out = String::new();
    let mut theme = String::new();
    let mut title = String::new();
    let mut print_path_only = false;
    let mut quiet = false;
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "-o" | "--out" => {
                i += 1;
                out = rest.get(i).cloned().ok_or("-o needs a value")?;
            }
            "-f" | "--format" => {
                i += 1;
                format = rest.get(i).cloned().ok_or("-f needs a value")?;
            }
            "--theme" => {
                i += 1;
                theme = rest.get(i).cloned().unwrap_or_default();
            }
            "--title" => {
                i += 1;
                title = rest.get(i).cloned().unwrap_or_default();
            }
            "-q" | "--quiet" => quiet = true,
            "--open" => print_path_only = true,
            other if other.starts_with('-') => {}
            other => {
                if out.is_empty() && (other.ends_with(".md") || other.ends_with(".html") || other.ends_with(".pdf")) {
                    out = other.to_string();
                }
            }
        }
        i += 1;
    }
    let src = std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", path.display(), e))?;
    let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut d = doc::parse_document(&src, &dir);
    if !title.is_empty() {
        d.meta.title = title;
    }
    if !theme.is_empty() {
        d.meta.theme = theme;
    }
    doc::run_cells(&mut d);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "nano".into());

    match cmd {
        "check" => {
            let mut errors = 0;
            for b in &d.blocks {
                match b {
                    doc::Block::Code { result, .. } => {
                        if let Some(r) = result {
                            if let Some(e) = &r.error {
                                eprintln!("error: {}", e);
                                errors += 1;
                            }
                        }
                    }
                    // a directive whose arguments could not be evaluated is an
                    // error too, even though the document still builds
                    doc::Block::Note(n) if n.error => {
                        eprintln!("error: {}", n.title);
                        errors += 1;
                    }
                    _ => {}
                }
            }
            if errors == 0 && !quiet {
                println!("ok: {} ({} blocks)", file, d.blocks.len());
            }
            return if errors > 0 { Err(format!("{} error(s)", errors)) } else { Ok(()) };
        }
        "run" => {
            if !d.blocks.iter().any(|b| matches!(b, doc::Block::Code { .. })) {
                let mut ip = interp::Interp::new();
                match parser::parse_program(&src) {
                    Ok(stmts) => {
                        for st in &stmts {
                            if let Err(e) = ip.exec(st) {
                                eprintln!("error: {}", e);
                                return Err(e);
                            }
                        }
                        for line in builtins::take_output() {
                            println!("{}", line);
                        }
                        return Ok(());
                    }
                    Err(_) => {}
                }
            }
            for b in &d.blocks {
                if let doc::Block::Code { result, .. } = b {
                    if let Some(r) = result {
                        if !r.stdout.is_empty() {
                            println!("{}", r.stdout);
                        }
                        if let Some(e) = &r.error {
                            eprintln!("error: {}", e);
                            return Err(e.clone());
                        }
                        if let Some(v) = &r.value {
                            println!("{}", v);
                        }
                    }
                }
            }
            return Ok(());
        }
        "build" => {}
        other => return Err(format!("unknown command `{}`", other)),
    }

    let mut formats: Vec<&str> = match format.as_str() {
        "all" => vec!["md", "html", "pdf"],
        "markdown" => vec!["md"],
        f => vec![f],
    };
    formats.dedup();

    if print_path_only {
        // --open reports one path, so it can only ever be the html one
        formats = vec!["html"];
    }

    let base = if out.is_empty() {
        dir.join(&stem)
    } else {
        let p = PathBuf::from(&out);
        if formats.len() > 1 {
            // `-o out.html` with several targets means out.md, out.html, out.pdf,
            // not out.html.md, out.html.html and out.html.pdf
            let ext = p.extension().map(|e| e.to_string_lossy().to_string());
            match ext {
                Some(e) if formats.iter().any(|f| *f == e) => {
                    p.parent().unwrap_or(Path::new(".")).join(p.file_stem().unwrap())
                }
                _ => p,
            }
        } else if p.is_dir() {
            p.join(&stem)
        } else {
            p
        }
    };

    let base_noext: PathBuf = {
        let ext = base.extension().map(|e| e.to_string_lossy().to_string());
        match (formats.len(), &ext) {
            (1, Some(_)) => base.parent().map(|p| p.join(base.file_stem().unwrap())).unwrap_or(base.clone()),
            _ => base.clone(),
        }
    };

    for f in formats {
        let target: PathBuf = PathBuf::from(format!("{}.{}", base_noext.display(), f));
        let text: String = match f {
            "md" => render::md::render(&d),
            "html" => render::html::render(&d),
            "pdf" => {
                let bytes = render::pdf::render(&d).map_err(|e| format!("pdf: {}", e))?;
                std::fs::write(&target, bytes).map_err(|e| format!("{}: {}", target.display(), e))?;
                if print_path_only {
                    if f == "html" {
                        println!("{}", target.display());
                    }
                } else if !quiet {
                    println!("  pdf   {}", target.display());
                }
                continue;
            }
            other => return Err(format!("unknown format `{}`", other)),
        };
        if let Some(parent) = Path::new(&target).parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        std::fs::write(&target, text).map_err(|e| format!("{}: {}", target.display(), e))?;
        if print_path_only {
            if f == "html" {
                println!("{}", target.display());
            }
        } else if !quiet {
            println!("  {:<5} {}", f, target.display());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir() -> std::path::PathBuf {
        static ONCE: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
        ONCE.get_or_init(|| {
            // wipe once per process: a stale file from an earlier run (the pid may
            // well have been reused) would make these assertions lie
            let d = std::env::temp_dir().join(format!("nano-cli-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&d);
            let _ = std::fs::create_dir_all(&d);
            d
        })
        .clone()
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = tmp_dir();
        let p = d.join(name);
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        p
    }

    fn argv(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn open_flag_builds_html_only() {
        let src = tmp("open.nano");
        let out = tmp("open-out.html");
        std::fs::write(&src, "hello\n\n```nano\nx = 1 + 1\n```\n").unwrap();
        run(&argv(&[
            "build",
            src.to_str().unwrap(),
            "--open",
            "-q",
            "-o",
            out.to_str().unwrap(),
        ]))
        .unwrap();
        assert!(out.exists(), "--open should still write the html file");
        assert!(
            !tmp("open-out.pdf").exists(),
            "--open should not build the pdf"
        );
    }

    #[test]
    fn formats_are_honoured() {
        let src = tmp("fmt.nano");
        std::fs::write(&src, "hello\n").unwrap();
        let out = tmp("fmt.md");
        run(&argv(&["build", src.to_str().unwrap(), "-f", "md", "-q", "-o", out.to_str().unwrap()])).unwrap();
        assert!(out.exists());
        assert_eq!(std::fs::read_to_string(&out).unwrap().trim(), "hello");
    }

    #[test]
    fn version_line_matches_the_manifest() {
        assert_eq!(version_line(), format!("nano {}", env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn version_flag_is_recognised_on_its_own_and_after_a_command() {
        for args in [
            vec!["--version"],
            vec!["-V"],
            vec!["build", "does-not-exist.nano", "--version"],
        ] {
            assert!(wants_version(&argv(&args)), "{args:?} should ask for the version");
        }
        for args in [vec!["build", "paper.nano"], vec!["--verbose"], vec!["-v"]] {
            assert!(!wants_version(&argv(&args)), "{args:?} should not ask for the version");
        }
    }

    /// `--version` must not be swallowed by the input-file lookup, which is what
    /// used to turn it into "no input file given".
    #[test]
    fn version_succeeds_without_an_input_file() {
        run(&argv(&["--version"])).unwrap();
        run(&argv(&["build", "missing.nano", "-V"])).unwrap();
    }
}
