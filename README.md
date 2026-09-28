# nano

**A research document language.** Prose, runnable code, data, figures, math and
media live in one file, and that file builds to Markdown, a self-contained
interactive HTML page, and a real PDF — with no dependencies, no browser, no
LaTeX install and no network.

[![CI](https://github.com/yasakei/nano/actions/workflows/test.yml/badge.svg)](https://github.com/yasakei/nano/actions/workflows/test.yml)
[![release](https://img.shields.io/github/v/release/yasakei/nano)](https://github.com/yasakei/nano/releases)
[![license](https://img.shields.io/badge/license-Unlicense-blue.svg)](LICENSE)
[![rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org)
[![platforms](https://img.shields.io/badge/platform-linux%20%7C%20macOS%20%7C%20Windows-lightgrey.svg)](#install)

```
paper.nano  ──▶  paper.md      paper.html      paper.pdf
```

The interpreter, the vector chart engine, a TeX typesetter and the PDF writer
all ship inside the one binary. Nothing is shelled out to.

---

## Quick start

```sh
cargo build --release      # ~2 MB binary, zero dependencies
./nano build paper.nano    # writes all three outputs
```

```
  md    paper.md
  html  paper.html
  pdf   paper.pdf
```

A whole document:

```nano
---
title: First steps
author: You
---

# Hello

Prose with **bold**, *italic* and `code`, plus a value computed from the cell
below: {{ 2 ** 10 }}.

@plot line
  x = [0, 1, 2, 3]
  y = [1, 4, 9, 16]
  title = "x squared"
  xlabel = "x"
  ylabel = "y"

@note
  Build it with `nano build paper.nano`.
```

**Code cells** are fenced with ` ```nano ` and are executed, so the document
reports the *actual* output of the code rather than a claim about it. A cell
that emits a string has its value interpolated into the prose around it:

```nano
let xs = linspace(0, 2 * pi, 64)
let ys = map(xs, sin)

"mean of sin over 0..2π is {round(mean(ys), 4)}"
```

`nano run` prints just the code output, which makes a cell easy to poke at on
its own.

## Why

- **One source of truth.** The prose, the code that produced the numbers, the
  figures and the tables sit in a single file that is both the document *and*
  the program.
- **Three real outputs.** The PDF is written byte by byte — embedded base-14
  fonts, vector plots, a working TeX subset. It is not HTML printed to paper.
- **Nothing to install.** No crates, no LaTeX, no headless browser, no
  plotting library, no CDN. The HTML is one file that works offline.
- **Fast enough to be a reflex.** A full build is milliseconds.

## The document format

**Front matter** is a `---` fenced key/value list: `title`, `subtitle`, `author`,
`date`, `description`, `keywords`, `theme`.

**Prose** is Markdown, plus `{{ expressions }}` that are evaluated at build
time.

**Code cells** are fenced with ` ```nano ` and are executed, so the document
reports the *actual* output of the code rather than a claim about it. Output
from a cell can be interpolated into prose with `{...}`.

Directives are blocks that begin with `@name`:

| directive | what it does |
| --- | --- |
| `@plot` (`@chart`, `@figure`) | a vector chart |
| `@table` | a table from `columns` and `rows` |
| `@math` | display math; the body is raw TeX |
| `@image` | a figure with `alt`, `width`, `float`, `caption` |
| `@video` / `@audio` | a `<video>` / `<audio>` player |
| `@embed` | an iframe for an external reference |
| `@note` (`@tip`, `@warning`, `@info`) | a callout panel |
| `@widget` | an interactive control: `slider`, `checkbox`, `select`, `number`, `text` |
| `@toc` | a table of contents |
| `@pagebreak` | start a new PDF page |
| `@raw` | emit the body verbatim |

Unrecognised keys are ignored rather than fatal, and `nano check` reports
anything that failed to evaluate.

## Charts

Thirteen types — `line`, `scatter`, `area`, `step`, `stem`, `bar`, `hbar`,
`hist`, `box`, `heatmap`, `errorbar`, `function`, `density` — drawn as real
vectors in both the HTML and the PDF, not as embedded images.

```nano
let tau = 2.5

@plot function
  f = "exp(-x / tau) * sin(4 * x)"
  domain = [0, 12]
  title = "A damped oscillation"
```

```nano
@plot box
  labels = ["control", "low dose", "high dose"]
  y = [[[2.1, 3.4, 2.8, 3.9, 3.1], [3.3, 4.0, 3.6, 4.4, 3.9], [5.2, 6.1, 5.8, 6.6, 6.0]]]
  ylabel = "response"
  caption = "Box plots of five samples per arm."
```

Series are named with `labels`, axes with `xlabel` / `ylabel`, and the usual
keys (`grid`, `legend`, `bins`, `ci`, `xmin`, `log_y`, `smooth`, `color`) are
accepted. `y = {values: [...], err: [...]}` adds error bars, and `ci = 1.96`
shades a confidence band. Figures are numbered across the document and the
caption appears in every output.

## Interactive figures

`@widget` declares a control and `interactive` binds a formula to it. In HTML
the figure redraws as you drag; Markdown and PDF render the initial value
normally, so the printed figure is still correct.

```nano
@widget slider
  name = "t"
  label = "damping"
  min = 0.1
  max = 3
  value = 0.9

@plot line
  x = t
  f = "exp(-t * x)"
  domain = [0, 5]
  interactive = "t"
  title = "Drag the slider"
```

## Math

Inline `$...$` and display `@math` are typeset by the built-in TeX subset, in
the PDF with a real Symbol-font mapping and in HTML by a small bundled runtime.
No MathJax, no network.

```nano
@math
  \hat{\beta} = (X^{\top} X)^{-1} X^{\top} y
```

Fractions, radicals, accents, sub/superscripts, big operators with limits,
matrices, Greek and symbol-font characters are all supported.

## Output formats

| | Markdown | HTML | PDF |
| --- | --- | --- | --- |
| prose, code, tables | ✅ | ✅ | ✅ |
| vector charts | as data | inline SVG | vector paths |
| math | source | rendered | TeX subset |
| images, audio, video | links | embedded | embedded |
| interactive widgets | — | live | initial value |
| self-contained | no | **yes** | **yes** |

Themes: `default`, `serif`, `dark`, `minimal`.

## CLI

```
nano build <file.nano> [options]
nano check <file.nano>          parse and evaluate, report errors
nano run <file.nano>            run the code, print output only

  -o, --out <path>   output file, or prefix (extension ignored) when several
                     formats are built
  -f, --format <f>   md | html | pdf | all            (default: all)
      --theme <t>    default | serif | dark | minimal
      --title <t>    override the document title
      --open         print only the path of the HTML output
  -q, --quiet        only report errors
  -V, --version      print the version and exit
```

```sh
nano build paper.nano -f pdf -o out/paper.pdf    # one target, custom path
nano build paper.nano -o out/paper                # out/paper.{md,html,pdf}
xdg-open "$(nano build paper.nano --open)"      # just the path, ready to open
```

## Install

Prebuilt binaries for Linux (glibc and musl, x86-64 and aarch64), macOS
(x86-64 and Apple Silicon) and Windows (x86-64) are attached to each
[release](https://github.com/yasakei/nano/releases).

```sh
# or build from source
cargo build --release
```

## Documentation

| | |
| --- | --- |
| [`examples/tour.nano`](examples/tour.nano) | a complete tour of every feature |
| [`docs/language.md`](docs/language.md) | the language reference |
| [`docs/guide.md`](docs/guide.md) | the long-form guide |

## Development

```sh
cargo test                      # 122 Rust tests
node tests_js/run.mjs           # document, chart and output invariants
node tests_js/all.mjs           # the above plus the browser suites
node tests_js/doc_examples.mjs  # every ```nano block in the docs is executed
```

`tests_js/all.mjs` drives the browser suites through Playwright, which nano
deliberately does not depend on; point it at an install and it skips cleanly
when none is present.

```sh
NANOPW=/path/to/playwright-core/index.mjs node tests_js/all.mjs
```

## License

Released into the [public domain](LICENSE) under [The Unlicense](https://unlicense.org/).
No attribution required — take it, change it, ship it.
