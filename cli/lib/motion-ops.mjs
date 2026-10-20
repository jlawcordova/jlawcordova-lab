// Scene motion edits (docs/intents/2026-10-mcp-gaps/spec.md, D8): tracks and
// their keys, the loop's length, the off-grid check and the path helper. The
// lab only ever keyed moves (scene-ops keyedMove); these follow the engine's
// motion rules (motion.mjs validateMotion) so what they write validates.
//
// A key is { t, move?, show?, pose? }: move is pixels from where the scene
// places the item, show and pose hold from their key on. Between two keys an
// item steps stepsFor(dx, dy) times; it's on the grid when each step is 1 px
// straight, 1:1 or 2:1, so the art never lands between pixels of its lines.

import { SceneOpError } from './scene-ops.mjs';

/** Each step of an on-grid move, in absolute pixels: straight, 1:1 or 2:1. */
export const ON_GRID = [
  [1, 0],
  [0, 1],
  [1, 1],
  [2, 1],
];

const KEY_ORDER = ['t', 'move', 'show', 'pose'];

/** Rebuilds a key's props in the canonical order: t, move, show, pose. */
function canonicalKey(/** @type {any} */ key) {
  /** @type {any} */
  const out = {};
  for (const prop of KEY_ORDER) if (key[prop] !== undefined) out[prop] = key[prop];
  return out;
}

/**
 * Whether a move of (dx, dy) between two keys steps on the grid.
 * @param {any} motionLib  the engine's motion module
 * @param {number} dx
 * @param {number} dy
 */
export function onGrid(motionLib, dx, dy) {
  const n = motionLib.stepsFor(dx, dy);
  if (n === 0) return true;
  const [ux, uy] = [Math.abs(dx) / n, Math.abs(dy) / n];
  return ON_GRID.some(([a, b]) => a === ux && b === uy);
}

/**
 * Every key-to-key move of a scene's motion that isn't on the grid, the
 * loop back from the last key to the first included. A move made while the
 * item is hidden (a reset to its start) isn't seen, so it isn't listed.
 * @param {any} motionLib
 * @param {any} scene
 * @returns {{ track: number, item: string, from: number, to: number, move: [number, number], steps: number, step: [number, number] }[]}
 */
export function offGridMoves(motionLib, scene) {
  const motion = scene?.motion;
  if (!motion || !Array.isArray(motion.tracks) || !Number.isInteger(motion.duration)) return [];
  const ids = motionLib.itemsById(scene.items);
  const out = [];
  motion.tracks.forEach((/** @type {any} */ track, /** @type {number} */ ti) => {
    const item = ids.get(track?.item);
    if (!item || !Array.isArray(track.keys) || track.keys.length === 0) return;
    let keys;
    try {
      keys = motionLib.filledKeys(track, item.object);
    } catch {
      return;
    }
    keys.forEach((/** @type {any} */ from, /** @type {number} */ i) => {
      const to = keys[i + 1] ?? { ...keys[0], t: motion.duration };
      const [dx, dy] = [to.move[0] - from.move[0], to.move[1] - from.move[1]];
      if (!from.show || onGrid(motionLib, dx, dy)) return;
      const steps = motionLib.stepsFor(dx, dy);
      out.push({ track: ti, item: track.item, from: from.t, to: to.t, move: [dx, dy], steps, step: [dx / steps, dy / steps] });
    });
  });
  return out;
}

/** The warning line for an off-grid move, as check lists it. */
export function offGridText(/** @type {string} */ file, /** @type {ReturnType<typeof offGridMoves>[number]} */ w) {
  const s = w.steps === 1 ? '1 step' : `${w.steps} steps`;
  return `${file}: motion.tracks[${w.track}] (${w.item}): ${w.from} → ${w.to} ms moves (${w.move.join(', ')}) in ${s} of (${w.step.join(', ')}); on the grid each step is 1 px straight, 1:1 or 2:1 (scene_motion_path makes such keys)`;
}

/** The track of an item id, or null. */
export function trackOf(/** @type {any} */ doc, /** @type {string} */ id) {
  return doc.motion?.tracks?.find((/** @type {any} */ t) => t.item === id) ?? null;
}

