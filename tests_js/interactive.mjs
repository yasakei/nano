import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';
import { existsSync, readdirSync } from 'node:fs';

const fileUrl = f => pathToFileURL(resolve(f)).href;

/* Playwright is not a dependency of nano (nano has no dependencies at all).
   Point NANOPW at a playwright-core install and/or NANOPW_CHROME at a chromium
   binary; otherwise look in the usual places and the browser cache. */
async function loadPlaywright() {
  const modules = [process.env.NANOPW, 'playwright-core'].filter(Boolean);
  const execs = [];
  if (process.env.NANOPW_CHROME) execs.push(process.env.NANOPW_CHROME);
  try {
    const root = (process.env.HOME || '') + '/.cache/ms-playwright';
    if (existsSync(root)) {
      for (const d of readdirSync(root)) {
        if (!/^chromium/.test(d)) continue;
        for (const leaf of [
          'chrome-headless-shell-linux64/chrome-headless-shell',
          'chrome-linux/headless_shell',
          'chrome-linux/chrome',
        ]) {
          const p = root + '/' + d + '/' + leaf;
          if (existsSync(p)) execs.push(p);
        }
      }
    }
  } catch {}
  for (const root of [
    'playwright-core',
    process.cwd() + '/node_modules/playwright-core/index.mjs',
    process.cwd() + '/../yasakei-harness/node_modules/playwright-core/index.mjs',
  ]) modules.push(root);

  let mod = null;
  for (const m of modules) {
    try { const r = await import(m); if (r.chromium) { mod = r; break; } } catch {}
  }
  if (!mod) {
    console.error('nano: playwright-core not found.');
    console.error('      set NANOPW=/path/to/playwright-core/index.mjs and');
    console.error('      NANOPW_CHROME=/path/to/chrome to run the browser tests.');
    process.exit(77);
  }
  return { chromium: mod.chromium, exe: execs[0] };
}

const { chromium, exe } = await loadPlaywright();
const browser = await chromium.launch({
  executablePath: process.env.NANOPW_CHROME || exe,
  args: ['--no-sandbox', '--disable-dev-shm-usage'],
});

let pass = 0;
const fails = [];
const ok = (c, n) => { c ? (pass++, console.log('  PASS ' + n)) : (fails.push(n), console.log('  FAIL ' + n)); };

