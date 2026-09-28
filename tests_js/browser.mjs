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

const target = process.argv[2] ?? 'examples/tour.html';
let pass = 0;
const fails = [];
const ok = (cond, name) => {
  if (cond) { pass++; console.log('  PASS ' + name); }
  else { fails.push(name); console.log('  FAIL ' + name); }
};

const { chromium, exe } = await loadPlaywright();
const browser = await chromium.launch({
  executablePath: process.env.NANOPW_CHROME || exe,
  args: ['--no-sandbox', '--disable-dev-shm-usage'],
});
const page = await browser.newPage({ viewport: { width: 1280, height: 1400 } });

const consoleErrors = [];
const pageErrors = [];
const requests = [];
page.on('console', m => { if (m.type() === 'error') consoleErrors.push(m.text()); });
page.on('pageerror', e => pageErrors.push(String(e)));
const docUrl = fileUrl(target);
page.on('request', r => {
  if (r.url() === docUrl) return;
  // a <video>/<audio> src pointing at a sibling local file is legitimate; a remote
  // fetch would break the "works offline" promise
  if (/^file:\/\//.test(r.url())) return;
  requests.push(r.url());
});

await page.goto(docUrl, { waitUntil: 'load' });
await page.waitForTimeout(400);

console.log('\n== no network requests');
ok(requests.length === 0, `zero network requests (saw ${requests.length}: ${requests.slice(0, 3).join(', ')})`);

console.log('\n== no js exceptions');
ok(pageErrors.length === 0, `no uncaught exceptions (${pageErrors.slice(0, 2).join(' | ')})`);
const realConsole = consoleErrors.filter(t => !/ERR_FILE_NOT_FOUND|Failed to load resource/.test(t));
ok(realConsole.length === 0, `no console errors (${realConsole.slice(0, 3).join(' | ')})`);

console.log('\n== math typesetting');
const mathCount = await page.locator('.math, .math-display').count();
ok(mathCount > 0, `math elements present (${mathCount})`);
const unrenderedMath = await page.evaluate(() =>
  [...document.querySelectorAll('.math, .math-display')]
    .filter(e => !e.querySelector('.tex') && !e.querySelector('svg') && !e.querySelector('img'))
    .map(e => e.getAttribute('data-tex') ?? e.textContent.slice(0, 30)));
ok(unrenderedMath.length === 0, `every formula typeset (${unrenderedMath.length} unrendered: ${unrenderedMath.slice(0, 3).join(' | ')})`);

console.log('\n== xss / escaping');
ok(await page.locator('script:not([type="text/plain"])', { hasText: 'alert(1)' }).count() === 0, 'no injected script node from user text');
const xss = await page.evaluate(() => document.querySelectorAll('img[onerror], iframe[srcdoc]').length);
ok(xss === 0, 'no onerror / srcdoc injection points');

console.log('\n== headings & structure');
const srcHas = await page.evaluate(() => document.querySelectorAll('h1, h2, h3, h4').length);
ok(srcHas > 0, `headings rendered (${srcHas} total)`);
for (const level of [1, 2]) {
  const n = await page.locator(`h${level}`).count();
  if (n) console.log(`  SKIP h${level} absent from this document`);
  else ok(false, `h${level} rendered (0)`);
}
ok(await page.locator('table').count() > 0, 'tables rendered');
ok(await page.locator('pre code').count() > 0, 'code cells rendered');

console.log('\n== code cells readable');
const codeInfo = await page.evaluate(() => {
  const cs = [...document.querySelectorAll('pre code')].filter(c => c.textContent.trim());
  const bad = cs.filter(c => {
    const s = getComputedStyle(c);
    return parseFloat(s.fontSize) < 10 || !/mono/i.test(s.fontFamily);
  });
  return { n: cs.length, bad: bad.length, ff: cs.length ? getComputedStyle(cs[0]).fontFamily.split(',')[0] : '' };
});
ok(codeInfo.n > 0, `code cells present (${codeInfo.n})`);
ok(codeInfo.bad === 0, `code cells styled (${codeInfo.bad} bad, font ${codeInfo.ff})`);

console.log('\n== notes');
const noteCount = await page.locator('.note, .callout, [class*="note-"]').count();
ok(noteCount > 0, `notes rendered (${noteCount})`);

console.log('\n== figures');
const figures = await page.locator('figure').count();
const svgs = await page.locator('figure svg').count();
ok(figures > 0, `figures rendered (${figures})`);
ok(svgs > 0, `figure svgs inlined (${svgs})`);

console.log('\n== theme');
const themeResults = await page.evaluate(() => {
  const body = document.body;
  const base = getComputedStyle(body);
  const seen = {};
  const probe = document.createElement('p');
  probe.textContent = 'probe';
  body.appendChild(probe);
  for (const t of ['default', 'serif', 'dark', 'minimal']) {
    body.className = 'theme-' + t;
    const s = getComputedStyle(probe);
    seen[t] = { fg: s.color, bg: getComputedStyle(body).backgroundColor, font: s.fontFamily.split(',')[0] };
  }
  body.className = '';
  probe.remove();
  return { base: { fg: base.color, bg: base.backgroundColor }, seen };
});
const distinct = new Set(Object.values(themeResults.seen).map(v => v.fg + v.bg));
ok(distinct.size >= 3, `themes visibly differ (${distinct.size} distinct fg/bg combos)`);
for (const [t, v] of Object.entries(themeResults.seen)) {
  ok(v.fg !== v.bg, `theme '${t}' has contrasting text (${v.fg} on ${v.bg})`);
}

console.log('\n== toc');
const toc = await page.locator('nav.toc a[href^="#"]').count();
if (toc > 0) {
  ok(true, `toc links present (${toc})`);
  const target0 = await page.locator('nav.toc a').first().getAttribute('href');
  const id = target0.replace('#', '');
  ok((await page.locator(`[id="${id}"]`).count()) > 0, `toc link #${id} resolves`);
} else {
  console.log('  SKIP no toc in this document');
}

console.log('\n== layout sanity');
const overflow = await page.evaluate(() => {
  const de = document.documentElement;
  return { scrollW: de.scrollWidth, clientW: de.clientWidth };
});
ok(overflow.scrollW <= overflow.clientW + 1, `no horizontal overflow (${overflow.scrollW} vs ${overflow.clientW})`);

const wide = await page.evaluate(() => {
  const bad = [];
  for (const el of document.querySelectorAll('p, li, td, h1, h2, h3, pre')) {
    if (el.scrollWidth > el.clientWidth + 2 && getComputedStyle(el).overflow === 'visible') {
      bad.push(el.tagName + ': ' + el.textContent.trim().slice(0, 40));
    }
  }
  return bad.slice(0, 5);
});
ok(wide.length === 0, `no clipped text (${wide.join(' | ')})`);

/* Vertical rhythm: the gap between stacked blocks has to land on one of the
   scale steps. Off-scale gaps are what make a page read as badly spaced, and
   they are invisible to an overflow or clipping check. */
const rhythm = await page.evaluate(() => {
  const main = document.querySelector('main');
  // read the scale the stylesheet declares rather than repeating it here
  const rootStyle = getComputedStyle(document.documentElement);
  const rem = parseFloat(rootStyle.fontSize);
  const steps = ['--s1', '--s2', '--s3', '--s4']
    .map(k => parseFloat(rootStyle.getPropertyValue(k)) * rem)
    .filter(n => Number.isFinite(n) && n > 0);
  const off = [];
  const seen = new Set();
  let prev = null;
  for (const el of main.children) {
    const cs = getComputedStyle(el);
    if (cs.display === 'none') continue;
    const r = el.getBoundingClientRect();
    if (r.height === 0) continue;
    if (prev !== null) {
      const gap = r.top - prev;
      if (!steps.some(s => Math.abs(s - gap) <= 1)) {
        const key = Math.round(gap);
        if (!seen.has(key)) { seen.add(key); off.push(`${Math.round(gap)}px before ${el.tagName.toLowerCase()}`); }
      }
    }
    prev = r.bottom;
  }
  return { off, steps: steps.map(Math.round) };
});
ok(rhythm.off.length === 0, `vertical rhythm is on scale [${rhythm.steps.join(', ')}] (${rhythm.off.join(' | ') || 'no off-scale gaps'})`);

// the front matter must sit inside the text column, not full-bleed across the window
const head = await page.evaluate(() => {
  const h = document.querySelector('.doc-head');
  if (!h) return { missing: true };
  const measure = parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--measure')) * parseFloat(getComputedStyle(document.documentElement).fontSize);
  const main = document.querySelector('main');
  const box = el => el ? el.getBoundingClientRect().width : null;
  return {
    inMain: h.parentElement === main,
    headerW: box(h),
    mainW: box(main) - parseFloat(getComputedStyle(main).paddingLeft) - parseFloat(getComputedStyle(main).paddingRight),
    leadW: box(document.querySelector('.lead')),
    keywordsW: box(document.querySelector('.keywords')),
    measure,
  };
});
ok(!head.missing, 'front matter (.doc-head) present');
ok(head.inMain, `front matter is inside <main>, not a full-bleed sibling (header ${Math.round(head.headerW)}px vs column ${Math.round(head.mainW)}px)`);
ok(head.leadW !== null && head.leadW <= head.measure, `description is set as an abstract block, no wider than the measure (${Math.round(head.leadW)}px <= ${Math.round(head.measure)}px)`);

const tiny = await page.evaluate(() => {
  const bad = [];
  for (const el of document.querySelectorAll('p, li, td, figcaption, pre, body *')) {
    const fs = parseFloat(getComputedStyle(el).fontSize);
    if (fs && fs < 8 && el.textContent.trim()) bad.push(el.tagName + ' ' + fs + 'px');
  }
  return [...new Set(bad)].slice(0, 5);
});
ok(tiny.length === 0, `no unreadable text (${tiny.join(' | ')})`);

console.log('\n== dark mode contrast (WCAG)');
const contrast = await page.evaluate(() => {
  const lum = ([r, g, b]) => {
    const f = c => { c /= 255; return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4); };
    return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
  };
  // accepts rgb()/rgba() (0-255) and color(srgb ..) (0-1)
  const parse = s => {
    if (!s) return null;
    const m = (s.match(/[\d.]+/g) || []).map(Number);
    if (m.length < 3) return null;
    const unit = /^color\(/.test(s) ? 255 : 1;
    const a = m.length > 3 ? m[3] : 1;
    if (a === 0) return null;
    return { c: m.slice(0, 3).map(v => v * unit), a };
  };
  const over = (fg, bg) => fg.c.map((v, i) => v * fg.a + bg[i] * (1 - fg.a));
  const bgOf = el => {
    const stack = [];
    let n = el;
    while (n && n !== document.documentElement) {
      const p = parse(getComputedStyle(n).backgroundColor);
      if (p) { stack.push(p); if (p.a === 1) break; }
      n = n.parentElement;
    }
    let base = [255, 255, 255];
    for (let i = stack.length - 1; i >= 0; i--) base = over(stack[i], base);
    return base;
  };
  const ratio = (a, b) => {
    const [x, y] = [lum(a), lum(b)].sort((m, n) => n - m);
    return (x + 0.05) / (y + 0.05);
  };
  const out = [];
  for (const el of document.querySelectorAll('p, li, td, th, h1, h2, h3, figcaption, code, a, .note, blockquote')) {
    if (!el.textContent.trim()) continue;
    if (el.querySelector('p, li, td')) continue;
    const fg = parse(getComputedStyle(el).color);
    if (!fg || fg.a === 0) continue;
    const r = ratio(fg.c, bgOf(el));
    const size = parseFloat(getComputedStyle(el).fontSize);
    const bold = parseInt(getComputedStyle(el).fontWeight) >= 700;
    const large = size >= 24 || (size >= 18.66 && bold);
    const need = large ? 3 : 4.5;
    if (r < need) out.push(`${el.tagName} ${size}px ratio ${r.toFixed(2)} < ${need}`);
  }
  return [...new Set(out)].slice(0, 8);
});
ok(contrast.length === 0, `text contrast ok (${contrast.join(' | ')})`);

await page.screenshot({ path: '/tmp/qa/shot-full.png', fullPage: true });
console.log('\nscreenshot -> /tmp/qa/shot-full.png');
console.log(`\n${fails.length === 0 ? 'OK' : 'FAILED'}: ${pass} passed, ${fails.length} failed`);
if (fails.length) { for (const f of fails) console.log('  - ' + f); }
await browser.close();
process.exit(fails.length ? 1 : 0);
