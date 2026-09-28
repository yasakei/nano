/* tests_js/run.mjs - node harness for src/render/tex.js + src/render/js_runtime.js */
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '..');
const TEX = path.join(root, 'src/render/tex.js');
const RT = path.join(root, 'src/render/js_runtime.js');

/* ---------------- tiny DOM shim (no innerHTML) ---------------- */

class ClassList {
  constructor(el) { this.el = el; }
  get list() { return (this.el.getAttribute('class') || '').split(/\s+/).filter(Boolean); }
  contains(c) { return this.list.indexOf(c) >= 0; }
  add(c) { if (!this.contains(c)) this.setAttribute('class', this.list.concat([c]).join(' ')); }
  remove(c) { this.setAttribute('class', this.list.filter((x) => x !== c).join(' ')); }
}

class Node {
  constructor(type) {
    this.nodeType = type;
    this.childNodes = [];
    this.parentNode = null;
  }
  get firstChild() { return this.childNodes[0] || null; }
  get children() { return this.childNodes.filter((n) => n.nodeType === 1); }
  appendChild(n) {
    if (n && n.parentNode) n.parentNode.removeChild(n);
    n.parentNode = this;
    this.childNodes.push(n);
    return n;
  }
  insertBefore(n, ref) {
    const i = ref ? this.childNodes.indexOf(ref) : -1;
    if (i < 0) return this.appendChild(n);
    if (n.parentNode) n.parentNode.removeChild(n);
    n.parentNode = this;
    this.childNodes.splice(i, 0, n);
    return n;
  }
  removeChild(n) {
    const i = this.childNodes.indexOf(n);
    if (i >= 0) this.childNodes.splice(i, 1);
    n.parentNode = null;
    return n;
  }
  get textContent() {
    if (this.nodeType === 3) return this.data;
    let s = '';
    for (const c of this.childNodes) s += c.textContent;
    return s;
  }
  set textContent(v) {
    if (this.nodeType === 3) { this.data = String(v); return; }
    for (const c of this.childNodes) c.parentNode = null;
    this.childNodes = [];
    if (v !== '' && v !== null && v !== undefined) this.appendChild(new Text(String(v)));
  }
}

class Text extends Node {
  constructor(v) { super(3); this.data = String(v); }
}

class Ctx2D {
  constructor(cv) { this.canvas = cv; this.calls = []; this.font = ''; this.lineWidth = 1; }
  rec(op, a) { this.calls.push([op, a]); }
  setTransform() { this.rec('setTransform'); }
  clearRect(a) { this.rec('clearRect', a); }
  save() { this.rec('save'); }
  restore() { this.rec('restore'); }
  beginPath() { this.rec('beginPath'); }
  closePath() { this.rec('closePath'); }
  moveTo(a, b) { this.rec('moveTo', a, b); }
  lineTo(a, b) { this.rec('lineTo', a, b); }
  arc(a, b, c) { this.rec('arc', a, b, c); }
  stroke() { this.rec('stroke'); }
  fill() { this.rec('fill'); }
  fillRect(a, b, c, d) { this.rec('fillRect', a, b, c, d); }
  strokeRect(a, b, c, d) { this.rec('strokeRect', a, b, c, d); }
  fillText(t, a, b) { this.rec('fillText', t, a, b); }
  measureText(t) { return { width: String(t).length * 5.5 }; }
  setLineDash(a) { this.rec('setLineDash', a); }
  translate(a, b) { this.rec('translate', a, b); }
  rotate(a) { this.rec('rotate', a); }
}

