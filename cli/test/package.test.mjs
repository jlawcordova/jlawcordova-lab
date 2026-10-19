// The package a site installs from git (docs/intents/2026-10-pixellab-document/spec.md, R15):
// its exports resolve to the engine modules, its files cover what the CLI needs,
// and its version is the CLI's.

import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, test } from 'node:test';

import { VERSION } from '../lib/server-stdio.mjs';
import { REPO } from './helpers.mjs';

const pkg = JSON.parse(readFileSync(join(REPO, 'package.json'), 'utf8'));

describe('package.json (R15)', () => {
  test('R15: it is named pixellab and its version is the CLI\'s', () => {
    assert.equal(pkg.name, 'pixellab');
    assert.equal(pkg.version, VERSION);
    assert.equal(pkg.bin.pixellab, 'cli/bin/pixellab.mjs');
  });

  test('R15: every export is an engine module that loads, and the engine modules are all exported', async () => {
    const expected = ['./engine', './iso', './motion', './serialize', './edit', './starter', './svg', './compile', './preview', './document'];
    assert.deepEqual(Object.keys(pkg.exports).sort(), expected.sort());
    for (const [name, file] of Object.entries(pkg.exports)) {
      assert.ok(existsSync(join(REPO, file)), `${name} → ${file} exists`);
      const mod = await import(join(REPO, file));
      assert.ok(Object.keys(mod).length > 0, `${name} exports something`);
    }
  });

  test('R15: the files a git install keeps hold the CLI and the engine', () => {
    for (const path of ['cli/bin', 'cli/lib', 'cli/README.md']) assert.ok(pkg.files.includes(path), path);
    assert.ok(!pkg.files.some((/** @type {string} */ f) => f.startsWith('app') || f.startsWith('docs')), 'the app and docs stay out');
  });
});