/** The track of an item id, or a no_track error. */
export function needTrack(/** @type {any} */ doc, /** @type {string} */ id) {
  const track = trackOf(doc, id);
  if (!track) throw new SceneOpError(`${id} has no motion track (scene.track.add makes one)`, 'no_track');
  return track;
}

const ms = (/** @type {number} */ t) => `${t} ms`;

/** Refuses a time outside the loop. */
function checkTime(/** @type {any} */ doc, /** @type {number} */ t) {
  if (!Number.isInteger(t) || t < 0) throw new SceneOpError(`a key's time is a whole number of ms from 0`, 'usage');
  if (t >= doc.motion.duration) throw new SceneOpError(`${ms(t)} is past the loop, which is ${ms(doc.motion.duration)} long (0 to ${doc.motion.duration - 1})`, 'out_of_range');
}

/**
 * Adds a track for an item id: one key at 0, the rest frame. Makes the
 * scene's motion with `duration` when it has none.
 * @param {any} doc
 * @param {string} id
 * @param {number} [duration]
 */
export function addTrack(doc, id, duration) {
  if (!doc.motion) {
    if (duration === undefined) throw new SceneOpError('the scene has no motion yet; give its loop length with --duration (ms)', 'no_motion');
    doc.motion = { duration, tracks: [] };
  } else if (duration !== undefined && duration !== doc.motion.duration) {
    throw new SceneOpError(`the scene's motion is ${ms(doc.motion.duration)} long; scene.motion.set changes that`, 'usage');
  }
  if (trackOf(doc, id)) throw new SceneOpError(`${id} already has a track`, 'track_exists');
  const track = { item: id, keys: [{ t: 0 }] };
  doc.motion.tracks.push(track);
  return { track, message: `Added a track for ${id}` };
}

/** Removes an item's track. */
export function removeTrack(/** @type {any} */ doc, /** @type {string} */ id) {
  needTrack(doc, id);
  doc.motion.tracks = doc.motion.tracks.filter((/** @type {any} */ t) => t.item !== id);
  return { message: `Removed ${id}'s track` };
}

/**
 * Sets a key's values, making the key (in time order) if there isn't one.
 * With no values, the key pins the item where it is at `t`: `pin` is the
 * item's move at that time (the engine's stateAt), written as the new key's
 * move; a key already there is left as it is.
 * @param {any} doc
 * @param {string} id
 * @param {number} t
 * @param {{ move?: number[], show?: boolean, pose?: string, clear?: string[], pin?: number[] }} values
 */
export function setKey(doc, id, t, { move, show, pose, clear = [], pin }) {
  const track = needTrack(doc, id);
  checkTime(doc, t);
  let i = track.keys.findIndex((/** @type {any} */ k) => k.t === t);
  const made = i < 0;
  const pinning = move === undefined && show === undefined && pose === undefined && clear.length === 0;
  if (pinning) {
    if (!made) return { key: track.keys[i], made: false, pinned: false, message: `${id} already has a key at ${ms(t)}` };
    if (!pin) throw new SceneOpError('a key needs a move, show or pose, or a pin', 'usage');
    move = pin;
  }
  if (made) {
    i = track.keys.filter((/** @type {any} */ k) => k.t < t).length;
    track.keys.splice(i, 0, { t });
  }
  const key = { ...track.keys[i] };
  for (const prop of clear) {
    if (!['move', 'show', 'pose'].includes(prop)) throw new SceneOpError(`clear takes move, show or pose, not ${prop}`, 'usage');
    delete key[prop];
  }
  if (move !== undefined) key.move = [move[0], move[1]];
  if (show !== undefined) key.show = show;
  if (pose !== undefined) key.pose = pose;
  track.keys[i] = canonicalKey(key);
  if (pinning) return { key: track.keys[i], made, pinned: true, message: `Made ${id}'s key at ${ms(t)}, pinned at ${move.join(', ')}` };
  const said = [move ? `move ${move.join(', ')}` : '', show !== undefined ? `show ${show}` : '', pose ? `pose ${pose}` : '', clear.length ? `cleared ${clear.join(', ')}` : ''].filter(Boolean);
  return { key: track.keys[i], made, pinned: false, message: `${made ? 'Made' : 'Set'} ${id}'s key at ${ms(t)}${said.length ? `: ${said.join(', ')}` : ''}` };
}

