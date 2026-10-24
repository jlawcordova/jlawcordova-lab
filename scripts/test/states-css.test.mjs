// The CSS a compiled SVG plays states with (docs/intents/2026-10-object-states/spec.md, R9 to R14,
// R16, D2, D6): a string check of the generated rules' shape, on the committed fixture
// (fixtures/states.pixellab). It never opens a browser: states-browser.test.mjs does that. This
// check says the rules are there, in the order and under the selectors the site contract promises.
//
// `npm test` builds the program first (cargo build --release).

import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { after, before, describe, test } from 'node:test';

import { PROGRAM, compileFixture, styleOf, tempArea } from './states-fixture.mjs';

const skip = existsSync(PROGRAM) ? false : `no program at ${PROGRAM}: run cargo build --release`;
const area = tempArea();
after(() => area.remove());

/** @type {string} */
let svg;
/** @type {string} */
let css;
/** The rules of the no-preference block, split at each `}` that ends a rule. */
/** @type {string[]} */
let play;
/** @type {string} */
let reduced;

before(() => {
  if (skip) return;
  svg = compileFixture(area.dir).text;
  css = styleOf(svg);
  const media = /@media \(prefers-reduced-motion:no-preference\)\{([\s\S]*?)\}\}(?=@keyframes|\[data-pixellab|@media|$)/.exec(css);
  assert.ok(media, 'a block for what plays when motion is welcome');
  play = media[1].split(/(?<=\})(?=\[data-pixellab)/);
  const r = /@media \(prefers-reduced-motion:reduce\)\{([\s\S]*)\}$/.exec(css);
  assert.ok(r, 'a block for reduced motion');
  reduced = r[1];
});

/** The rules (in the no-preference block) that apply `animation` to an element class under a state's selectors. */
const rulesFor = (/** @type {string} */ cls, /** @type {string} */ attr, /** @type {string} */ state) =>
  play.filter((rule) => rule.includes(`[${attr}~="${state}"] .${cls}{`) || rule.includes(`[${attr}~="${state}"] .${cls},`));

/** The rule under a state's data-state selectors for an element class. */
const ruleOf = (/** @type {string} */ cls, /** @type {string} */ state) =>
  play.find((rule) => rule.includes(`[data-state~="${state}"] .${cls},`) || rule.includes(`[data-state~="${state}"] .${cls}{`));

