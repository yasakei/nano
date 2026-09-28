# The nano language

A small, ordinary programming language that lives inside documents. Cells
execute in document order and share one scope.

```nano
let xs = linspace(0, 1, 5)
map(xs, fn(x) => round(x * x, 3))
```

Values, then. There are eight:

| type | literal | notes |
| --- | --- | --- |
| number | `3`, `3.14`, `1e-3`, `1_000` | one numeric type; integers print without a decimal point |
| string | `"hi"`, `'hi'`, `` `hi` `` | escapes: `\n \t \\ \" \'` |
| boolean | `true`, `false` | |
| null | `null` | the result of a function that returns nothing |
| list | `[1, 2, 3]` | mutable, and shared by reference |
| map | `{a: 1, b: 2}` | ordered, string keys, mutable |
| function | `fn(x) => x * 2` | first-class |
| range | `0..5`, `0..1..0.1` | a lazy list of numbers |

## Statements

```nano
let x = 1                  // binding
let y: number = 2          // the type annotation is optional and ignored
x = 3                      // assignment
x += 1  x -= 1  x *= 2  x /= 2
a[0] = 9                   // index assignment
m.key = 9                  // field assignment
if x > 2 { ... } else if x { ... } else { ... }
for item in list { ... }   // iterates lists, ranges, strings and map entries
while cond { ... }
break   continue   return value
```

`{ }` is a block. The value of the last expression in a block, in an `if` used
as an expression position, and at the end of a function body is the result, so
there is no need to sprinkle `return` around:

```nano
fn classify(n) {
  if n < 0 { "negative" } else if n == 0 { "zero" } else { "positive" }
}
```

`for` and `if` propagate `break`, `continue` and `return` out of the code they
contain, including from nested blocks.

## Operators

From loosest to tightest:

```
or
and
==  !=  <  <=  >  >=
..
+  -
*  /  %
-2 ^ 2 ^ 3        // ^ is right-associative; -2^2 is -4
x ? a : b         // ternary
```

`+` concatenates strings and lists, `*` repeats a string by a count and a list
by a count, `%` is modulo, `and`/`or`/`not` short-circuit and return the
operand (so `x or default` is a defaulting operator), and `..` builds a range.
Indexing is zero-based and negative indices count from the end: `xs[-1]`.

## Functions

```nano
fn lorentz(x, gamma) {          // named, with a block body
  gamma / (pi * (x * x + gamma * gamma))
}

let half = lorentz(_, 0.7)       // partially applied
let sq = fn(v) => v * v          // anonymous, expression body
let with_default = fn(a, b = 10) => a + b
```

Functions are values, they close over their environment, and they may recurse.
`_` marks a hole in a partial application, so `map(xs, sqrt(_))` and
`map(sqrt(_), xs)` both work. `map`, `filter`, `reduce`, `sort` and `each`
accept their arguments in either order.

## The standard library

**Inspection** `type` `str` `num` `len` `is_null` `is_num` `is_str` `is_list`
`is_map` `is_fn` `print` `format` `get` `has` `keys` `values` `pairs` `merge`
`set` `set` `slice`

**Math** `abs` `sign` `sqrt` `floor` `ceil` `round` `pow` `min` `max` `clamp`
`sum` `prod` `mean` `median` `mode` `std` `var` `sem` `quantile` `cumsum`
`cumprod` `diff` `corr` `dot` `norm` `linspace` `arange` `range` `gcd` `exp`
`log` `log2` `log10` `sin` `cos` `tan` `asin` `acos` `atan` `atan2` `sinh`
`cosh` `tanh` `hypot`

**Random** `seed` `random` `randint` `randn` `shuffle` `choice` — seeded, so
figures are reproducible.

**Lists** `map` `filter` `reduce` `each` `sort` `sorted` `reverse` `unique`
`zip` `zip_with` `enumerate` `concat` `extend` `slice` `take` `drop` `first`
`last` `flatten` `contains` `count` `any` `all` `group_by` `find` `index_of`
`index_where` `argmax` `argmin` `interp` `join` `push` `pop` `insert` `remove`

**Strings** `upper` `lower` `trim` `title` `slug` `pad` `pad_start` `replace`
`split` `lines` `starts_with` `ends_with` `find`

### Methods

Values have methods too, and the receiver is passed implicitly:

```nano
xs.map(fn(v) => v * 2)
xs.filter(fn(v) => v > 1)
xs.sort_by("name")          // by field, or by a function
s.upper()  s.split(",")  s.replace("a", "b")
m.keys()  m.values()  m.get("k")
```

### Sorting and grouping

```nano
sort(rows, "score")                 // by field
sort(rows, fn(r) => -r.score)       // by expression
sort(xs, null, true)                // descending
group_by(rows, fn(r) => r.site)     // -> map of lists
```

### Interpolation

Inside any string, `{expr}` interpolates and `{expr:spec}` formats:

```nano
"n = {len(xs)}, mean = {mean(xs):.3f}, p = {0.4213:.1%}, signed = {1.5:+}"
```

Spec supports `f e g x d s`, precision, width, fill and alignment (`<`, `>`,
`^`), thousands separators, and `%`.

## Maths

`$...$` is inline maths, `$$ ... $$` on its own line is display maths. All three
outputs typeset it, so the PDF is not a picture of the source:

```nano
Energy is $E = mc^2$, and the error bound is

$$
  \int_0^\infty e^{-x^2}\,dx = \frac{\sqrt{\pi}}{2}
$$
```

`@math`, `@eq` and `@equation` do the same thing as a directive, if you would
rather keep the formula out of the prose.

Understood: `^` and `_` scripts, `\frac`, `\sqrt`, `\hat`, `\bar`, `\vec`,
`\tilde`, `\dot`, `\overline`, `\underline`, `\left(...\right)` fences that
grow to fit, `\begin{matrix}` / `pmatrix` / `bmatrix` / `vmatrix`, Greek
(`\alpha` ... `\omega`, `\Gamma` ... `\Omega`), relations (`\le`, `\ge`,
`\ne`, `\approx`, `\equiv`, `\sim`, `\propto`), arrows, `\to`,
`\infty`, `\partial`, `\nabla`, `\in`, `\subset`, `\cup`, `\cap`,
`\emptyset`, `\forall`, `\exists`, `\neg`, `\times`, `\div`, `\cdot`,
`\pm`, big operators with limits (`\sum`, `\prod`, `\int`, `\oint`,
`\bigcup`, `\bigcap`), function names (`\sin`, `\cos`, `\log`, `\ln`,
`\exp`, `\max`, `\min`, `\det`, ...), `\text{...}` for words inside a
formula, and the spacing commands `\,` `\;` `\!` `\quad` `\qquad`.

Limits sit above and below a big operator in display maths, and beside it inline,
which is what you would expect from TeX. An unknown command is typeset as
italic text rather than being dropped, so a typo is visible instead of silent.

## Errors

Errors are values, not panics: division by zero, an out-of-range index, an
undefined variable or a bad argument produce a readable message with a line
number, the cell is marked, and the rest of the document still builds. Inside a
code cell the error text is shown under the code; in a directive the block is
replaced by a callout naming the offending argument.

## Things nano does not have

On purpose: no classes, no modules, no `import`, no exceptions, no async, no
`while true` with no exit (there is an iteration guard), no hidden state. The
scope of a document is the scope of the file, and that stays true as the file
grows.
