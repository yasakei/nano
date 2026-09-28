# Using nano

A hands-on guide. It starts from an empty file and ends with the recipes you
reach for most: a preprint, a lab handout, a results page.

`README.md` is the reference (every directive, every option). `language.md` is
the language reference (every builtin). This document is the *how*.

---

## 1. Install

```sh
git clone <this repo> && cd nano
cargo build --release          # ~1.8 MB binary, zero dependencies
```

Put it on your `PATH`:

```sh
cp target/release/nano ~/.local/bin/     # or: cargo install --path .
```

Check it:

```sh
nano --help
```

There is nothing else to install. No LaTeX, no browser, no plotting library, no
network. Rust is the only build dependency, and only if you want to rebuild the
binary.

## 2. Your first document

Create `hello.nano`:

```nano
---
title: First look
author: You
date: 2026-03-01
---

# First look

Let us start with something small.

```nano
let t = linspace(0, 10, 100)
let y = map(t, fn(x) => sin(x) * exp(-x / 5))
```

The signal peaks at {{ round(max(y), 4) }}.

@plot line
  x = t
  y = y
  title = "A damped wave"
```

Build it:

```sh
nano build hello.nano
```

```
  md    hello.md
  html  hello.html
  pdf   hello.pdf
```

Open `hello.html`. The text, the figure, and the number in the sentence all
came from one file, and the number in the sentence is the number in the chart
because both were computed from the same variable.

> Nested code fences are awkward in a tutorial. In a real `.nano` file the
> fence is written exactly as above — a line of three backticks, the word
> `nano`, your code, three backticks.

## 3. The mental model

Four rules explain almost everything about how a nano document behaves.

**1. It is Markdown first.** Headings, lists, tables, links, emphasis, block
quotes, inline `` `code` ``, `$math$` — all plain Markdown, all passed through.
You only need to learn the two things Markdown lacks: code that runs, and
`@directives`.

**2. Cells run in document order and share one scope.** A variable defined in
cell 4 is visible in cell 40 and in every `@plot` below it. There is no
`import`, no module system, and no need to re-run a cell from the top.

**3. The build captures results.** When `nano build` runs, every cell is
executed, `print` output and the value of the last expression are stored, and
`{{ ... }}` in the prose is substituted with the computed value. The exported
files contain numbers, not code to compute numbers. A reader without nano sees
the same results you do.

**4. Exports never run anything.** Markdown, HTML and PDF are static. The only
interactive parts are the HTML widgets, and they degrade to their initial value
in the other two formats.

## 4. Numbers in prose

Use `{{ expression }}` anywhere in the text:

````nano
```nano
let xs = linspace(10, 20, 11)
```

The sample has n = {{ len(xs) }} points and a mean of {{ round(mean(xs), 3) }}.
````

Rounding is your job, so decide what a reader should see:

````nano
```nano
let ys = [1.234, 5.678, 9.1011]
```

{{ mean(ys) }}        {{ round(mean(ys), 3) }}        {{ round(mean(ys) * 100, 1) }}%
{{ mean(ys) }}        {{ format(mean(ys), ".3f") }}
````

If an expression fails, the document still builds and the failure is shown in
place as a visible error panel, so a broken cell never silently produces a
plausible-looking PDF.

## 5. Tables

```nano
@table
  columns = ["method", "rmse", "params"]
  rows = [["baseline", 0.812, 3], ["ours", 0.344, 11]]
  align = ["l", "r", "r"]
  caption = "Held-out error on the validation split."
  max_rows = 20
```

`rows` can be any list of lists, or a list of records:

````nano
```nano
let records = [
  {site: "north", temp: 4.1, n: 88},
  {site: "north", temp: 6.3, n: 91},
  {site: "south", temp: 19.8, n: 74}
]

let by_site = group_by(records, fn(r) => r.site)
```

@table
  columns = ["site", "n", "mean temp"]
  rows = map(keys(by_site), fn(k) => [k, len(by_site[k]), round(mean(map(by_site[k], fn(r) => r.temp)), 1)])
  align = ["l", "r", "r"]
  caption = "Grouped by site."
````

Keys: `columns`, `rows`, `align`, `caption`, `zebra` (striped rows), `max_rows`
(collapse longer tables with a "… N more rows" line).

