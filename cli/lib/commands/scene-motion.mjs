// Scene motion commands (docs/intents/2026-10-mcp-gaps/spec.md, D8): tracks,
// keys (move, show, pose), the loop's length and the path helper. The edits
// are motion-ops.mjs; each runs as one undo step through ctx.edit, and the
// key edits return the off-grid warnings of the track they changed.

import { pathString } from '../render.mjs';
import * as motionOps from '../motion-ops.mjs';
import * as ops from '../scene-ops.mjs';
import { editOf, formatChange, needOne } from './common.mjs';
import { UsageError } from '../errors.mjs';

const NAME = { type: 'string', description: 'scene name (or scenes/<name>)' };
const ITEM = { type: 'string', description: 'the item: its path from scene items (12, 5.2) or its id; an item needs an id to have a track (scene.set --id)' };
const TIME = { type: 'integer', minimum: 0, description: 'ms into the loop' };
const MOVE = { type: 'array', items: { type: 'integer' }, minItems: 2, maxItems: 2, description: 'pixels from where the scene places the item: x,y' };

/** The scene's name, checked. */
async function sceneName(/** @type {any} */ ctx, /** @type {string} */ query) {
  const { name } = await ctx.find(query, 'scene');
  await ctx.doc('scene', name);
  return name;
}

/** The id of the item `ref` names, which must have one. */
function idOf(/** @type {any} */ ctx, /** @type {any} */ doc, /** @type {string} */ ref) {
  const path = ops.findItem(ctx.site, doc, ref);
  const item = ops.placedAt(doc, path);
  if (item.id === undefined) throw new ops.SceneOpError(`${item.object} at ${pathString(path)} has no id, so it can't have a track; give it one with scene.set --id`, 'no_id');
  return { id: item.id, item, path };
}

/** The off-grid warnings of one item's track. */
function warningsFor(/** @type {any} */ ctx, /** @type {any} */ doc, /** @type {string} */ name, /** @type {string} */ id) {
  return motionOps
    .offGridMoves(ctx.site.motion, doc)
    .filter((w) => w.item === id)
    .map((w) => motionOps.offGridText(`scenes/${name}.mjs`, w));
}

/**
 * One motion edit on a scene as one undo step.
 * @param {any} ctx
 * @param {string} query
 * @param {(doc: any, name: string) => Record<string, unknown> & { message: string }} fn
 */
async function motionEdit(ctx, query, fn) {
  const name = await sceneName(ctx, query);
  return editOf(ctx)({ kind: 'scene', name }, (/** @type {any} */ doc) => fn(doc, name));
}

