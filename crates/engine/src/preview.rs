//! PNG previews of pixel art (ports cli/lib/engine/preview.mjs; the site's
//! pixel-art engine spec R29): an object on one tile outline, a scene at its
//! viewBox with only its first data-class variant, both on the document's
//! card color, at frame 0 of every loop; and the palette's swatch sheet.

use crate::engine::Sources;
use crate::png::Image;
use crate::Result;

/// The preview colors a document gets when its manifest names none.
pub const DEFAULT_CARD: &str = "#E9DCC6";
pub const DEFAULT_GRID: &str = "rgba(90, 62, 43, 0.06)";

/// The card color and the grid line color over it, as opaque RGB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviewColors {
    pub card: [u8; 3],
    pub grid: [u8; 3],
}

/// The card and grid colors from a manifest's `preview` (`previewColors`),
/// or the error message previewColors threw.
pub fn preview_colors(card: Option<&str>, grid: Option<&str>) -> std::result::Result<PreviewColors, String> {
    let _ = (card, grid);
    todo!("U8: port preview.mjs previewColors")
}

/// What a preview shows: an object or a scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target<'a> {
    Object(&'a str),
    Scene(&'a str),
}

/// Renders an object (on a tile) or a scene (at its viewBox) to RGBA
/// (`renderPreview`), with the image's top-left in scene pixels.
pub fn render_preview(sources: &Sources, target: Target<'_>, colors: PreviewColors) -> Result<(Image, [i32; 2])> {
    let _ = (sources, target, colors);
    todo!("U8: port preview.mjs renderPreview")
}

/// The materials of the palette sheet's rows (`MATERIALS`).
pub const MATERIALS: [&str; 7] = ["grass", "soil", "wood", "path", "water", "roof", "gold"];

/// The world palette's swatch sheet (`renderPalette`).
pub fn render_palette(sources: &Sources, colors: PreviewColors) -> Image {
    let _ = (sources, colors);
    todo!("U8: port preview.mjs renderPalette")
}

/// An RGBA PNG of the image, scaled up by whole-pixel repetition
/// (`encodePng`).
pub fn encode_png(image: &Image, scale: u32) -> Vec<u8> {
    crate::png::encode(image, scale)
}
