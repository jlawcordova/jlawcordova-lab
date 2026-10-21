// The parity check (docs/intents/2026-10-rust-engine/spec.md, R3) in npm test:
// the Node CLI and the Rust program over the fixture and both examples, with
// no differences. `npm test` builds the program first (cargo build --release).
// The owner's document is checked at the parity gate (plan.md step 10), not
// here: nothing in this repo's tests needs a site checkout (R20).

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const PROGRAM = join(REPO, 'target/release/pixellab');

test('R3: the Node CLI and the program agree on the fixture and both examples', { skip: existsSync(PROGRAM) ? false : `no program at ${PROGRAM}: run cargo build --release` }, () => {
  const r = spawnSync('node', [join(REPO, 'scripts/parity.mjs'), '--docs', 'sample,harbor,town', '--program', PROGRAM], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  assert.equal(r.status, 0, `${r.stdout}\n${r.stderr}`);
  assert.match(r.stdout, /\*\*No differences\.\*\*/);
});
