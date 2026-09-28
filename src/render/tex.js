/* tex.js - nano: LaTeX-to-DOM typesetter (inline browser runtime) */
(function (global) {
  'use strict';

  var RE_L = /[A-Za-z]/;
  var RE_D = /[0-9]/;
  var PRIME = '′';
  var PRIME2 = '″';
  var UNK = 'color:#b23';

  /* ---------------- node model ----------------
     text   : { t:'x', v:'...' }
     element: { t:'e', n:'span', c:'class', s:'style', k:[...] }          */

  function tx(v) { return { t: 'x', v: String(v) }; }
  function el(n, c, k, s) { return { t: 'e', n: n, c: c || '', k: k || [], s: s || '' }; }
  function one(ns) { return !ns.length ? null : ns.length === 1 ? ns[0] : grp(ns); }
  function grp(ns) { return el('span', 'grp', ns); }
  function hasc(n, c) { return (' ' + n.c + ' ').indexOf(' ' + c + ' ') >= 0; }
  function add(dst, ns) { for (var i = 0; i < ns.length; i++) dst.push(ns[i]); return dst; }

  function lvl(n) {
    if (!n || n.t !== 'e') return 0;
    var m = 0;
    for (var i = 0; i < n.k.length; i++) { var l = lvl(n.k[i]); if (l > m) m = l; }
    if (hasc(n, 'frac') || hasc(n, 'sqrt') || hasc(n, 'matrix') || hasc(n, 'tex-align') || hasc(n, 'tex-env')) return m + 1;
    if (hasc(n, 'opbig') || hasc(n, 'acc') || hasc(n, 'scripts')) return m > 0 ? m : 1;
    return m;
  }
  function maxLvl(ns) { var m = 0; for (var i = 0; i < ns.length; i++) { var l = lvl(ns[i]); if (l > m) m = l; } return m; }

  /* ---------------- serializers ---------------- */

  function esc(s) {
    return String(s)
      .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;').replace(/'/g, '&#39;');
  }

  function toHtml(n) {
    if (!n) return '';
    if (n.t === 'x') return esc(n.v);
    var s = '<' + n.n, i, out = '';
    if (n.c) s += ' class="' + esc(n.c) + '"';
    if (n.s) s += ' style="' + esc(n.s) + '"';
    if (!n.k.length) return s + '></' + n.n + '>';
    for (i = 0; i < n.k.length; i++) out += toHtml(n.k[i]);
    return s + '>' + out + '</' + n.n + '>';
  }

  function toDom(n, d) {
    if (!n) return d.createTextNode('');
    if (n.t === 'x') return d.createTextNode(n.v);
    var e = d.createElement(n.n);
    if (n.c) e.setAttribute('class', n.c);
    if (n.s) e.setAttribute('style', n.s);
    for (var i = 0; i < n.k.length; i++) e.appendChild(toDom(n.k[i], d));
    return e;
  }

  /* ---------------- symbol tables ---------------- */

  var GREEK = {
    alpha: 'α', beta: 'β', gamma: 'γ', delta: 'δ', epsilon: 'ϵ', varepsilon: 'ε',
    zeta: 'ζ', eta: 'η', theta: 'θ', vartheta: 'ϑ', iota: 'ι', kappa: 'κ',
    lambda: 'λ', mu: 'μ', nu: 'ν', xi: 'ξ', pi: 'π', rho: 'ρ', sigma: 'σ',
    tau: 'τ', upsilon: 'υ', phi: 'ϕ', varphi: 'φ', chi: 'χ', psi: 'ψ', omega: 'ω',
    digamma: 'ϝ', varpi: 'ϖ', varrho: 'ϱ', varsigma: 'ς', epsilon1: 'ε'
  };
  var GREEK_UP = {
    Gamma: 'Γ', Delta: 'Δ', Theta: 'Θ', Lambda: 'Λ', Xi: 'Ξ', Pi: 'Π',
    Sigma: 'Σ', Upsilon: 'Υ', Phi: 'Φ', Psi: 'Ψ', Omega: 'Ω'
  };
  var REL = {
    ne: '≠', neq: '≠', le: '≤', leq: '≤', ge: '≥', geq: '≥', approx: '≈',
    equiv: '≡', sim: '∼', simeq: '≃', cong: '≅', propto: '∝', subset: '⊂',
    supset: '⊃', subseteq: '⊆', supseteq: '⊇', in: '∈', notin: '∉', ni: '∋',
    to: '→', rightarrow: '→', gets: '←', leftarrow: '←', mapsto: '↦',
    Rightarrow: '⇒', implies: '⇒', iff: '⟺', Leftrightarrow: '⟺',
    leftrightarrow: '↔', perp: '⊥', parallel: '∥', mid: '∣', doteq: '≐',
    ll: '≪', gg: '≫', prec: '≺', succ: '≻', preceq: '⪯', succeq: '⪰',
    asymp: '≍', nsim: '≁', models: '⊨', vdash: '⊢', bowtie: '⋈', nmid: '∤',
    nparallel: '∦', nsubseteq: '⊈', ngeq: '≱', nleq: '≰', iff1: '⟺'
  };
  var OPS = {
    pm: '±', mp: '∓', times: '×', div: '÷', cdot: '⋅', ast: '∗', star: '⋆',
    circ: '∘', bullet: '•', oplus: '⊕', ominus: '⊖', otimes: '⊗',
    oslash: '⊘', odot: '⊙', cap: '∩', cup: '∪', wedge: '∧', land: '∧',
    vee: '∨', lor: '∨', neg: '¬', lnot: '¬', emptyset: '∅',
    varnothing: '∅', forall: '∀', exists: '∃', nexists: '∄', top: '⊤',
    bot: '⊥', dagger: '†', ddagger: '‡', dots: '…', ldots: '…', sdot: '⋅',
    colon: ':', ratio: '∕', slash: '/', backslash: '∖', uplus: '⊎',
    sqcup: '⊔', veebar: '⊻', barwedge: '⊼', triangleq: '≜', circeq: '≗',
    bumpeq: '≎', cupdot: '⨿', pitchfork: '⋔', dotplus: '∔'
  };
  var MISC = {
    infty: '∞', partial: '∂', nabla: '∇', cdots: '⋯', vdots: '⋮',
    ddots: '⋱', hbar: 'ℏ', ell: 'ℓ', Re: 'ℜ', Im: 'ℑ', angle: '∠',
    degree: '°', prime: PRIME, ellipsis: '…', surd: '√', flat: '♭',
    natural: '♮', sharp: '♯', clubsuit: '♣', diamondsuit: '♢', heartsuit: '♥',
    spadesuit: '♠', checkmark: '✓', therefore: '∴', because: '∵', complement: '∁'
  };
  var OPNAME = {
    sin: 'sin', cos: 'cos', tan: 'tan', cot: 'cot', sec: 'sec', csc: 'csc',
    arcsin: 'arcsin', arccos: 'arccos', arctan: 'arctan', arccot: 'arccot',
    arccsc: 'arccsc', arcsec: 'arcsec', sinh: 'sinh', cosh: 'cosh',
    tanh: 'tanh', coth: 'coth', exp: 'exp', log: 'log', ln: 'ln', lg: 'lg',
    det: 'det', dim: 'dim', ker: 'ker', deg: 'deg', gcd: 'gcd', arg: 'arg',
    hom: 'hom', Pr: 'Pr', injlim: 'inj lim'
  };
  var BIGOP = {
    sum: '∑', prod: '∏', coprod: '∐', int: '∫', iint: '∬', iiint: '∭',
    oint: '∮', bigcup: '⋃', bigcap: '⋂', bigvee: '⋁', bigwedge: '⋀',
    bigoplus: '⨁', bigotimes: '⨂', bigsqcup: '⨆', lim: 'lim',
    limsup: 'lim sup', liminf: 'lim inf', max: 'max', min: 'min',
    sup: 'sup', inf: 'inf', argmax: 'arg max', argmin: 'arg min'
  };
  var LIMLIKE = { lim: 1, limsup: 1, liminf: 1, max: 1, min: 1, sup: 1, inf: 1, argmax: 1, argmin: 1 };
  var ACCENT_MARK = {
    hat: '̂', widehat: '̂', bar: '̄', vec: '⃗', dot: '̇', ddot: '̈',
    tilde: '̃', widetilde: '̃', mathring: '̊', breve: '̆', check: '̌',
    acute: '́', grave: '̀', overrightarrow: '⟶', overleftarrow: '⟵',
    overbrace: null, underbrace: null
  };
  var OVERLINE = { overline: 1, underline: 1 };
  var CSP = {
    ',': ['sp-thin', 0.167], ':': ['sp-med', 0.222], ';': ['sp-thick', 0.278],
    '>': ['sp-thick', 0.278], '!': ['sp-neg', -0.167], ' ': ['sp-norm', 0.333]
  };
  var DELIM = {
    '{': '{', '}': '}', langle: '⟨', rangle: '⟩', Vert: '‖', vert: '|',
    backslash: '∖', uparrow: '↑', downarrow: '↓', lfloor: '⌊', rfloor: '⌋',
    lceil: '⌈', rceil: '⌉', upharpoonright: '↾', upharpoonleft: '↼'
  };
  var DELIMCHAR = { '(': '(', ')': ')', '[': '[', ']': ']', '|': '|', '/': '/', '.': '', '<': '⟨', '>': '⟩' };
  var BB = Array.from('𝔸𝔹ℂ𝔻𝔼𝔽𝔾ℍ𝕀𝕁𝕂𝕃𝕄ℕ𝕆ℙℚℝ𝕊𝕋𝕌𝕍𝕎𝕏𝕐ℤ');
  var SERIF = "Georgia,'Times New Roman',serif";
  var SANS = "Helvetica,Arial,'Segoe UI','Liberation Sans',sans-serif";
  var MONO = "Menlo,Consolas,'Liberation Mono',monospace";

  /* ---------------- tokenizer ---------------- */

  function tokenize(src) {
    var out = [], i = 0, n = src.length, j, c, d, name, num;
    while (i < n) {
      c = src.charAt(i);
      if (c === '\\') {
        d = src.charAt(i + 1);
        if (d === '') { out.push({ k: 'cs', v: '', s: i, e: i + 1 }); i++; continue; }
        if (RE_L.test(d)) {
          j = i + 1; name = '';
          while (j < n && RE_L.test(src.charAt(j))) { name += src.charAt(j); j++; }
          out.push({ k: 'cmd', v: name, s: i, e: j }); i = j; continue;
        }
        j = i + 2;
        out.push(d === '\\' ? { k: 'cmd', v: '\\', s: i, e: j } : { k: 'cs', v: d, s: i, e: j });
        i = j; continue;
      }
      if (c === '{') { out.push({ k: 'lb', s: i, e: i + 1 }); i++; continue; }
      if (c === '}') { out.push({ k: 'rb', s: i, e: i + 1 }); i++; continue; }
      if (c === '^') { out.push({ k: 'sup', s: i, e: i + 1 }); i++; continue; }
      if (c === '_') { out.push({ k: 'sub', s: i, e: i + 1 }); i++; continue; }
      if (c === "'") { out.push({ k: 'prime', s: i, e: i + 1 }); i++; continue; }
      if (c === '&') { out.push({ k: 'amp', s: i, e: i + 1 }); i++; continue; }
      if (c === '~') { out.push({ k: 'nbsp', s: i, e: i + 1 }); i++; continue; }
      if (c === ' ' || c === '\t' || c === '\n' || c === '\r') { out.push({ k: 'sp', s: i, e: i + 1 }); i++; continue; }
      if (RE_D.test(c)) {
        num = ''; j = i;
        while (j < n && RE_D.test(src.charAt(j))) { num += src.charAt(j); j++; }
        if (src.charAt(j) === '.' && RE_D.test(src.charAt(j + 1) || '')) {
          num += '.'; j++;
          while (j < n && RE_D.test(src.charAt(j))) { num += src.charAt(j); j++; }
        }
        out.push({ k: 'num', v: num, s: i, e: j }); i = j; continue;
      }
      if (RE_L.test(c)) {
        j = i; name = '';
        while (j < n && RE_L.test(src.charAt(j))) { name += src.charAt(j); j++; }
        out.push({ k: 'id', v: name, s: i, e: j }); i = j; continue;
      }
      out.push({ k: 'chr', v: c, s: i, e: i + 1 }); i++;
    }
    return out;
  }

  /* ---------------- parser state ---------------- */

  var p = null;
  var ST = { up: false, bf: false, it: false, bb: false, cal: false, sf: false, tt: false };

  function st(over) {
    var c = {}, k;
    for (k in ST) c[k] = ST[k];
    for (k in over) c[k] = over[k];
    return c;
  }
  function inSt(over, fn) { var old = p.st; p.st = st(over); var r = fn(); p.st = old; return r; }

  function isRb(t) { return t.k === 'rb'; }
  function isScript(t) { return t.k === 'sup' || t.k === 'sub' || t.k === 'rb' || t.k === 'amp' || (t.k === 'cmd' && t.v === '\\'); }
  function isRight(t) { return t.k === 'cmd' && t.v === 'right'; }
  function isEnd(t) { return t.k === 'cmd' && t.v === 'end'; }
  function isRowEnd(t) { return t.k === 'cmd' && t.v === '\\'; }
  function isAmp(t) { return t.k === 'amp'; }
  function noStop() { return false; }

  function parseUntil(stop) {
    var out = [];
    while (p.i < p.t.length) {
      if (stop(p.t[p.i])) break;
      var before = p.i, ns = parseItem();
      if (p.i === before) p.i++;
      add(out, ns);
    }
    return out;
  }

  function parseArg() {
    var t = p.t[p.i];
    if (!t) return [];
    if (t.k === 'sp') { p.i++; return parseArg(); }
    if (t.k === 'lb') {
      p.i++;
      var inner = parseUntil(isRb);
      if (p.t[p.i] && p.t[p.i].k === 'rb') p.i++;
      return inner;
    }
    return parseItem();
  }

  /* one atom, no scripts collected at this level */
  function oneAtom() {
    var outer = p.noScript, last = p.last;
    p.noScript++;
    p.last = 'none';
    var a = parseAtom();
    p.noScript = outer;
    p.last = last;
    return a;
  }

  function isRelTok(t) {
    if (!t) return false;
    if (t.k === 'chr') return t.v === '=' || t.v === '<' || t.v === '>' || t.v === ':';
    if (t.k === 'cmd') return !!REL[t.v];
    return false;
  }

  /* argument of a sub/superscript: unbraced forms take a single atom, plus
     "rel atom" pairs when a further script follows (e.g. \_i=1^n)          */
  function scriptArg() {
    var t = p.t[p.i];
    if (!t) return [];
    if (t.k === 'sp') { p.i++; return scriptArg(); }
    if (t.k === 'prime') { p.i++; return [el('span', 'prime', [tx(PRIME)], 'font-style:normal')]; }
    if (t.k === 'lb') return parseArg();
    var out = oneAtom();
    var mark = p.i, extra = [], j, u;
    for (;;) {
      j = p.i;
      while (p.t[p.i] && p.t[p.i].k === 'sp') p.i++;
      u = p.t[p.i];
      if (!isRelTok(u)) { if (p.i !== j) p.i = j; break; }
      p.i++;
      add(extra, oneAtom());
    }
    u = p.t[p.i];
    if (u && (u.k === 'sup' || u.k === 'sub')) add(out, extra);
    else p.i = mark;
    return out;
  }

  function parseItem() {
    var base = parseAtom();
    if (p.pendingNot) {
      for (var q = 0; q < base.length; q++) {
        if (base[q].t === 'e' && (hasc(base[q], 'mo') || hasc(base[q], 'delim') || hasc(base[q], 'op'))) {
          base[q] = notted(base[q]);
          p.pendingNot = false;
          break;
        }
      }
    }
    if (!base.length) {
      var t = p.t[p.i];
      if (t && !p.noScript && (t.k === 'sup' || t.k === 'sub' || t.k === 'prime')) {
        var orphan = collect();
        if (orphan) return [makeScripts(null, orphan)];
      }
      return base;
    }
    var s = p.noScript ? null : collect();
    if (!s) return base;
    return [makeScripts(one(base), s)];
  }

  function collect() {
    var s = null, t;
    while (p.i < p.t.length) {
      t = p.t[p.i];
      if (t.k === 'sup') { p.i++; s = s || {}; s.sup = scriptArg(); }
      else if (t.k === 'sub') { p.i++; s = s || {}; s.sub = scriptArg(); }
      else if (t.k === 'prime') { p.i++; s = s || {}; s.prime = (s.prime || 0) + 1; }
      else break;
    }
    return s;
  }

  function notted(n) {
    return el('span', 'not', [el('span', 'notslash', [tx('/')], 'position:absolute;left:-.08em;top:-.12em;font-size:.85em;line-height:1'), n],
      'position:relative;display:inline-block');
  }

  function makeScripts(base, s) {
    var sup = s.sup, sub = s.sub, i;
    if (s.prime) {
      var pn = el('span', 'prime', [tx(s.prime > 1 ? PRIME2 : PRIME)], 'font-style:normal');
      if (sup) {
        sup = sup.concat([el('span', 'pr', [], 'padding:0 .04em'), pn]);
      } else sup = [pn];
    }
    if (base && base.bigop) {
      var lim = p.display && !p.noLimits;
      var kk = [];
      if (lim && sup) kk.push(el('span', 'lim-over', sup, LIMSTYLE));
      kk.push(base.k[0]);
      if (lim && sub) kk.push(el('span', 'lim-under', sub, LIMSTYLE));
      else if (sub || sup) kk.push(el('span', 'scripts', scriptKids(sub, sup)));
      var o = el('span', 'opbig', kk);
      o.bigop = true;
      return o;
    }
    if (!base) return el('span', 'scripts', scriptKids(sub, sup));
    return el('span', 'mscripts', [base, el('span', 'scripts', scriptKids(sub, sup))]);
  }
  function scriptKids(sub, sup) {
    var k = [];
    if (sub) k.push(el('sub', 'sub', sub));
    if (sup) k.push(el('sup', 'sup', sup));
    return k;
  }
  var LIMSTYLE = 'display:block;text-align:center;font-size:.7em;line-height:1.15';

  /* ---------------- atoms ---------------- */

  function parseAtom() {
    var t = p.t[p.i];
    if (!t) return [];
    if (t.k === 'sp' || t.k === 'amp' || t.k === 'rb' || t.k === 'sup' || t.k === 'sub' || t.k === 'prime') return [];
    p.i++;
    if (t.k === 'lb') {
      p.last = 'atom';
      var outer = p.noScript, lastl = p.last;
      p.noScript = 0;
      p.last = 'none';
      var inner = parseUntil(isRb);
      p.noScript = outer;
      p.last = lastl;
      if (p.t[p.i] && p.t[p.i].k === 'rb') p.i++;
      return [grp(inner)];
    }
    if (t.k === 'id') { p.last = 'atom'; return [identNode(t.v)]; }
    if (t.k === 'num') { p.last = 'atom'; return [el('span', 'mn', [tx(t.v)], 'font-style:normal')]; }
    if (t.k === 'chr') return [chrNode(t.v)];
    if (t.k === 'nbsp') return [el('span', 'space sp-nbsp', [tx(' ')], 'white-space:pre')];
    if (t.k === 'cmd') return runCmd(t.v);    if (t.k === 'cs') return runCs(t.v);
    return [el('span', 'unk', [tx(t.v)], UNK)];
  }

  function identNode(name) {
    var s = p.st;
    if (s.bb) {
      var b = '', i;
      for (i = 0; i < name.length; i++) {
        var ix = name.charCodeAt(i) & ~32;
        b += (ix >= 65 && ix <= 90) ? BB[ix - 65] : name.charAt(i);
      }
      return el('span', 'bb', [tx(b)], 'font-style:normal;font-weight:400');
    }
    if (s.cal) return el('span', 'cal', [tx(name)], 'font-family:' + SERIF + ';font-style:italic');
    if (s.sf) return el('span', 'mi sf', [tx(name)], 'font-family:' + SANS + ';font-style:normal');
    if (s.tt) return el('span', 'mi tt', [tx(name)], 'font-family:' + MONO + ';font-style:normal');
    if (s.bf) return el('span', 'mi bf', [tx(name)], 'font-weight:700;font-style:' + (s.up ? 'normal' : 'italic'));
    if (s.up) return el('span', 'mi rm', [tx(name)], 'font-style:normal');
    return el('span', 'mi', [tx(name)], 'font-style:italic');
  }

  function chrNode(v) {
    if (v === '(' || v === '[') { p.last = 'open'; return el('span', 'delim', [tx(v)]); }
    if (v === ')' || v === ']' || v === '|' || v === '/' || v === '.' || v === '!') {
      p.last = 'close';
      return el('span', 'delim' + (v === '|' ? ' vert' : ''), [tx(v)]);
    }
    if (v === '=' || v === '<' || v === '>') { p.last = 'rel'; return el('span', 'mo rel', [tx(v)]); }
    if (v === '+' || v === '-') {
      var bin = p.last === 'atom' || p.last === 'close' || p.last === 'rel';
      p.last = 'op';
      return el('span', 'mo' + (bin ? ' bin' : ' un'), [tx(v === '-' ? '−' : v)]);
    }
    if (v === '*') { p.last = 'op'; return el('span', 'mo bin', [tx('∗')]); }
    if (v === ',' || v === ';') { p.last = 'punct'; return el('span', 'mo punct', [tx(v)]); }
    if (v === ':') { p.last = 'rel'; return el('span', 'mo rel', [tx(':')]); }
    if (v === '?') { p.last = 'close'; return el('span', 'mo', [tx(v)]); }
    p.last = 'atom';
    return el('span', 'mo', [tx(v)]);
  }

  /* ---------------- node builders ---------------- */

  function spNode(cls, w) { return el('span', 'space ' + cls, [], 'width:' + w + 'em'); }
  function opnameNode(name) { return el('span', 'opname', [tx(name)], 'padding-right:.15em'); }
  function unkNode(s) { return el('span', 'unk', [tx(s)], UNK); }

  function fracNode(num, den, extra) {
    return el('span', 'frac' + (extra ? ' ' + extra : ''),
      [el('span', 'num', num, 'display:block;padding:0 .2em'),
       el('span', 'bar', [], 'display:block;border-top:1px solid currentColor'),
       el('span', 'den', den, 'display:block;padding:0 .2em')],
      'display:inline-block;vertical-align:middle;text-align:center;margin:0 .12em');
  }

  function sqrtNode(inner, idx) {
    var k = [];
    if (idx) k.push(el('span', 'idx', idx, 'font-size:.6em;margin:0 -.05em -.2em 0'));
    k.push(el('span', 'root',
      [el('span', 'rad', [tx('√')], 'font-size:1.5em;line-height:1'),
       el('span', 'radx', inner)],
      'display:inline-block;border-top:1px solid currentColor;padding:0 .1em 0 0'));
    return el('span', 'sqrt', k, 'display:inline-block;white-space:nowrap;margin:0 .1em');
  }

  function delimNode(d, level) {
    if (d === '') return null;
    return el('span', 'delim' + (level > 0 ? ' big big' + Math.min(level, 5) : ''), [tx(d)]);
  }

  function readDelim() {
    var t = p.t[p.i];
    if (!t) return '';
    if (t.k === 'chr') { p.i++; return DELIMCHAR[t.v] !== undefined ? DELIMCHAR[t.v] : t.v; }
    if (t.k === 'cs') { p.i++; return t.v === '|' ? '‖' : (DELIM[t.v] !== undefined ? DELIM[t.v] : t.v); }
    if (t.k === 'cmd') { p.i++; return DELIM[t.v] !== undefined ? DELIM[t.v] : (MISC[t.v] || t.v); }
    return '';
  }

  function rawArg() {
    var t = p.t[p.i];
    if (!t) return '';
    if (t.k === 'sp') { p.i++; return rawArg(); }
    if (t.k !== 'lb') { p.i++; return typeof t.v === 'string' ? t.v : ''; }
    var open = t, depth = 1, j = p.i + 1;
    for (; j < p.t.length; j++) {
      if (p.t[j].k === 'lb') depth++;
      else if (p.t[j].k === 'rb') { depth--; if (depth === 0) break; }
    }
    var body = j < p.t.length ? p.src.slice(open.e, p.t[j].s) : p.src.slice(open.e);
    p.i = j < p.t.length ? j + 1 : p.t.length;
    return body;
  }

  /* ---------------- commands ---------------- */

  var CMD = {};

  CMD.frac = CMD.dfrac = CMD.cfrac = function () { return [fracNode(parseArg(), parseArg(), '')]; };
  CMD.tfrac = function () { return [fracNode(parseArg(), parseArg(), 'tfrac')]; };
  CMD.binom = CMD.dbinom = function () { return [fracNode(parseArg(), parseArg(), 'binom')]; };
  CMD.tbinom = function () { return [fracNode(parseArg(), parseArg(), 'binom tfrac')]; };

  CMD.sqrt = function () {
    var idx = null, t = p.t[p.i];
    while (t && t.k === 'sp') { p.i++; t = p.t[p.i]; }
    if (t && (t.k === 'chr' || t.k === 'cs') && t.v === '[') {
      p.i++;
      var sub = [];
      while (p.i < p.t.length) {
        var u = p.t[p.i];
        if ((u.k === 'chr' || u.k === 'cs') && u.v === ']') { p.i++; break; }
        if (u.k === 'rb') break;
        var before = p.i;
        add(sub, parseItem());
        if (p.i === before) p.i++;
      }
      idx = sub;
    }
    return [sqrtNode(parseArg(), idx)];
  };

  CMD.left = function () {
    var open = readDelim();
    var inner = parseUntil(isRight);
    if (p.t[p.i] && isRight(p.t[p.i])) p.i++;
    var close = readDelim();
    var lv = maxLvl(inner);
    var pair = [delimNode(open, lv)];
    if (inner.length) pair.push(el('span', 'delim-in', [one(inner)], 'display:inline-block;vertical-align:middle'));
    var cn = delimNode(close, lv);
    if (cn) pair.push(cn);
    return [el('span', 'delimpair', pair, 'display:inline-block;white-space:nowrap')];
  };

  function overUnder(name, under) {
    CMD[name] = function () {
      var a = parseArg(), b = parseArg();
      return [under
        ? el('span', 'underset', [one(b), el('span', 'scripts', a, 'display:block;text-align:center')], 'display:inline-block;text-align:center;vertical-align:middle')
        : el('span', 'overset', [el('span', 'scripts', b, 'display:block;text-align:center'), one(a)], 'display:inline-block;text-align:center;vertical-align:middle')];
    };
  }
  overUnder('overset', false);
  overUnder('stackrel', false);
  overUnder('underset', true);

  /* text */
  CMD.text = CMD.textnormal = CMD.textrm = CMD.mbox = CMD.hbox = function () {
    return [el('span', 'mtext', [tx(rawArg())], 'font-style:normal')];
  };
  CMD.textbf = function () { return [el('span', 'mtext bf', [tx(rawArg())], 'font-style:normal;font-weight:700')]; };
  CMD.textit = CMD.emph = function () { return [el('span', 'mtext it', [tx(rawArg())], 'font-style:italic')]; };
  CMD.textsf = function () { return [el('span', 'mtext sf', [tx(rawArg())], 'font-style:normal;font-family:' + SANS)]; };
  CMD.texttt = function () { return [el('span', 'mtext tt', [tx(rawArg())], 'font-style:normal;font-family:' + MONO)]; };
  CMD.verb = function () { return [el('span', 'mtext', [tx(rawArg())])]; };
  CMD.label = CMD.ref = CMD.cite = CMD.eqref = function () {
    return [el('span', 'mtext ref', [tx(rawArg())], 'color:#7a8292')];
  };

  /* math styles */
  CMD.mathrm = CMD.mathnormal = function () { return inSt({ up: true }, parseArg); };
  CMD.mathbf = function () { return inSt({ bf: true }, parseArg); };
  CMD.mathit = function () { return inSt({ it: true }, parseArg); };
  CMD.mathsf = function () { return inSt({ sf: true, up: true }, parseArg); };
  CMD.mathtt = function () { return inSt({ tt: true, up: true }, parseArg); };
  CMD.mathbb = CMD.Bbb = function () { return inSt({ bb: true }, parseArg); };
  CMD.mathcal = function () { return inSt({ cal: true }, parseArg); };
  CMD.mathscr = function () { return inSt({ cal: true }, parseArg); };
  CMD.operatorname = function () { return [opnameNode(rawArg())]; };
  CMD.operatornamewithlimits = function () {
    var o = el('span', 'opbig', [opnameNode(rawArg())]);
    o.bigop = true;
    return [o];
  };
  CMD.not = function () { p.pendingNot = true; return []; };
  CMD.bmod = function () { return [el('span', 'mo bin', [tx('mod')])]; };
  CMD.bf = CMD.bold = function () { return inSt({ bf: true }, parseArg); };
  CMD.it = CMD.italic = function () { return inSt({ it: true, up: false }, parseArg); };
  CMD.rm = function () { return inSt({ up: true, bf: false, it: false, cal: false, bb: false, sf: false, tt: false }, parseArg); };
  CMD.em = CMD.it;
  CMD.sf = function () { return inSt({ sf: true, tt: false }, parseArg); };
  CMD.tt = function () { return inSt({ tt: true, sf: false }, parseArg); };
  CMD.pmod = function () { return [el('span', 'mo', [tx('mod')])]; };

  /* accents */
  function accent(name, mark) {
    CMD[name] = function () {
      var inner = parseArg(), b = one(inner);
      if (!b) b = el('span', 'accb', []);
      if (OVERLINE[name] && name === 'overline') {
        return [el('span', 'acc acc-over', [b], 'display:inline-block;border-top:1px solid currentColor')];
      }
      if (name === 'underline') {
        return [el('span', 'acc acc-under', [b], 'display:inline-block;border-bottom:1px solid currentColor')];
      }
      if (name === 'overbrace') {
        return [el('span', 'acc acc-over', [el('span', 'accb', [], 'border-top:2px solid currentColor;border-radius:.4em .4em 0 0')], 'display:inline-block;width:100%')];
      }
      if (name === 'underbrace') {
        return [el('span', 'acc acc-under', [el('span', 'accb', [], 'border-bottom:2px solid currentColor;border-radius:0 0 .4em .4em')], 'display:inline-block;width:100%')];
      }
      return [el('span', 'acc acc-' + name, [b, el('span', 'accm', [tx(mark)],
        'position:absolute;left:50%;top:0;transform:translateX(-50%);line-height:1;font-style:normal')],
        'position:relative;display:inline-block')];
    };
  }
  for (var an in ACCENT_MARK) accent(an, ACCENT_MARK[an]);
  accent('underline', null);
  accent('overline', null);
  accent('underbrace', null);

  /* spacing */
  CMD.quad = function () { return [spNode('sp-quad', 1)]; };
  CMD.qquad = function () { return [spNode('sp-qquad', 2)]; };
  CMD.thinspace = function () { return [spNode('sp-thin', 0.167)]; };
  CMD.medspace = function () { return [spNode('sp-med', 0.222)]; };
  CMD.thickspace = function () { return [spNode('sp-thick', 0.278)]; };
  CMD.negthinspace = function () { return [spNode('sp-neg', -0.167)]; };
  CMD.space = function () { return [spNode('sp-norm', 0.25)]; };
  CMD.nobreakspace = function () { return [el('span', 'space sp-nbsp', [tx('\u00a0')])]; };
  CMD.hskip = CMD.mskip = CMD.kern = function () { return [spNode('sp-hskip', 0.25)]; };
  CMD.phantom = CMD.hphantom = CMD.vphantom = function () { return parseArg(); };
  CMD['\\'] = function () { return p.display ? [el('span', 'space sp-row', [], 'display:block;height:0')] : [spNode('sp-row', 0.4)]; };
  CMD.linebreak = CMD['\\'];
  CMD.newline = CMD['\\'];
  CMD.hline = function () { return [el('span', 'hline', [], 'display:block;border-top:1px solid #9aa0ae')]; };
  CMD.nolinebreak = CMD.nobreak = CMD.allowbreak = CMD.nnonumber = function () { return []; };
  CMD.displaystyle = function () { p.display = true; return []; };
  CMD.textstyle = CMD.scriptstyle = function () { p.display = false; return []; };
  CMD.limits = function () { p.noLimits = false; return []; };
  CMD.nolimits = function () { p.noLimits = true; return []; };
  CMD.color = function () { var c = rawArg(); return [el('span', 'colored', parseArg(), 'color:' + (c || 'inherit'))]; };
  CMD.raisebox = function () { parseArg(); return parseArg(); };
  CMD.lowerbox = function () { parseArg(); return parseArg(); };
  CMD.tag = function () { return [el('span', 'eqtag', [tx(rawArg())], 'float:right')]; };

  /* sized delimiters */
  var SIZED = { bigl: 2, bigr: 2, Bigl: 3, Bigr: 3, biggl: 4, biggr: 4, Biggl: 5, Biggr: 5 };
  (function () {
    for (var k in SIZED) (function (name) {
      CMD[name] = function () {
        var n = delimNode(readDelim(), SIZED[name]);
        return n ? [n] : [];
      };
    })(k);
  })();

  /* ---------------- environments ---------------- */

  function envName() {
    var t = p.t[p.i];
    while (t && t.k === 'sp') { p.i++; t = p.t[p.i]; }
    if (t && t.k === 'lb') { p.i++; t = p.t[p.i]; }
    var name = '';
    if (t && (t.k === 'id' || t.k === 'chr')) { name = t.v; p.i++; }
    while (p.t[p.i] && p.t[p.i].k === 'sp') p.i++;
    if (p.t[p.i] && p.t[p.i].k === 'rb') p.i++;
    return name;
  }

  function skipGroupToks(toks) {
    if (!toks.length || toks[0].k !== 'lb') return;
    var depth = 1, j = 1;
    for (; j < toks.length; j++) {
      if (toks[j].k === 'lb') depth++;
      else if (toks[j].k === 'rb') { depth--; if (depth === 0) break; }
    }
    toks.splice(0, j + 1);
  }

  function skipGroup() {
    if (!p.t[p.i] || p.t[p.i].k !== 'lb') return;
    var depth = 1, j = p.i + 1;
    for (; j < p.t.length; j++) {
      if (p.t[j].k === 'lb') depth++;
      else if (p.t[j].k === 'rb') { depth--; if (depth === 0) break; }
    }
    p.i = j < p.t.length ? j + 1 : p.t.length;
  }

  function readBody() {
    var start = p.i, depth = 0, j = p.i, t;
    if (p.t[j] && p.t[j].k === 'lb') { depth = 1; j++; }
    for (; j < p.t.length; j++) {
      t = p.t[j];
      if (t.k === 'lb') depth++;
      else if (t.k === 'rb') { depth--; if (depth <= 0) break; }
      else if (t.k === 'cmd' && t.v === 'end' && depth === 0) break;
    }
    var toks = p.t.slice(start, j);
    p.i = j;
    if (p.t[p.i] && isEnd(p.t[p.i])) {
      p.i++;
      while (p.t[p.i] && p.t[p.i].k === 'sp') p.i++;
      if (p.t[p.i] && (p.t[p.i].k === 'id' || p.t[p.i].k === 'chr')) p.i++;
      skipGroup();
    }
    if (p.t[p.i] && p.t[p.i].k === 'rb') p.i++;
    return toks;
  }

  function parseToks(toks) {
    var t0 = p.t, i0 = p.i;
    p.t = toks;
    p.i = 0;
    var out = parseUntil(noStop);
    p.t = t0;
    p.i = i0;
    return out;
  }

  function splitDepth(toks, kind) {
    var rows = [], cur = [], depth = 0, i, t;
    for (i = 0; i < toks.length; i++) {
      t = toks[i];
      if (t.k === 'lb') depth++;
      else if (t.k === 'rb' && depth > 0) depth--;
      if (depth === 0 && kind(t)) { rows.push(cur); cur = []; continue; }
      cur.push(t);
    }
    rows.push(cur);
    return rows;
  }

  function inDisplay(fn) {
    var was = p.display;
    p.display = true;
    var r = fn();
    p.display = was;
    return r;
  }

  var ENV = {};

  function envDisplay(name) {
    return function () {
      var toks = readBody();
      var inner = inDisplay(function () { return parseToks(toks); });
      return [el('span', 'tex-env env-' + name, inner, 'display:block;margin:.7em 0;text-align:center')];
    };
  }
  ENV.equation = envDisplay('equation');
  ENV['equation*'] = envDisplay('equation');
  ENV.displaymath = envDisplay('displaymath');
  ENV.dmath = envDisplay('dmath');
  ENV.multline = envDisplay('multline');
  ENV['multline*'] = envDisplay('multline');
  ENV.theorem = envDisplay('theorem');
  ENV.proof = envDisplay('proof');
  ENV.lemma = envDisplay('lemma');
  ENV.definition = envDisplay('definition');
  ENV.remark = envDisplay('remark');
  ENV.corollary = envDisplay('corollary');
  ENV.example = envDisplay('example');
  ENV.subequations = envDisplay('equation');

  function envAlign(name) {
    return function () {
      var rows = splitDepth(readBody(), isRowEnd);
      while (rows.length > 1 && !rows[rows.length - 1].length) rows.pop();
      var out = [], i, c, cells, k;
      inDisplay(function () {
        for (i = 0; i < rows.length; i++) {
          cells = splitDepth(rows[i], isAmp);
          if (cells.length < 2) {
            out.push(el('span', 'mrow al-row al-center', parseToks(rows[i]), 'display:block;text-align:center;width:100%'));
            continue;
          }
          k = [el('span', 'al-r', parseToks(cells[0]), 'flex:1 1 0;text-align:right'),
               el('span', 'al-l', parseToks(cells[1]), 'flex:1 1 0;text-align:left')];
          for (c = 2; c < cells.length; c++) add(k, parseToks(cells[c]));
          out.push(el('span', 'mrow al-row', k, 'display:flex;align-items:center;gap:.9em;width:100%'));
        }
      });
      return [el('span', 'tex-align env-' + name, out, 'display:block;margin:.7em 0;width:100%')];
    };
  }
  ENV.align = envAlign('align');
  ENV['align*'] = envAlign('align');
  ENV.alignat = ENV['alignat*'] = envAlign('align');
  ENV.flalign = ENV['flalign*'] = envAlign('align');
  ENV.gather = ENV['gather*'] = envAlign('gather');

  function envMatrix(name, spec) {
    return function () {
      if (spec) skipGroup();
      var toks = readBody();
      var rows = splitDepth(toks, isRowEnd);
      while (rows.length > 1 && !rows[rows.length - 1].length) rows.pop();
      var grid = [], cells = [], r, c, ncol = 0, ck;
      for (r = 0; r < rows.length; r++) {
        cells.push(splitDepth(rows[r], isAmp));
        if (cells[r].length > ncol) ncol = cells[r].length;
      }
      if (!ncol) ncol = 1;
      for (r = 0; r < cells.length; r++) {
        ck = [];
        for (c = 0; c < ncol; c++) ck.push(el('span', 'mcell', parseToks(cells[r][c] || []), 'padding:.12em .4em'));
        grid.push(el('span', 'mrow mrow-grid', ck, 'display:contents'));
      }
      var base = name.replace('small', '');
      var k = [];
      if (base !== 'matrix') k.push(el('span', 'mbrace mbrace-l', [], 'display:inline-block;vertical-align:middle;width:.3em;border:1px solid currentColor;border-right:0'));
      k.push(el('span', 'mgrid', grid,
        'display:inline-grid;grid-template-columns:repeat(' + ncol + ',auto);justify-items:center;align-items:center;gap:.15em .1em;vertical-align:middle'));
      if (base !== 'matrix' && base !== 'cases') k.push(el('span', 'mbrace mbrace-r', [], 'display:inline-block;vertical-align:middle;width:.3em;border:1px solid currentColor;border-left:0'));
      return [el('span', 'matrix matrix-' + name, k, 'display:inline-block;vertical-align:middle;margin:0 .15em')];
    };
  }
  var MATRIXES = ['matrix', 'smallmatrix', 'pmatrix', 'bmatrix', 'Bmatrix', 'vmatrix', 'Vmatrix', 'cases', 'dcases'];
  for (var mi = 0; mi < MATRIXES.length; mi++) ENV[MATRIXES[mi]] = envMatrix(MATRIXES[mi], false);
  ENV.array = envMatrix('array', true);

  CMD.begin = function () {
    var name = envName();
    if (!name) return [unkNode('\\begin')];
    var f = ENV[name];
    if (f) return f();
    var toks = readBody();
    return [unkNode('\\begin{' + name + '}')].concat(parseToks(toks));
  };
  CMD.end = function () { return [unkNode('\\end')]; };

  /* ---------------- control symbols ---------------- */

  function runCs(v) {
    if (v === '\\') return CMD['\\']();
    if (CSP[v]) { p.last = 'atom'; return [spNode(CSP[v][0], CSP[v][1])]; }
    if (v === '{' || v === '}') { p.last = 'close'; return [el('span', 'delim', [tx(v)])]; }
    if (v === '|') { p.last = 'close'; return [el('span', 'delim vert', [tx('‖')])]; }
    if (v === '%') { p.last = 'atom'; return [el('span', 'mo', [tx('%')])]; }
    if (v === '$') { p.last = 'atom'; return [el('span', 'mo', [tx('$')])]; }
    if (v === '&') { p.last = 'atom'; return [el('span', 'mo', [tx('&')])]; }
    if (v === '#' || v === '_' || v === '"' || v === "'" || v === '`' || v === '=') {
      p.last = 'atom';
      return [el('span', 'mo', [tx(v)])];
    }
    if (v === '') return [];
    return [unkNode('\\' + v)];
  }

  /* ---------------- command dispatch ---------------- */

  function runCmd(name) {
    var f = CMD[name];
    if (f) return f();
    if (GREEK[name]) { p.last = 'atom'; return [el('span', 'mi', [tx(GREEK[name])], 'font-style:italic')]; }
    if (GREEK_UP[name]) { p.last = 'atom'; return [el('span', 'mi', [tx(GREEK_UP[name])], 'font-style:normal')]; }
    if (REL[name]) { p.last = 'rel'; return [el('span', 'mo rel', [tx(REL[name])])]; }
    if (OPS[name]) { p.last = 'op'; return [el('span', 'mo bin', [tx(OPS[name])])]; }
    if (OPNAME[name]) { p.last = 'atom'; return [opnameNode(OPNAME[name])]; }
    if (BIGOP[name]) {
      p.last = 'op';
      var inner = LIMLIKE[name] ? opnameNode(BIGOP[name])
        : el('span', 'op', [tx(BIGOP[name])], 'font-size:1.35em;line-height:1;padding:0 .05em');
      var o = el('span', 'opbig', [inner]);
      o.bigop = true;
      return [o];
    }
    if (MISC[name]) { p.last = 'atom'; return [el('span', 'mo', [tx(MISC[name])], 'font-style:normal')]; }
    if (ACCENT_MARK[name]) return CMD[name]();
    return [unkNode('\\' + name)];
  }

  /* ---------------- entry points ---------------- */

  function strip(src) {
    var s = String(src == null ? '' : src).replace(/\r\n?/g, '\n').trim();
    if (s.length > 3 && s.slice(0, 2) === '$$' && s.slice(-2) === '$$') return s.slice(2, -2).trim();
    if (s.length > 1 && s.charAt(0) === '$' && s.charAt(s.length - 1) === '$') return s.slice(1, -1).trim();
    return s;
  }

  function build(tex, options) {
    var o = options || {};
    var src = strip(tex);
    p = { t: tokenize(src), i: 0, src: src, display: !!o.display, last: 'none', noLimits: false, noScript: 0, pendingNot: false, st: st({}) };
    var nodes;
    try {
      nodes = parseUntil(noStop);
    } catch (err) {
      if (global.console && console.warn) console.warn('nano/tex:', err);
      nodes = [unkNode(src)];
    }
    var root = el('span', 'tex' + (o.display ? ' tex-display' : ''), nodes,
      o.display ? 'display:block;margin:.6em 0;text-align:center' : '');
    p = null;
    return root;
  }

  function render(tex, options) {
    var root = build(tex, options);
    var d = global.document || (typeof document !== 'undefined' ? document : null);
    if (!d || !d.createElement) return root;
    return toDom(root, d);
  }

  function renderToString(tex, options) { return toHtml(build(tex, options)); }

  global.NanoTex = { render: render, renderToString: renderToString };
})(typeof window !== 'undefined' ? window : (typeof globalThis !== 'undefined' ? globalThis : this));
