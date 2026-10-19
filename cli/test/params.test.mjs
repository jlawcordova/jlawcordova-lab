// The shared param checker and the command-line parser.

import assert from 'node:assert/strict';
import { describe, test } from 'node:test';

import { parseCommandLine } from '../lib/argv.mjs';
import { UsageError } from '../lib/errors.mjs';
import { checkParams } from '../lib/params.mjs';
import { createRegistry, registry } from '../lib/registry.mjs';

const schema = {
  type: /** @type {const} */ ('object'),
  properties: {
    name: { type: /** @type {const} */ ('string') },
    scale: { type: /** @type {const} */ ('integer'), minimum: 1, maximum: 16, default: 1 },
    ratio: { type: /** @type {const} */ ('number') },
    flag: { type: /** @type {const} */ ('boolean'), default: false },
    at: { type: /** @type {const} */ ('array'), items: { type: /** @type {const} */ ('integer') }, minItems: 3, maxItems: 3 },
    kind: { type: /** @type {const} */ ('string'), enum: ['sprite', 'block'] },
    data: { type: /** @type {const} */ ('object') },
    path: { type: /** @type {const} */ ('string'), pattern: '^\\d+(\\.\\d+)*$' },
  },
  required: ['name'],
};

describe('checkParams', () => {
  test('fills defaults and keeps valid values', () => {
    assert.deepEqual(checkParams(schema, { name: 'a' }), { name: 'a', scale: 1, flag: false });
    assert.deepEqual(checkParams(schema, { name: 'a', scale: 3, at: [1, 2, 0], kind: 'block' }), { name: 'a', scale: 3, flag: false, at: [1, 2, 0], kind: 'block' });
  });

  test('coerces command-line strings to the schema types', () => {
    const out = checkParams(schema, { name: 'a', scale: '4', ratio: '0.5', flag: 'true', at: '14,6,1', data: '{"x":1}', path: '5.2' }, { coerce: true });
    assert.deepEqual(out, { name: 'a', scale: 4, ratio: 0.5, flag: true, at: [14, 6, 1], data: { x: 1 }, path: '5.2' });
    assert.deepEqual(checkParams(schema, { name: 'a', at: ['14', '6', '-1'] }, { coerce: true }).at, [14, 6, -1]);
    assert.equal(checkParams(schema, { name: 'a', flag: 'no' }, { coerce: true }).flag, false);
  });

  test('without coerce, strings are not numbers', () => {
    assert.throws(() => checkParams(schema, { name: 'a', scale: '4' }), /scale must be a whole number, got "4"/);
  });

  test('reports every problem at once, as a usage error', () => {
    assert.throws(
      () => checkParams(schema, { scale: 0, kind: 'blob', at: [1, 2], sacle: 2, path: 'x' }, { command: 'demo' }),
      (err) => {
        assert.ok(err instanceof UsageError);
        assert.equal(err.exitCode, 2);
        assert.match(err.message, /^demo: /);
        for (const part of ['unknown param sacle (did you mean scale?)', 'name is required', 'scale must be at least 1', 'kind must be one of sprite, block', 'at needs at least 3 items', 'path "x" doesn\'t match']) {
          assert.ok(err.message.includes(part), `${err.message} should include ${part}`);
        }
        return true;
      },
    );
  });

  test('rejects non-integers, non-finite numbers and bad booleans', () => {
    assert.throws(() => checkParams(schema, { name: 'a', scale: '2.5' }, { coerce: true }), /scale must be a whole number/);
    assert.throws(() => checkParams(schema, { name: 'a', ratio: 'abc' }, { coerce: true }), /ratio must be a number/);
    assert.throws(() => checkParams(schema, { name: 'a', flag: 'maybe' }, { coerce: true }), /flag must be true or false/);
    assert.throws(() => checkParams(schema, { name: 'a', data: '{bad' }, { coerce: true }), /data must be an object/);
  });
});

