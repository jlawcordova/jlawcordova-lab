// The states' CSS in real browsers (docs/intents/2026-10-object-states/spec.md, R9 to R14, R16, D6):
// the compiled fixture SVG (fixtures/states.pixellab) opened in WebKit, Chromium and Firefox. The
// string check (states-css.test.mjs) says the rules are there; this says three engines play them
// the way `object state run` and `scene render --time` say they do:
//
// - R10, R11: which frame group shows and where the item stands at set times, which a state's
//   `data-state` switches to at once, from the first frame;
// - a `then` hand-over, a hold, and the list rule;
// - R9, R12: `animationend` with `<scene>--<state>` for a one-shot state, from data-state and data-play,
//   and none from a looping one;
// - R11: a state an item starts in restarts when a page names it, takes it off and names it again;
// - R16: a track that keys states, frame for frame against `scene render --time`;
// - R14: under reduced motion nothing plays or fires, and each item shows the frame it settles in.
//
// The browsers are Playwright's: `npx playwright install` (CI installs them, with --with-deps on Linux).
// A browser that can't start fails its tests, with that command in the message; it is never skipped.
// PIXELLAB_BROWSERS=chromium,webkit limits the run to some of them, for a machine where one can't
// start (CI never sets it).

import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { after, before, describe, test } from 'node:test';
import { pathToFileURL } from 'node:url';

import { chromium, firefox, webkit } from 'playwright';

import { PROGRAM, compileFixture, compileWith, json, tempArea } from './states-fixture.mjs';

const skip = existsSync(PROGRAM) ? false : `no program at ${PROGRAM}: run cargo build --release`;
const ENGINES = { webkit, chromium, firefox };
const wanted = (process.env.PIXELLAB_BROWSERS ?? 'webkit,chromium,firefox').split(',').map((s) => s.trim()).filter(Boolean);
for (const w of wanted) assert.ok(w in ENGINES, `PIXELLAB_BROWSERS names ${w}, which is not webkit, chromium or firefox`);
if (wanted.length < 3) console.warn(`# states-browser: only ${wanted.join(', ')} (PIXELLAB_BROWSERS)`);

const area = tempArea();
after(() => area.remove());

/** @type {string} */
let url;
/** The fixture with a walker that starts in dead (a hold), in a group of its own. */
/** @type {string} */
let opener;
/** What `object state run walker <state>` says, by state. */
/** @type {Record<string, any>} */
const runs = {};

before(() => {
  if (skip) return;
  url = pathToFileURL(compileFixture(area.dir).path).href;
  opener = pathToFileURL(
    compileWith(area.dir, (scene) => scene.items.push({ group: { class: 'opener' }, items: [{ object: 'walker', state: 'dead', at: { tile: [4, 4, 0] } }] })),
  ).href;
  for (const state of ['idle', 'walk', 'dead', 'hop', 'ping', 'pong']) runs[state] = json(area.dir, ['object', 'state', 'run', 'walker', state, '--horizon', '3000']);
  for (const state of ['idle', 'flicker']) runs[`lamp-${state}`] = json(area.dir, ['object', 'state', 'run', 'lamp', state, '--horizon', '3000']);
});

/** What a run has showing `t` ms after entering: the last change at or before it. */
function truth(/** @type {any} */ run, /** @type {number} */ t) {
  let found = run.changes[0];
  for (const c of run.changes) if (c.t <= t) found = c;
  return { frame: found.frame, move: found.move };
}

/** Times that aren't on a change, where an engine and the run can't differ by rounding: just after each change and between. */
function samples(/** @type {any} */ run, /** @type {number} */ upto) {
  const ts = run.changes.map((/** @type {any} */ c) => c.t).filter((/** @type {number} */ t) => t < upto);
  const out = new Set([upto - 1]);
  ts.forEach((t, i) => {
    out.add(t + 1);
    const next = ts[i + 1] ?? upto;
    if (next - t > 4) out.add(Math.floor((t + next) / 2));
  });
  return [...out].filter((t) => t < upto).sort((a, b) => a - b);
}