class Element extends Node {
  constructor(tag) {
    super(1);
    this.tagName = String(tag).toUpperCase();
    this.attributes = {};
    this.style = {};
    this.classList = new ClassList(this);
    this.listeners = {};
    this.id = '';
  }
  setAttribute(k, v) {
    this.attributes[k] = String(v);
    if (k === 'id') this.id = String(v);
    if (k === 'value' || k === 'checked') this[k] = v === '' || v === k ? true : String(v);
    if (k === 'type') this.type = String(v);
    if (k === 'class') this.classList = new ClassList(this);
  }
  get text() { return this.textContent; }
  get value() {
    if (this._value !== undefined) return this._value;
    if (this.tagName === 'SELECT') {
      const o = this.options[this.selectedIndex];
      if (o) {
        const v = o.getAttribute('value');
        return v === null ? o.textContent : v;
      }
    }
    return undefined;
  }
  set value(v) { this._value = v; }
  get options() { return this.tagName === 'SELECT' ? this.querySelectorAll('option') : []; }
  get selectedIndex() {
    if (this._selectedIndex !== undefined) return this._selectedIndex;
    const o = this.options;
    for (let i = 0; i < o.length; i++) if (o[i].getAttribute('selected') !== null) return i;
    return 0;
  }
  set selectedIndex(i) { this._selectedIndex = i; }
  getAttribute(k) { return owns(this.attributes, k) ? this.attributes[k] : null; }
  hasAttribute(k) { return owns(this.attributes, k); }
  removeAttribute(k) { delete this.attributes[k]; }
  addEventListener(type, fn) { (this.listeners[type] = this.listeners[type] || []).push(fn); }
  removeEventListener(type, fn) {
    const l = this.listeners[type] || [];
    const i = l.indexOf(fn);
    if (i >= 0) l.splice(i, 1);
  }
  dispatch(type, ev) {
    for (const fn of (this.listeners[type] || []).slice()) fn(ev || { type, preventDefault() {} });
  }
  getContext(kind) {
    if (kind !== '2d') return null;
    if (!this._ctx) this._ctx = new Ctx2D(this);
    return this._ctx;
  }
  querySelector(sel) { return this.querySelectorAll(sel)[0] || null; }
  querySelectorAll(sel) {
    const out = [];
    const parts = sel.split(',').map((s) => s.trim()).filter(Boolean);
    const walk = (n) => {
      for (const c of n.childNodes) {
        if (c.nodeType !== 1) continue;
        for (const p of parts) if (matches(c, p)) { out.push(c); break; }
        walk(c);
      }
    };
    walk(this);
    return out;
  }
  select() { this.selected = true; }
  setSelectionRange() {}
}

function owns(obj, k) { return Object.prototype.hasOwnProperty.call(obj, k); }