describe('parseCommandLine', () => {
  const parse = (/** @type {string} */ line) => parseCommandLine(line.split(' ').filter(Boolean), registry);

  test('leading words name the command; positionals fill its args', () => {
    const r = parse('scene render hero-island --scale 2');
    assert.equal(r.kind, 'command');
    assert.equal(r.kind === 'command' && r.command.name, 'scene.render');
    assert.deepEqual(r.kind === 'command' && r.params, { name: 'hero-island', scale: '2' });
  });

  test('a single-word command takes positionals (check <name>)', () => {
    const r = parse('check hero-island');
    assert.equal(r.kind === 'command' && r.command.name, 'check');
    assert.deepEqual(r.kind === 'command' && r.params, { name: 'hero-island' });
  });

  test('flags: --flag value, --flag=value, booleans and --no-flag', () => {
    const a = parse('scene render --name=hero-island --time=400');
    assert.deepEqual(a.kind === 'command' && a.params, { name: 'hero-island', time: '400' });
    const b = parse('scene show hero-island --doc');
    assert.deepEqual(b.kind === 'command' && b.params, { name: 'hero-island', doc: 'true' });
    const c = parse('scene show hero-island --no-doc');
    assert.deepEqual(c.kind === 'command' && c.params, { name: 'hero-island', doc: 'false' });
  });

  test('global flags go anywhere and are not params', () => {
    const r = parse('--json scene list --site /x');
    assert.equal(r.kind === 'command' && r.command.name, 'scene.list');
    assert.deepEqual(r.globals, { json: true, help: false, site: '/x' });
    assert.deepEqual(r.kind === 'command' && r.params, {});
  });

  test('lists: comma-separated, repeated flags, or the rest of the positionals', () => {
    const reg = createRegistry([
      {
        name: 'demo.place',
        summary: 'demo',
        params: { type: 'object', properties: { scene: { type: 'string' }, at: { type: 'array', items: { type: 'integer' } }, tags: { type: 'array', items: { type: 'string' } } } },
        args: ['scene', 'tags'],
        mutates: false,
        run: () => ({}),
      },
    ]);
    const r = parseCommandLine(['demo', 'place', 'island', 'a', 'b', '--at', '14,6', '--at', '1'], reg);
    assert.deepEqual(r.kind === 'command' && r.params, { scene: 'island', tags: ['a', 'b'], at: ['14,6', '1'] });
    assert.deepEqual(checkParams(reg.get('demo.place').params, r.kind === 'command' ? r.params : {}, { coerce: true }), { scene: 'island', tags: ['a', 'b'], at: [14, 6, 1] });
  });

  test('help topics', () => {
    assert.deepEqual(parse('help').kind, 'help');
    assert.deepEqual(parse('').kind === 'help' && parse('').usage, true);
    const g = parse('scene --help');
    assert.deepEqual(g.kind === 'help' && g.topic, ['scene']);
    const c = parse('scene place-nothing --help');
    assert.deepEqual(c.kind === 'help' && c.topic, ['scene']);
    const d = parse('scene render --help');
    assert.deepEqual(d.kind === 'help' && d.topic, ['scene', 'render']);
    const e = parse('scene');
    assert.equal(e.kind === 'help' && e.usage, true);
  });

  test('usage errors', () => {
    assert.throws(() => parse('scene lsit'), /unknown command scene lsit \(did you mean scene list\?\)/);
    assert.throws(() => parse('scene frobnicate'), (err) => err instanceof UsageError && /unknown command scene frobnicate/.test(err.message));
    assert.throws(() => parse('scnee list'), /unknown command scnee \(did you mean scene\?\)/);
    assert.throws(() => parse('scene render x --scael 2'), /unknown flag --scael \(did you mean --scale\?\)/);
    assert.throws(() => parse('scene render x y'), /unexpected y/);
    assert.throws(() => parse('scene render x --name y'), /name is given twice/);
    assert.throws(() => parse('scene render x --scale'), /--scale needs a value/);
    assert.throws(() => parse('scene render x --site'), /--site needs a value/);
    assert.throws(() => parse('scene render x -z'), /unknown flag -z/);
  });

  test('negative numbers are values, not flags', () => {
    const reg = createRegistry([
      { name: 'demo.nudge', summary: 'demo', params: { type: 'object', properties: { dx: { type: 'integer' } } }, args: ['dx'], mutates: false, run: () => ({}) },
    ]);
    const r = parseCommandLine(['demo', 'nudge', '-3'], reg);
    assert.deepEqual(r.kind === 'command' && r.params, { dx: '-3' });
  });
});

