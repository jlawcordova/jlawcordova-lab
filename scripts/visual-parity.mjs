#!/usr/bin/env node
// The visual gate (docs/intents/2026-10-rust-engine/spec.md, R16, D9): for
// every SVG the owner's document publishes, the Node CLI's format-1 build on
// a page with the site's pixel-art.css and the program's build of a migrated
// copy, with the site's swap rehearsed by the program's own commands, on a
// bare page, must show the same pixels:
//   - at t = 0;
//   - at 24 times across each loop's cycle;
//   - every 40 ms through the hop, started at the same instant on both pages;
//   - under reduced motion (Chromium's --force-prefers-reduced-motion).
// Sample times sit at least 5 ms from every step boundary on either page, so
// R16's 4 ms tolerance for converted timings can't flip a frame.
//
//   node scripts/visual-parity.mjs [--site <checkout>] [--program <pixellab>]
//                                  [--out <dir>] [--write-swapped <dir>]
//
// It only reads the site checkout (document copy and pixel-art.css). It needs
// Chrome or Chromium: $CHROME, else the usual macOS and Linux paths. The
// report (and the frames that differ, as PNGs) goes to --out, a scratch
// folder by default. --write-swapped keeps the migrated, swapped copy, for
// the site's change to start from. Exits 1 when any frame differs. Removed
// at cutover with the Node CLI.