/* supports tag, #id, .class and [attr] / [attr="v"] in any combination */
function matches(el, sel) {
  if (sel === '*') return true;
  const tm = /^([a-zA-Z][\w-]*)?/.exec(sel);
  let rest = tm ? tm[0] : '';
  if (tm && tm[1] && el.tagName !== tm[1].toUpperCase()) return false;
  rest = sel.slice(rest.length);
  let i = 0;
  while (i < rest.length) {
    const ch = rest.charAt(i);
    if (ch === '#') {
      let j = i + 1;
      while (j < rest.length && /[\w-]/.test(rest.charAt(j))) j++;
      if (el.getAttribute('id') !== rest.slice(i + 1, j)) return false;
      i = j;
    } else if (ch === '.') {
      let j = i + 1;
      while (j < rest.length && /[\w-]/.test(rest.charAt(j))) j++;
      if (!el.classList.contains(rest.slice(i + 1, j))) return false;
      i = j;
    } else if (ch === '[') {
      const end = rest.indexOf(']', i);
      if (end < 0) return false;
      const body = rest.slice(i + 1, end);
      const am = /^([\w-]+)(?:=("?)([\s\S]*?)\2)?$/.exec(body);
      if (!am) return false;
      const v = el.getAttribute(am[1]);
      if (am[3] === undefined) { if (v === null) return false; }
      else if (v !== am[3]) return false;
      i = end + 1;
    } else return false;
  }
  return true;
}

class Doc extends Element {
  constructor() {
    super('#document');
    this.readyState = 'loading';
    this.execCalls = [];
  }
  get body() { return this.querySelector('body'); }
  get documentElement() { return this.querySelector('html'); }
  createElement(tag) { return new Element(tag); }
  createTextNode(v) { return new Text(v); }
  getElementById(id) { return this.querySelector('[id="' + id + '"]'); }
  execCommand(cmd) { this.execCalls.push(cmd); return true; }
  dispatchEvent(a, b) {
    const ev = typeof a === 'string' ? (b || { type: a }) : a;
    this.dispatch(ev.type, ev);
    return true;
  }
}

/* ---------------- build the sandbox ---------------- */

function parseAttrs(str) {
  const attrs = {};
  const re = /([a-zA-Z-]+)(?:="([^"]*)")?/g;
  let m;
  while ((m = re.exec(str))) attrs[m[1]] = m[2] === undefined ? '' : m[2];
  return attrs;
}

function elFrom(tag, attrs = {}, text = null) {
  const e = new Element(tag);
  for (const k of Object.keys(attrs)) e.setAttribute(k, attrs[k]);
  if (attrs.class) e.classList = new ClassList(e);
  if (attrs.checked !== undefined) e.checked = true;
  if (text !== null) e.textContent = text;
  return e;
}

function html(str) {
  const doc = new Doc();
  const stack = [doc];
  const re = /<\/?([a-zA-Z0-9]+)((?:\s+[a-zA-Z-]+(?:="[^"]*")?)*)\s*(\/?)>|([^<]+)/g;
  let m;
  while ((m = re.exec(str))) {
    const top = stack[stack.length - 1];
    if (m[4] !== undefined) {
      const t = m[4];
      if (t.trim() === '' || /^\s+$/.test(t)) {
        if (t.indexOf('\n') >= 0) continue;
        top.appendChild(new Text(' '));
      } else top.appendChild(new Text(t));
      continue;
    }
    const tag = m[1];
    if (m[0][1] === '/') { if (stack.length > 1) stack.pop(); continue; }
    const e = elFrom(tag, parseAttrs(m[2] || ''));
    top.appendChild(e);
    if (!m[3] && !/^(br|hr|img|input|meta|link)$/.test(tag)) stack.push(e);
  }
  return doc;
}

const document = html(`
<body>
  <p>Inline <span class="math" id="m1">\\frac{a}{b}</span> and
     <span class="math" id="m2">x^{2}_{i}</span>.</p>
  <div class="math-display" id="m3">\\sum_{i=1}^{n} i</div>
  <div class="math" id="m4">\\badcmd{x}</div>
  <pre id="code1">let x = 1;</pre>
  <div class="codeblock"><pre id="code2">let y = 2;</pre>
    <button data-copy-target="code1" id="btn1">copy</button>
    <button data-copy-target="missing" id="btn2">copy</button>
  </div>
  <span data-var-readout="amp">0</span>
  <input type="range" data-var="amp" min="0" max="10" step="0.1" value="2" id="amp">
  <input type="checkbox" data-var="flag" id="flag">
  <select data-var="mode" id="mode"><option value="a">Alpha</option><option value="b" selected>Beta</option></select>
  <canvas class="live" id="cv1" data-var="amp" data-formula="amp*sin(x)"
          data-var-name="x" data-xmin="0" data-xmax="6.28" data-points="8"
          data-ymin="-3" data-ymax="3" data-labels="sine" data-xlabel="t" data-ylabel="A"></canvas>
  <canvas class="live" id="cv2" data-var="amp" data-formula="amp*x;amp*cos(x)"
          data-var-name="x" data-xmin="0" data-xmax="1" data-points="4" data-labels="lin;cos"></canvas>
</body>`);

/* simulate layout for the canvases (the shim has no CSS engine) */
for (const id of ['cv1', 'cv2']) {
  const c = document.getElementById(id);
  c.clientWidth = 480;
  c.clientHeight = 260;
}

let rafQueue = [];
const timers = [];
const clip = [];

const sandbox = {
  console,
  document,
  navigator: { clipboard: { writeText: (t) => { clip.push(t); return Promise.resolve(); } } },
  devicePixelRatio: 2,
  setTimeout: (fn, ms) => { timers.push({ fn, ms }); return timers.length; },
  clearTimeout: () => {},
  Promise,
  Math,
  JSON,
  Object,
  Array,
  Number,
  String,
  Boolean,
  isFinite,
  parseFloat,
  parseInt,
  Error,
};
sandbox.window = sandbox;
sandbox.globalThis = sandbox;
sandbox.requestAnimationFrame = (fn) => { rafQueue.push(fn); return rafQueue.length; };
sandbox.setInterval = () => 0;

vm.createContext(sandbox);

/* ---------------- load both files, concatenated ---------------- */

const texSrc = fs.readFileSync(TEX, 'utf8');
const rtSrc = fs.readFileSync(RT, 'utf8');
const bundle = texSrc + '\n' + rtSrc;
try {
  vm.runInContext(bundle, sandbox, { filename: 'nano-tex+runtime.js' });
} catch (e) {
  console.error('bundle threw at load: ' + e.stack);
  process.exit(1);
}

const NanoTex = sandbox.NanoTex;
const Nano = sandbox.Nano;

/* ---------------- assertions ---------------- */

let checks = 0;
const fails = [];

function ok(cond, msg) {
  checks++;
  if (!cond) fails.push(msg);
}
function eq(actual, expected, msg) {
  checks++;
  if (!(Object.is(actual, expected) || (Number.isNaN(actual) && Number.isNaN(expected)))) {
    fails.push(msg + '  (got ' + String(actual) + ', want ' + String(expected) + ')');
  }
}
function has(hay, needle, msg) { ok(String(hay).indexOf(needle) >= 0, msg + ' :: missing ' + JSON.stringify(needle) + ' in ' + JSON.stringify(String(hay).slice(0, 300))); }
function hasClass(hay, name, msg) {
  const h = String(hay);
  ok(h.indexOf('class="' + name + '"') >= 0 || h.indexOf('class="' + name + ' ') >= 0,
    msg + ' :: no class "' + name + '" in ' + JSON.stringify(h.slice(0, 300)));
}
function hasNot(hay, needle, msg) { ok(String(hay).indexOf(needle) < 0, msg + ' :: unexpected ' + JSON.stringify(needle)); }
function near(a, b, tol, msg) { ok(Math.abs(a - b) <= (tol === undefined ? 1e-9 : tol), msg + '  (got ' + a + ', want ~' + b + ')'); }

function run(name, fn) {
  const before = fails.length;
  try { fn(); } catch (e) { fails.push(name + ' threw: ' + (e.stack || e.message)); }
  if (fails.length === before) console.log('  ok  ' + name);
}

/* serialize a shim node back to html, to prove render == renderToString */
function escAttr(s) {
  return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;').replace(/'/g, '&#39;');
}
function serialize(n) {
  if (n.nodeType === 3) return String(n.data).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  let out = '<' + n.tagName.toLowerCase();
  const cls = n.getAttribute('class');
  if (cls) out += ' class="' + escAttr(cls) + '"';
  if (n.getAttribute('style')) out += ' style="' + escAttr(n.getAttribute('style')) + '"';
  out += '>';
  for (const c of n.childNodes) out += serialize(c);
  return out + '</' + n.tagName.toLowerCase() + '>';
}

/* -- tex -- */
run('tex: api', () => {
  ok(NanoTex && typeof NanoTex.render === 'function', 'render is a function');
  ok(typeof NanoTex.renderToString === 'function', 'renderToString is a function');
  ok(Nano && Nano.TeX === NanoTex, 'Nano.TeX === NanoTex');
});

run('tex: fraction markup', () => {
  const h = NanoTex.renderToString('\\frac{a}{b}');
  has(h, 'class="frac"', 'frac');
  has(h, 'class="num"', 'num');
  has(h, 'class="den"', 'den');
  has(h, 'class="bar"', 'bar');
  has(h, '>a<', 'numerator text');
  has(h, '>b<', 'denominator text');
});

run('tex: sub and superscript together', () => {
  const h = NanoTex.renderToString('x^{2}_{i}');
  has(h, '<sup', 'sup element');
  has(h, '<sub', 'sub element');
  has(h, 'class="scripts"', 'scripts wrapper');
  has(h, '>2<', 'superscript text');
  has(h, '>i<', 'subscript text');
});

run('tex: big operator limits in display mode', () => {
  const h = NanoTex.renderToString('\\sum_{i=1}^{n} i', { display: true });
  has(h, 'class="lim-under"', 'lim-under');
  has(h, 'class="lim-over"', 'lim-over');
  has(h, 'class="op"', 'op');
  has(h, '∑', 'sum glyph');
  has(h, '>i<', 'lower limit text');
  has(h, '>n<', 'upper limit text');
  const inline = NanoTex.renderToString('\\sum_{i=1}^{n} i');
  hasNot(inline, 'lim-under', 'inline mode uses scripts, not limits');
  has(inline, 'class="scripts"', 'inline scripts');
});

run('tex: unknown command is marked unk', () => {
  const h = NanoTex.renderToString('\\foo{x}');
  has(h, 'class="unk"', 'unk class');
  has(h, '#b23', 'unk colour');
  has(h, '\\foo', 'literal command kept');
  has(h, '>x<', 'following text survives');
  const h2 = NanoTex.renderToString('\\begin{unknownenv}a\\end{unknownenv}');
  has(h2, 'class="unk"', 'unknown environment unk');
  has(h2, '>a<', 'unknown environment body survives');
});

run('tex: pmatrix builds a matrix', () => {
  const h = NanoTex.renderToString('\\begin{pmatrix}a&b\\\\c&d\\end{pmatrix}');
  has(h, 'class="matrix', 'matrix wrapper');
  has(h, 'matrix-pmatrix', 'matrix kind');
  hasClass(h, 'mgrid', 'grid');
  hasClass(h, 'mrow', 'rows');
  has(h, 'repeat(2,auto)', 'two columns');
  has(h, '>a<', 'cell a');
  has(h, '>d<', 'cell d');
  const c = NanoTex.renderToString('\\begin{cases}1 & x>0\\\\0 & x\\le 0\\end{cases}');
  has(c, 'matrix-cases', 'cases kind');
  hasNot(c, 'mbrace-r', 'cases has no right brace');
});

run('tex: html escaping', () => {
  const h = NanoTex.renderToString('\\text{a & b <script>alert(1)</script> \\frac{x}{y}}');
  has(h, '&amp;', 'ampersand escaped');
  hasNot(h, '<script>', 'script tag never unescaped');
  hasNot(h, 'alert(1)</script>', 'raw script close never present');
  has(h, '&lt;script&gt;', 'escaped script');
  const d = NanoTex.render('\\text{<img src=x onerror=alert(1)>}');
  eq(d.getAttribute('class'), 'tex', 'dom: root class');
  eq(d.childNodes.length, 1, 'dom: single child');
  eq(d.childNodes[0].nodeType, 1, 'dom: child is an element');
  eq(d.textContent, '<img src=x onerror=alert(1)>', 'dom: text is unescaped in the DOM');
});

run('tex: display markup', () => {
  has(NanoTex.renderToString('\\sqrt[3]{x+1}'), 'class="sqrt"', 'sqrt');
  has(NanoTex.renderToString('\\sqrt[3]{x+1}'), 'class="idx"', 'sqrt index');
  has(NanoTex.renderToString('\\sqrt[3]{x+1}'), 'class="rad"', 'radical');
  has(NanoTex.renderToString('\\left(\\frac{a}{b}\\right)'), 'big', 'grown delimiter');
  has(NanoTex.renderToString('\\mathbb{R}'), 'class="bb"', 'mathbb');
  has(NanoTex.renderToString('\\mathcal{L}'), 'class="cal"', 'mathcal');
  hasClass(NanoTex.renderToString('\\hat{x}'), 'acc', 'accent');
  hasClass(NanoTex.renderToString('a\\,b'), 'space', 'explicit spacing');
  hasClass(NanoTex.renderToString('\\sin x'), 'opname', 'upright function name');
  hasClass(NanoTex.renderToString('\\alpha'), 'mi', 'greek identifier');
  hasClass(NanoTex.renderToString('12'), 'mn', 'number');
  hasClass(NanoTex.renderToString('a+b'), 'mo', 'operator');
  has(NanoTex.renderToString('\\begin{align}a&=b\\\\c&=d\\end{align}'), 'al-r', 'align right cell');
  has(NanoTex.renderToString('\\begin{equation}E=mc^2\\end{equation}'), 'env-equation', 'equation env');
  has(NanoTex.renderToString("f'"), 'class="prime"', 'prime');
});

run('tex: string markup matches the dom tree', () => {
  const cases = [
    '\\frac{a}{b}',
    'x^{2}_{i}',
    '\\sum_{i=1}^{n} i',
    '\\sqrt[3]{1+x}',
    '\\begin{pmatrix}a&b\\\\c&d\\end{pmatrix}',
    '\\begin{align}a&=b\\\\c&=d\\end{align}',
    '\\begin{cases}1 & x>0\\\\0 & x\\le 0\\end{cases}',
    '\\text{a < b & c}',
    '\\badcmd{x}',
    '\\left(\\frac{\\frac{a}{b}}{c}\\right)',
    '\\hat{x} \\bar{y} \\vec{v} \\overline{ab}',
    '\\mathbb{R} \\mathcal{L} \\mathbf{v} \\mathrm{d}x',
    "f'' \\int_0^\\infty e^{-x^2}\\,dx",
    '\\operatorname{tr}(A) \\log_2 n \\neq a'
  ];
  for (const src of cases) {
    for (const display of [false, true]) {
      const a = NanoTex.renderToString(src, { display });
      const b = serialize(NanoTex.render(src, { display }));
      eq(b, a, 'markup identical for ' + JSON.stringify(src) + ' display=' + display);
    }
  }
});

run('tex: render and renderToString agree', () => {
  const src = '\\frac{x_{i}^{2}}{\\sqrt{1+y}}';
  const html = NanoTex.renderToString(src);
  const dom = NanoTex.render(src);
  eq(dom.textContent, 'xi2\u221a1+y', 'searchable text content');
  const strip = (s) => s.replace(/<[^>]*>/g, '');
  eq(strip(html), dom.textContent, 'string and dom carry the same text');
});

/* -- evaluator -- */
run('evaluator: arithmetic and functions', () => {
  const ev = (s, x, v) => Nano.evalFormula(s, x === undefined ? 0 : x, v);
  eq(ev('2x', 3), 6, '2x');
  eq(ev('3sin(0)'), 0, '3sin(0)');
  eq(ev('2^3^2'), 512, '2^3^2 right associative');
  eq(ev('-(2+3)*2'), -10, '-(2+3)*2');
  eq(ev('abs(-4)'), 4, 'abs(-4)');
  eq(ev('max(1,2)'), 2, 'max(1,2)');
  ok(Number.isNaN(ev('foo(1)')), 'foo(1) is NaN');
  ok(Number.isNaN(ev('unknownvar')), 'unknown identifier is NaN');
  eq(ev('3(x+1)', 2), 9, 'implicit multiplication with parens');
  eq(ev('4/2x', 3), 6, '4/2x means (4/2)*x');
  eq(ev('-x^2', 3), -9, 'unary minus binds looser than ^');
  eq(ev('2*3^2'), 18, 'exponent before multiply');
  eq(ev('pi'), Math.PI, 'pi constant');
  eq(ev('e'), Math.E, 'e constant');
  near(ev('sin(pi/2)'), 1, 1e-12, 'sin(pi/2)');
  near(ev('ln(e)'), 1, 1e-12, 'ln(e)');
  near(ev('log10(1000)'), 3, 1e-12, 'log10(1000)');
  near(ev('log2(8)'), 3, 1e-12, 'log2(8)');
  eq(ev('min(3,1,2)'), 1, 'min with three args');
  eq(ev('clamp(5,0,1)'), 1, 'clamp');
  eq(ev('sign(-2)'), -1, 'sign');
  eq(ev('signum(2)'), 1, 'signum');
  eq(ev('floor(1.7)'), 1, 'floor');
  eq(ev('ceil(1.2)'), 2, 'ceil');
  eq(ev('round(1.5)'), 2, 'round');
  eq(ev('trunc(-1.7)'), -1, 'trunc');
  eq(ev('pow(2,10)'), 1024, 'pow');
  near(ev('hypot(3,4)'), 5, 1e-12, 'hypot');
  eq(ev('x*2', 5), 10, 'named variable x');
  eq(ev('t*2', 5, 't'), 10, 'named variable t');
  eq(ev('7%3'), 1, 'modulo');
  ok(Number.isNaN(ev('2+')), 'malformed expression is NaN');
  ok(Number.isNaN(ev('(1+2')), 'unbalanced parens is NaN');
  ok(Number.isNaN(ev('')), 'empty expression is NaN');
});

/* -- runtime wiring -- */
function fireInput(el) { el.dispatch('input', { type: 'input', preventDefault() {} }); }
function fireDOMReady() { document.readyState = 'interactive'; document.dispatchEvent('DOMContentLoaded', { type: 'DOMContentLoaded' }); }
function flushFrames(n) { for (let i = 0; i < (n || 1); i++) { const q = rafQueue; rafQueue = []; for (const f of q) f(); } }

run('runtime: api surface', () => {
  for (const k of ['register', 'redrawAll', 'setVar', 'getVar', 'on']) {
    ok(typeof Nano[k] === 'function', 'Nano.' + k + ' is a function');
  }
});

run('runtime: typesets math on load', () => {
  fireDOMReady();
  const m1 = document.getElementById('m1');
  eq(m1.getAttribute('data-tex'), '\\frac{a}{b}', 'data-tex preserved');
  ok(m1.querySelector('.tex') !== null, 'rendered .tex root');
  eq(m1.querySelector('.frac').getAttribute('class'), 'frac', 'fraction node in the dom');
  eq(m1.textContent, 'ab', 'selectable text');
  const m3 = document.getElementById('m3');
  ok(m3.querySelector('.tex-display') !== null, 'display class');
  ok(m3.querySelector('.lim-under') !== null, 'display limits');
  const m4 = document.getElementById('m4');
  ok(m4.querySelector('.unk') !== null, 'unknown command marked in the dom');
});

run('runtime: copy buttons', () => {
  const btn1 = document.getElementById('btn1');
  eq(btn1.textContent, 'copy', 'initial label');
  btn1.dispatch('click', { type: 'click', preventDefault() {} });
  eq(clip[clip.length - 1], 'let x = 1;', 'copied the target text');
  eq(btn1.textContent, 'copied', 'label swapped');
  const pending = timers.filter((t) => t.ms === 1200);
  eq(pending.length, 1, 'restore timer scheduled');
  pending[pending.length - 1].fn();
  eq(btn1.textContent, 'copy', 'label restored');
  const btn2 = document.getElementById('btn2');
  btn2.dispatch('click', { type: 'click', preventDefault() {} });
  eq(clip[clip.length - 1], 'let y = 2;', 'falls back to the sibling pre');
  /* clipboard unavailable -> execCommand path */
  const nav = sandbox.navigator;
  const saved = nav.clipboard;
  nav.clipboard = undefined;
  document.execCalls.length = 0;
  btn1.dispatch('click', { type: 'click', preventDefault() {} });
  has(document.execCalls.join(','), 'copy', 'execCommand fallback used');
  nav.clipboard = saved;
});

run('runtime: live plots', () => {
  const cv1 = document.getElementById('cv1');
  const ctx = cv1.getContext('2d');
  ok(ctx.calls.length > 0, 'canvas was painted');
  has(ctx.calls.map((c) => c[0]).join(','), 'clearRect', 'cleared before painting');
  has(ctx.calls.map((c) => c[0]).join(','), 'fillText', 'tick labels drawn');
  has(ctx.calls.map((c) => c[0]).join(','), 'stroke', 'series drawn');
  const labels = ctx.calls.filter((c) => c[0] === 'fillText').map((c) => c[1]);
  has(labels.join('|'), 't', 'x axis title');
  has(labels.join('|'), 'A', 'y axis title');
  has(labels.join('|'), '-2', 'y tick from the explicit data-ymin/data-ymax range');
  hasNot(labels.join('|'), 'sine', 'single series draws no legend');
  eq(cv1.width, Math.round(480 * 2), 'device pixel ratio width');
  eq(cv1.height, Math.round(260 * 2), 'device pixel ratio height');
  const cv2 = document.getElementById('cv2');
  const c2 = cv2.getContext('2d').calls;
  const l2 = c2.filter((c) => c[0] === 'fillText').map((c) => c[1]).join('|');
  has(l2, 'lin', 'first series legend label');
  has(l2, 'cos', 'second series legend label');
  has(c2.map((c) => c[0]).join(','), 'arc', 'markers drawn for small sample counts');
  /* programmatic register */
  const cv3 = document.createElement('canvas');
  cv3.setAttribute('class', 'live');
  document.body.appendChild(cv3);
  const rec = Nano.register(cv3, { var: 'amp', formula: 'sin(x)', varName: 'x' });
  ok(rec !== null && rec !== undefined, 'register returns a record');
  ok(cv3.getContext('2d').calls.length > 0, 'programmatic plot painted');
});

run('runtime: controls, variables, readouts', () => {
  const amp = document.getElementById('amp');
  const flag = document.getElementById('flag');
  const mode = document.getElementById('mode');
  const out = document.querySelector('[data-var-readout="amp"]');
  eq(Nano.getVar('amp'), 2, 'startup sync called setVar');
  eq(out.textContent, '2', 'numeric readout');
  ok(Nano.getVar('flag') === false, 'checkbox synced as false');
  eq(Nano.getVar('mode'), 'b', 'select synced to its value');
  const seen = [];
  Nano.on('amp', (v) => seen.push(v));
  amp.value = '4.5';
  fireInput(amp);
  eq(Nano.getVar('amp'), 4.5, 'setVar from range input');
  eq(seen[seen.length - 1], 4.5, 'subscriber notified');
  eq(out.textContent, '4.5', 'readout updated');
  eq(rafQueue.length, 1, 'redraw batched into one frame');
  flushFrames(1);
  eq(rafQueue.length, 0, 'frame queue drained');
  flag.checked = true;
  flag.dispatch('change', { type: 'change', preventDefault() {} });
  eq(Nano.getVar('flag'), true, 'checkbox change');
  mode.value = 'a';
  mode.dispatch('change', { type: 'change', preventDefault() {} });
  eq(Nano.getVar('mode'), 'a', 'select change');
  amp.value = '7';
  fireInput(amp);
  amp.value = '8';
  fireInput(amp);
  eq(rafQueue.length, 1, 'slider drag still batches to one frame');
  flushFrames(1);
  eq(out.textContent, '8', 'readout after drag');
  Nano.redrawAll();
  ok(document.getElementById('cv1').getContext('2d').calls.length > 0, 'redrawAll repaints');
});

run('runtime: value formatting', () => {
  eq(Nano.fmt(1.234), '1.234', '1.234');
  eq(Nano.fmt(12.34), '12.34', '12.34');
  eq(Nano.fmt(123.4), '123.4', '123.4');
  eq(Nano.fmt(0.000123), '0.000123', '0.000123');
  eq(Nano.fmt(42), '42', 'integer stays integer');
  eq(Nano.fmt(3.14159), '3.142', 'four significant digits');
  eq(Nano.fmt(true), 'true', 'boolean');
  eq(Nano.fmt('text'), 'text', 'string');
  eq(Nano.fmt(NaN), 'NaN', 'NaN');
});

run('runtime: never throws', () => {
  Nano.setVar('', null);
  Nano.setVar('nothing', 1);
  eq(Nano.getVar('unknown-var'), 0, 'unknown variable reads as 0');
  Nano.on('x', 'not a function');
  ok(Number.isNaN(Nano.evalFormula('$$$', 1)), 'garbage formula is NaN, not a throw');
  ok(Nano.register(null, null) === null, 'register(null) is safe');
  const bad = document.createElement('canvas');
  bad.setAttribute('class', 'live');
  bad.setAttribute('data-formula', 'sin(');
  ok(Nano.register(bad, null) !== null, 'broken formula still registers a canvas');
  bad.getContext('2d');
  Nano.redrawAll();
});

/* ---------------- report ---------------- */

console.log('');
if (fails.length) {
  console.log('FAILED ' + fails.length + ' of ' + checks + ' checks:');
  for (const f of fails) console.log('  - ' + f);
  process.exit(1);
}
console.log(checks + ' checks passed');
console.log('OK');
