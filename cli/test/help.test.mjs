// Help output and exit codes, through the real binary.

import assert from 'node:assert/strict';
import { after, before, describe, test } from 'node:test';

import { registry } from '../lib/registry.mjs';
import { commandWords } from '../lib/argv.mjs';
import { makeSite, needsSite, runCli, spawnCli } from './helpers.mjs';

describe('help', () => {
  test('pixellab help lists every command, the global flags, the mapping and exit codes', async () => {
    const r = await spawnCli(['help']);
    assert.equal(r.code, 0);
    assert.equal(r.stderr, '');
    for (const cmd of registry.commands.values()) {
      assert.match(r.stdout, new RegExp(`^  ${commandWords(cmd.name)}\\b.*${cmd.summary.replace(/[()]/g, '\\$&')}$`, 'm'), cmd.name);
    }
    for (const part of ['--json', '--site <path>', '--out <file.png>', 'Positionals fill', 'kebab-case', '--no-flag', 'comma-separated', 'Exit codes: 0 ok, 1 command error', '2 usage error']) {
      assert.ok(r.stdout.includes(part), part);
    }
    assert.equal((await spawnCli(['--help'])).stdout, r.stdout);
  });

  test('bare pixellab prints help to stderr with exit code 2', async () => {
    const r = await spawnCli([]);
    assert.equal(r.code, 2);
    assert.equal(r.stdout, '');
    assert.match(r.stderr, /^pixellab: read and edit/);
  });

  test('group help: pixellab scene --help, pixellab help scene', async () => {
    const r = await spawnCli(['scene', '--help']);
    assert.equal(r.code, 0);
    assert.match(r.stdout, /^pixellab scene: 21 commands$/m);
    assert.match(r.stdout, /^ {2}pixellab scene render <name> \[--scale <n>\] \[--time <n>\] \[--theme <light\|dark>\] \[--show <text>\] \[--out <file\.png>\]$/m);
    assert.match(r.stdout, /^ {2}pixellab scene list$/m);
    assert.equal((await spawnCli(['help', 'scene'])).stdout, r.stdout);
    const bare = await spawnCli(['scene']);
    assert.equal(bare.code, 2);
    assert.equal(bare.stderr, r.stdout);
  });

  test('command help: pixellab scene render --help', async () => {
    const r = await spawnCli(['scene', 'render', '--help']);
    assert.equal(r.code, 0);
    assert.match(r.stdout, /^Usage: pixellab scene render <name> \[--scale <n>\]/);
    assert.match(r.stdout, /^Command: scene\.render \(read-only\)$/m);
    assert.match(r.stdout, /^ {2}<name>, --name +scene name \(or scenes\/<name>\) \(string, required\)$/m);
    assert.match(r.stdout, /^ {2}--scale <n> +whole-pixel zoom of the PNG \(integer, default 1, 1 to 16\)$/m);
    assert.match(r.stdout, /^Examples:\n {2}pixellab scene render hero-island --scale 2 --out/m);
    assert.equal((await spawnCli(['help', 'scene', 'render'])).stdout, r.stdout);
    assert.equal((await spawnCli(['scene', 'render', 'hero-island', '-h'])).stdout, r.stdout);
  });

  test('help for every command and group renders', async () => {
    for (const cmd of registry.commands.values()) {
      const r = await runCli([...cmd.name.split('.'), '--help']);
      assert.equal(r.code, 0, cmd.name);
      assert.match(r.stdout, new RegExp(`^Usage: pixellab ${commandWords(cmd.name)}`), cmd.name);
    }
    const check = await runCli(['check', '--help']);
    assert.match(check.stdout, /^Usage: pixellab check \[name\] \[--kind <object\|scene>\]/);
  });

  test('usage errors exit 2 with a pointer to help', async () => {
    for (const args of [['scene', 'frobnicate'], ['nope'], ['scene', 'render', 'x', '--bogus'], ['help', 'nope'], ['scene', 'render']]) {
      const r = await spawnCli(args);
      assert.equal(r.code, 2, args.join(' '));
      assert.equal(r.stdout, '');
      assert.match(r.stderr, /^pixellab: /, args.join(' '));
    }
    const json = await spawnCli(['scene', 'frobnicate', '--json']);
    assert.equal(json.code, 2);
    assert.equal(JSON.parse(json.stderr).error.code, 'usage');
  });
});

describe('exit codes', needsSite, () => {
  /** @type {Awaited<ReturnType<typeof makeSite>>} */
  let site;
  before(async () => {
    site = await makeSite();
  });
  after(() => site.cleanup());

  test('0 ok, 1 command error, 2 usage error', async () => {
    const env = site.env({ PIXELLAB_SITE: site.root });
    assert.equal((await spawnCli(['scene', 'list'], { env })).code, 0);
    assert.equal((await spawnCli(['scene', 'show', 'no-such-scene'], { env })).code, 1);
    assert.equal((await spawnCli(['scene', 'render', 'hero-island', '--scale', 'big'], { env })).code, 2);
    // A missing required positional is a usage error, before the site is read.
    const missing = await spawnCli(['scene', 'show'], { env });
    assert.equal(missing.code, 2);
    assert.match(missing.stderr, /scene\.show: name is required/);
  });

  test('stdout piped into head closes quietly', async () => {
    const { spawn } = await import('node:child_process');
    const { BIN } = await import('./helpers.mjs');
    const code = await new Promise((resolve) => {
      const child = spawn(process.execPath, [BIN, 'palette', 'get', '--site', site.root], { env: site.env() });
      let stderr = '';
      child.stderr.on('data', (d) => (stderr += d));
      child.stdout.once('data', () => child.stdout.destroy());
      child.on('close', (c) => resolve({ c, stderr }));
    });
    assert.equal(/** @type {any} */ (code).stderr, '');
  });
});

describe('docs/cli.md', () => {
  test('is generated from the registry and up to date (npm run docs:cli)', async () => {
    const { spawn } = await import('node:child_process');
    const { join } = await import('node:path');
    const { REPO } = await import('./helpers.mjs');
    const code = await new Promise((resolve) => {
      const child = spawn(process.execPath, [join(REPO, 'scripts', 'docs-cli.mjs'), '--check'], { stdio: 'inherit' });
      child.on('close', resolve);
    });
    assert.equal(code, 0, 'docs/cli.md is out of date: run npm run docs:cli');
  });
});