/** Moves a key to another time (not the first key, not onto another). */
export function moveKey(/** @type {any} */ doc, /** @type {string} */ id, /** @type {number} */ t, /** @type {number} */ to) {
  const track = needTrack(doc, id);
  const key = track.keys.find((/** @type {any} */ k) => k.t === t);
  if (!key) throw new SceneOpError(`${id} has no key at ${ms(t)}; its keys are at ${track.keys.map((/** @type {any} */ k) => k.t).join(', ')} ms`, 'not_found');
  if (t === 0) throw new SceneOpError("the key at 0 is the rest frame and stays there; set its values instead", 'first_key');
  if (to === 0) throw new SceneOpError('0 is the rest frame, which the first key holds', 'first_key');
  checkTime(doc, to);
  if (to !== t && track.keys.some((/** @type {any} */ k) => k.t === to)) throw new SceneOpError(`${id} already has a key at ${ms(to)}`, 'key_exists');
  key.t = to;
  track.keys.sort((/** @type {any} */ a, /** @type {any} */ b) => a.t - b.t);
  return { key, message: `Moved ${id}'s key from ${ms(t)} to ${ms(to)}` };
}

/** Deletes a key (not the first: remove the track for that). */
export function deleteKey(/** @type {any} */ doc, /** @type {string} */ id, /** @type {number} */ t) {
  const track = needTrack(doc, id);
  const i = track.keys.findIndex((/** @type {any} */ k) => k.t === t);
  if (i < 0) throw new SceneOpError(`${id} has no key at ${ms(t)}; its keys are at ${track.keys.map((/** @type {any} */ k) => k.t).join(', ')} ms`, 'not_found');
  if (i === 0) throw new SceneOpError('the key at 0 is the rest frame; remove the track to drop it (scene.track.remove)', 'first_key');
  track.keys.splice(i, 1);
  return { message: `Deleted ${id}'s key at ${ms(t)}` };
}

/** Sets (or makes) the loop's length; refuses one that would cut off keys. */
export function setDuration(/** @type {any} */ doc, /** @type {number} */ duration) {
  if (!doc.motion) {
    doc.motion = { duration, tracks: [] };
    return { message: `Made the scene's motion, ${ms(duration)} long` };
  }
  const late = doc.motion.tracks.flatMap((/** @type {any} */ tr) => tr.keys.filter((/** @type {any} */ k) => k.t >= duration).map((/** @type {any} */ k) => `${tr.item} at ${ms(k.t)}`));
  if (late.length) throw new SceneOpError(`a ${ms(duration)} loop would cut off key${late.length === 1 ? '' : 's'}: ${late.join(', ')}; move or delete ${late.length === 1 ? 'it' : 'them'} first`, 'out_of_range');
  doc.motion.duration = duration;
  return { message: `The loop is ${ms(duration)} long` };
}

/**
 * A move split into on-grid runs: for |dx| ≥ 2|dy| a 2:1 run then a straight
 * one; between, a 2:1 run and a 1:1 run; steeper, a 1:1 run then a straight
 * vertical one.
 * @param {number} dx
 * @param {number} dy
 * @returns {[number, number][]}
 */
export function gridRuns(dx, dy) {
  const [a, b] = [Math.abs(dx), Math.abs(dy)];
  const [sx, sy] = [Math.sign(dx), Math.sign(dy)];
  /** @type {[number, number][]} */
  const runs = [];
  const add = (/** @type {number} */ x, /** @type {number} */ y) => {
    if (x !== 0 || y !== 0) runs.push([x * sx + 0, y * sy + 0]);
  };
  if (a >= 2 * b) {
    add(2 * b, b);
    add(a - 2 * b, 0);
  } else if (a >= b) {
    add(2 * (a - b), a - b);
    add(2 * b - a, 2 * b - a);
  } else {
    add(a, a);
    add(0, b - a);
  }
  return runs;
}

