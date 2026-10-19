// @ts-check
// Starter sources for new objects and scenes (pixel-art engine spec R38,
// D9.6). `npm run art -- --new` and the editor's New dialogs both build their
// documents here, so a new file is the same whichever way it's made.

/** Names are at most this long, so file names and the editor stay readable. */
export const MAX_NAME = 64;

/** An empty scene: a 128×128 view around tile [0, 0, 0]. */
export const starterScene = () => ({ viewBox: [-64, -64, 128, 128], origin: [0, 0], items: [] });

/**
 * An empty sprite, anchored at its bottom center, so it stands on a tile.
 * @param {number} width
 * @param {number} height
 */
export const starterSprite = (width, height) => ({
  kind: 'sprite',
  anchor: [Math.floor(width / 2), height - 1],
  keys: { o: 'ink' },
  layers: [{ map: Array(height).fill('.'.repeat(width)) }],
});

/** A one-level block, lit like the island: light top, mid left, shadow right. */
export const starterBlock = () => ({ kind: 'block', size: [1, 1, 1], faces: { top: 'grass-2', left: 'soil-2', right: 'soil-3' } });

/**
 * An object that extends `base` with no rows overridden yet. It validates as
 * it is, and its rows can be overridden one at a time.
 * @param {string} base
 */
export const starterExtends = (base) => ({ kind: 'sprite', extends: base, keys: {}, rows: {} });