describe('the CSS states play with (R9 to R16, D6)', { skip }, () => {
  test('R10: a state applies under a group or on the root that has its name in data-state, and with no attribute an item plays its start', () => {
    const [group, root] = ['[data-pixellab="states"] [data-state~="walk"] .states--s1-f1', '[data-pixellab="states"][data-state~="walk"] .states--s1-f1'];
    const rule = play.find((r) => r.includes(group) && r.includes(root));
    assert.ok(rule, 'both selectors in one rule');
    assert.match(rule, /\{animation:states--s1-walk-f1-l 600ms step-end infinite\}$/);
    // The start state needs no attribute: the plain rule has the lower specificity.
    assert.ok(play.some((r) => r.startsWith('[data-pixellab="states"] .states--s1-f0{animation:states--s1--idle-f0-l 1000ms step-end infinite}')));
    // The item that starts in walk plays it with no attribute.
    assert.ok(play.some((r) => r.startsWith('[data-pixellab="states"] .states--s2-f1{animation:states--s2--walk-f1-l 600ms step-end infinite}')));
  });

  test('R11: with no attribute the start state plays under names of its own, so naming it or taking the name off restarts it', () => {
    // <scene>--s<n>--<state>…: a state's name has no double hyphen, so no state's rules can have these names.
    assert.ok(css.includes('@keyframes states--s1--idle-f0-l{'), 'the start state\'s own keyframes');
    assert.ok(css.includes('@keyframes states--s1-idle-f0-l{'), 'beside the ones data-state="idle" plays');
    // The plain rules say no end: <scene>--<state> fires for a state a page names (R12).
    const plain = play.filter((r) => /^\[data-pixellab="states"\] \.states--s\d+(-f\d+)?\{/.test(r));
    assert.ok(plain.length > 0);
    assert.ok(plain.every((r) => !/states--[a-z-]+ \d+ms linear 1/.test(r)), plain.join('\n'));
  });

  test('R10: the state listed later in the object has its rules later, so it wins', () => {
    const order = (/** @type {string} */ state) => play.findIndex((r) => r.includes(`[data-state~="${state}"] .states--s1-f0`));
    const states = ['idle', 'walk', 'dead', 'hop', 'ping', 'pong'].map(order);
    assert.ok(states.every((n) => n >= 0), `every state has rules: ${states}`);
    assert.deepEqual([...states].sort((a, b) => a - b), states, 'in the order the states are listed');
  });

  test('R11: each state has animations of its own names, so a change of attribute restarts them', () => {
    const names = [...css.matchAll(/@keyframes (states--s1-[a-z-]+-f\d(?:-l)?)\{/g)].map((m) => m[1]);
    assert.equal(new Set(names).size, names.length, 'no keyframes twice');
    for (const state of ['idle', 'walk', 'dead', 'hop', 'ping', 'pong']) assert.ok(names.some((n) => n.startsWith(`states--s1-${state}-f`)), state);
  });

  test('R9, R12: a state that plays once has an end animation named <scene>--<state>, and data-play plays it as data-state does', () => {
    for (const [state, ms] of [['hop', 480], ['ping', 200], ['pong', 200], ['dead', 100], ['flicker', 320]]) {
      assert.ok(css.includes(`@keyframes states--${state}{from{--pl-end:0}to{--pl-end:1}}`), `${state}'s end animation`);
      assert.ok(play.some((r) => r.includes(`states--${state} ${ms}ms linear 1`)), `${state} ends after ${ms} ms (a hand-over when it hands over, when its last frame starts for a hold)`);
    }
    // dead holds: it says so when its second frame starts, 100 ms in; the other three hand over at the end of their frames.
    assert.ok(rulesFor('states--s1', 'data-play', 'hop').length > 0, 'hop answers to data-play');
    assert.equal(rulesFor('states--s1', 'data-play', 'walk').length, 0, 'a looping state does not');
    assert.equal(rulesFor('states--s1', 'data-play', 'idle').length, 0);
    assert.ok(!css.includes('@keyframes states--walk') && !css.includes('@keyframes states--idle'), 'a looping state fires nothing');
  });

  test('D2: the finite part plays once and holds, and the settle loop is a second animation delayed by it', () => {
    const element = ruleOf('states--s1', 'hop');
    assert.ok(element);
    assert.match(
      element,
      /\{animation:states--s1-hop-m 480ms step-end 1 forwards,states--s1-hop-ml 1000ms step-end 480ms infinite,states--hop 480ms linear 1\}$/,
      'the move, in two parts, then the end event',
    );
    const frame = ruleOf('states--s1-f0', 'hop');
    assert.ok(frame?.endsWith('{animation:states--s1-hop-f0-l 1000ms step-end 480ms infinite}'));
    // A hold keeps its last frame with the fill, with no loop after it.
    const dead = ruleOf('states--s1-f2', 'dead');
    assert.ok(dead?.endsWith('{animation:states--s1-dead-f2 200ms step-end 1 forwards}'), dead);
    assert.ok(css.includes('@keyframes states--s1-dead-f2{0%{opacity:0}50%{opacity:1}100%{opacity:1}}'), 'the last stop is what the fill holds');
  });

  test('D2: every frame group and the element are set under each state, so one state never carries into another', () => {
    for (const state of ['idle', 'walk', 'dead', 'hop', 'ping', 'pong']) {
      for (const k of [0, 1, 2]) {
        assert.ok(rulesFor(`states--s1-f${k}`, 'data-state', state).length > 0, `${state} sets frame group ${k}`);
      }
      assert.ok(rulesFor('states--s1', 'data-state', state).length > 0, `${state} sets the element`);
    }
    // A frame a state never shows is stopped.
    assert.ok(play.some((r) => r.includes('[data-state~="idle"] .states--s1-f1') && r.endsWith('{animation:none}')));
  });

  test('R14: with reduced motion nothing plays, and each state shows the frame it settles in', () => {
    assert.ok(reduced.includes('[class*="states--"]'), 'the generated classes stop');
    assert.ok(!reduced.includes('animation:states--'), 'nothing plays');
    // Everything that plays is in the no-preference block, so the reduced one has no animation but none.
    assert.ok(![...css.matchAll(/animation:states--/g)].some((m) => css.lastIndexOf('@media (prefers-reduced-motion:reduce)') < /** @type {number} */ (m.index)));
    const shows = (/** @type {string} */ state, /** @type {number} */ frame) =>
      reduced.includes(`[data-state~="${state}"] .states--s1-f${frame},[data-pixellab="states"][data-state~="${state}"] .states--s1-f${frame}`);
    for (const [state, settle] of [['idle', 0], ['walk', 0], ['dead', 2], ['hop', 0], ['ping', 0], ['pong', 1]]) {
      for (const k of [0, 1, 2]) {
        const rule = reduced.split('}').find((r) => r.includes(`[data-state~="${state}"] .states--s1-f${k},`));
        assert.ok(rule?.endsWith(`{opacity:${k === settle ? 1 : 0}`), `${state}: frame ${k} is ${k === settle ? 'shown' : 'hidden'}: ${rule}`);
      }
    }
    assert.ok(shows('dead', 2));
    assert.ok(reduced.includes('[data-pixellab="states"] .states--s1-f0{opacity:1}'), 'with no attribute, the start state\'s settle frame');
  });

  test('R16, R17: a track that keys a state bakes the frames and moves over the loop, with no data-state selector', () => {
    assert.ok(css.includes('@keyframes states--s4-f0{0%{opacity:1}36%{opacity:0}38.666%{opacity:1}41.333%{opacity:0}44%{opacity:1}}'));
    assert.ok(play.some((r) => r.startsWith('[data-pixellab="states"] .states--s4-f0{animation:states--s4-f0 3000ms step-end infinite}')));
    assert.ok(!/data-(state|play)~="[a-z-]+"\] \.states--s4/.test(css), 'the page cannot switch a keyed item');
    // s3 is the lamp with no track, which a page can switch.
    assert.ok(/data-state~="flicker"\] \.states--s3-f1/.test(css));
  });

  test('R13: nothing but the SVG\'s own style plays: a state is only attributes on the page\'s side', () => {
    assert.ok(!/<script/.test(svg) && !/\son[a-z]+=/.test(svg));
    // Every frame of the walker is drawn once, in a group of its own.
    for (const cls of ['states--s1-f0', 'states--s1-f1', 'states--s1-f2']) assert.equal(svg.split(`class="${cls}"`).length - 1, 2, `${cls} for the two walkers that share s1`);
  });
});
