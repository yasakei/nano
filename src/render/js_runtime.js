/* js_runtime.js - nano: interactive runtime for generated documents */
(function (global) {
  'use strict';

  var doc = global.document;
  var TeX = global.NanoTex;
  var con = global.console;

  function warn() {
    try {
      if (con && con.warn) con.warn.apply(con, ['nano:'].concat(Array.prototype.slice.call(arguments)));
    } catch (e) {}
  }
  function guard(name, fn) {
    return function () {
      try {
        return fn.apply(null, arguments);
      } catch (e) {
        warn(name, e && e.message ? e.message : e);
      }
    };
  }

  var raf = global.requestAnimationFrame || function (fn) {
    var t = global.setTimeout;
    return t ? t(fn, 16) : null;
  };

  var hasOwn = Object.prototype.hasOwnProperty;

  /* ================= expression evaluator ================= */

  var CONSTS = { pi: Math.PI, e: Math.E, tau: 2 * Math.PI };
  var FUNCS = {
    sin: Math.sin, cos: Math.cos, tan: Math.tan,
    asin: Math.asin, acos: Math.acos, atan: Math.atan,
    atan2: Math.atan2,
    sinh: Math.sinh, cosh: Math.cosh, tanh: Math.tanh,
    exp: Math.exp, ln: Math.log,
    log2: function (x) { return Math.log(x) / Math.LN2; },
    log10: function (x) { return Math.log(x) / Math.LN10; },
    log: function (x, b) { return b === undefined ? Math.log(x) : Math.log(x) / Math.log(b); },
    sqrt: Math.sqrt, abs: Math.abs,
    sign: function (x) { return x > 0 ? 1 : x < 0 ? -1 : x === 0 ? 0 : NaN; },
    signum: function (x) { return x > 0 ? 1 : x < 0 ? -1 : x === 0 ? 0 : NaN; },
    floor: Math.floor, ceil: Math.ceil,
    round: function (x, d) {
      if (d === undefined) return Math.round(x);
      var m = Math.pow(10, d | 0);
      return Math.round(x * m) / m;
    },
    trunc: function (x) { return x < 0 ? Math.ceil(x) : Math.floor(x); },
    pow: function (a, b) { return Math.pow(a, b); },
    min: function () { return Math.min.apply(Math, arguments); },
    max: function () { return Math.max.apply(Math, arguments); },
    clamp: function (x, a, b) { return Math.min(Math.max(x, a), b); },
    hypot: function (a, b) { return Math.sqrt(a * a + b * b); },
    mod: function (a, b) { return a % b; }
  };

  function lex(src) {
    var out = [], i = 0, n = src.length, c, j;
    while (i < n) {
      c = src.charAt(i);
      if (c === ' ' || c === '\t' || c === '\n' || c === '\r') { i++; continue; }
      if (c >= '0' && c <= '9') {
        j = i;
        while (j < n && src.charAt(j) >= '0' && src.charAt(j) <= '9') j++;
        if (src.charAt(j) === '.' && /\d/.test(src.charAt(j + 1) || '')) {
          j++;
          while (j < n && /\d/.test(src.charAt(j) || '')) j++;
        }
        out.push({ k: 'num', v: parseFloat(src.slice(i, j)) });
        i = j;
        continue;
      }
      if (c === '.' && /\d/.test(src.charAt(i + 1) || '')) {
        j = i + 1;
        while (j < n && /\d/.test(src.charAt(j) || '')) j++;
        out.push({ k: 'num', v: parseFloat(src.slice(i, j)) });
        i = j;
        continue;
      }
      if (/[A-Za-z_]/.test(c)) {
        j = i;
        while (j < n && /[A-Za-z0-9_]/.test(src.charAt(j))) j++;
        out.push({ k: 'id', v: src.slice(i, j) });
        i = j;
        continue;
      }
      if ('+-*/%^(),'.indexOf(c) >= 0) { out.push({ k: 'op', v: c }); i++; continue; }
      throw new Error('unexpected character "' + c + '"');
    }
    out.push({ k: 'end' });
    return out;
  }

  function Parser(toks) { this.t = toks; this.i = 0; }
  Parser.prototype.peek = function () { return this.t[this.i]; };
  Parser.prototype.take = function () { return this.t[this.i++]; };
  Parser.prototype.isOp = function (v) { var t = this.peek(); return t.k === 'op' && t.v === v; };
  Parser.prototype.startsValue = function (t) {
    return t.k === 'num' || t.k === 'id' || (t.k === 'op' && t.v === '(');
  };
  Parser.prototype.expr = function () {
    var node = this.term();
    for (;;) {
      var t = this.peek();
      if (t.k !== 'op' || (t.v !== '+' && t.v !== '-')) break;
      this.take();
      node = { k: 'bin', v: t.v, a: node, b: this.term() };
    }
    return node;
  };
  Parser.prototype.term = function () {
    var node = this.unary(), t;
    for (;;) {
      t = this.peek();
      if (t.k === 'op' && (t.v === '*' || t.v === '/' || t.v === '%')) {
        this.take();
        node = { k: 'bin', v: t.v, a: node, b: this.unary() };
        continue;
      }
      if (this.startsValue(t)) { node = { k: 'bin', v: '*', a: node, b: this.unary() }; continue; }
      break;
    }
    return node;
  };
  Parser.prototype.unary = function () {
    var t = this.peek();
    if (t.k === 'op' && (t.v === '-' || t.v === '+')) {
      this.take();
      return { k: 'un', v: t.v, a: this.unary() };
    }
    return this.power();
  };
  Parser.prototype.power = function () {
    var base = this.value();
    if (this.isOp('^')) {
      this.take();
      return { k: 'bin', v: '^', a: base, b: this.unary() };
    }
    return base;
  };
  Parser.prototype.value = function () {
    var t = this.take();
    if (t.k === 'num') return { k: 'num', v: t.v };
    if (t.k === 'id') {
      if (this.isOp('(')) {
        this.take();
        var args = [];
        if (!this.isOp(')')) {
          args.push(this.expr());
          while (this.isOp(',')) { this.take(); args.push(this.expr()); }
        }
        if (!this.isOp(')')) throw new Error('missing )');
        this.take();
        return { k: 'call', v: t.v, args: args };
      }
      return { k: 'var', v: t.v };
    }
    if (t.k === 'op' && t.v === '(') {
      var e = this.expr();
      if (!this.isOp(')')) throw new Error('missing )');
      this.take();
      return e;
    }
    throw new Error('unexpected token');
  };

  function evaluate(node, x, xname, scope) {
    switch (node.k) {
      case 'num': return node.v;
      case 'var':
        if (node.v === xname || node.v === 'x') return x;
        if (hasOwn.call(CONSTS, node.v)) return CONSTS[node.v];
        if (scope) {
          var sv = scope(node.v);
          if (typeof sv === 'number') return sv;
        }
        return NaN;
      case 'un': {
        var a = evaluate(node.a, x, xname, scope);
        return node.v === '-' ? -a : +a;
      }
      case 'bin': {
        var l = evaluate(node.a, x, xname, scope), r = evaluate(node.b, x, xname, scope);
        switch (node.v) {
          case '+': return l + r;
          case '-': return l - r;
          case '*': return l * r;
          case '/': return l / r;
          case '%': return l % r;
          case '^': return Math.pow(l, r);
        }
        return NaN;
      }
      case 'call': {
        var f = FUNCS[node.v];
        if (!f) return NaN;
        var args = [], i;
        for (i = 0; i < node.args.length; i++) args.push(evaluate(node.args[i], x, xname, scope));
        try {
          return f.apply(null, args);
        } catch (e) {
          return NaN;
        }
      }
    }
    return NaN;
  }

  function compile(src, xname) {
    try {
      var ps = new Parser(lex(String(src)));
      var ast = ps.expr();
      if (ps.peek().k !== 'end') throw new Error('trailing input');
      return function (x, scope) {
        try {
          return evaluate(ast, x, xname || 'x', scope);
        } catch (e) {
          return NaN;
        }
      };
    } catch (e) {
      warn('formula "' + src + '":', e.message);
      return null;
    }
  }

  function evalFormula(src, x, xname) {
    var f = compile(src, xname || 'x');
    if (!f) return NaN;
    return f(typeof x === 'number' ? x : 0, scope);
  }

  function scope(name) {
    return hasOwn.call(vars, name) ? vars[name] : undefined;
  }

  /* ================= numbers ================= */

  function fmt4(v) {
    if (typeof v === 'boolean') return v ? 'true' : 'false';
    if (typeof v === 'string') return v;
    if (v === null || v === undefined) return '';
    if (typeof v !== 'number' || !isFinite(v)) return String(v);
    if (v === Math.round(v) && Math.abs(v) < 1e15) return String(v);
    var r = Number(v.toPrecision(4));
    var a = Math.abs(r);
    if (a !== 0 && (a < 1e-4 || a >= 1e7)) return r.toExponential(3).replace('e+', 'e');
    return String(r);
  }

  function niceNum(range, round) {
    if (!(range > 0)) return 1;
    var exp = Math.floor(Math.log(range) / Math.LN10);
    var f = range / Math.pow(10, exp);
    var nf;
    if (round) nf = f < 1.5 ? 1 : f < 3 ? 2 : f < 7 ? 5 : 10;
    else nf = f <= 1 ? 1 : f <= 2 ? 2 : f <= 5 ? 5 : 10;
    return nf * Math.pow(10, exp);
  }

  function ticks(lo, hi, target) {
    if (!isFinite(lo) || !isFinite(hi) || hi <= lo) return [lo];
    var step = niceNum(niceNum(hi - lo, false) / Math.max(target, 2), true);
    var out = [], v = Math.ceil(lo / step) * step, guardCount = 0;
    while (v <= hi + step * 1e-9 && guardCount < 200) {
      out.push(Math.round(v * 1e9) / 1e9);
      v += step;
      guardCount++;
    }
    return out;
  }

  function decimalsFor(ts) {
    var d = 0, i, s, j;
    for (i = 0; i < ts.length; i++) {
      s = String(ts[i]);
      j = s.indexOf('.');
      if (j >= 0) d = Math.max(d, Math.min(s.length - j - 1, 6));
    }
    return d;
  }

  function tickLabel(v, dec) {
    if (!isFinite(v)) return '';
    var m = Math.pow(10, dec);
    return fmtTick(Math.round(v * m) / m);
  }
  function fmtTick(v) {
    if (v === Math.round(v) && Math.abs(v) < 1e15) return String(v);
    var a = Math.abs(v);
    if (a !== 0 && (a < 1e-4 || a >= 1e7)) return v.toExponential(1).replace('e+', 'e');
    return String(Number(v.toPrecision(6)));
  }

  /* ================= plot spec ================= */

  var PALETTE = ['#346aba', '#e0693e', '#48a05a', '#8c5cc4', '#269494', '#d66e6e', '#e0a028', '#5a6476', '#78aa3c', '#be643c'];
  var INK = '#1e222a', MUTED = '#7a8292', GRID = '#e4e7ee', AXIS = '#788090';
  var FONT = "Helvetica, Arial, 'Segoe UI', 'Liberation Sans', sans-serif";

  function attr(el, name) {
    if (!el) return null;
    if (el.getAttribute) {
      var v = el.getAttribute(name);
      if (v !== null && v !== undefined) return v;
    }
    if (el.dataset && hasOwn.call(el.dataset, name)) return el.dataset[name];
    return null;
  }

  function num(v) {
    if (v === null || v === undefined || v === '') return NaN;
    var f = parseFloat(v);
    return isFinite(f) ? f : NaN;
  }

  function list(v) {
    if (!v) return [];
    return String(v).split(';').map(function (s) { return s.trim(); }).filter(function (s) { return s !== ''; });
  }

  function specOf(el, over) {
    var o = over || {};
    function pick(key, name) { return o[key] !== undefined && o[key] !== null ? o[key] : attr(el, name); }
    var formulas = list(pick('formula', 'data-formula'));
    if (!formulas.length && typeof o.formula === 'string') formulas = [o.formula];
    var labels = list(pick('labels', 'data-labels'));
    var style = (pick('style', 'data-style') || 'line').toLowerCase();
    var fillRaw = pick('fill', 'data-fill');
    return {
      el: el,
      varName: pick('var', 'data-var') || '',
      xname: pick('varName', 'data-var-name') || 'x',
      formulas: formulas,
      labels: labels,
      style: style,
      fill: fillRaw === '1' || fillRaw === 'true' || fillRaw === 'yes' || fillRaw === 'area' || style === 'area',
      color: pick('color', 'data-color') || '',
      xmin: num(pick('xmin', 'data-xmin')),
      xmax: num(pick('xmax', 'data-xmax')),
      ymin: num(pick('ymin', 'data-ymin')),
      ymax: num(pick('ymax', 'data-ymax')),
      points: Math.max(8, Math.min(4000, num(pick('points', 'data-points')) || 240)),
      xlabel: pick('xlabel', 'data-xlabel') || '',
      ylabel: pick('ylabel', 'data-ylabel') || ''
    };
  }

  function sampleAll(s) {
    var out = [], i, j, n = s.points, dx;
    var xlo = isFinite(s.xmin) ? s.xmin : 0;
    var xhi = isFinite(s.xmax) ? s.xmax : 1;
    if (xhi <= xlo) xhi = xlo + 1;
    dx = (xhi - xlo) / (n - 1);
    for (i = 0; i < s.formulas.length; i++) {
      var fn = compile(s.formulas[i], s.xname), xs = [], ys = [];
      for (j = 0; j < n; j++) {
        var x = xlo + j * dx;
        xs.push(x);
        ys.push(fn ? fn(x, scope) : NaN);
      }
      out.push({ x: xs, y: ys, color: i === 0 && s.color ? s.color : PALETTE[i % PALETTE.length], label: s.labels[i] || '' });
    }
    return out;
  }

  function bounds(s, series) {
    var i, j, lo = Infinity, hi = -Infinity, v;
    for (i = 0; i < series.length; i++) {
      for (j = 0; j < series[i].y.length; j++) {
        v = series[i].y[j];
        if (isFinite(v)) {
          if (v < lo) lo = v;
          if (v > hi) hi = v;
        }
      }
    }
    if (!isFinite(lo) || !isFinite(hi)) { lo = 0; hi = 1; }
    var span = hi - lo || Math.abs(hi) || 1;
    var ylo = isFinite(s.ymin) ? s.ymin : (lo >= 0 ? 0 : lo - span * 0.05);
    var yhi = isFinite(s.ymax) ? s.ymax : hi + span * 0.05;
    if (yhi <= ylo) yhi = ylo + 1;
    return { ylo: ylo, yhi: yhi };
  }

  /* ================= canvas drawing ================= */

  var sizes = [];

  function sizeOf(cv, dpr) {
    var w = cv.clientWidth, h = cv.clientHeight, rec = null, i, pw, ph;
    for (i = 0; i < sizes.length; i++) if (sizes[i].c === cv) { rec = sizes[i]; break; }
    if (!w || !h) {
      if (rec) { w = rec.w; h = rec.h; }
      else { w = cv.width || 320; h = cv.height || 200; }
    }
    w = Math.max(40, w);
    h = Math.max(40, h);
    pw = Math.max(1, Math.round(w * dpr));
    ph = Math.max(1, Math.round(h * dpr));
    if (cv.width !== pw) cv.width = pw;
    if (cv.height !== ph) cv.height = ph;
    if (!rec) sizes.push({ c: cv, w: w, h: h });
    else { rec.w = w; rec.h = h; }
    return { w: w, h: h };
  }

  function draw(rec) {
    var s = rec.spec, cv = s.el;
    var ctx = cv.getContext ? cv.getContext('2d') : null;
    if (!ctx) return;
    var dpr = global.devicePixelRatio || 1;
    var sz = sizeOf(cv, dpr);
    var W = sz.w, H = sz.h;
    var series = sampleAll(s);
    if (!series.length) return;
    var bd = bounds(s, series);

    var px = s.ylabel ? 62 : 54, py = 16, pr = 16;
    var pb = 46 + (s.xlabel ? 18 : 0);
    var pw = W - px - pr, ph = H - py - pb;
    if (pw < 10 || ph < 10) return;
    var xlo = isFinite(s.xmin) ? s.xmin : series[0].x[0];
    var xhi = isFinite(s.xmax) ? s.xmax : series[0].x[series[0].x.length - 1];
    if (xhi <= xlo) xhi = xlo + 1;
    var ylo = bd.ylo, yhi = bd.yhi;

    function X(v) { return px + (v - xlo) / (xhi - xlo) * pw; }
    function Y(v) { return py + ph - (v - ylo) / (yhi - ylo) * ph; }

    if (ctx.setTransform) ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    if (ctx.clearRect) ctx.clearRect(0, 0, W, H);
    ctx.lineWidth = 1;
    ctx.lineCap = 'butt';
    if (ctx.setLineDash) ctx.setLineDash([]);

    var yt = ticks(ylo, yhi, 6), xt = ticks(xlo, xhi, 8);
    var i, j, t, y, x;

    ctx.strokeStyle = GRID;
    ctx.lineWidth = 0.7;
    if (ctx.setLineDash) ctx.setLineDash([2, 3]);
    ctx.beginPath();
    for (i = 0; i < yt.length; i++) {
      y = Y(yt[i]);
      if (y < py - 0.5 || y > py + ph + 0.5) continue;
      ctx.moveTo(px, y);
      ctx.lineTo(px + pw, y);
    }
    for (i = 0; i < xt.length; i++) {
      x = X(xt[i]);
      if (x < px - 0.5 || x > px + pw + 0.5) continue;
      ctx.moveTo(x, py);
      ctx.lineTo(x, py + ph);
    }
    ctx.stroke();
    if (ctx.setLineDash) ctx.setLineDash([]);

    ctx.strokeStyle = AXIS;
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(px, py + ph);
    ctx.lineTo(px + pw, py + ph);
    ctx.moveTo(px, py);
    ctx.lineTo(px, py + ph);
    ctx.stroke();

    ctx.fillStyle = MUTED;
    ctx.font = '10px ' + FONT;
    ctx.textAlign = 'right';
    ctx.textBaseline = 'middle';
    var ydec = decimalsFor(yt);
    for (i = 0; i < yt.length; i++) {
      y = Y(yt[i]);
      if (y < py - 0.5 || y > py + ph + 0.5) continue;
      ctx.beginPath();
      ctx.moveTo(px - 3, y);
      ctx.lineTo(px, y);
      ctx.stroke();
      ctx.fillText(tickLabel(yt[i], ydec), px - 7, y);
    }
    var xdec = decimalsFor(xt);
    ctx.textAlign = 'center';
    ctx.textBaseline = 'top';
    for (i = 0; i < xt.length; i++) {
      x = X(xt[i]);
      if (x < px - 0.5 || x > px + pw + 0.5) continue;
      ctx.beginPath();
      ctx.moveTo(x, py + ph);
      ctx.lineTo(x, py + ph + 3);
      ctx.stroke();
      ctx.fillText(tickLabel(xt[i], xdec), x, py + ph + 6);
    }

    var showLine = s.style === '' || s.style === 'line' || s.style === 'both' || s.style === 'area' || s.style === 'step' || s.style === 'stem' || s.style === 'stairs';
    var showPts = s.style === 'both' || s.style === 'dot' || s.style === 'points' || s.style === 'area' || s.style === 'scatter';
    var n = series[0].x.length;
    if (n <= 40) showPts = true;
    var step = s.style === 'step' || s.style === 'stairs';

    for (i = 0; i < series.length; i++) {
      var S = series[i];
      if (s.fill && i === 0 && showLine) {
        ctx.beginPath();
        var started = false, base = Math.max(py, Math.min(py + ph, Y(Math.max(ylo, Math.min(yhi, 0)))));
        for (j = 0; j < S.x.length; j++) {
          if (!isFinite(S.y[j])) continue;
          x = X(S.x[j]);
          y = Y(S.y[j]);
          if (!started) { ctx.moveTo(x, base); started = true; }
          ctx.lineTo(x, y);
        }
        if (started) {
          ctx.lineTo(x, base);
          ctx.closePath();
          ctx.fillStyle = rgba(S.color, 0.14);
          ctx.fill();
        }
      }
      if (showLine) {
        ctx.beginPath();
        var pen = false, prevY = 0;
        for (j = 0; j < S.x.length; j++) {
          if (!isFinite(S.y[j])) { pen = false; continue; }
          x = X(S.x[j]);
          y = Y(S.y[j]);
          if (!pen) { ctx.moveTo(x, y); pen = true; }
          else if (step) {
            ctx.lineTo(x, prevY);
            ctx.lineTo(x, y);
          } else ctx.lineTo(x, y);
          prevY = y;
        }
        ctx.strokeStyle = S.color;
        ctx.lineWidth = s.style === 'stem' ? 1.4 : 2;
        ctx.lineJoin = 'round';
        ctx.lineCap = 'round';
        ctx.stroke();
        if (step) {
          ctx.fillStyle = S.color;
          for (j = 0; j < S.x.length; j++) {
            if (!isFinite(S.y[j])) continue;
            x = X(S.x[j]);
            y = Y(S.y[j]);
            ctx.fillRect(x - 1.5, y - 1.5, 3, 3);
          }
        }
      }
      if (showPts) {
        ctx.fillStyle = S.color;
        for (j = 0; j < S.x.length; j++) {
          if (!isFinite(S.y[j])) continue;
          x = X(S.x[j]);
          y = Y(S.y[j]);
          ctx.beginPath();
          ctx.arc(x, y, 2.5, 0, Math.PI * 2);
          ctx.fill();
        }
      }
    }

    if (series.length > 1 && s.labels.length) {
      var rowH = 15, bw = 0, k;
      ctx.font = '10px ' + FONT;
      for (k = 0; k < series.length; k++) {
        if (!series[k].label) continue;
        bw = Math.max(bw, (ctx.measureText ? ctx.measureText(series[k].label).width : series[k].label.length * 5.5) + 34);
      }
      if (bw > 0) {
        var rows = 0;
        for (k = 0; k < series.length; k++) if (series[k].label) rows++;
        var bh = rows * rowH + 8;
        var bx = px + pw - bw - 8, by = py + 8;
        ctx.fillStyle = 'rgba(255,255,255,0.88)';
        ctx.strokeStyle = GRID;
        ctx.lineWidth = 1;
        if (ctx.fillRect) ctx.fillRect(bx, by, bw, bh);
        if (ctx.strokeRect) ctx.strokeRect(bx, by, bw, bh);
        ctx.textAlign = 'left';
        ctx.textBaseline = 'top';
        var ry = by + 6;
        for (k = 0; k < series.length; k++) {
          if (!series[k].label) continue;
          ctx.strokeStyle = series[k].color;
          ctx.lineWidth = 2;
          ctx.beginPath();
          ctx.moveTo(bx + 7, ry + 5);
          ctx.lineTo(bx + 19, ry + 5);
          ctx.stroke();
          ctx.fillStyle = series[k].color;
          ctx.fillRect(bx + 11, ry + 3, 4, 4);
          ctx.fillStyle = INK;
          ctx.fillText(series[k].label, bx + 24, ry);
          ry += rowH;
        }
      }
    }

    if (s.xlabel) {
      ctx.fillStyle = MUTED;
      ctx.font = '11px ' + FONT;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'bottom';
      ctx.fillText(s.xlabel, px + pw / 2, H - 4);
    }
    if (s.ylabel) {
      ctx.save();
      ctx.fillStyle = MUTED;
      ctx.font = '11px ' + FONT;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.translate(13, py + ph / 2);
      ctx.rotate(-Math.PI / 2);
      ctx.fillText(s.ylabel, 0, 0);
      ctx.restore();
    }
  }

  function rgba(hex, a) {
    var h = String(hex).replace('#', '');
    if (h.length === 3) h = h.charAt(0) + h.charAt(0) + h.charAt(1) + h.charAt(1) + h.charAt(2) + h.charAt(2);
    if (h.length !== 6) return 'rgba(0,0,0,' + a + ')';
    var r = parseInt(h.slice(0, 2), 16), g = parseInt(h.slice(2, 4), 16), b = parseInt(h.slice(4, 6), 16);
    if (!isFinite(r)) return 'rgba(0,0,0,' + a + ')';
    return 'rgba(' + r + ',' + g + ',' + b + ',' + a + ')';
  }

  /* ================= state ================= */

  var plots = [];
  var byVar = {};
  var vars = {};
  var meta = {};
  var subs = {};
  var readouts = [];
  var dirty = {};
  var queued = false;
  var booted = false;

  function redrawFor(name) {
    var list2 = byVar[name], i;
    if (!list2) return;
    for (i = 0; i < list2.length; i++) draw(list2[i]);
  }

  function flush() {
    queued = false;
    for (var k in dirty) {
      if (hasOwn.call(dirty, k)) redrawFor(k);
    }
    dirty = {};
  }

  function schedule(name) {
    dirty[name] = true;
    if (!queued) {
      queued = true;
      if (!raf(flush)) flush();
    }
  }

  function fire(name, value) {
    var list2 = subs[name];
    if (!list2) return;
    for (var i = 0; i < list2.length; i++) {
      try {
        list2[i](value, name);
      } catch (e) {
        warn('listener for "' + name + '":', e.message);
      }
    }
  }

  function optionText(el) {
    try {
      if (el.options && el.options.length && el.selectedIndex >= 0) {
        var o = el.options[el.selectedIndex];
        if (o.text !== undefined && o.text !== '') return o.text;
        if (o.label) return o.label;
      }
    } catch (e) {}
    return el.value === undefined ? '' : String(el.value);
  }

  function displayFor(name) {
    var m = meta[name];
    if (m && m.el) {
      var type = (m.el.type || '').toLowerCase();
      if (type === 'checkbox') return m.el.checked ? 'true' : 'false';
      if (m.el.tagName === 'SELECT' || type === 'select-one' || type === 'select') return optionText(m.el);
    }
    return fmt4(vars[name]);
  }

  function updateReadouts(name) {
    for (var i = 0; i < readouts.length; i++) {
      if (name !== null && readouts[i].name !== name) continue;
      try {
        readouts[i].el.textContent = displayFor(readouts[i].name);
      } catch (e) {
        warn('readout:', e.message);
      }
    }
  }

  function setVar(name, value) {
    if (!name) return;
    vars[name] = value;
    meta[name] = meta[name] || {};
    fire(name, value);
    updateReadouts(name);
    if (byVar[name]) schedule(name);
  }

  function getVar(name) {
    return hasOwn.call(vars, name) ? vars[name] : 0;
  }

  function on(name, fn) {
    if (typeof fn !== 'function') return;
    subs[name] = subs[name] || [];
    subs[name].push(fn);
  }

  function register(el, spec) {
    if (!el) return null;
    var s = specOf(el, spec);
    if (!s.formulas.length) {
      warn('canvas live: no data-formula');
      return null;
    }
    var rec = { el: el, spec: s };
    plots.push(rec);
    if (s.varName) {
      byVar[s.varName] = byVar[s.varName] || [];
      byVar[s.varName].push(rec);
    }
    try {
      draw(rec);
    } catch (e) {
      warn('plot:', e.message);
    }
    return rec;
  }

  function redrawAll() {
    for (var i = 0; i < plots.length; i++) {
      try {
        draw(plots[i]);
      } catch (e) {
        warn('plot:', e.message);
      }
    }
  }

  /* ================= document wiring ================= */

  function qsa(sel) {
    try {
      return doc.querySelectorAll(sel) || [];
    } catch (e) {
      return [];
    }
  }

  function toArray(list2) {
    var out = [], i;
    for (i = 0; i < list2.length; i++) out.push(list2[i]);
    return out;
  }

  function hasClass(el, name) {
    if (!el) return false;
    if (el.classList) return el.classList.contains(name);
    var c = attr(el, 'class') || '';
    return (' ' + c + ' ').indexOf(' ' + name + ' ') >= 0;
  }

  function typesetMath() {
    var els = toArray(qsa('.math, .math-display')), i, el, src, node;
    for (i = 0; i < els.length; i++) {
      el = els[i];
      try {
        src = attr(el, 'data-tex');
        if (src === null || src === undefined) src = el.textContent;
        if (!TeX) throw new Error('NanoTex missing');
        node = TeX.render(src, { display: hasClass(el, 'math-display') });
        el.setAttribute('data-tex', src);
        while (el.firstChild) el.removeChild(el.firstChild);
        el.appendChild(node);
      } catch (e) {
        warn('math:', e.message);
      }
    }
  }

  function byId(id) {
    if (!id) return null;
    if (doc.getElementById) {
      var el = doc.getElementById(id);
      if (el) return el;
    }
    var all = toArray(qsa('[id]')), i;
    for (i = 0; i < all.length; i++) if (attr(all[i], 'id') === id) return all[i];
    return null;
  }

  function firstPre(root) {
    if (!root) return null;
    if (root.tagName === 'PRE') return root;
    if (root.querySelector) {
      try {
        return root.querySelector('pre');
      } catch (e) {
        return null;
      }
    }
    return null;
  }

  function copyTarget(btn) {
    var id = attr(btn, 'data-copy-target');
    var el = id ? byId(id) : null;
    var pre = el ? firstPre(el) : null;
    if (pre) return pre;
    if (el) return el;
    return firstPre(btn.parentNode);
  }

  function legacyCopy(text) {
    var ta = null, host = doc.body || doc.documentElement;
    try {
      ta = doc.createElement('textarea');
      ta.value = text;
      ta.setAttribute('readonly', 'readonly');
      ta.style.position = 'fixed';
      ta.style.top = '-1000px';
      ta.style.left = '0';
      ta.style.opacity = '0';
      if (host && host.appendChild) host.appendChild(ta);
      if (ta.select) ta.select();
      else if (ta.setSelectionRange) ta.setSelectionRange(0, text.length);
      if (doc.execCommand) doc.execCommand('copy');
      if (ta.parentNode) ta.parentNode.removeChild(ta);
    } catch (e) {
      warn('copy:', e.message);
      if (ta && ta.parentNode) {
        try { ta.parentNode.removeChild(ta); } catch (e2) {}
      }
    }
  }

  function writeClipboard(text) {
    var nav = global.navigator;
    if (nav && nav.clipboard && typeof nav.clipboard.writeText === 'function') {
      try {
        var r = nav.clipboard.writeText(text);
        if (r && typeof r.catch === 'function') r.catch(function () { legacyCopy(text); });
        return true;
      } catch (e) {
        legacyCopy(text);
        return true;
      }
    }
    legacyCopy(text);
    return true;
  }

  function wireCopy() {
    var btns = toArray(qsa('button[data-copy-target]')), i;
    for (i = 0; i < btns.length; i++) {
      (function (btn) {
        if (btn.__nanoCopy) return;
        btn.__nanoCopy = 1;
        var label = btn.textContent;
        var timer = null;
        try {
          btn.addEventListener('click', function (ev) {
            if (ev && ev.preventDefault) ev.preventDefault();
            var target = copyTarget(btn);
            var text = target && target.textContent !== undefined ? target.textContent : '';
            writeClipboard(String(text));
            btn.textContent = 'copied';
            var t = global.setTimeout;
            if (timer && global.clearTimeout) global.clearTimeout(timer);
            if (t) timer = t(function () { btn.textContent = label; }, 1200);
          });
        } catch (e) {
          warn('copy button:', e.message);
        }
      })(btns[i]);
    }
  }

  function controlValue(el) {
    var type = (el.type || '').toLowerCase();
    if (type === 'checkbox') return !!el.checked;
    var v = el.value;
    if (type === 'number' || type === 'range') {
      var f = parseFloat(v);
      return isFinite(f) ? f : 0;
    }
    if (el.tagName === 'SELECT' || type === 'select-one' || type === 'select') {
      var f2 = parseFloat(v);
      return isFinite(f2) && String(f2) === String(v).trim() ? f2 : v;
    }
    return v;
  }

  function wireControls() {
    var els = toArray(qsa('input[data-var], select[data-var], textarea[data-var]')), i;
    for (i = 0; i < els.length; i++) {
      (function (el) {
        var name = attr(el, 'data-var');
        if (!name) return;
        if (el.__nanoBound) return;
        el.__nanoBound = 1;
        meta[name] = { el: el };
        var handler = guard('control "' + name + '"', function () {
          setVar(name, controlValue(el));
        });
        try {
          el.addEventListener('input', handler);
          el.addEventListener('change', handler);
        } catch (e) {
          warn('control:', e.message);
        }
      })(els[i]);
    }
    var outs = toArray(qsa('[data-var-readout]')), k;
    for (k = 0; k < outs.length; k++) {
      var nm = attr(outs[k], 'data-var-readout');
      if (!nm) continue;
      var dup = false, m;
      for (m = 0; m < readouts.length; m++) if (readouts[m].el === outs[k]) dup = true;
      if (!dup) readouts.push({ el: outs[k], name: nm });
    }
  }

  function syncControls() {
    var seen = {}, els = toArray(qsa('input[data-var], select[data-var], textarea[data-var]')), i, el, name;
    for (i = 0; i < els.length; i++) {
      el = els[i];
      name = attr(el, 'data-var');
      if (!name || seen[name]) continue;
      seen[name] = 1;
      try {
        setVar(name, controlValue(el));
      } catch (e) {
        warn('sync:', e.message);
      }
    }
    updateReadouts(null);
  }

  function registerLive() {
    var cvs = toArray(qsa('canvas.live, canvas[data-formula]')), i;
    for (i = 0; i < cvs.length; i++) {
      if (cvs[i].__nanoLive) continue;
      cvs[i].__nanoLive = 1;
      try {
        register(cvs[i], null);
      } catch (e) {
        warn('live plot:', e.message);
      }
    }
  }

  function init() {
    if (booted) return;
    booted = true;
    try { typesetMath(); } catch (e) { warn('math:', e.message); }
    try { wireCopy(); } catch (e) { warn('copy:', e.message); }
    try { wireControls(); } catch (e) { warn('controls:', e.message); }
    try { syncControls(); } catch (e) { warn('sync:', e.message); }
    try { registerLive(); } catch (e) { warn('plots:', e.message); }
  }

  if (doc) {
    try {
      if (doc.readyState === 'loading' || !doc.body) {
        if (doc.addEventListener) doc.addEventListener('DOMContentLoaded', init);
        else init();
      } else init();
    } catch (e) {
      warn('init:', e.message);
    }
  }

  global.Nano = {
    TeX: TeX,
    register: guard('register', register),
    redrawAll: guard('redrawAll', redrawAll),
    setVar: guard('setVar', setVar),
    getVar: guard('getVar', getVar),
    on: guard('on', on),
    evalFormula: guard('evalFormula', evalFormula),
    fmt: fmt4
  };
})(typeof window !== 'undefined' ? window : (typeof globalThis !== 'undefined' ? globalThis : this));
