// What the states tests share (docs/intents/2026-10-object-states/spec.md, D6): the program, the
// committed fixture document with states (fixtures/states.pixellab), and its scene compiled to the SVG
// a site inlines. Everything is made in a temp folder: nothing is written to the repo.

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
export const PROGRAM = join(REPO, 'target/release/pixellab');
export const DOCUMENT = join(REPO, 'fixtures/states.pixellab');

/** A temp folder for one test file, with the program's state dir and HOME inside it. */
export function tempArea() {
  const dir = mkdtempSync(join(tmpdir(), 'pixellab-states-'));
  return { dir, remove: () => rmSync(dir, { recursive: true, force: true }) };
}

/** Runs the program on the fixture document; returns its JSON. */
export function json(/** @type {string} */ dir, /** @type {string[]} */ args) {
  const r = spawnSync(PROGRAM, [...args, '--document', DOCUMENT, '--json'], {
    encoding: 'utf8',
    env: { ...process.env, HOME: dir, PIXELLAB_STATE: join(dir, 'state') },
    maxBuffer: 64 * 1024 * 1024,
  });
  assert.equal(r.status, 0, `pixellab ${args.join(' ')}: ${r.stdout}${r.stderr}`);
  return JSON.parse(r.stdout);
}

/** Compiles the fixture's scene with `art build` and returns the SVG's path and text. */
export function compileFixture(/** @type {string} */ dir) {
  const out = join(dir, 'out');
  const r = spawnSync(PROGRAM, ['art', 'build', '--dir', out, '--document', DOCUMENT], {
    encoding: 'utf8',
    env: { ...process.env, HOME: dir, PIXELLAB_STATE: join(dir, 'state') },
  });
  assert.equal(r.status, 0, `art build: ${r.stdout}${r.stderr}`);
  const path = join(out, 'states.svg');
  return { path, text: readFileSync(path, 'utf8') };
}

/** The `<style>` of a compiled SVG. */
export function styleOf(/** @type {string} */ svg) {
  const m = /<style>([\s\S]*?)<\/style>/.exec(svg);
  assert.ok(m, 'the SVG has a style');
  return m[1];
}
