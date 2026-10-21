#!/usr/bin/env node
// Writes docs/cli.md from the program's command registry, so the command list
// never drifts from the code: `npm run docs:cli`. The text is the program's
// `help --markdown` (target/release/pixellab, built first). With --check it
// only compares, and exits 1 if docs/cli.md is out of date (the tests run that).

import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const FILE = fileURLToPath(new URL('../docs/cli.md', import.meta.url));
const REPO = fileURLToPath(new URL('..', import.meta.url));
const PROGRAM = join(REPO, 'target/release/pixellab');

function markdown() {
  const env = { ...process.env, PATH: `${join(homedir(), '.cargo/bin')}:${process.env.PATH ?? ''}` };
  // A no-op when the program is up to date.
  const built = spawnSync('cargo', ['build', '--release', '--quiet', '--bin', 'pixellab'], { cwd: REPO, env, stdio: ['ignore', 'inherit', 'inherit'] });
  if (built.status !== 0) {
    process.stderr.write('docs/cli.md: cargo build --release failed\n');
    process.exit(1);
  }
  // A clean env: no document, so the text never depends on the machine.
  delete env.PIXELLAB_DOC;
  delete env.PIXELLAB_SITE;
  const r = spawnSync(PROGRAM, ['help', '--markdown'], { cwd: REPO, env, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  if (r.status !== 0 || !r.stdout.startsWith('#')) {
    process.stderr.write(`docs/cli.md: ${PROGRAM} help --markdown failed\n${r.stderr}`);
    process.exit(1);
  }
  return r.stdout.replace(/\n{3,}/g, '\n\n').replace(/\n*$/, '\n');
}

const text = markdown();
if (process.argv.includes('--check')) {
  let current = '';
  try {
    current = readFileSync(FILE, 'utf8');
  } catch {
    // missing
  }
  if (current !== text) {
    process.stderr.write('docs/cli.md is out of date: run npm run docs:cli\n');
    process.exitCode = 1;
  }
} else {
  writeFileSync(FILE, text);
  process.stdout.write(`wrote docs/cli.md (${(text.match(/^#### /gm) ?? []).length} commands)\n`);
}
