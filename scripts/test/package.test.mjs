// package.json and the cutover (docs/intents/2026-10-rust-engine/spec.md, R2, R10 and R29):
// the engine is Rust, so nothing in the repo is a Node package that exports one, and
// nothing reads the old PIXELLAB_LEGACY_SVG switch. Node is repo tooling only.

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, test } from 'node:test';
import { fileURLToPath } from 'node:url';

const REPO = fileURLToPath(new URL('../..', import.meta.url));
const pkg = JSON.parse(readFileSync(join(REPO, 'package.json'), 'utf8'));

describe('package.json (R29)', () => {
  test('R29: it is named pixellab, its version is 0.0.0-dev and it has no bin, exports or files', () => {
    assert.equal(pkg.name, 'pixellab');
    assert.equal(pkg.version, '0.0.0-dev', 'releases take their version from the tag; the repo is never bumped');
    assert.equal(pkg.bin, undefined, 'the program is Rust: no bin');
    assert.equal(pkg.exports, undefined, 'no module is exported');
    assert.equal(pkg.files, undefined, 'nothing is installed from git');
  });

  test('R2: nothing exports an engine, and cli/ is gone', () => {
    assert.ok(!existsSync(join(REPO, 'cli')), 'cli/ is deleted');
    for (const value of Object.values(pkg.scripts)) assert.ok(!/cli\/(bin|lib|test)/.test(value), value);
    assert.match(pkg.scripts.test, /cargo build --release/);
    assert.match(pkg.scripts.test, /scripts\/test\//);
  });

  test('R10: nothing in the repo reads PIXELLAB_LEGACY_SVG', () => {
    // This file names the variable, so it is left out of the search.
    const search = ['crates', 'scripts', 'app', '.github'].filter((dir) => existsSync(join(REPO, dir)));
    let out = '';
    try {
      out = execFileSync('git', ['grep', '-n', 'PIXELLAB_LEGACY_SVG', '--', ...search, ':!scripts/test/package.test.mjs'], { cwd: REPO, encoding: 'utf8' });
    } catch (e) {
      if (e.status !== 1) throw e; // 1: no match
    }
    assert.equal(out, '');
  });
});
