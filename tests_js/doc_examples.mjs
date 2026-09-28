/* Runs every ```nano block in the docs to prove the documentation is true.
   Run: node tests_js/doc_examples.mjs                                       */
import { readFileSync, writeFileSync, mkdtempSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const bin = './target/release/nano';
const docs = ['README.md', 'docs/language.md', 'docs/guide.md'];

let total = 0;
let bad = 0;

/* A block is a runnable example unless it is a shell command, a bash fence, a
   directory listing, or deliberately fragmentary. */
function runnable(lang, body) {
  if (lang !== 'nano') return false;
  if (/^\s*(title|author|date|description|keywords|theme|subtitle)\s*[:=]/m.test(body) && !/^```/.test(body)) {
    // bare front-matter field list inside a fence: still fine to evaluate
  }
  // skip the "here is a front matter block" illustration that is a bare key list
  if (/^\s*(title|subtitle|author|date|description|keywords|theme)\s*=/m.test(body) &&
      !/[=!.]/.test(body.replace(/^\s*\w+\s*=\s*$/gm, ''))) {
    return body.split('\n').every(l => !l.trim() || /^\s*\w+\s*=/.test(l));
  }
  return true;
}

const work = mkdtempSync(join(tmpdir(), 'nano-doctest-'));

for (const doc of docs) {
  const text = readFileSync(doc, 'utf8');
  const lines = text.split('\n');
  console.log('\n### ' + doc);
  let i = 0;
  let idx = 0;
  while (i < lines.length) {
    // a fence may be longer than three backticks so that a document excerpt can
    // contain a nested ```nano fence
    const m = lines[i].match(/^(\s*)(`{3,})(\w*)\s*$/);
    if (!m) { i++; continue; }
    const indent = m[1].length;
    const ticks = m[2].length;
    const lang = m[3];
    let j = i + 1;
    const body = [];
    const close = new RegExp('^\\s*`' + '{' + ticks + ',}\\s*$');
    while (j < lines.length && !close.test(lines[j])) { body.push(lines[j]); j++; }
    const code = body.map(l => l.slice(Math.min(indent, l.length - l.trimStart().length))).join('\n');
    const startLine = i + 1;
    i = j + 1;
    if (!runnable(lang, code)) continue;

    idx++;
    total++;
    const f = join(work, `ex${idx}.nano`);
    writeFileSync(f, code + '\n');
    let out = '';
    let code1 = 0;
    try {
      out = execFileSync(bin, ['check', f], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
    } catch (e) {
      code1 = e.status || 1;
      out = (e.stdout || '') + (e.stderr || '');
    }
    const errs = out
      .split('\n')
      .filter(l => /^(error|warning):/.test(l) || /undefined|unterminated|expected|too many|not a function|no such/i.test(l));
    if (code1 !== 0 || errs.length) {
      bad++;
      console.log(`  FAIL example #${idx} at ${doc}:${startLine}`);
      for (const l of errs.slice(0, 4)) console.log('       ' + l);
      console.log('       ' + code.split('\n').slice(0, 3).join(' | '));
    } else {
      console.log(`  ok   ${doc}:${startLine}  (${code.split('\n').length} lines)`);
    }
  }
}

console.log(`\n${bad ? 'FAILED' : 'OK'}: ${total - bad}/${total} documented examples run`);
process.exit(bad ? 1 : 0);