## 6. Charts

Every chart is one directive whose first line names the type.

````nano
```nano
let days = linspace(0, 10, 40)
let control = map(days, fn(d) => 9 - 0.3 * d)
let treated = map(days, fn(d) => 12 - 0.7 * d)
```

@plot line
  x = days
  y = [control, treated]
  labels = ["control", "treated"]
  title = "Response over time"
  xlabel = "day"
  ylabel = "concentration"
  width = 620
  height = 320
  caption = "Both arms, same axis."
````

Pass several series in one `y` and name them in `labels`. A single value works
too — `y = [4.1, 6.3, 11.8]`.

The thirteen types, with the minimum you need. This one preamble defines every
variable the examples below use, so you can paste the whole block as it is:

````nano
```nano
let t = linspace(0, 10, 60)
let y = map(t, fn(x) => sin(x) * exp(-x / 6))
let x_measured = t
let noise = map(range(60), fn(i) => randn(1)[0])
let y_measured = map(zip(y, noise), fn(p) => p[0] + 0.05 * p[1])
let impulses = linspace(0, 6, 7)
let heights = [0.3, 1.1, 0.2, 2.0, 0.5, 1.6, 0.4]
let arm_a = [1.0, 1.2, 0.9, 1.1]
let arm_b = [2.2, 2.4, 2.1, 2.6]
let arm_c = [0.5, 0.7, 0.4, 0.6]
let samples = map(range(300), fn(i) => randn(1))
let matrix = [[23, 4, 1], [2, 19, 6], [0, 3, 22]]
let means = [1.0, 1.4, 1.2]
let errors = [0.2, 0.3, 0.15]
let mean_curve = y
let sem = map(y, fn(v) => 0.05 * v + 0.02)
```

@plot line
  x = t
  y = y

@plot scatter
  x = x_measured
  y = y_measured
  point_size = 2.6

@plot area
  x = t
  y = y
  fill = 0.35

@plot step
  x = t
  y = y

@plot stem
  x = impulses
  y = heights

@plot bar
  labels = ["control", "low", "high"]
  y = [4.1, 6.3, 11.8]

@plot hbar
  labels = ["north", "south", "east"]
  y = [12, 27, 9]

@plot hist
  y = samples
  bins = 24

@plot box
  y = [arm_a, arm_b, arm_c]
  labels = ["a", "b", "c"]

@plot heatmap
  y = matrix
  xlabel = "predicted"
  ylabel = "actual"

@plot errorbar
  y = {values: means, err: errors}

@plot density
  y = samples
  bins = 30

@plot function
  f = "exp(-x / tau) * sin(4 * x)"
  domain = [0, 12]
````

Useful keys beyond the obvious: `grid`, `legend`, `smooth`, `color`/`colors`
(`teal`, `coral`, `navy`, …), `xmin`/`xmax`/`ymin`/`ymax`, `log_x`, `log_y`,
`linewidth`, `points`, `ci` (shade a confidence band), `caption`.

Errors per series, and a confidence band:

````nano
```nano
let grid = linspace(0, 10, 40)
let mean_curve = map(grid, fn(x) => sin(x) * exp(-x / 6))
let sem = map(mean_curve, fn(v) => 0.05 * v + 0.02)
```

@plot line
  x = grid
  y = {values: mean_curve, err: sem}
  ci = 1.96
````

Figures are numbered across the document (`Figure 1.`, `Figure 2.`, …) in all
three outputs, with matching captions.

## 7. Interactive figures (HTML only)

Declare a control, then bind a plot to it.

```nano
@widget slider
  name = "t"
  label = "damping rate"
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
```

Show the current value with a readout:

```nano
Current damping rate: <span data-var-readout="t">0.9</span>
```

Widget kinds: `slider`, `checkbox`, `select` (add `options = ["a", "b"]`),
`number`, `text`. Several plots can share one widget, and several widgets can
drive one plot.

In Markdown and PDF the figure is drawn at the initial `value`, so the printed
version is still complete and self-contained.

## 8. Math

Inline with `$…$`, display with `@math`:

```nano
The solution of $ax^2 + bx + c = 0$ is $x = \frac{-b \pm \sqrt{b^2-4ac}}{2a}$.

@math
  \hat{\beta} = (X^{\top} X)^{-1} X^{\top} y
```

