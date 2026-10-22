// scripts/acceptance.sh --only <name>[,<name>...] (and PIXELLAB_ACCEPT_ONLY): a stage name is a file in
// scripts/acceptance/ without its extension, every one of them is behind a `stage <name>` guard, and an
// unknown name fails before anything runs. The app is never started here: every case exits while the
// arguments are parsed, before the script looks for build/Pixel Lab.app or makes its out dir.
//
// The runs need zsh (macOS has it; the Linux CI runner may not), so they're skipped without it.

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { after, describe, test } from 'node:test';
import { fileURLToPath } from 'node:url';

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const SCRIPT = join(REPO, 'scripts/acceptance.sh');
const STAGES = readdirSync(join(REPO, 'scripts/acceptance'))
  .filter((f) => /\.(txt|mjs)$/.test(f))
  .map((f) => f.replace(/\.(txt|mjs)$/, ''))
  .sort();
const skip = spawnSync('zsh', ['-c', 'true']).status === 0 ? false : 'no zsh';

const tmp = mkdtempSync(join(tmpdir(), 'pixellab-acceptance-only-'));
after(() => rmSync(tmp, { recursive: true, force: true }));

/** Runs the script with a temp HOME and a missing site, so nothing real could be read even if parsing let it through. */
function accept(/** @type {string[]} */ args, /** @type {Record<string, string>} */ env = {}) {
  const base = { ...process.env, HOME: tmp, PIXELLAB_REAL_SITE: join(tmp, 'no-site') };
  delete base.PIXELLAB_ACCEPT_ONLY;
  return spawnSync('zsh', [SCRIPT, ...args], { encoding: 'utf8', env: { ...base, ...env } });
}

describe('scripts/acceptance.sh --only', () => {
  test('every stage in scripts/acceptance/ is behind a stage guard of its own name', () => {
    const source = readFileSync(SCRIPT, 'utf8');
    for (const name of STAGES) assert.match(source, new RegExp(`^if stage ${name}; then$`, 'm'), `${name} has no "if stage ${name}; then" in acceptance.sh`);
  });

  test('an unknown name fails early, lists the valid names, and makes no out dir', { skip }, () => {
    const out = join(tmp, 'out-unknown');
    const r = accept(['--only', 'layers,nope', out]);
    assert.equal(r.status, 2, r.stderr);
    assert.match(r.stderr, /unknown stage 'nope'/);
    for (const name of STAGES) assert.ok(r.stderr.includes(name), `the error lists ${name}: ${r.stderr}`);
    assert.equal(r.stdout, '', 'nothing ran');
    assert.ok(!existsSync(out), 'the out dir was not made');
  });

  test('PIXELLAB_ACCEPT_ONLY is read the same way, and --only wins over it', { skip }, () => {
    const viaEnv = accept([join(tmp, 'out-env')], { PIXELLAB_ACCEPT_ONLY: 'nope' });
    assert.equal(viaEnv.status, 2, viaEnv.stderr);
    assert.match(viaEnv.stderr, /unknown stage 'nope'/);
    const both = accept(['--only=also-nope', join(tmp, 'out-both')], { PIXELLAB_ACCEPT_ONLY: 'layers' });
    assert.equal(both.status, 2, both.stderr);
    assert.match(both.stderr, /unknown stage 'also-nope'/);
  });

  test('--only without names, an unknown option and two out dirs are refused', { skip }, () => {
    for (const args of [['--only'], ['--only', ','], ['--bogus'], [join(tmp, 'a'), join(tmp, 'b')]]) {
      const r = accept(args);
      assert.equal(r.status, 2, `${args.join(' ')}: ${r.stderr}`);
      assert.equal(r.stdout, '', `${args.join(' ')} ran nothing`);
    }
  });
});
