// @ts-check
// Scene motion (island level up spec): a scene's `motion` is one loop of
// `duration` ms, with a track of keys per item it moves. An item opts in with
// an `id`, and its track's keys say where it is (`move`, pixels from where the
// scene places it), whether it shows (`show`) and which object it draws
// (`pose`) from that time on. Moves step a pixel at a time between keys, so
// the art stays on the grid. The engine compiles the tracks into CSS inside
// the scene's SVG, and the editor plays them with stateAt(). Time 0 is the
// rest frame, what reduced motion shows. No Node or DOM APIs.

/**
 * @typedef {{ t: number, move?: number[], show?: boolean, pose?: string }} Key
 * @typedef {{ item: string, keys: Key[] }} Track
 * @typedef {{ duration: number, tracks: Track[] }} Motion
 * @typedef {{ move: [number, number], show: boolean, pose: string }} State
 */

/** Each frame loop's full cycle in ms, as pixel-art.css runs it; other loops step every 200ms. */
export const LOOP_CYCLES = /** @type {Record<string, number>} */ ({ wf: 1000, ff: 600, hf: 3000 });

/**
 * The frame loops pixel-art.css plays, with the class prefix of their frames
 * and how many frames it shows (w0–w4, f0–f3, h0–h5). A loop with more frames
 * than that never shows the rest on the site.
 */
export const SITE_LOOPS = /** @type {Record<string, { prefix: string, frames: number }>} */ ({
  ff: { prefix: 'f', frames: 4 },
  wf: { prefix: 'w', frames: 5 },
  hf: { prefix: 'h', frames: 6 },
});

/**
 * The frame a loop of `count` frames shows at `time` ms.
 * @param {string} loop
 * @param {number} count
 * @param {number} time
 */
export function loopFrame(loop, count, time) {
  const cycle = LOOP_CYCLES[loop] ?? 200 * count;
  return Math.floor(((((time % cycle) + cycle) % cycle) / cycle) * count) % count;
}

export const MOTION_PROPS = ['duration', 'tracks'];
export const TRACK_PROPS = ['item', 'keys'];
export const KEY_PROPS = ['t', 'move', 'show', 'pose'];

const isRecord = (/** @type {unknown} */ v) => typeof v === 'object' && v !== null && !Array.isArray(v);
const q = (/** @type {unknown} */ v) => `'${String(v)}'`;

/**
 * Every object item with an id, by id, searching groups too.
 * @param {any[]} items
 * @param {Map<string, any>} [out]
 * @returns {Map<string, any>}
 */
export function itemsById(items, out = new Map()) {
  for (const item of items ?? []) {
    if (!isRecord(item)) continue;
    if ('group' in item) itemsById(item.items, out);
    else if (typeof item.id === 'string') out.set(item.id, item);
  }
  return out;
}

/**
 * Checks a scene's `motion` against its items and the objects.
 * @param {{ objects: Map<string, any> }} sources
 * @param {any} scene
 * @param {(msg: string) => void} add
 */
export function validateMotion(sources, scene, add) {
  const motion = scene.motion;
  if (!isRecord(motion)) return add('motion must be { duration, tracks }');
  for (const key of Object.keys(motion)) if (!MOTION_PROPS.includes(key)) add(`motion: unknown property ${q(key)}`);
  const { duration } = motion;
  if (!Number.isInteger(duration) || duration < 100) add('motion: duration must be a whole number of ms, at least 100');
  if (!Array.isArray(motion.tracks)) return add('motion: tracks must be a list');
  const ids = itemsById(scene.items);
  const seen = new Set();
  motion.tracks.forEach((/** @type {any} */ track, /** @type {number} */ i) => {
    const where = `motion.tracks[${i}]`;
    if (!isRecord(track)) return add(`${where}: must be { item, keys }`);
    for (const key of Object.keys(track)) if (!TRACK_PROPS.includes(key)) add(`${where}: unknown property ${q(key)}`);
    if (typeof track.item !== 'string' || !ids.has(track.item)) add(`${where}: item ${q(track.item)} is no item's id`);
    else if (seen.has(track.item)) add(`${where}: item ${q(track.item)} already has a track`);
    seen.add(track.item);
    if (!Array.isArray(track.keys) || track.keys.length === 0) return add(`${where}: keys must be a list of at least one key`);
    let last = -1;
    track.keys.forEach((/** @type {any} */ key, /** @type {number} */ k) => {
      const at = `${where}.keys[${k}]`;
      if (!isRecord(key)) return add(`${at}: must be { t, move?, show?, pose? }`);
      for (const prop of Object.keys(key)) if (!KEY_PROPS.includes(prop)) add(`${at}: unknown property ${q(prop)}`);
      if (!Number.isInteger(key.t)) add(`${at}: t must be a whole number of ms`);
      else if (k === 0 && key.t !== 0) add(`${at}: the first key must be at t 0, the rest frame`);
      else if (key.t <= last) add(`${at}: keys must be in time order, each later than the last`);
      else if (Number.isInteger(duration) && key.t >= duration) add(`${at}: t must be less than the duration`);
      if (Number.isInteger(key.t)) last = key.t;
      if ('move' in key && !(Array.isArray(key.move) && key.move.length === 2 && key.move.every(Number.isInteger))) {
        add(`${at}: move must be two whole numbers`);
      }
      if ('show' in key && typeof key.show !== 'boolean') add(`${at}: show must be true or false`);
      if ('pose' in key && (typeof key.pose !== 'string' || !sources.objects.has(key.pose))) add(`${at}: pose ${q(key.pose)} does not exist`);
    });
  });
}

