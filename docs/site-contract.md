# The site contract

What a site (a page that inlines a compiled SVG) may rely on in the SVG that `pixellab art build` writes. This page is the whole promise: anything it doesn't say can change in a release. The design is in [the Rust engine spec](intents/2026-10-rust-engine/spec.md) (R11 to R15, R19 and D7); the document that produces these SVGs is described in [Document format](document-format.md).

## The root

Every compiled SVG's root `<svg>` carries:

| Attribute | Value |
|---|---|
| `data-pixellab` | The scene's name, for example `data-pixellab="hero-island"`. Select a scene's SVG with `[data-pixellab="hero-island"]`. |
| `shape-rendering` | `crispEdges`, so the art renders crisply without a stylesheet rule on the page. |

The compiler **never gives the root a `class`**. A page may add its own, by a string replace or with script, and the SVG will not have one to clash with.

## The root's size

A scene that fixes its `viewBox` publishes with exactly that `viewBox`. A scene that does not publishes with the smallest box that holds everything it paints over its loop, plus its `margin`, so the `viewBox` changes when the art changes. A page that depends on a scene's size should have the document fix it (`pixellab scene frame <scene> --box x,y,w,h`). A release before this one rejects a scene without a `viewBox`, so the site pins a release that has it before its document gets such a scene.

## Classes and data attributes from the document

A group in a scene's `items` gives its attributes in `group` (`class` and `data-*` only), and an item may carry a `class`. The compiler writes them to the output **unchanged**, so a page can target and read them:

- `class`, such as `enter`, on a group or an item.
- `data-*` attributes on a group, such as `data-class`, `data-enter`, `data-row` and `data-x`.

The art decides these names, and they are the part of the SVG a page's own script and CSS may be written against. Order, nesting and the element names around them are not promised.

## Animation plays by itself

An SVG carries its animation in its own `<style>`, with no stylesheet needed on the page:

- **Frame loops** (an object's layer with `loop`, `prefix` and `frames`) cycle on their own, one frame at a time.
- **Ambient loops** (`ambient` on a scene item or an object's map layer) repeat on their own.
- **Scene motion** plays on its own too.
- A scene with nothing animated has no `<style>`.

Every rule is scoped to its scene, so two SVGs inlined on one page never clash: each selector starts with `[data-pixellab="<scene>"]`.

## Actions

An **action** is a named one-shot move that plays only when the page asks. The contract:

1. **To play an action,** add its name to a `data-play` attribute on the `<svg>`, or on any group that contains the item. The value is a space-separated list, for example `data-play="hop"` or `data-play="hop wave"`. While it plays, the action replaces the element's ambient loop.
2. **To restart it,** remove the name and add it back. Let the browser apply the removal first (read a layout property between the two, or wait a frame).
3. **When it ends,** the browser fires the standard `animationend` event. It bubbles, so one listener on the `<svg>` or a group hears every action below it. Its `animationName` is `<scene>--<action>`, for example `range--hop`.
4. **Under reduced motion** an action does not play and no event fires. A page that waits for `animationend` to clear `data-play` must not depend on it arriving.

The names of actions are in the document (see [Ambient loops and actions](document-format.md#ambient-loops-and-actions)).

### Example: play `hop`, clear it when it ends

```js
const svg = document.querySelector('[data-pixellab="range"]');
const group = svg.querySelector('[data-class="knight"]'); // or svg itself

function hop() {
  group.removeAttribute('data-play');
  void group.getBoundingClientRect(); // apply the removal so the action restarts
  group.setAttribute('data-play', 'hop');
}

svg.addEventListener('animationend', (event) => {
  if (event.animationName === 'range--hop') group.removeAttribute('data-play');
});

document.querySelector('#a-button').addEventListener('click', hop);
```

For a scene named `s`, the compiler plays an item's action with a rule that matches both placements, a `data-play` on a group around the item and on the root:

```css
[data-pixellab="s"] [data-play~="hop"] .s--aN
[data-pixellab="s"][data-play~="hop"] .s--aN
```

Those generated class names (`s--aN`) are not part of the contract. Only `data-play` and the event are.

## Reduced motion

When the visitor prefers reduced motion (`prefers-reduced-motion: reduce`):

- Everything stops: ambient loops, scene motion and actions.
- Frame loops show their first frame (frame 0).
- No `animationend` fires for an action.

## Nothing else is a promise

The generated `<scene>--…` class and keyframe names, the style's rules and their order, the paths and rects, the element nesting and the attribute order may all change in a release. Page code should use `data-pixellab`, the document's own classes and `data-*` attributes, `data-play`, and `animationend` with the `<scene>--<action>` name.