import { spawn, spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { inflateSync } from 'node:zlib';

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const NODE_CLI = join(REPO, 'cli/bin/pixellab.mjs');

// ------------------------------------------------------------- the site swap

/** The site's cloud classes as ambient loops (pixel-art.css: .pcloud 16 s, .pc1 20 s at -7 s, .pc2 13 s at -3 s). */
export const CLOUDS = {
  pc0: { duration: 16000, offset: 0 },
  pc1: { duration: 20000, offset: 7000 },
  pc2: { duration: 13000, offset: 3000 },
};

/** pcloud's drift: 12 pixels right at half the loop, a pixel at a time, and back. */
export const cloudKeys = (/** @type {number} */ duration) => [
  { t: 0, move: [0, 0] },
  { t: duration / 2, move: [12, 0] },
];

/** cbob: up a pixel at half of its 1 s loop. */
export const BOB = { duration: 1000, keys: [{ t: 0, move: [0, 0] }, { t: 500, move: [0, -1] }] };

/** chop's keyframes in pixel-art.css (percent, translateY), played step-end over 480 ms. */
const CHOP = [
  [0, 0], [9.09, -2], [18.18, -4], [27.27, -5], [36.36, -6], [45.45, -6], [54.55, -5], [63.64, -4], [72.73, -2], [81.82, 0], [90.91, -1],
];

/**
 * The hop as an action: step-end jumps become a hold key one ms before each
 * jump and the new place at it (whole ms, so within R16's 4 ms).
 * @returns {{ duration: number, keys: { t: number, move: number[] }[] }}
 */
export function hopAction(duration = 480) {
  const keys = [{ t: 0, move: [0, 0] }];
  let y = 0;
  for (const [pct, to] of CHOP.slice(1)) {
    if (to === y) continue;
    const t = Math.round((pct / 100) * duration);
    keys.push({ t: t - 1, move: [0, y] }, { t, move: [0, to] });
    y = to;
  }
  return { duration, keys };
}

// ------------------------------------------------------------- sample times

/**
 * The step boundaries of one animation in its own cycle, in ms from the
 * cycle's start: where any keyframe interval's steps jump.
 * @param {{ duration: number, frames: { offset: number, easing: string }[] }} a
 */
export function boundaries(a) {
  const out = new Set([0]);
  for (let i = 0; i + 1 < a.frames.length; i++) {
    const [from, to] = [a.frames[i].offset, a.frames[i + 1].offset];
    const m = /^steps\((\d+)(?:,\s*(?:jump-)?end)?\)$/.exec(a.frames[i].easing) ?? (a.frames[i].easing === 'step-end' ? [0, '1'] : null);
    const n = m ? Number(m[1]) : 1;
    for (let k = 1; k <= n; k++) out.add(((from + ((to - from) * k) / n) % 1) * a.duration);
  }
  return [...out];
}

/**
 * How far time `t` (page time) is from the nearest step boundary of any of
 * the animations, each `{ duration, delay, frames }`.
 * @param {number} t @param {{ duration: number, delay: number, frames: any[], bounds?: number[] }[]} anims
 */
export function margin(t, anims) {
  let best = Infinity;
  for (const a of anims) {
    if (!(a.duration > 0)) continue;
    a.bounds ??= boundaries(a);
    const phase = (((t - a.delay) % a.duration) + a.duration) % a.duration;
    for (const b of a.bounds) {
      const d = Math.abs(phase - b);
      best = Math.min(best, d, a.duration - d);
    }
  }
  return best;
}

/**
 * The time nearest `t` that is at least `min` ms from every boundary,
 * searched up to `reach` ms either way; else the one with the most room.
 * @param {number} t @param {any[]} anims
 */
export function cleanTime(t, anims, min = 5, reach = 60) {
  let best = { t, margin: margin(t, anims) };
  for (let d = 0; d <= reach; d += 0.5) {
    for (const c of d === 0 ? [t] : [t + d, t - d]) {
      if (c < 0) continue;
      const m = margin(c, anims);
      if (m >= min) return { t: c, margin: m };
      if (m > best.margin) best = { t: c, margin: m };
    }
  }
  return best;
}

// --------------------------------------------------------------------- Chrome

function findChrome() {
  const candidates = [
    process.env.CHROME,
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    '/Applications/Chromium.app/Contents/MacOS/Chromium',
    join(homedir(), 'Applications/Google Chrome.app/Contents/MacOS/Google Chrome'),
    '/usr/bin/chromium',
    '/usr/bin/chromium-browser',
    '/usr/bin/google-chrome',
    '/usr/bin/google-chrome-stable',
  ];
  return candidates.find((p) => p && existsSync(p));
}

/** A headless Chrome over the DevTools protocol, with Node's built-in WebSocket. */
async function launch(/** @type {string} */ chrome, /** @type {string[]} */ extra, /** @type {string} */ profile) {
  const child = spawn(chrome, ['--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`, '--hide-scrollbars', '--force-device-scale-factor=1', '--no-first-run', '--no-default-browser-check', ...extra, 'about:blank'], {
    stdio: ['ignore', 'ignore', 'pipe'],
  });
  const url = await new Promise((done, fail) => {
    let text = '';
    /** @type {any} */ (child.stderr).on('data', (/** @type {Buffer} */ b) => {
      text += b.toString();
      const m = /DevTools listening on (ws:\/\/\S+)/.exec(text);
      if (m) done(m[1]);
    });
    child.on('exit', () => fail(new Error(`Chrome exited: ${text}`)));
  });
  const ws = new WebSocket(/** @type {string} */ (url));
  await new Promise((done) => ws.addEventListener('open', done, { once: true }));
  let id = 0;
  /** @type {Map<number, { done: (v: any) => void, fail: (e: Error) => void }>} */
  const waiting = new Map();
  /** @type {((msg: any) => void)[]} */
  const listeners = [];
  ws.addEventListener('message', (event) => {
    const msg = JSON.parse(String(event.data));
    if (msg.id !== undefined && waiting.has(msg.id)) {
      const w = /** @type {any} */ (waiting.get(msg.id));
      waiting.delete(msg.id);
      if (msg.error) w.fail(new Error(JSON.stringify(msg.error)));
      else w.done(msg.result);
    } else for (const l of listeners) l(msg);
  });
  const send = (/** @type {string} */ method, /** @type {any} */ params = {}, /** @type {string | undefined} */ sessionId = undefined) =>
    new Promise((done, fail) => {
      const n = ++id;
      waiting.set(n, { done, fail });
      ws.send(JSON.stringify({ id: n, method, params, ...(sessionId ? { sessionId } : {}) }));
    });
  return {
    send,
    /** A new tab: { send(method, params), evaluate(js) }. */
    async page() {
      const { targetId } = /** @type {any} */ (await send('Target.createTarget', { url: 'about:blank' }));
      const { sessionId } = /** @type {any} */ (await send('Target.attachToTarget', { targetId, flatten: true }));
      await send('Target.activateTarget', { targetId });
      const s = (/** @type {string} */ method, /** @type {any} */ params = {}) => send(method, params, sessionId);
      await s('Page.enable');
      return {
        send: s,
        async open(/** @type {string} */ file, /** @type {number} */ width, /** @type {number} */ height) {
          await s('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });
          const loaded = new Promise((done) => {
            const l = (/** @type {any} */ msg) => {
              if (msg.sessionId === sessionId && msg.method === 'Page.loadEventFired') {
                listeners.splice(listeners.indexOf(l), 1);
                done(undefined);
              }
            };
            listeners.push(l);
          });
          await s('Page.navigate', { url: pathToFileURL(file).href });
          await loaded;
        },
        async evaluate(/** @type {string} */ expression) {
          const r = /** @type {any} */ (await s('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true }));
          if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails));
          return r.result.value;
        },
        async shot() {
          const r = /** @type {any} */ (await s('Page.captureScreenshot', { format: 'png' }));
          return Buffer.from(r.data, 'base64');
        },
      };
    },
    async close() {
      ws.close();
      child.kill();
    },
  };
}

/** The script both pages carry: pause everything, set every animation's time, list the animations. */
const PAGE_SCRIPT = `
window.__anims = () => document.getAnimations();
window.__at = (t) => {
  for (const a of document.getAnimations()) { a.pause(); a.currentTime = t; }
  // A screenshot renders a fresh frame, so nothing waits on requestAnimationFrame (a background tab never runs it).
  document.body.getBoundingClientRect();
  return document.getAnimations().length;
};
window.__timing = () => document.getAnimations().map((a) => {
  const t = a.effect.getComputedTiming();
  return { name: a.animationName ?? '', duration: Number(t.duration), delay: Number(a.effect.getTiming().delay) || 0,
    frames: a.effect.getKeyframes().map((k) => ({ offset: k.computedOffset, easing: k.easing })) };
});
`;

/** An HTML page holding one SVG at 1×, with or without the site's CSS. */
function page(/** @type {string} */ svg, /** @type {string | null} */ css) {
  const vb = /viewBox="([^"]+)"/.exec(svg)?.[1].split(/\s+/).map(Number) ?? [0, 0, 1, 1];
  const [w, h] = [vb[2], vb[3]];
  const inlined = css ? svg.replace('<svg ', '<svg class="pixel-art" ') : svg;
  return {
    w,
    h,
    html: `<!doctype html><html><head><meta charset="utf-8"><style>html,body{margin:0;padding:0;background:#fff}svg{display:block;width:${w}px;height:${h}px}</style>${css ? `<style>${css}</style>` : ''}<script>${PAGE_SCRIPT}</script></head><body>${inlined}</body></html>`,
  };
}

