//! Layer tree → rect SVG (ports cli/lib/engine/svg.mjs; pixel-art engine
//! spec D6 step 3): in each layer there's one `<g fill>` per color, in order
//! of first paint, holding one `<rect>` per horizontal run, sorted by y and
//! then x. `compile::optimize_svg` turns it into paths.

use crate::engine::Rendered;

/// A scene with motion carries its CSS in a `<style>` first (`toRectSvg`).
/// `hex_of` maps a color name to "#RRGGBB".
pub fn to_rect_svg(scene: &Rendered, hex_of: &dyn Fn(&str) -> String) -> String {
    let _ = (scene, hex_of);
    todo!("U2: port svg.mjs toRectSvg")
}