/**
 * Each key with its inherited values filled in: move [0, 0], show true and
 * the item's own object until a key says otherwise.
 * @param {Track} track
 * @param {string} object
 * @returns {{ t: number, move: [number, number], show: boolean, pose: string }[]}
 */
export function filledKeys(track, object) {
  /** @type {State} */
  let state = { move: [0, 0], show: true, pose: object };
  return track.keys.map((key) => {
    state = {
      move: key.move ? [key.move[0], key.move[1]] : state.move,
      show: key.show ?? state.show,
      pose: key.pose ?? state.pose,
    };
    return { t: key.t, ...state };
  });
}

/** @type {(a: number, b: number) => number} */
const gcd = (a, b) => (b === 0 ? a : gcd(b, a % b));

/**
 * How many pixel steps a move of (dx, dy) takes: one per pixel along a
 * straight or 2:1 line (2 px across and 1 down is one step), so every step
 * lands on whole pixels. 0 for no move.
 * @param {number} dx
 * @param {number} dy
 */
export function stepsFor(dx, dy) {
  const [ax, ay] = [Math.abs(dx), Math.abs(dy)];
  if (ax === 0 || ay === 0) return Math.max(ax, ay);
  return gcd(ax, ay);
}

/**
 * Where a tracked item is at `time` ms: its keys in order, the last one
 * looping back to the first at `duration`. Between two keys it steps toward
 * the next key's move; show and pose change at their key's time.
 * @param {Track} track
 * @param {string} object  the item's own object
 * @param {number} duration
 * @param {number} time
 * @returns {State}
 */
export function stateAt(track, object, duration, time) {
  const keys = filledKeys(track, object);
  const t = ((time % duration) + duration) % duration;
  let i = keys.length - 1;
  while (i > 0 && keys[i].t > t) i--;
  const from = keys[i];
  const to = keys[i + 1] ?? { ...keys[0], t: duration };
  const [dx, dy] = [to.move[0] - from.move[0], to.move[1] - from.move[1]];
  const n = stepsFor(dx, dy);
  const done = n === 0 ? 0 : Math.floor(((t - from.t) / (to.t - from.t)) * n);
  return { move: [from.move[0] + (dx / (n || 1)) * done, from.move[1] + (dy / (n || 1)) * done], show: from.show, pose: from.pose };
}

/**
 * The scene's items as they stand at `time`: each tracked item placed by px
 * at its moved point, drawing its pose, and marked `hidden` while it doesn't
 * show. Untracked items are returned as they are.
 * @param {any[]} items
 * @param {Motion | undefined} motion
 * @param {number} time
 * @param {(item: any) => number[]} pointOf  an item's placement point in px
 */