/** The page side: in the page, so each is a string of code the engine runs. */
const PAGE = `
  window.pl = {
    // Pauses every animation and puts each t ms into its own run (what its delay and fill make of that).
    seek(t) { for (const a of document.getAnimations()) { a.pause(); a.currentTime = t; } },
    stated(cls) { return [...document.querySelectorAll('.' + cls)]; },
    // The frame whose group shows in an element of a sprite with states, and where it stands.
    shown(el) {
      const on = [...el.children].filter((c) => getComputedStyle(c).opacity === '1');
      const frames = on.map((c) => Number(/-f(\\d+)$/.exec(c.getAttribute('class'))[1]));
      const m = /matrix\\(([^)]*)\\)/.exec(getComputedStyle(el).transform);
      const [x, y] = m ? m[1].split(',').slice(4).map(Number) : [0, 0];
      return { frames, move: [x, y] };
    },
    ended: [],
  };
  document.documentElement.addEventListener('animationend', (e) => {
    if (/^states--[a-z-]+$/.test(e.animationName) && !/^states--s\\d/.test(e.animationName)) pl.ended.push({ name: e.animationName, at: performance.now(), elapsed: Math.round(e.elapsedTime * 1000) });
  });
`;

/** Starts an engine, or fails with what to do. */
async function launch(/** @type {string} */ name, /** @type {any} */ options = {}) {
  try {
    const browser = await ENGINES[/** @type {keyof typeof ENGINES} */ (name)].launch();
    const context = await browser.newContext(options);
    return { browser, context };
  } catch (e) {
    throw new Error(`can't start ${name}: ${String(/** @type {any} */ (e).message).split('\n')[0]}. Install the browsers with: npx playwright install${process.platform === 'linux' ? ' --with-deps' : ''}`);
  }
}