The bundled typesetter covers fractions, roots, big operators with limits,
matrices (`pmatrix`), `align`, and `\left…\right` sizing. Anything it does not
recognise stays as readable source rather than disappearing.

In the PDF, math is typeset with the base-14 fonts; there is no LaTeX
dependency.

## 9. Media

```nano
@image
  src = assets/pipeline.svg
  width = 70%
  alt = "Pipeline from raw data to figure"
  caption = "The whole method on one page."

@video
  src = assets/demo.mp4
  poster = assets/demo.png
  controls = true

@audio
  src = assets/tone.wav
  caption = "Reference tone."

@embed
  src = https://example.org/reference
  caption = "External reference."
```

How each is handled:

| | Markdown | HTML | PDF |
| --- | --- | --- | --- |
| PNG / JPEG | copied next to the doc | embedded as base64 | embedded, alpha via soft mask |
| SVG | copied | inlined | imported as **real vector graphics** |
| video / audio | link | player | labelled placeholder |
| `@embed` | link | `<iframe>` | labelled placeholder |

Relative paths are resolved against the `.nano` file, so a document and its
`assets/` folder move together.

## 10. Structure

Front matter is optional and goes between two `---` lines at the top:

```nano
---
title: A study of drift
subtitle: Two years of data
author: You
date: 2026-03-01
description: One sentence, used as the HTML meta description and the PDF abstract.
keywords: nano, drift, ecology
theme: dark
---
```

Recognised keys: `title`, `subtitle`, `author` (or `authors`, a list), `date`,
`description` (or `abstract`), `keywords`, `theme`.

In the body:

```nano
@toc

## A section
### A subsection

@note
  Cells share one scope, so a variable from cell 4 is available here.

@tip
  Tips, warnings and notes all become callout panels in every format.

@pagebreak

## The appendix
```

`@pagebreak` starts a new PDF page — useful when a wide table or a figure must
not be split.

## 11. Choosing output

```sh
nano build paper.nano                     # all three
nano build paper.nano -f pdf              # just the PDF
nano build paper.nano -f html --theme serif
nano build paper.nano -o out/paper        # out/paper.{md,html,pdf}
nano build paper.nano -o out/paper.pdf -f pdf
nano build paper.nano --open              # prints just the HTML path
```

Themes: `default`, `serif`, `dark`, `minimal`. Set it in the front matter or
override per build.

Other commands:

```sh
nano check paper.nano     # parse and evaluate, report errors, exit 1 on failure
nano run script.nano      # run a bare script, print output only
```

`nano run` is a quick calculator:

```sh
$ printf 'print(sum(linspace(1, 100, 100)))' > /tmp/x.nano && nano run /tmp/x.nano
5050
```

## 12. When something goes wrong

| message | meaning |
| --- | --- |
| `undefined variable \`x\`` | the name is not in scope. Cells share one scope top to bottom, so usually the defining cell is *below*, or it failed. |
| `` `_` is only a hole in a partial application `` | `_` is a whole-argument placeholder (`pow(_, 2)`), not a lambda. Use `fn(x) => x + 1`. |
| `expected ')'` and a column number | the error carries the line, column and a caret. |
| `plot: non-numeric column` | a series held a string or null. Check with `map(series, type)`. |
| `image: no such file` | the path is resolved relative to the `.nano` file. |
| `recursion too deep` | the 400-frame guard. Usually an accidental cycle. |

`nano check` is the fastest way to find out: it evaluates every cell and
reports each failure, then exits non-zero.

## 13. Recipes

### A preprint

Front matter for the metadata, `@pagebreak` before the references, `-f pdf`.
Use `@toc` and section numbering you control with the heading levels.

### A lab handout

A document with a widget per parameter and one interactive figure per question.
`nano build handout.nano -f html` gives students one file with no network
access; the Markdown build is the printable version.

### A results page for a colleague

One table of summary statistics, one figure per hypothesis, and every number in
the text coming from `{{ }}` so nothing can drift out of sync.

### A reproducible analysis

Keep the raw numbers in a cell, compute everything above the fold, and never
type a result by hand. If the data changes, rebuild — the prose updates with
it.

---

Next: [language.md](language.md) for the standard library, or `README.md` for
the full directive and option reference.