export function poseItems(items, motion, time, pointOf) {
  if (!motion || !Array.isArray(motion.tracks) || motion.tracks.length === 0) return items;
  const tracks = new Map(motion.tracks.map((track) => [track.item, track]));
  /** @type {(list: any[]) => any[]} */
  const walk = (list) =>
    list.map((item) => {
      if ('group' in item) return { ...item, items: walk(item.items ?? []) };
      const track = item.id !== undefined ? tracks.get(item.id) : undefined;
      if (!track) return item;
      const { move, show, pose } = stateAt(track, item.object, motion.duration, time);
      const [x, y] = pointOf(item);
      return { ...item, object: pose, at: { px: [x + move[0], y + move[1]] }, ...(show ? {} : { hidden: true }) };
    });
  return walk(items);
}

/**
 * The items with every hidden one taken out.
 * @param {any[]} items
 * @returns {any[]}
 */
export function shownItems(items) {
  return items
    .filter((item) => !item.hidden)
    .map((item) => ('group' in item ? { ...item, items: shownItems(item.items ?? []) } : item));
}

const px = (/** @type {number} */ v) => (v === 0 ? '0' : `${v}px`);
const pct = (/** @type {number} */ t, /** @type {number} */ duration) => `${+((t / duration) * 100).toFixed(3)}%`;

/** The CSS class of a tracked item's group, and of each of its poses. */
export const motionClass = (/** @type {string} */ id) => `mv-${id}`;
export const poseClass = (/** @type {string} */ id, /** @type {number} */ k) => `mv-${id}-${k}`;

/**
 * The poses a track uses, the item's own object first.
 * @param {Track} track
 * @param {string} object
 */
export function posesOf(track, object) {
  const out = [object];
  for (const key of track.keys) if (key.pose && !out.includes(key.pose)) out.push(key.pose);
  return out;
}

/**
 * The CSS that plays a scene's motion: per track, a transform animation that
 * steps between keys (relative to the rest frame, which the art is drawn
 * at), an opacity animation for show, and one per pose. The base rules hold
 * the rest frame, so reduced motion, which turns every animation off, shows
 * time 0.
 * @param {Motion} motion
 * @param {Map<string, any>} ids  items by id
 */
export function motionCss(motion, ids) {
  const { duration } = motion;
  const rules = [];
  const run = (/** @type {string} */ name, /** @type {string} */ timing) => `animation:${name} ${duration}ms ${timing} infinite`;
  for (const track of motion.tracks) {
    const item = ids.get(track.item);
    const keys = filledKeys(track, item.object);
    const cls = motionClass(track.item);
    const [rx, ry] = keys[0].move;
    const moves = keys.some((k) => k.move[0] !== rx || k.move[1] !== ry);
    const shows = keys.some((k) => !k.show);
    const animations = [];
    if (moves) {
      const frames = [...keys, { ...keys[0], t: duration }].map((key, i, all) => {
        const next = all[i + 1];
        const n = next ? stepsFor(next.move[0] - key.move[0], next.move[1] - key.move[1]) : 0;
        const timing = n ? `;animation-timing-function:steps(${n},end)` : '';
        return `${pct(key.t, duration)}{transform:translate(${px(key.move[0] - rx)},${px(key.move[1] - ry)})${timing}}`;
      });
      rules.push(`@keyframes ${cls}{${frames.join('')}}`);
      animations.push(`${cls} ${duration}ms step-end infinite`);
    }
    if (shows) {
      const frames = keys.map((key) => `${pct(key.t, duration)}{opacity:${key.show ? 1 : 0}}`);
      rules.push(`@keyframes ${cls}-s{${frames.join('')}}`);
      animations.push(`${cls}-s ${duration}ms step-end infinite`);
    }
    const base = keys[0].show ? '' : 'opacity:0;';
    if (animations.length || base) rules.push(`.${cls}{${base}${animations.length ? `animation:${animations.join(',')}` : ''}}`);
    const poses = posesOf(track, item.object);
    if (poses.length > 1) {
      poses.forEach((pose, k) => {
        const name = poseClass(track.item, k);
        const frames = keys.map((key) => `${pct(key.t, duration)}{opacity:${key.pose === pose ? 1 : 0}}`);
        rules.push(`@keyframes ${name}{${frames.join('')}}`);
        rules.push(`.${name}{${keys[0].pose === pose ? '' : 'opacity:0;'}${run(name, 'step-end')}}`);
      });
    }
  }
  if (rules.length === 0) return '';
  rules.push('@media (prefers-reduced-motion:reduce){[class^="mv-"],[class*=" mv-"]{animation:none}}');
  return rules.join('');
}
