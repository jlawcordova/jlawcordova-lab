//! The pixel-art engine (ports cli/lib/engine/engine.mjs; pixel-art engine
//! spec D2–D6). It takes the palette, objects and scenes as plain data,
//! validates them, resolves objects, and renders scenes and objects to layer
//! trees, each list of items in depth order.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use serde_json::Value;

use crate::depth::Bounds;
use crate::iso::Pixel;
use crate::omap::{OMap, OSet};
use crate::Result;

/// The palette's tiers, in the order the engine reads them.
pub const TIERS: [&str; 2] = ["world", "outfit"];

/// The design language's caps (R26, R27).
pub struct Caps {
    pub world: usize,
    pub outfit: usize,
    pub size: i64,
    pub colors: usize,
    pub character_width: i64,
    pub character_height: i64,
}

pub const CAPS: Caps = Caps { world: 32, outfit: 16, size: 64, colors: 12, character_width: 24, character_height: 32 };

/// A sprite's footprint is whole tiles, 1 to this many each way.
pub const MAX_FOOTPRINT: i64 = 16;
/// A side map is one tile's face wide and one level high.
pub const SIDE_W: i32 = crate::iso::TILE_W / 2;
pub const SIDE_H: i32 = crate::iso::LEVEL_H;

/// A palette color: its hex and its tier ("world" or "outfit").
#[derive(Clone, Debug, PartialEq)]
pub struct PaletteColor {
    pub hex: String,
    pub tier: &'static str,
}

/// What an object needs for the depth order, cached per set of sources
/// (`extentOf`).
#[derive(Clone, Debug, PartialEq)]
pub struct Extent {
    pub bounds: Option<Bounds>,
    pub footprint: [i64; 2],
    pub height: f64,
    pub block: Option<[i64; 3]>,
}

/// The three source kinds, wrapped for the rest of the engine
/// (`loadSources`): objects and scenes in code-point order by name, and the
/// palette's colors by name, world before outfit.
#[derive(Debug, Default)]
pub struct Sources {
    pub palette: Value,
    pub colors: OMap<String, PaletteColor>,
    pub objects: BTreeMap<String, Value>,
    pub scenes: BTreeMap<String, Value>,
    /// `extentOf`'s cache. Whoever changes `objects` clears it.
    pub extents: RefCell<HashMap<String, Extent>>,
}

/// Wraps the three source kinds (`loadSources`).
pub fn load_sources(palette: Value, objects: BTreeMap<String, Value>, scenes: BTreeMap<String, Value>) -> Sources {
    let _ = (palette, objects, scenes);
    todo!("U3: port engine.mjs loadSources")
}

/// Code-point order, never locale order (`byCodePoint`).
pub fn by_code_point(a: &str, b: &str) -> std::cmp::Ordering {
    a.cmp(b)
}

/// A map key: one printable ASCII character other than space and '.'.
pub fn is_key(k: &str) -> bool {
    let mut chars = k.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if c > ' ' && c <= '~' && c != '.')
}

/// Lowercase kebab-case (`isName`).
pub fn is_name(name: &str) -> bool {
    let _ = name;
    todo!("U3: port engine.mjs isName (NAME)")
}

/// `objects/<name>.json`: the file a problem names.
pub fn object_file(name: &str) -> String {
    crate::format::object_file(name)
}

/// `scenes/<name>.json`.
pub fn scene_file(name: &str) -> String {
    crate::format::scene_file(name)
}

/// Every problem in the sources, as "<file>: <where>: <rule>" lines. It
/// never stops at the first problem.
pub fn validate(sources: &Sources) -> Vec<String> {
    let _ = sources;
    todo!("U3: port engine.mjs validate")
}

/// A sprite's layer: a map (with an optional class), or a frame loop.
#[derive(Clone, Debug, PartialEq)]
pub enum SpriteLayer {
    Map { class: Option<String>, map: Vec<String> },
    Frames { loop_name: String, prefix: String, frames: Vec<Vec<String>> },
}

/// A block's side textures.
#[derive(Clone, Debug, PartialEq)]
pub struct Sides {
    pub keys: OMap<String, String>,
    pub left: Vec<String>,
    pub right: Vec<String>,
}

/// A resolved block: its size, faces, surface (as written) and sides.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub size: [i64; 3],
    pub faces: OMap<String, String>,
    pub surface: Option<Value>,
    pub sides: Option<Sides>,
}

/// An object with `extends` applied (`resolve`).
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    pub name: String,
    pub block: Option<Block>,
    pub character: bool,
    pub anchor: [i64; 2],
    pub footprint: [i64; 2],
    /// Key → color name, in the order the keys paint.
    pub keys: OMap<String, String>,
    pub layers: Vec<SpriteLayer>,
    pub width: i64,
    pub height: i64,
}

/// Applies `extends` (`resolve`). Errs on a missing object or a cycle.
pub fn resolve(sources: &Sources, name: &str) -> Result<Resolved> {
    let _ = (sources, name);
    todo!("U3: port engine.mjs resolve")
}

/// A layer's maps: one, or one per frame (`layerMaps`).
pub fn layer_maps(layer: &SpriteLayer) -> Vec<&Vec<String>> {
    match layer {
        SpriteLayer::Map { map, .. } => vec![map],
        SpriteLayer::Frames { frames, .. } => frames.iter().collect(),
    }
}

/// The keys some map uses, in key order (`usedKeys`).
pub fn used_keys(resolved: &Resolved) -> Vec<String> {
    let _ = resolved;
    todo!("U3: port engine.mjs usedKeys")
}