/** RGBA of a PNG screenshot (8-bit RGB or RGBA, any filter). */
function pixels(/** @type {Buffer} */ png) {
  let pos = 8;
  let [width, height, channels] = [0, 0, 4];
  /** @type {Buffer[]} */
  const idat = [];
  while (pos < png.length) {
    const len = png.readUInt32BE(pos);
    const type = png.toString('latin1', pos + 4, pos + 8);
    const data = png.subarray(pos + 8, pos + 8 + len);
    if (type === 'IHDR') {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      channels = data[9] === 6 ? 4 : 3;
    } else if (type === 'IDAT') idat.push(data);
    pos += 12 + len;
  }
  const raw = inflateSync(Buffer.concat(idat));
  const row = width * channels;
  const out = Buffer.alloc(row * height);
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (row + 1)];
    for (let i = 0; i < row; i++) {
      const x = raw[y * (row + 1) + 1 + i];
      const a = i >= channels ? out[y * row + i - channels] : 0;
      const b = y > 0 ? out[(y - 1) * row + i] : 0;
      const c = y > 0 && i >= channels ? out[(y - 1) * row + i - channels] : 0;
      const p = a + b - c;
      const [pa, pb, pc] = [Math.abs(p - a), Math.abs(p - b), Math.abs(p - c)];
      const paeth = pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      out[y * row + i] = (x + [0, a, b, (a + b) >> 1, paeth][filter]) & 255;
    }
  }
  return { width, height, channels, data: out };
}