describe('registry', () => {
  test('every command is well formed and has a group', () => {
    const names = [...registry.commands.keys()];
    const readOnly = [
      'activity.list', 'art.preview', 'art.sizes', 'check', 'doc.close', 'doc.open', 'doc.status', 'document.info', 'object.list', 'object.pick', 'object.render', 'object.show',
      'palette.get', 'proposal.list', 'proposal.render', 'proposal.show', 'scene.items', 'scene.list', 'scene.render', 'scene.show',
    ];
    const mutating = [
      'doc.merge', 'doc.redo', 'doc.reload', 'doc.revert', 'doc.save', 'doc.undo', 'document.copy', 'document.new',
      'object.animate', 'object.erase', 'object.fill', 'object.frame.add', 'object.frame.delete', 'object.frame.duplicate', 'object.frame.move',
      'object.layer.add', 'object.layer.delete', 'object.layer.duplicate', 'object.layer.move', 'object.map.set', 'object.new', 'object.paint', 'object.resize', 'object.still',
      'proposal.apply', 'proposal.reject',
      'scene.duplicate', 'scene.group.set', 'scene.key.delete', 'scene.key.move', 'scene.key.set', 'scene.lower', 'scene.motion.path', 'scene.motion.set', 'scene.move',
      'scene.new', 'scene.place', 'scene.raise', 'scene.remove', 'scene.set', 'scene.snap', 'scene.track.add', 'scene.track.remove',
    ];
    assert.deepEqual(names, [...readOnly, ...mutating, 'art.build'].sort());
    for (const name of readOnly) assert.equal(registry.get(name)?.mutates, false, name);
    for (const name of mutating) assert.equal(registry.get(name)?.mutates, true, name);
    assert.deepEqual(
      names.filter((n) => registry.get(n)?.session),
      ['doc.close', 'doc.merge', 'doc.open', 'doc.redo', 'doc.reload', 'doc.revert', 'doc.save', 'doc.status', 'doc.undo'],
    );
    assert.deepEqual(names.filter((n) => registry.get(n)?.writes), ['art.build', 'document.copy', 'document.new', 'proposal.apply', 'proposal.reject']);
    assert.deepEqual([...registry.groups().keys()], ['activity', 'art', 'check', 'doc', 'document', 'object', 'palette', 'proposal', 'scene']);
  });

  test('malformed definitions are refused', () => {
    const base = { name: 'a.b', summary: 's', params: { type: 'object', properties: {} }, mutates: false, run: () => 1 };
    assert.throws(() => createRegistry([{ ...base, name: 'A.b' }]), /name must be dotted/);
    assert.throws(() => createRegistry([{ ...base, args: ['x'] }]), /args names x/);
    assert.throws(() => createRegistry([base, base]), /defined twice/);
    // @ts-expect-error: mutates is required
    assert.throws(() => createRegistry([{ ...base, mutates: undefined }]), /mutates/);
  });

  test('call checks params before running', async () => {
    let ran = false;
    const reg = createRegistry([
      { name: 'demo.x', summary: 's', params: { type: 'object', properties: { n: { type: 'integer' } }, required: ['n'] }, mutates: false, run: (_ctx, p) => ((ran = true), p) },
    ]);
    await assert.rejects(reg.call({}, 'demo.x', {}), (err) => err instanceof UsageError);
    assert.equal(ran, false);
    assert.deepEqual(await reg.call({}, 'demo.x', { n: '2' }, { coerce: true }), { n: 2 });
    await assert.rejects(reg.call({}, 'demo.y'), (err) => err.code === 'not_found' && /did you mean demo\.x/.test(err.message));
  });
});