/// The keys a block's side maps use, in key order (`usedSideKeys`).
pub fn used_side_keys(block: &Block) -> Vec<String> {
    let _ = block;
    todo!("U3: port engine.mjs usedSideKeys")
}

/// The width and height of the pixels an object paints (`paintedSize`).
pub fn painted_size(resolved: &Resolved) -> [i64; 2] {
    let _ = resolved;
    todo!("U3: port engine.mjs paintedSize")
}

/// The distinct palette colors (by hex) an object paints (`usedColors`).
pub fn used_colors(sources: &Sources, resolved: &Resolved) -> Vec<String> {
    let _ = (sources, resolved);
    todo!("U3: port engine.mjs usedColors")
}

/// The one-line summary `check` prints for an object (`describeObject`).
pub fn describe_object(sources: &Sources, resolved: &Resolved) -> String {
    let _ = (sources, resolved);
    todo!("U3: port engine.mjs describeObject")
}

/// A pixel of a side face and the cell of its side map (`sideCells`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SideCell {
    pub p: Pixel,
    pub col: i32,
    pub row: i32,
}

/// Where each pixel of a side face falls in its side map (`sideCells`).
pub fn side_cells(face: &[Pixel]) -> Vec<SideCell> {
    let _ = face;
    todo!("U3: port engine.mjs sideCells")
}

// -------------------------------------------------------------------- trees

/// A color name, shared by every pixel painted with it.
pub type ColorName = Rc<str>;

/// Paint: pixels in the order first painted (later paint wins), and the
/// colors in the order first used.
#[derive(Clone, Debug, Default)]
pub struct Layer {
    pub pixels: OMap<Pixel, ColorName>,
    pub colors: OSet<ColorName>,
}

/// A `<g>`: its attributes, a frame loop's frame and count, whether it is
/// hidden at rest, and its children.
#[derive(Clone, Debug, Default)]
pub struct Group {
    pub attrs: Vec<(String, String)>,
    pub frame: Option<usize>,
    pub frames: Option<usize>,
    pub hidden: bool,
    pub children: Vec<Child>,
}

/// A group's child: paint, or a group.
#[derive(Clone, Debug)]
pub enum Child {
    Layer(Layer),
    Group(Group),
}

/// A rendered scene: its viewBox, its tree, and its motion's CSS.
#[derive(Clone, Debug)]
pub struct Rendered {
    pub view_box: Vec<Value>,
    pub root: Group,
    pub style: Option<String>,
}

/// The order a scene's items paint in: depth order, or as the file lists
/// them (what `scene.sort` compares against).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Order {
    #[default]
    Depth,
    List,
}

/// Renders a scene to a tree that mirrors its groups, placement classes and
/// frame loops (`renderScene`).
pub fn render_scene(sources: &Sources, name: &str, order: Order) -> Result<Rendered> {
    let _ = (sources, name, order);
    todo!("U3/U4/U5: port engine.mjs renderScene")
}

/// Every item that moves among others into several places of the depth
/// order, with how many copies the SVG draws of it (`motionCopies`).
pub fn motion_copies(sources: &Sources, name: &str) -> Result<Vec<(String, usize)>> {
    let _ = (sources, name);
    todo!("U5: port engine.mjs motionCopies")
}

/// A scene as it stands at `time` ms, still (`sceneAt`).
pub fn scene_at(sources: &Sources, name: &str, time: f64) -> Result<Value> {
    let _ = (sources, name, time);
    todo!("U5: port engine.mjs sceneAt")
}

/// The order one list of items draws in, as indices into it (`depthOrder`).
pub fn depth_order(sources: &Sources, items: &[Value], origin: [f64; 2], tracks: &OMap<String, Value>) -> Vec<usize> {
    let _ = (sources, items, origin, tracks);
    todo!("U4: port engine.mjs depthOrder")
}

/// A copy of the scene with every list of items in depth order
/// (`sceneInOrder`).
pub fn scene_in_order(sources: &Sources, scene: &Value) -> Value {
    let _ = (sources, scene);
    todo!("U4: port engine.mjs sceneInOrder")
}

/// Whether every list of the scene's items is in depth order already
/// (`isInDepthOrder`).
pub fn is_in_depth_order(sources: &Sources, scene: &Value) -> bool {
    let _ = (sources, scene);
    todo!("U4: port engine.mjs isInDepthOrder")
}

/// Renders one object on its own, with its anchor at (0, 0)
/// (`renderObject`).
pub fn render_object(sources: &Sources, name: &str) -> Result<Group> {
    let _ = (sources, name);
    todo!("U3: port engine.mjs renderObject")
}

/// What `composite` shows: only the first of each run of variant groups,
/// each loop on `frame`, or on the frame it shows at `time`.
#[derive(Clone, Copy, Debug, Default)]
pub struct CompositeOptions {
    pub first_variant: bool,
    pub frame: usize,
    pub time: Option<f64>,
}

/// Flattens a tree to what shows (`composite`): pixel → color name, later
/// paint winning.
pub fn composite(root: &Group, options: CompositeOptions) -> OMap<Pixel, ColorName> {
    let _ = (root, options);
    todo!("U3: port engine.mjs composite")
}

/// The bounding box of a set of pixels, or None if it's empty (`bounds`).
pub fn bounds<'a>(pixels: impl IntoIterator<Item = &'a Pixel>) -> Option<Bounds> {
    let _ = pixels.into_iter();
    todo!("U3: port engine.mjs bounds")
}