for (const name of wanted) {
  describe(`${name}: the states' CSS plays as the run says`, { skip }, () => {
    /** @type {any} */
    let browser;
    /** @type {any} */
    let page;
    before(async () => {
      const started = await launch(name);
      browser = started.browser;
      page = await started.context.newPage();
    });
    after(async () => browser?.close());
    const fresh = async () => {
      await page.goto(url);
      await page.evaluate(PAGE);
    };

    /** Compares an element's frame and move with a run at the times, in the page. */
    async function matches(/** @type {string} */ cls, /** @type {number} */ index, /** @type {any} */ run, /** @type {number} */ upto, /** @type {string} */ what) {
      for (const t of samples(run, upto)) {
        const got = await page.evaluate(([c, i, at]) => { pl.seek(at); return pl.shown(pl.stated(c)[i]); }, [cls, index, t]);
        const want = truth(run, t);
        assert.deepEqual(got.frames, [want.frame], `${what}: frame at ${t} ms`);
        assert.deepEqual(got.move, want.move, `${what}: move at ${t} ms`);
      }
    }

    test('R10: with no attribute an item plays the state it starts in, frame and move as the run says', async () => {
      await fresh();
      await matches('states--s1', 0, runs.idle, 2000, 'walker, idle');
      await matches('states--s2', 0, runs.walk, 1800, 'the walker that starts in walk');
      await matches('states--s3', 0, runs['lamp-idle'], 2000, 'lamp');
    });

    test('R11: data-state switches at once, from the first frame, whatever was playing', async () => {
      await fresh();
      // Let idle run on for a while, then switch.
      await page.waitForTimeout(150);
      const before = await page.evaluate(() => { document.documentElement.setAttribute('data-state', 'walk'); return document.getAnimations().map((a) => ({ name: a.animationName, at: a.currentTime })); });
      const walk = before.filter((a) => a.name.startsWith('states--s1-walk-'));
      assert.ok(walk.length >= 3, `walk's animations are in: ${JSON.stringify(before)}`);
      assert.ok(walk.every((a) => a.at < 100), `they start from the beginning: ${JSON.stringify(walk)}`);
      assert.ok(!before.some((a) => /^states--s1--?idle-/.test(a.name)), 'and idle\'s are gone');
      await matches('states--s1', 0, runs.walk, 1800, 'walker switched to walk');
      // The item whose object has no such state keeps playing what it was: the lamp has no walk.
      const lamp = await page.evaluate(() => { pl.seek(10); return pl.shown(pl.stated('states--s3')[0]); });
      assert.deepEqual(lamp.frames, [0]);
    });

    test('R10: the list rule, a group on the way, and taking the attribute off', async () => {
      await fresh();
      // Later in the object wins, whichever way round the page lists them (hop comes after walk).
      for (const list of ['walk hop', 'hop walk']) {
        await page.evaluate((l) => document.documentElement.setAttribute('data-state', l), list);
        await matches('states--s1', 0, runs.hop, 1800, `walker with data-state="${list}"`);
      }
      await page.evaluate(() => document.documentElement.removeAttribute('data-state'));
      await matches('states--s1', 0, runs.idle, 1500, 'walker with the attribute off');
      // On a group around some items: those whose objects have the state switch, the others and the items outside don't.
      await page.evaluate(() => document.querySelector('[data-zone="a"]').setAttribute('data-state', 'flicker'));
      await matches('states--s3', 0, runs['lamp-flicker'], 1500, 'the lamp in the group, flicker');
      const zoneWalker = await page.evaluate(() => { pl.seek(600); return pl.shown(pl.stated('states--s1').at(-1)); });
      assert.deepEqual([zoneWalker.frames, zoneWalker.move], [[truth(runs.idle, 600).frame], truth(runs.idle, 600).move], 'the walker in the group has no flicker: it stays in idle');
      const outside = await page.evaluate(() => { pl.seek(10); return pl.shown(pl.stated('states--s1')[0]); });
      assert.deepEqual(outside.frames, [0]);
    });

    test('a then hands over to the next state, a hold keeps its last frame, and a cycle of one-shots loops', async () => {
      await fresh();
      await page.evaluate(() => document.documentElement.setAttribute('data-state', 'hop'));
      await matches('states--s1', 0, runs.hop, 3000, 'hop, then idle');
      await page.evaluate(() => document.documentElement.setAttribute('data-state', 'dead'));
      await matches('states--s1', 0, runs.dead, 250, 'dead');
      const held = await page.evaluate(() => { pl.seek(9000); return pl.shown(pl.stated('states--s1')[0]); });
      assert.deepEqual(held.frames, [2], 'a hold keeps its last frame for good');
      await page.evaluate(() => document.documentElement.setAttribute('data-state', 'ping'));
      await matches('states--s1', 0, runs.ping, 1200, 'ping, pong, ping');
    });

    test('R9, R12: a one-shot state fires animationend as <scene>--<state> when it ends, from data-state and data-play; a looping one fires nothing', async () => {
      await fresh();
      const fired = async (/** @type {string} */ attr, /** @type {string} */ value, /** @type {number} */ waitMs) => {
        await fresh();
        const t0 = await page.evaluate(([a, v]) => { pl.ended.length = 0; const t = performance.now(); document.documentElement.setAttribute(a, v); return t; }, [attr, value]);
        await page.waitForTimeout(waitMs);
        const ended = await page.evaluate(() => pl.ended);
        return ended.map((/** @type {any} */ e) => ({ name: e.name, after: e.at - t0, elapsed: e.elapsed }));
      };
      // hop hands over to idle after 480 ms.
      for (const attr of ['data-state', 'data-play']) {
        const got = await fired(attr, 'hop', 1500);
        // One from each of the three walkers, which the attribute on the root switched.
        assert.deepEqual(got.map((e) => e.name), ['states--hop', 'states--hop', 'states--hop'], `${attr}=hop: ${JSON.stringify(got)}`);
        // When it fired in the animation's own time, which a loaded machine can't delay; the wall clock only bounds it below.
        assert.ok(got.every((e) => e.elapsed === 480 && e.after >= 400), `ended ${JSON.stringify(got)}`);
      }
      // dead holds: it fires when its last frame starts, 100 ms in.
      const dead = await fired('data-state', 'dead', 900);
      assert.deepEqual(dead.map((e) => e.name), ['states--dead', 'states--dead', 'states--dead']);
      assert.ok(dead.every((e) => e.elapsed === 100 && e.after >= 60), `ended ${JSON.stringify(dead)}`);
      // A looping state fires nothing, and neither does a state that started in a looping one after its hand-over.
      assert.deepEqual(await fired('data-state', 'walk', 1300), []);
      assert.deepEqual(await fired('data-play', 'walk', 600), [], 'a looping state has no data-play');
    });

    test('R11, R12: a state an item starts in restarts when a page names it, takes the name off and names it again', async () => {
      await page.goto(opener);
      await page.evaluate(PAGE);
      const group = '.opener';
      // The walker that starts in dead plays it with no attribute, and that play says no end: no page asked for it.
      await page.waitForTimeout(300);
      assert.deepEqual(await page.evaluate(() => pl.ended), []);
      const shown = () => page.evaluate((g) => pl.shown(document.querySelector(g).firstElementChild), group);
      assert.deepEqual((await shown()).frames, [2], 'held on its last frame');
      // Each time the page names it, it plays from its first frame and fires animationend as states--dead.
      // How far into their runs the walker's animations are, just after the page names dead.
      const play = () =>
        page.evaluate((g) => {
          const el = document.querySelector(g);
          pl.ended.length = 0;
          el.setAttribute('data-state', 'dead');
          return el.getAnimations({ subtree: true }).map((a) => ({ name: a.animationName, at: a.currentTime }));
        }, group);
      for (const round of [1, 2]) {
        const running = await play();
        assert.ok(running.some((a) => a.name === 'states--dead'), `round ${round}: ${JSON.stringify(running)}`);
        assert.ok(running.every((a) => a.at < 50), `round ${round}: from its first frame: ${JSON.stringify(running)}`);
        await page.waitForTimeout(400);
        assert.deepEqual(await page.evaluate(() => pl.ended.map((e) => e.name)), ['states--dead'], `round ${round}`);
        assert.deepEqual((await shown()).frames, [2]);
        // Off, with the style worked out in between, then on again.
        await page.evaluate((g) => { const el = document.querySelector(g); el.removeAttribute('data-state'); getComputedStyle(el.firstElementChild).transform; }, group);
      }
    });

    test('R16: a track that keys states plays frame for frame as scene render --time, at every 100 ms of the loop', async () => {
      await fresh();
      const at = (/** @type {number} */ t) => json(area.dir, ['scene', 'render', 'states', '--time', String(t)]).items;
      for (let t = 0; t < 3000; t += 100) {
        const items = at(t);
        const lamp = items.find((/** @type {any} */ i) => i.id === 'lamp-b');
        const got = await page.evaluate((tt) => { pl.seek(tt); return pl.shown(pl.stated('states--s4')[0]); }, t);
        assert.deepEqual(got.frames, [lamp.frame], `lamp-b at ${t} ms (${lamp.state})`);
        // The walker that starts in walk is page-driven, and the render places it from time 0 as the CSS plays it.
        const walker = items.find((/** @type {any} */ i) => i.object === 'walker' && i.state === 'walk');
        const own = await page.evaluate(() => pl.shown(pl.stated('states--s2')[0]));
        assert.deepEqual(own.frames, [walker.frame], `the walker in walk at ${t} ms`);
      }
    });

    test('R16: the baked track also plays from the middle of a state, a flicker that hands over to idle', async () => {
      await fresh();
      // flicker starts at 1000 ms: frames 0,1,0,1 every 80 ms, then idle (frame 0) from 1320 ms.
      const seen = [];
      for (const t of [1010, 1090, 1170, 1250, 1330, 1900, 2100]) seen.push(await page.evaluate((tt) => { pl.seek(tt); return pl.shown(pl.stated('states--s4')[0]).frames[0]; }, t));
      assert.deepEqual(seen, [0, 1, 0, 1, 0, 0, 0]);
    });
  });

  describe(`${name}: reduced motion`, { skip }, () => {
    /** @type {any} */
    let browser;
    /** @type {any} */
    let page;
    before(async () => {
      const started = await launch(name, { reducedMotion: 'reduce' });
      browser = started.browser;
      page = await started.context.newPage();
    });
    after(async () => browser?.close());

    test('R14: nothing plays, and each item shows the first frame of the state it settles in; data-state still switches which', async () => {
      await page.goto(url);
      await page.evaluate(PAGE);
      assert.equal(await page.evaluate(() => document.getAnimations().length), 0, 'no animation runs');
      const shown = (/** @type {string} */ cls, /** @type {number} */ i) => page.evaluate(([c, n]) => pl.shown(pl.stated(c)[n]), [cls, i]);
      for (const [cls, frame] of [['states--s1', 0], ['states--s2', 0], ['states--s3', 0], ['states--s4', 0]]) {
        const got = await shown(cls, 0);
        assert.deepEqual([got.frames, got.move], [[frame], [0, 0]], `${cls}: rest`);
      }
      // dead settles on its second frame, ping and pong on the first frame of the cycle they are in, hop on idle's first.
      for (const [state, frame] of [['dead', 2], ['hop', 0], ['ping', 0], ['pong', 1], ['walk', 0], ['idle', 0]]) {
        await page.evaluate((s) => document.documentElement.setAttribute('data-state', s), state);
        const got = await shown('states--s1', 0);
        assert.deepEqual(got.frames, [frame], `data-state="${state}"`);
        assert.equal(await page.evaluate(() => document.getAnimations().length), 0);
      }
      // The keyed lamp ignores the page, and still shows its rest frame.
      assert.deepEqual((await shown('states--s4', 0)).frames, [0]);
    });

    test('R14: no animationend fires, from data-state or data-play', async () => {
      await page.goto(url);
      await page.evaluate(PAGE);
      await page.evaluate(() => { document.documentElement.setAttribute('data-state', 'hop'); document.documentElement.setAttribute('data-play', 'dead'); });
      await page.waitForTimeout(700);
      assert.deepEqual(await page.evaluate(() => pl.ended), []);
    });
  });
}