/** @type {import('../registry.mjs').Command[]} */
export default [
  {
    name: 'scene.track.add',
    summary: 'Add a motion track for an item (it needs an id): one key at 0, the rest frame',
    description: "Makes the scene's motion (one loop of --duration ms) when it has none. Then scene.key.set or scene.motion.path keys it.",
    params: {
      type: 'object',
      properties: { name: NAME, item: ITEM, duration: { type: 'integer', minimum: 100, description: "the loop's length in ms, when the scene has no motion yet" } },
      required: ['name', 'item'],
    },
    args: ['name', 'item'],
    mutates: true,
    examples: ['pixellab scene track add hero-island truck-a', 'pixellab scene track add meadow bird --duration 4000'],
    run: (ctx, { name, item, duration }) =>
      motionEdit(ctx, name, (doc) => {
        const { id } = idOf(ctx, doc, item);
        const r = motionOps.addTrack(doc, id, duration);
        return { message: r.message, track: structuredClone(r.track), duration: doc.motion.duration };
      }),
    format: formatChange,
  },
  {
    name: 'scene.track.remove',
    summary: "Remove an item's motion track, with all its keys",
    params: { type: 'object', properties: { name: NAME, item: ITEM }, required: ['name', 'item'] },
    args: ['name', 'item'],
    mutates: true,
    run: (ctx, { name, item }) =>
      motionEdit(ctx, name, (doc) => {
        const { id } = idOf(ctx, doc, item);
        return motionOps.removeTrack(doc, id);
      }),
    format: formatChange,
  },
  {
    name: 'scene.key.set',
    summary: "Set a key of an item's track: its move, show and pose at a time (made if there's no key there)",
    description:
      '--move x,y is pixels from where the scene places the item; --show false hides it from that key on; --pose <object> draws another object from that key on. ' +
      '--clear move,show,pose removes those values from the key, so they carry on from the key before. Returns the key and the track\'s off-grid warnings ' +
      '(moves whose steps aren\'t 1 px straight, 1:1 or 2:1).',
    params: {
      type: 'object',
      properties: {
        name: NAME,
        item: ITEM,
        time: TIME,
        move: MOVE,
        show: { type: 'boolean', description: 'whether the item shows from this key on' },
        pose: { type: 'string', description: 'the object the item draws from this key on' },
        clear: { type: 'array', items: { type: 'string', enum: ['move', 'show', 'pose'] }, description: 'values to remove from the key' },
      },
      required: ['name', 'item', 'time'],
    },
    args: ['name', 'item'],
    mutates: true,
    examples: ['pixellab scene key set hero-island truck-a --time 4200 --move 32,16', 'pixellab scene key set hero-island crane-hook --time 9000 --pose crane-hook-down', 'pixellab scene key set hero-island parcel --time 12000 --no-show'],
    async run(ctx, params) {
      needOne(params, ['move', 'show', 'pose', 'clear'], 'scene.key.set');
      if (params.pose !== undefined && !(await ctx.names('object')).includes(params.pose)) throw new UsageError(`scene.key.set: no object named ${params.pose} to pose as (pixellab object list)`);
      return motionEdit(ctx, params.name, (doc, name) => {
        const { id } = idOf(ctx, doc, params.item);
        const r = motionOps.setKey(doc, id, params.time, { move: params.move, show: params.show, pose: params.pose, clear: params.clear });
        return { message: r.message, key: structuredClone(r.key), warnings: warningsFor(ctx, doc, name, id) };
      });
    },
    format: (r) => [r.summary, ...(r.warnings ?? []).map((/** @type {string} */ w) => `warning: ${w}`)].join('\n'),
  },
  {
    name: 'scene.key.move',
    summary: "Move a key of an item's track to another time",
    description: "The key at 0 (the rest frame) stays; a key can't land on another.",
    params: { type: 'object', properties: { name: NAME, item: ITEM, time: TIME, to: { ...TIME, description: 'its new time, ms into the loop' } }, required: ['name', 'item', 'time', 'to'] },
    args: ['name', 'item'],
    mutates: true,
    run: (ctx, { name: query, item, time, to }) =>
      motionEdit(ctx, query, (doc, name) => {
        const { id } = idOf(ctx, doc, item);
        const r = motionOps.moveKey(doc, id, time, to);
        return { message: r.message, key: structuredClone(r.key), warnings: warningsFor(ctx, doc, name, id) };
      }),
    format: (r) => [r.summary, ...(r.warnings ?? []).map((/** @type {string} */ w) => `warning: ${w}`)].join('\n'),
  },
  {
    name: 'scene.key.delete',
    summary: "Delete a key of an item's track (not the one at 0: remove the track for that)",
    params: { type: 'object', properties: { name: NAME, item: ITEM, time: TIME }, required: ['name', 'item', 'time'] },
    args: ['name', 'item'],
    mutates: true,
    run: (ctx, { name: query, item, time }) =>
      motionEdit(ctx, query, (doc, name) => {
        const { id } = idOf(ctx, doc, item);
        const r = motionOps.deleteKey(doc, id, time);
        return { message: r.message, warnings: warningsFor(ctx, doc, name, id) };
      }),
    format: (r) => [r.summary, ...(r.warnings ?? []).map((/** @type {string} */ w) => `warning: ${w}`)].join('\n'),
  },
  {
    name: 'scene.motion.set',
    summary: "Set the length of a scene's motion loop (making the motion if it has none)",
    description: 'Refuses a length that would cut off keys.',
    params: { type: 'object', properties: { name: NAME, duration: { type: 'integer', minimum: 100, description: "the loop's length in ms" } }, required: ['name', 'duration'] },
    args: ['name'],
    mutates: true,
    run: (ctx, { name, duration }) => motionEdit(ctx, name, (doc) => ({ ...motionOps.setDuration(doc, duration), duration })),
    format: formatChange,
  },
  {
    name: 'scene.motion.path',
    summary: "Key an item along a polyline on the grid, at a steady speed or between two times",
    description:
      '--points are moves (pixels from where the scene places the item), the first at --start. Each segment is split into runs that step 1 px straight, 1:1 or 2:1, ' +
      'each ending in a key; times follow the distance at --speed px per second, or spread from --start to --end. Keys inside the path lose their move ' +
      '(and go, if that leaves nothing); a key there with show or pose keeps its place on the path. Makes the track if the item (with an id) has none. ' +
      "Returns the keys written and the track's remaining off-grid warnings (such as the step from the path's end to the next key).",
    params: {
      type: 'object',
      properties: {
        name: NAME,
        item: ITEM,
        points: { type: 'array', items: { type: 'array', items: { type: 'integer' }, minItems: 2, maxItems: 2 }, minItems: 2, description: 'x,y moves along the path; on the command line x,y pairs separated by spaces' },
        start: { ...TIME, default: 0, description: 'when the path starts, ms into the loop' },
        speed: { type: 'number', description: 'pixels per second along the path' },
        end: { ...TIME, description: 'when the path ends, ms into the loop (instead of --speed)' },
      },
      required: ['name', 'item', 'points'],
    },
    args: ['name', 'item', 'points'],
    mutates: true,
    examples: ['pixellab scene motion path hero-island truck-a 0,0 16,-3 48,-3 --start 2000 --speed 20'],
    async run(ctx, params) {
      needOne(params, ['speed', 'end'], 'scene.motion.path');
      if (params.speed !== undefined && params.end !== undefined) throw new UsageError('scene.motion.path: give --speed or --end, not both');
      if (params.speed !== undefined && !(params.speed > 0)) throw new UsageError('scene.motion.path: --speed must be more than 0');
      return motionEdit(ctx, params.name, (doc, name) => {
        const { id } = idOf(ctx, doc, params.item);
        if (!motionOps.trackOf(doc, id)) {
          if (!doc.motion) throw new ops.SceneOpError('the scene has no motion yet; make it with scene.motion.set --duration', 'no_motion');
          motionOps.addTrack(doc, id);
        }
        const r = motionOps.motionPath(ctx.site.motion, doc, id, params.points, { start: params.start, speed: params.speed, end: params.end });
        return { message: r.message, keys: r.keys, end: r.end, warnings: warningsFor(ctx, doc, name, id) };
      });
    },
    format: (r) => [r.summary, ...(r.warnings ?? []).map((/** @type {string} */ w) => `warning: ${w}`)].join('\n'),
  },
];
