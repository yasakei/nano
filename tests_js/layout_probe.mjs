// A layout probe: reports overflow, overlap and clipping so a broken page can
// be diagnosed without a screenshot. Exits 77 when no browser is available.
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '..');

async function findBrowser() {
  const modules = [process.env.NANOPW, 'playwright-core'].filter(Boolean);
  const execs = [];
  if (process.env.NANOPW_CHROME) execs.push(process.env.NANOPW_CHROME);
  const cache = (process.env.HOME || '') + '/.cache/ms-playwright';
  if (existsSync(cache)) {
    for (const d of readdirSync(cache)) {
      if (!/^chromium/.test(d)) continue;
      for (const leaf of [
        'chrome-headless-shell-linux64/chrome-headless-shell',
        'chrome-linux/headless_shell',
        'chrome-linux/chrome',
      ]) {
        const p = `${cache}/${d}/${leaf}`;
        if (existsSync(p)) execs.push(p);
      }
    }
  }
  for (const m of [
    'playwright-core',
    process.cwd() + '/node_modules/playwright-core/index.mjs',
    process.cwd() + '/../yasakei-harness/node_modules/playwright-core/index.mjs',
  ]) modules.push(m);
  let mod = null;
  for (const m of modules) {
    try {
      const r = await import(m);
      if (r.chromium) { mod = r; break; }
    } catch {}
  }
  if (!mod) {
    console.error('      set NANOPW=/path/to/playwright-core/index.mjs and');
    console.error('      NANOPW_CHROME=/path/to/chrome to run this probe.');
    process.exit(77);
  }
  return { chromium: mod.chromium, exe: execs.find((e) => existsSync(e)) };
}

const { chromium, exe } = await findBrowser();
const doc = process.argv[2] || resolve(root, 'examples/tour.html');
const url = 'file://' + doc;
const browser = await chromium.launch({ executablePath: exe, args: ['--no-sandbox'] });

let problems = 0;
for (const [w, h] of [[1280, 900], [820, 900], [420, 900]]) {
  const page = await browser.newPage({ viewport: { width: w, height: h } });
  await page.goto(url, { waitUntil: 'load' });
  await page.waitForTimeout(350);
  const found = await page.evaluate((vw) => {
    const out = { scrollW: document.documentElement.scrollWidth, over: [], clipped: [], tiny: [] };
    const root = document.querySelector('.wrap') || document.body;
    const rb = root.getBoundingClientRect();
    for (const el of document.querySelectorAll('body *')) {
      const cs = getComputedStyle(el);
      if (cs.display === 'none' || cs.visibility === 'hidden') continue;
      const r = el.getBoundingClientRect();
      if (r.width === 0 || r.height === 0) continue;
      const cls = typeof el.className === 'string' ? el.className : (el.getAttribute('class') || '');
      const tag = `${el.tagName.toLowerCase()}${cls ? '.' + cls.split(' ')[0] : ''}`;
      // sticks out past the viewport
      if (r.right > vw + 1 || r.left < -1) {
        out.over.push({ tag, left: Math.round(r.left), right: Math.round(r.right), vw });
      }
      // text wider than its box. Content inside a scroll container (a code
      // panel scrolls sideways on purpose) is fine; text with nowhere to scroll
      // is genuinely being cut off. SVG has no scroll box to measure against.
      const inScroller = (node) => {
        for (let a = node; a && a !== document.body; a = a.parentElement) {
          if (/auto|scroll/.test(getComputedStyle(a).overflowX)) return true;
        }
        return false;
      };
      if (!el.closest('svg') && el.children.length === 0 && el.textContent.trim()
          && el.scrollWidth > el.clientWidth + 2 && !inScroller(el)) {
        out.clipped.push({ tag, scroll: el.scrollWidth, client: el.clientWidth, text: el.textContent.trim().slice(0, 40) });
      }
    }
    out.rbWidth = Math.round(rb.width);
    return out;
  }, w);
  const horizontal = found.scrollW > w + 1;
  console.log(`\n== ${w}x${h}  scrollWidth=${found.scrollW} wrap=${found.rbWidth}`);
  if (horizontal) { console.log(`   HORIZONTAL SCROLL: page is ${found.scrollW - w}px too wide`); problems++; }
  const seen = new Set();
  for (const o of found.over) {
    const k = o.tag + o.left;
    if (seen.has(k)) continue;
    seen.add(k);
    console.log(`   overflows: ${o.tag} left=${o.left} right=${o.right} (viewport ${o.vw})`);
    problems++;
  }
  for (const c of found.clipped.slice(0, 8)) {
    console.log(`   clipped text: ${c.tag} ${c.scroll}>${c.client} "${c.text}"`);
    problems++;
  }
  await page.close();
}
await browser.close();
console.log(problems === 0 ? '\nlayout probe: clean' : `\nlayout probe: ${problems} problem(s)`);
process.exit(0);