function samePixels(/** @type {Buffer} */ a, /** @type {Buffer} */ b) {
  if (a.equals(b)) return true;
  const [x, y] = [pixels(a), pixels(b)];
  return x.width === y.width && x.height === y.height && x.channels === y.channels && x.data.equals(y.data);
}

// ----------------------------------------------------------------------- main

async function main() {
  const args = process.argv.slice(2);
  const flag = (/** @type {string} */ name, /** @type {string | null} */ fallback = null) => {
    const i = args.indexOf(name);
    return i < 0 ? fallback : (args[i + 1] ?? fallback);
  };
  const PROGRAM = resolve(flag('--program', join(REPO, 'target/release/pixellab')) ?? '');
  const SITE = resolve(flag('--site', join(homedir(), 'Projects', 'jlawcordova.github.io')) ?? '');
  const base = mkdtempSync(join(tmpdir(), 'pixellab-visual-'));
  const OUT = resolve(flag('--out', join(base, 'report')) ?? '');
  const SWAPPED = flag('--write-swapped');
  const chrome = findChrome();
  if (!chrome) throw new Error('no Chrome or Chromium: set CHROME to one');
  if (!existsSync(PROGRAM)) throw new Error(`no program at ${PROGRAM}: run cargo build --release first`);
  if (!existsSync(SITE)) throw new Error(`no site checkout at ${SITE}`);
  mkdirSync(OUT, { recursive: true });
  const env = { ...process.env, HOME: join(base, 'home'), PIXELLAB_STATE: join(base, 'state') };
  delete env.PIXELLAB_DOC;
  delete env.PIXELLAB_SITE;
  delete env.PIXELLAB_LEGACY_SVG;
  mkdirSync(env.HOME, { recursive: true });
  const run = (/** @type {string[]} */ cmd) => {
    const r = spawnSync(cmd[0], cmd.slice(1), { env, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
    return { status: r.status ?? -1, stdout: r.stdout ?? '', stderr: r.stderr ?? '' };
  };
  const must = (/** @type {string[]} */ cmd) => {
    const r = run(cmd);
    if (r.status !== 0) throw new Error(`${cmd.slice(1).join(' ')} failed: ${r.stderr || r.stdout}`);
    return r.stdout;
  };
  const program = (/** @type {string[]} */ argv) => must([PROGRAM, ...argv]);

  // The old pages: the Node CLI's build of a format-1 copy (document copy only reads the checkout).
  const format1 = join(base, 'old.pixellab');
  must(['node', NODE_CLI, 'document', 'copy', '--to', format1, '--site', SITE]);
  const oldOut = join(base, 'old-out');
  const built = run(['node', NODE_CLI, 'art', 'build', '--document', format1, '--dir', oldOut]);
  if (!existsSync(oldOut)) throw new Error(`the Node CLI's build failed: ${built.stderr || built.stdout}`);
  const css = readFileSync(join(SITE, 'src/styles/pixel-art.css'), 'utf8');

  // The new pages: the program migrates a copy, and the site's swap is rehearsed with its commands.
  const swapped = join(base, 'new.pixellab');
  program(['document', 'migrate', '--document', format1, '--to', swapped]);
  const doc = ['--document', swapped];
  /** @type {string[]} */
  const swaps = [];
  for (const file of readdirSync(join(swapped, 'scenes'))) {
    const scene = file.replace(/\.json$/, '');
    const items = JSON.parse(program(['scene', 'items', scene, ...doc, '--json'])).items;
    for (const item of items) {
      const classes = String(item.class ?? '').split(' ').filter(Boolean);
      if (!classes.includes('pcloud')) continue;
      const loop = CLOUDS[/** @type {'pc0'} */ (classes.find((c) => /^pc\d$/.test(c)) ?? 'pc0')];
      const rest = classes.filter((c) => c !== 'pcloud' && !/^pc\d$/.test(c)).join(' ');
      program(['scene', 'set', scene, item.path, '--class', rest, ...doc]);
      program(['scene', 'ambient', scene, item.path, '--duration', String(loop.duration), '--offset', String(loop.offset), '--keys', JSON.stringify(cloudKeys(loop.duration)), ...doc]);
      swaps.push(`${scene} ${item.path} (${item.object}): ${classes.join(' ')} → an ambient loop of ${loop.duration} ms at ${loop.offset} ms`);
    }
  }
  for (const file of readdirSync(join(swapped, 'objects'))) {
    const object = file.replace(/\.json$/, '');
    const source = JSON.parse(readFileSync(join(swapped, 'objects', file), 'utf8'));
    (source.layers ?? []).forEach((/** @type {any} */ layer, /** @type {number} */ i) => {
      if (!String(layer.class ?? '').split(' ').includes('cbob')) return;
      const hop = hopAction();
      program(['object', 'ambient', object, String(i), '--duration', String(BOB.duration), '--keys', JSON.stringify(BOB.keys), ...doc]);
      program(['object', 'action', object, String(i), 'hop', '--duration', String(hop.duration), '--keys', JSON.stringify(hop.keys), ...doc]);
      swaps.push(`${object} layer ${i}: cbob → an ambient loop of 1000 ms and the action hop (480 ms)`);
    });
  }
  if (SWAPPED) {
    rmSync(SWAPPED, { recursive: true, force: true });
    cpSync(swapped, resolve(SWAPPED), { recursive: true });
  }
  const newOut = join(base, 'new-out');
  program(['art', 'build', ...doc, '--dir', newOut]);

  const svgs = readdirSync(oldOut).filter((f) => f.endsWith('.svg')).sort();
  const pagesDir = join(base, 'pages');
  mkdirSync(pagesDir, { recursive: true });
  /** @type {string[]} */
  const report = [];
  /** @type {string[]} */
  const failures = [];
  let frames = 0;
  let tightest = Infinity;

  for (const reduced of [false, true]) {
    const browser = await launch(chrome, reduced ? ['--force-prefers-reduced-motion'] : [], join(base, reduced ? 'profile-reduced' : 'profile'));
    try {
      for (const svg of svgs) {
        const oldSvg = readFileSync(join(oldOut, svg), 'utf8');
        const newSvg = readFileSync(join(newOut, svg), 'utf8');
        const oldPage = page(oldSvg, css);
        const newPage = page(newSvg, null);
        const files = [join(pagesDir, `old-${svg}.html`), join(pagesDir, `new-${svg}.html`)];
        writeFileSync(files[0], oldPage.html);
        writeFileSync(files[1], newPage.html);
        // One tab, in front: each page in turn, every frame of it captured, then compared.
        const tab = await browser.page();
        await tab.open(files[0], oldPage.w, oldPage.h);
        const oldTiming = await tab.evaluate('__timing()');
        await tab.open(files[1], newPage.w, newPage.h);
        const newTiming = await tab.evaluate('__timing()');
        const anims = [...oldTiming, ...newTiming];
        /** @type {{ t: number, why: string, margin: number, hop?: boolean }[]} */
        const times = [{ t: 0, why: 't = 0', margin: margin(0, anims) }];
        if (!reduced) {
          const cycles = [...new Set(anims.map((x) => x.duration).filter((d) => d > 0 && Number.isFinite(d)))].sort((x, y) => x - y);
          for (const cycle of cycles) {
            for (let k = 0; k < 24; k++) {
              const picked = cleanTime(((k + 0.5) * cycle) / 24, anims);
              times.push({ t: picked.t, why: `${cycle} ms cycle, sample ${k + 1}/24`, margin: picked.margin });
            }
          }
        } else {
          for (const t of [1234, 5678]) times.push({ t, why: 'reduced motion', margin: Infinity });
        }
        const hops = oldSvg.includes('cbob');
        const trigger = [`document.querySelectorAll('.cbob').forEach((e) => e.classList.add('is-hopping')), 0`, `document.querySelector('svg').setAttribute('data-play', 'hop'), 0`];
        const capture = async (/** @type {string} */ file, /** @type {number} */ w, /** @type {number} */ h, /** @type {number} */ side) => {
          await tab.open(file, w, h);
          /** @type {Buffer[]} */
          const shots = [];
          for (const at of times) {
            await tab.evaluate(`__at(${at.t})`);
            shots.push(await tab.shot());
          }
          /** @type {{ t: number, why: string, margin: number }[]} */
          const hopTimes = [];
          if (hops) {
            await tab.evaluate(trigger[side]);
            const hopAnims = await tab.evaluate('__timing()');
            for (let at = 0; at < 480; at += 40) hopTimes.push({ ...cleanTime(at + 20, hopAnims, 5, 20), why: '' });
            for (const at of hopTimes) {
              await tab.evaluate(`__at(${at.t})`);
              shots.push(await tab.shot());
            }
          }
          return { shots, hopTimes };
        };
        const oldShots = await capture(files[0], oldPage.w, oldPage.h, 0);
        const newShots = await capture(files[1], newPage.w, newPage.h, 1);
        // Both pages' hop boundaries count: a hop sample sits clear of either.
        const hopAt = oldShots.hopTimes.map((o, i) => {
          const n = newShots.hopTimes[i];
          return Math.abs(o.t - n.t) < 0.01 ? o : { t: o.t, why: '', margin: Math.min(o.margin, n.margin) };
        });
        const all = [...times, ...hopAt.map((x) => ({ ...x, why: `the hop at ${x.t} ms`, hop: true }))];
        all.forEach((at, i) => {
          frames++;
          // t = 0 is a boundary by definition (R16 asks for it); the margin is the other samples'.
          if (Number.isFinite(at.margin) && at.t !== 0) tightest = Math.min(tightest, at.margin);
          const [x, y] = [oldShots.shots[i], newShots.shots[i]];
          if (!samePixels(x, y)) {
            const stem = `${svg.replace(/\.svg$/, '')}-${reduced ? 'reduced-' : ''}${at.hop ? 'hop' : 'loop'}-${at.t}`.replace(/[^a-z0-9.-]+/gi, '_');
            writeFileSync(join(OUT, `${stem}-old.png`), x);
            writeFileSync(join(OUT, `${stem}-new.png`), y);
            failures.push(`${svg}${reduced ? ' (reduced motion)' : ''}: ${at.why} at ${at.t} ms differs (${stem}-old.png, -new.png)`);
          }
        });
        process.stderr.write(`${svg}${reduced ? ' (reduced motion)' : ''}: ${all.length} frames\n`);
        // That the pages move at all: how many animations each plays, and how many different frames it showed.
        const distinct = (/** @type {Buffer[]} */ shots) => new Set(shots.map((b) => b.toString('base64'))).size;
        report.push(
          `- ${svg}${reduced ? ' (reduced motion)' : ''}: ${all.length} frames compared${hops ? ', the hop included' : ''}; ` +
            `old page ${oldTiming.length} animations, ${distinct(oldShots.shots)} different frames; new page ${newTiming.length} animations, ${distinct(newShots.shots)} different frames`,
        );
        await tab.send('Page.close');
      }
    } finally {
      await browser.close();
    }
  }

  const text = [
    '# Visual gate (R16)',
    '',
    `The Node CLI's format-1 build with the site's pixel-art.css vs the program's build of a migrated copy with the swap below, bare. Chrome: ${chrome}.`,
    '',
    '**The swap, rehearsed with the program\'s commands:**',
    ...swaps.map((s) => `- ${s}`),
    '',
    '**Frames:**',
    ...report,
    `- ${frames} frames in all; every sample time after t = 0 at least ${tightest === Infinity ? '∞' : tightest.toFixed(1)} ms from a step boundary on either page`,
    '',
    failures.length ? `**${failures.length} frames differ:**\n${failures.map((f) => `- ${f}`).join('\n')}` : '**Every frame matches.**',
    '',
  ].join('\n');
  writeFileSync(join(OUT, 'report.md'), text);
  process.stdout.write(text);
  process.stderr.write(`report: ${join(OUT, 'report.md')}\n`);
  if (!SWAPPED) rmSync(join(base, 'new.pixellab'), { recursive: true, force: true });
  process.exitCode = failures.length ? 1 : 0;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  main().catch((err) => {
    process.stderr.write(`visual-parity: ${err.stack ?? err}\n`);
    process.exit(2);
  });
}
