import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';

const exes = process.platform === 'win32' ? 'nano.exe' : 'nano';
const bin = './target/' + (process.env.NANO_DEBUG ? 'debug/' : 'release/') + exes;
if (!existsSync(bin)) {
  console.error('nano: build the binary first:  cargo build --release');
  process.exit(1);
}

const build = (theme, out) => {
  const r = spawnSync(bin, ['build', 'examples/tour.nano', '--theme', theme, '-o', out, '-f', 'html'],
    { encoding: 'utf8' });
  if (r.status !== 0) {
    console.error('nano: build failed for theme ' + theme + '\n' + r.stderr);
    process.exit(1);
  }
};

const docs = ['examples/tour.html'];
build('default', docs[0]);
for (const theme of ['serif', 'dark', 'minimal']) {
  const out = '/tmp/nano-theme-' + theme + '.html';
  build(theme, out);
  docs.push(out);
}
let failed = 0;
const run = (script, args) => {
  console.log('\n=== ' + script + ' ' + args.join(' '));
  const r = spawnSync(process.execPath, ['tests_js/' + script, ...args], { stdio: 'inherit' });
  // 77 = playwright not installed, which is a skip rather than a failure
  if (r.status === 0 || r.status === 77) {
    if (r.status === 77) console.log('(skipped: no browser available)');
  } else failed++;
};

run('run.mjs', []);
for (const d of docs) {
  run('browser.mjs', [d]);
  run('interactive.mjs', [d]);
}

console.log(failed === 0 ? '\nALL OK' : '\n' + failed + ' SUITE(S) FAILED');
process.exit(failed ? 1 : 0);