const files = process.argv.slice(2);
for (const f of files) {
  console.log('\n########## ' + f);
  const page = await browser.newPage({ viewport: { width: 1280, height: 1200 } });
  const errs = [];
  page.on('pageerror', e => errs.push(String(e)));
  page.on('console', m => { if (m.type() === 'error') errs.push('console: ' + m.text()); });
  const url = fileUrl(f);
  await page.goto(url, { waitUntil: 'load' });
  await page.waitForTimeout(400);

  const realErrs = errs.filter(t => !/ERR_FILE_NOT_FOUND|Failed to load resource/.test(t));
  ok(realErrs.length === 0, `no js errors (${realErrs.slice(0, 2).join(' | ')})`);

  // 1. every live canvas actually painted something
  const canv = await page.evaluate(() => {
    const out = [];
    for (const c of document.querySelectorAll('canvas.live, canvas[data-formula]')) {
      const g = c.getContext('2d');
      const d = g.getImageData(0, 0, c.width, c.height).data;
      let painted = 0;
      for (let i = 3; i < d.length; i += 4) if (d[i] > 8) painted++;
      out.push({ id: c.id, w: c.width, h: c.height, painted, frac: painted / (c.width * c.height) });
    }
    return out;
  });
  if (!canv.length) console.log('  SKIP this document has no live plot');
  else ok(canv.length > 0, `live canvases found (${canv.length})`);
  for (const c of canv) ok(c.frac > 0.01, `canvas #${c.id} painted (${(c.frac * 100).toFixed(1)}% of pixels)`);

  // 2. widgets are wired and readable
  const widgets = await page.evaluate(() => {
    const out = [];
    for (const el of document.querySelectorAll('[data-var]')) {
      if (el.tagName === 'CANVAS') continue;
      const v = el.getAttribute('data-var');
      const ro = document.querySelector(`[data-var-readout="${v}"]`);
      out.push({ kind: el.tagName.toLowerCase(), type: el.type, var: v, readout: ro ? ro.textContent.trim() : null });
    }
    return out;
  });
  console.log('  widgets: ' + JSON.stringify(widgets));
  for (const w of widgets) ok(w.readout !== null, `${w.kind} '${w.var}' has a readout`);

  // 3. driving a control updates its readout AND repaints a dependent live plot
  const driven = [];
  for (const w of widgets) {
    if (w.kind === 'input' && (w.type === 'range' || w.type === 'number')) driven.push({ ...w, set: w.type === 'range' ? '0.83' : '7' });
    if (w.kind === 'input' && w.type === 'checkbox') driven.push({ ...w, set: 'flip' });
    if (w.kind === 'select') driven.push({ ...w, set: 'select' });
  }
  for (const d of driven) {
    const before = await page.evaluate(v => {
      const c = document.querySelector(`canvas.live[data-var="${v}"]`);
      if (!c) return null;
      const g = c.getContext('2d');
      const px = g.getImageData(0, 0, c.width, c.height).data;
      let h = 0; for (let i = 0; i < px.length; i += 97) h = (h * 31 + px[i]) >>> 0;
      return { h, ro: document.querySelector(`[data-var-readout="${v}"]`)?.textContent.trim() };
    }, d.var);
    if (!before) { console.log(`  SKIP no live plot bound to '${d.var}'`); continue; }
    await page.evaluate(({ tag, kind, set }) => {
      const el = [...document.querySelectorAll(tag + '[data-var]')]
        .find(e => e.tagName.toLowerCase() === kind);
      if (!el) return;
      if (kind === 'select') { el.selectedIndex = Math.min(1, el.options.length - 1); el.dispatchEvent(new Event('change', { bubbles: true })); }
      else if (set === 'flip') { el.checked = !el.checked; el.dispatchEvent(new Event('change', { bubbles: true })); }
      else { el.value = set; el.dispatchEvent(new Event('input', { bubbles: true })); el.dispatchEvent(new Event('change', { bubbles: true })); }
    }, { tag: d.kind === 'select' ? 'select' : d.kind === 'input' ? 'input' : d.kind, kind: d.kind, set: d.set });
    await page.waitForTimeout(120);
    const after = await page.evaluate(v => {
      const c = document.querySelector(`canvas.live[data-var="${v}"]`);
      const g = c.getContext('2d');
      const px = g.getImageData(0, 0, c.width, c.height).data;
      let h = 0; for (let i = 0; i < px.length; i += 97) h = (h * 31 + px[i]) >>> 0;
      return { h, ro: document.querySelector(`[data-var-readout="${v}"]`)?.textContent.trim() };
    }, d.var);
    ok(after.ro !== before.ro, `control '${d.var}' readout updated (${before.ro} -> ${after.ro})`);
    ok(after.h !== before.h, `control '${d.var}' repainted its live plot`);
  }

  // 4. formula evaluator + Nano API reachable from user script
  const api = await page.evaluate(() => {
    const out = {};
    out.tex = typeof NanoTex?.renderToString === 'function';
    try { out.norm = NanoTex.renderToString('\\frac{a}{b}').includes('<'); } catch (e) { out.norm = 'err ' + e.message; }
    out.api = ['register', 'redrawAll', 'setVar', 'getVar', 'evalFormula', 'on', 'fmt']
      .filter(k => typeof Nano?.[k] === 'function');
    try { out.f1 = Nano.evalFormula('2 * 3 + 4 ^ 2', {}); } catch (e) { out.f1 = 'err ' + e.message; }
    try { out.f2 = Nano.evalFormula('sin(0) + 2 ^ 3 ^ 2', {}); } catch (e) { out.f2 = 'err ' + e.message; }
    try { out.f3 = Nano.evalFormula('2x + 1', 10); } catch (e) { out.f3 = 'err ' + e.message; }
    try { out.f4 = Nano.evalFormula('x^2 - 4', 3); } catch (e) { out.f4 = 'err ' + e.message; }
    try { out.f5 = Nano.evalFormula('(1 + 2) * 3', 0); } catch (e) { out.f5 = 'err ' + e.message; }
    try { out.f6 = Nano.evalFormula('hypot(3, 4)', 0); } catch (e) { out.f6 = 'err ' + e.message; }
    try { out.f7 = Nano.evalFormula('2x', 5, 't'); } catch (e) { out.f7 = 'err ' + e.message; }
    try { out.f8 = Nano.evalFormula('1/0', 0); } catch (e) { out.f8 = 'err ' + e.message; }
    try { out.f9 = Nano.evalFormula('2 +', 0); } catch (e) { out.f9 = 'err ' + e.message; }
    return out;
  });
  ok(api.tex, 'NanoTex.renderToString present');
  ok(api.norm === true, 'NanoTex renders to markup');
  ok(api.api.length === 7, `Nano API complete (${api.api.length}/7: ${api.api.join(',')})`);
  ok(api.f1 === 22, `precedence 2*3+4^2 = ${api.f1} (want 22)`);
  ok(api.f2 === 512, `right-assoc ^ : 2^3^2 = ${api.f2} (want 512)`);
  ok(api.f3 === 21, `implicit multiply 2x = ${api.f3} (want 21)`);
  ok(api.f4 === 5, `x^2 - 4 at x=3 = ${api.f4} (want 5)`);
  ok(api.f5 === 9, `parens (1+2)*3 = ${api.f5} (want 9)`);
  ok(api.f6 === 5, `hypot(3,4) = ${api.f6} (want 5)`);
  ok(api.f7 === 10, `renamed axis 2t at t=5 = ${api.f7} (want 10)`);
  ok(api.f8 === Infinity, `1/0 = ${api.f8} (want Infinity)`);
  ok(typeof api.f9 === 'number' && isNaN(api.f9), `malformed formula yields NaN, not a throw (${api.f9})`);

  // 5. copy buttons exist and do not throw
  const copies = await page.evaluate(() => {
    const bs = [...document.querySelectorAll('.copy, button.copy, [data-copy]')];
    return { n: bs.length };
  });
  if (copies.n) {
    const errCount = errs.length;
    await page.evaluate(() => { for (const b of document.querySelectorAll('.copy, button.copy, [data-copy]')) b.click(); });
    await page.waitForTimeout(80);
    ok(errs.length === errCount, `clicking ${copies.n} copy buttons does not throw`);
  } else console.log('  SKIP no copy buttons');

  await page.close();
}

console.log(`\n${fails.length ? 'FAILED' : 'OK'}: ${pass} passed, ${fails.length} failed`);
for (const f of fails) console.log('  - ' + f);
await browser.close();
process.exit(fails.length ? 1 : 0);
