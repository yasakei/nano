# nano

**A research document language.** Prose, runnable code, data, figures, math
and media live in one file, and that file builds to **Markdown**, a
**self-contained interactive HTML page**, and a real **PDF** — with no
dependencies, no browser, no LaTeX install, and no network.

```
paper.nano  ──▶  paper.md     paper.html     paper.pdf
```

```sh
cargo build --release        # one binary, 1.8 MB, zero dependencies
./nano build paper.nano      # writes all three
```

```
  md    paper.md
  html  paper.html
  pdf   paper.pdf
```

Everything ships in the binary: the interpreter, the chart engine (which draws
real vector figures, it does not wrap a plotting library), a TeX typesetter, and
a PDF writer.

---

## Contents

- [Why](#why)
- [Quick start](#quick-start)
- [Task-oriented guide](docs/guide.md)
- [The document format](#the-document-format)
- [Directives](#directives)
- [Charts](#charts)
- [Interactive figures](#interactive-figures)
- [Output formats](#output-formats)
- [CLI](#cli)
- [Language reference](docs/language.md)
- [Project layout](#project-layout)

---

## Why

Research writing is a pipeline: you compute something, you plot it, you write
about it, and eventually you export it for someone who cannot run your code.
Every tool forces that pipeline apart. Notebooks are good at computing and bad
at typesetting. LaTeX is good at typesetting and bad at computing. Markdown is
portable and cannot compute at all.

`nano` keeps them in one file, and treats export as a first-class output rather
than a print dialog. The same file that you edit is the file that produces the
PDF you attach to an email.

Three properties drove the design:

1. **The document is the source of truth.** Code results are captured at build
   time, so a number in a sentence and a number in a table cannot disagree.
2. **The output is yours.** Markdown is plain text. HTML is one file with no
   external requests. The PDF is written byte by byte, with selectable text,
   real vector figures, and document outlines.
3. **No dependencies.** `cargo build` is the whole toolchain. Nothing to
   install, nothing to break, and the same binary on any machine.

## Quick start

Write `hello.nano`:

```nano
---
title: First steps
author: You
---

# Hello

Prose, with **bold**, *italic*, `code`, a [link](https://example.org), and a
number computed from the cell below: {{ 2 ** 10 }}.

```nano
let xs = linspace(0, 2 * pi, 64)
let ys = map(xs, sin)

"mean of sin over 0..2π is {round(mean(ys), 4)}"
```

@plot line
  x = xs
  y = ys
  title = "One period of sin"
  xlabel = "x"
  ylabel = "sin x"
  caption = "Drawn by the built-in chart engine."

@note
  Build it with `nano build hello.nano`.
```

```sh
nano build hello.nano          # hello.md, hello.html, hello.pdf
nano run hello.nano            # just the code output
nano check hello.nano          # syntax + evaluation check
```

A complete tour of every feature is in [`examples/tour.nano`](examples/tour.nano):

```sh
nano build examples/tour.nano
```

## The document format

A `.nano` file is [Markdown](https://commonmark.org) plus two additions:
**directives**, and code cells that execute.

### Prose

Headings, paragraphs, lists, task lists, block quotes, `---` rules, thematic
breaks, tables you write by hand, footnotes-ish `[ref]`, and the usual inline
marks. Escape any of the syntax characters with a backslash.

| you write | you get |
| --- | --- |
| `# H1` … `###### H6` | headings, with a slug id and a hover anchor |
| `**bold**`, `*em*`, `~~strike~~` | styled runs |
| `` `code` `` | inline code, with a chip background in HTML and a panel in PDF |
| `[text](url)`, `![alt](img.png)` | links and inline images |
| `$e^{i\pi}$` | typeset math (inline) |
| `$$ … $$` | typeset math (display) |
| `{{ expression }}` | a value computed at build time, inlined into the prose |
| `https://…` | auto-linked |

`{{ }}` is the small idea that makes documents feel alive: the value is
evaluated when you build, so prose and data can never drift apart.

```nano
let n = 137
"there are {n} subjects, so {round(log(n, 2), 1)} bits of address space"
```

### Code cells

A fenced block tagged `nano` (or `run`, or empty) **executes**. Any other tag
(`python`, `rust`, `text`) is shown verbatim and not run. Cells share one
scope, top to bottom, in document order — like a notebook, but in a file you
can diff and grep.

```nano
let data = [{name: "a", v: 3}, {name: "b", v: 5}, {name: "c", v: 4}]
let by_name = group_by(data, fn(r) => r.name)

"mean value = {mean(map(data, fn(r) => r.v))}"
```

Each cell renders as a code panel with its language chip, a copy button in
HTML, and a result pane: whatever `print` produced, then the value of the last
expression. An error is shown inline with a red marker and stops the build of
that cell (later cells still render, so one mistake does not hide the rest of
your document).

### Directives

A directive starts a line with `@` and is followed by `key = value` lines.
Values are expressions in the document language, so directives see your
variables. Data keys (`x`, `y`, `rows`, `columns`, `f`, `domain`, …) report
errors loudly; a value that cannot be evaluated is reported in place as a
callout rather than silently becoming a string.

## Directives

| directive | what it does |
| --- | --- |
| `@plot` (`@chart`, `@figure`) | a chart — see [Charts](#charts) |
| `@table` | a table from `columns` and `rows` |
| `@image` | a figure, with `alt`, `width`, `float`, `caption` |
| `@video` | a `<video>` with `poster`, `autoplay`, `loop`, `muted`, `controls` |
| `@audio` | an `<audio>` player |
| `@embed` | an iframe (for external references) |
| `@math` | display math; the body is raw TeX |
| `@note` (`@tip`, `@warning`, `@info`) | a callout panel; the body is prose |
| `@widget` | an interactive control: `slider`, `checkbox`, `select`, `number`, `text` |
| `@toc` | a table of contents |
| `@pagebreak` | start a new PDF page |
| `@raw` | emit the body verbatim |

Common keys: `title`, `caption`, `width`, `float`, plus whatever the block
accepts. Unrecognised keys are ignored rather than fatal.

```nano
@table
  columns = ["method", "rmse"]
  rows = [["baseline", 0.81], ["ours", 0.34]]
  align = ["l", "r"]
  caption = "Held-out error."
  max_rows = 20
```

## Charts

One directive, thirteen chart types, and the same data you already computed.

```nano
seed(11)
let xs = linspace(0, 10, 60)
let ys = map(xs, fn(x) => sin(x) + 0.06 * randn(1))
let fitted = map(xs, fn(x) => sin(x))
```

@plot scatter
  x = xs
  y = [ys, fitted]
  labels = ["measured", "model"]
  title = "Measured against model"
  grid = true
  smooth = false
  log_y = false
````

**Types:** `line`, `scatter`, `area`, `step`, `stem`, `bar`, `hbar`, `hist`,
`box`, `heatmap`, `errorbar`, `function`, `density`.

**Keys:** `x`, `y`, `labels`, `title`, `xlabel`, `ylabel`, `width`, `height`,
`color`, `colors`, `grid`, `legend`, `smooth`, `style`, `fill`, `bins`,
`points`, `point_size`, `linewidth`, `xmin`, `xmax`, `ymin`, `ymax`, `log_x`,
`log_y`, `ci`, `caption`, `f`, `domain`, `interactive`.

A few shapes worth knowing (each block below is self-contained — paste it as it
is):

```nano
@plot bar
  labels = ["control", "low", "high"]
  y = [4.1, 6.3, 11.8]
  title = "One series per bar"
```

```nano
let days = linspace(0, 12, 60)
let control  = map(days, fn(d) => 9 - 0.3 * d)
let treated  = map(days, fn(d) => 12 - 0.7 * d)
let followup = map(days, fn(d) => 6 - 0.1 * d)
```

@plot line
  x = days
  y = [control, treated, followup]
  labels = ["control", "treated", "follow-up"]
  xlabel = "day"
  ylabel = "concentration"
````

```nano
seed(3)
let samples = map(range(400), fn(i) => randn(1) + (i % 7) * 0.4)
```

@plot hist
  y = samples
  bins = 24
  color = teal
````

```nano
let matrix = [[23, 4, 1], [2, 19, 6], [0, 3, 22]]
```

@plot heatmap
  y = matrix
  xlabel = "predicted"
  ylabel = "actual"
  title = "Confusion matrix"
````

```nano
let tau = 2.5
```

@plot function
  f = "exp(-x / tau) * sin(4 * x)"
  domain = [0, 12]
  title = "A damped oscillation"
````

Errors are carried per series: set `y = {values: [...], err: [...]}` to get
error bars, and `ci = 1.96` to shade a confidence band.

Figures are numbers (`Figure 1.`, `Figure 2.`) across the document, and the
caption appears in all three outputs.

## Interactive figures

Declare a widget, then bind a formula to it. In HTML the figure redraws in
real time as you drag; in Markdown and PDF the initial value is rendered
normally.

@widget slider
  name = "t"
  label = "damping"
  min = 0.1
  max = 3
  step = 0.05
  value = 0.9

@plot line
  f = "exp(-t * x)"
  x = t
  domain = [0, 5]
  interactive = "t"
  title = "Drag the slider"
````

`f` is a formula in terms of `x` (rename it with `x = t`). It is evaluated by a
small expression language that ships inside the HTML page: `+ - * / % ^`,
parentheses, implicit multiplication (`2x`), `pi`, and
`sin cos tan asin acos atan sinh cosh tanh exp log ln log2 log10 sqrt abs sign
floor ceil round trunc pow min max clamp hypot`. Multiple series are separated
by `;`.

Any element with `data-var-readout="t"` shows the live value:

```nano
Current rate: <span data-var-readout="t">0.9</span>
```

Everything degrades: with JavaScript disabled the static figure is still there
and still correct.

## Output formats

**Markdown.** GitHub-flavoured. Charts and local images are written to
`assets/plot-N.svg` and `assets/…` next to the document, so the file is
portable. Tables get alignment markers, notes become GitHub alerts, math
becomes `$…$` / `$$…$$`.

**HTML.** One file, no external requests. A system-font typography-first layout
with four themes (`default`, `serif`, `dark`, `minimal`), sticky header,
print stylesheet, and a live canvas under every interactive figure. Images are
embedded as base64 data URIs, SVGs are inlined, fonts are system fonts. Math is
typeset by a bundled TeX renderer (fractions, roots, big operators with limits,
`align`, `pmatrix`, `\left…\right` scaling) that degrades to readable source
for anything it does not know.

**PDF.** A PDF 1.4 file written from scratch: A4 with real margins, base-14
fonts, selectable and searchable text, running heads, a two-pass table of
contents with page numbers, PDF bookmarks, repeated table headers, code panels
with result panes, callout panels, embedded PNG and JPEG (including alpha via
soft masks), **SVG figures imported as real vector graphics**, and media that
cannot exist in a PDF (video, audio, embeds) drawn as clearly labelled
placeholders rather than dropped silently. Math is typeset by a built-in TeX
subset engine — fractions, roots, scripts, big operators with limits, growing
`\left…\right` fences and matrices are drawn as real rules, paths and glyphs,
using the Symbol font for Greek and operators, so a formula in the PDF is a
formula and not a picture of its source. Output is deterministic.

## CLI

```
nano build <file.nano> [options]
nano check <file.nano>          parse and evaluate, report errors
nano run <file.nano>            run the code, print output only

  -o, --out <path>   output file, or prefix (extension ignored) when several formats are built
  -f, --format <f>   md | html | pdf | all          (default: all)
      --theme <t>    default | serif | dark | minimal
      --title <t>    override the document title
      --open         print only the path of the HTML output
  -q, --quiet        only report errors
  -V, --version      print the version and exit
```

```sh
nano build paper.nano -f pdf -o out/paper.pdf    # one target, custom path
nano build paper.nano -o out/paper                # out/paper.{md,html,pdf}
nano build notes.nano -f html --theme dark
nano run data.nano                              # a plain script also works
nano --version                                  # nano 0.1.0
xdg-open "$(nano build paper.nano --open)"      # just the path, ready to open
```

`nano run` accepts either a document (it runs the cells) or a bare script with
no fences, which makes it usable as a quick calculator:

```sh
$ printf 'print(sum(linspace(1, 100, 100)))' > /tmp/x.nano && nano run /tmp/x.nano
5050
```

## Project layout

```
src/
  lexer.rs parser.rs ast.rs    the language front end
  interp.rs value.rs builtins.rs  the evaluator, values, standard library
  doc.rs                       the document format: blocks, directives, cells
  plot/mod.rs                  the chart engine (13 chart types -> vector scenes)
  render/mod.rs                the display list shared by every backend
  render/svg.rs                scene -> SVG
  render/md.rs                 scene -> Markdown
  render/html.rs               scene -> single-file HTML
  render/tex.js                the bundled TeX typesetter
  render/js_runtime.js         widgets, live plots, copy buttons
  render/pdf.rs                the PDF writer
  svgpdf.rs                    SVG -> display list, for vector figures in PDF
  image.rs                     a PNG decoder (with its own inflate) and JPEG sizing
tests_js/run.mjs               185 checks against the bundled JavaScript
tests_js/browser.mjs           26 checks in a real browser (needs playwright)
tests_js/interactive.mjs       19 checks that drive the widgets and live plots
tests_js/all.mjs               runs everything above
examples/tour.nano             a tour of every feature
```

Tests: `cargo test` (122 checks across the language, charts, every backend, the
PDF structure and the SVG importer), `node tests_js/run.mjs` for the browser
runtime, and `node tests_js/all.mjs` for the whole suite.

`all.mjs` also drives the generated HTML in headless Chromium: it checks that the
page issues no network requests, that no script throws, that every formula is
typeset, that there is no injected markup, that nothing overflows or is clipped,
that text meets WCAG AA contrast in all four themes, and that moving a slider
actually repaints the live plot bound to it. Playwright is not a dependency of
nano, so the browser tests are skipped (exit 77) unless you point `NANOPW` at a
`playwright-core` install, or let it find a cached browser:

```sh
NANOPW=/path/to/playwright-core/index.mjs node tests_js/all.mjs
```

## License

MIT.