/**
 * Keys along a polyline, on the grid, at a steady speed (px per second) or
 * spread from `start` to `end`. Existing keys in (start, end] lose their
 * move; those left empty go, and those with show or pose get the point on
 * the path at their time, so the path stays on the grid through them.
 * @param {any} motionLib
 * @param {any} doc
 * @param {string} id
 * @param {number[][]} points  moves (pixels from the item's placement), the first at `start`
 * @param {{ start: number, speed?: number, end?: number }} timing
 */
export function motionPath(motionLib, doc, id, points, { start, speed, end }) {
  const track = needTrack(doc, id);
  checkTime(doc, start);
  if (points.length < 2) throw new SceneOpError('a path needs at least two points', 'usage');
  /** @type {{ from: number[], to: number[], length: number }[]} */
  const runs = [];
  for (let i = 1; i < points.length; i++) {
    let at = points[i - 1];
    for (const [rx, ry] of gridRuns(points[i][0] - points[i - 1][0], points[i][1] - points[i - 1][1])) {
      const to = [at[0] + rx, at[1] + ry];
      runs.push({ from: at, to, length: Math.hypot(rx, ry) });
      at = to;
    }
  }
  if (runs.length === 0) throw new SceneOpError('the path goes nowhere: every point is the same', 'usage');
  const total = runs.reduce((n, r) => n + r.length, 0);
  if (end !== undefined && end <= start) throw new SceneOpError(`end (${ms(end)}) must be after start (${ms(start)})`, 'usage');
  /** @type {number[]} */
  const times = [];
  let cum = 0;
  for (const r of runs) {
    cum += r.length;
    const t = start + Math.round(speed !== undefined ? (cum / speed) * 1000 : (cum / total) * (/** @type {number} */ (end) - start));
    times.push(Math.max(t, (times.at(-1) ?? start) + 1));
  }
  const last = /** @type {number} */ (times.at(-1));
  if (last >= doc.motion.duration || (end !== undefined && last > end)) {
    const takes = speed !== undefined ? `at ${speed} px/s the path takes ${last - start} ms` : `the path needs ${last - start} ms`;
    throw new SceneOpError(`${takes} from ${ms(start)}, ending at ${ms(last)}, past ${end !== undefined && last > end ? ms(end) : `the ${ms(doc.motion.duration)} loop`}`, 'too_long');
  }
  // Keys inside the path: they lose their move, and keep a point on it if they hold show or pose.
  /** @type {any[]} */
  const kept = [];
  for (const key of track.keys) {
    if (key.t <= start || key.t > last) {
      kept.push(key);
      continue;
    }
    const { move: _m, ...rest } = key;
    if (rest.show === undefined && rest.pose === undefined) continue;
    const k = times.findIndex((t) => t >= key.t);
    const run = runs[k];
    const t0 = k === 0 ? start : times[k - 1];
    const n = motionLib.stepsFor(run.to[0] - run.from[0], run.to[1] - run.from[1]);
    const done = Math.floor(((key.t - t0) / (times[k] - t0)) * n);
    const move = [run.from[0] + ((run.to[0] - run.from[0]) / n) * done, run.from[1] + ((run.to[1] - run.from[1]) / n) * done];
    kept.push(canonicalKey({ ...rest, move }));
  }
  const byT = new Map(kept.map((k) => [k.t, k]));
  const startKey = byT.get(start) ?? { t: start };
  byT.set(start, canonicalKey({ ...startKey, move: [points[0][0], points[0][1]] }));
  /** @type {{ t: number, move: number[] }[]} */
  const written = [{ t: start, move: [points[0][0], points[0][1]] }];
  runs.forEach((r, i) => {
    const t = times[i];
    byT.set(t, canonicalKey({ ...(byT.get(t) ?? { t }), move: r.to }));
    written.push({ t, move: r.to });
  });
  track.keys = [...byT.values()].sort((a, b) => a.t - b.t);
  return { keys: written, end: last, message: `Keyed ${id} along ${points.length} points: ${runs.length} on-grid run${runs.length === 1 ? '' : 's'}, ${ms(start)} to ${ms(last)}` };
}
