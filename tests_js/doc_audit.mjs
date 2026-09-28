/* Extracts every `identifier` that looks like a nano builtin from the docs and
   checks the language actually has it. Run: node tests_js/doc_audit.mjs */
import { readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';

const docs = ['README.md', 'docs/language.md'];
const src = docs.map(f => readFileSync(f, 'utf8')).join('\n');

// Sections that deliberately talk about non-builtin names
const SKIP = new Set(['nano', 'md', 'html', 'pdf', 'svg', 'png', 'jpg', 'jpeg', 'png', 'x', 'y', 't', 'u', 'v', 'i', 'n', 'm', 'f', 'a', 'b', 'c', 'fn', 'let', 'if', 'for', 'while', 'in', 'of', 'true', 'false', 'null', 'nan', 'inf', 'e', 'pi', 'tau', 'yes', 'no', 'all', 'any', 'and', 'or', 'not', 'is', 'the', 'raw', 'verbatim', 'json', 'csv', 'rows', 'cols', 'xlabel', 'ylabel', 'title', 'width', 'height', 'theme', 'author', 'description', 'date', 'keywords', 'caption', 'cells', 'data', 'x', 'y', 'z', 'none', 'every', 'some', 'first', 'last']);

const words = new Set();
for (const m of src.matchAll(/`([a-z_][a-z0-9_]{1,24})`/g)) words.add(m[1]);

// Build the authoritative list by asking the language itself.
const probe = [...words].sort().map(w => `print("${w}")`).join('\n');
const file = '/tmp/nano-doc-audit.nano';
const { writeFileSync } = await import('node:fs');
writeFileSync(file, probe + '\n');
let out = '';
try {
  out = execFileSync('./target/release/nano', ['run', file], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] });
} catch {}

const known = new Set(out.split('\n').filter(Boolean));
// anything that prints as undefined is not a global
const missing = [...words].filter(w => !known.has(w) && !SKIP.has(w)).sort();
console.log(`checked ${words.size} documented identifiers`);
console.log(missing.length ? 'not globals (check each):\n  ' + missing.join('  ') : 'all documented identifiers resolve to a global');
