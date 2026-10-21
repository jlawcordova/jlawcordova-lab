//! Scene motion (ports cli/lib/engine/motion.mjs; island level up spec): a
//! scene's `motion` is one loop of `duration` ms, with a track of keys per
//! item it moves. Moves step a pixel at a time between keys. The engine
//! compiles the tracks into CSS inside the scene's SVG; `state_at` plays
//! them. Time 0 is the rest frame.

use serde_json::Value;

use crate::depth::Copy;
use crate::engine::Sources;
use crate::omap::OMap;

/// Each frame loop's full cycle in ms, as pixel-art.css runs it; other loops
/// step every 200 ms (`LOOP_CYCLES`).
pub const LOOP_CYCLES: [(&str, f64); 3] = [("wf", 1000.0), ("ff", 600.0), ("hf", 3000.0)];

/// A frame loop pixel-art.css plays: its frames' class prefix and how many
/// it shows (`SITE_LOOPS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SiteLoop {
    pub name: &'static str,
    pub prefix: &'static str,
    pub frames: usize,
}

pub const SITE_LOOPS: [SiteLoop; 3] = [
    SiteLoop { name: "ff", prefix: "f", frames: 4 },
    SiteLoop { name: "wf", prefix: "w", frames: 5 },
    SiteLoop { name: "hf", prefix: "h", frames: 6 },
];

pub const MOTION_PROPS: [&str; 2] = ["duration", "tracks"];
pub const TRACK_PROPS: [&str; 2] = ["item", "keys"];
pub const KEY_PROPS: [&str; 4] = ["t", "move", "show", "pose"];

/// The frame a loop of `count` frames shows at `time` ms (`loopFrame`).
pub fn loop_frame(loop_name: &str, count: usize, time: f64) -> usize {
    let _ = (loop_name, count, time);
    todo!("U5: port motion.mjs loopFrame")
}

/// Every object item with an id, by id, searching groups too (`itemsById`).
pub fn items_by_id(items: &Value) -> OMap<String, Value> {
    let _ = items;
    todo!("U5: port motion.mjs itemsById")
}

/// Checks a scene's `motion` against its items and the objects
/// (`validateMotion`).
pub fn validate_motion(sources: &Sources, scene: &Value, add: &mut dyn FnMut(String)) {
    let _ = (sources, scene, add);
    todo!("U5: port motion.mjs validateMotion")
}

/// A key with its inherited values filled in (`filledKeys`).
#[derive(Clone, Debug, PartialEq)]
pub struct Filled {
    pub t: f64,
    pub moved: [f64; 2],
    pub show: bool,
    pub pose: String,
}

/// Each key of a track with its inherited values filled in (`filledKeys`).
pub fn filled_keys(track: &Value, object: &str) -> Vec<Filled> {
    let _ = (track, object);
    todo!("U5: port motion.mjs filledKeys")
}

/// How many pixel steps a move of (dx, dy) takes (`stepsFor`).
pub fn steps_for(dx: f64, dy: f64) -> f64 {
    let _ = (dx, dy);
    todo!("U5: port motion.mjs stepsFor")
}

/// Where a tracked item is, whether it shows and what it draws.
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub moved: [f64; 2],
    pub show: bool,
    pub pose: String,
}

/// Where a tracked item is at `time` ms (`stateAt`).
pub fn state_at(track: &Value, object: &str, duration: f64, time: f64) -> State {
    let _ = (track, object, duration, time);
    todo!("U5: port motion.mjs stateAt")
}

/// The scene's items as they stand at `time` (`poseItems`).
pub fn pose_items(items: &[Value], motion: Option<&Value>, time: f64, point_of: &dyn Fn(&Value) -> [f64; 2]) -> Vec<Value> {
    let _ = (items, motion, time, point_of);
    todo!("U5: port motion.mjs poseItems")
}

/// The items with every hidden one taken out (`shownItems`).
pub fn shown_items(items: &[Value]) -> Vec<Value> {
    let _ = items;
    todo!("U5: port motion.mjs shownItems")
}

/// A key's place in the loop as a percentage, rounded to 3 decimals as the
/// CSS writes it (`pctOf`, with `jsnum::to_fixed`).
pub fn pct_of(t: f64, duration: f64) -> f64 {
    let _ = (t, duration);
    todo!("U5: port motion.mjs pctOf")
}

/// The CSS class of a tracked item's group (`motionClass`).
pub fn motion_class(id: &str) -> String {
    format!("mv-{id}")
}

/// The class of a tracked item's pose `k` (`poseClass`).
pub fn pose_class(id: &str, k: usize) -> String {
    format!("mv-{id}-{k}")
}

/// The class of copy `k` of an item drawn in several places (`copyClass`).
pub fn copy_class(id: &str, k: usize) -> String {
    format!("mv-{id}-z{k}")
}

/// The poses a track uses, the item's own object first (`posesOf`).
pub fn poses_of(track: &Value, object: &str) -> Vec<String> {
    let _ = (track, object);
    todo!("U5: port motion.mjs posesOf")
}

/// Whether a track moves its item (`isMoving`).
pub fn is_moving(track: &Value, object: &str) -> bool {
    let _ = (track, object);
    todo!("U5: port motion.mjs isMoving")
}

/// A move a track's item takes from a percentage of the loop on.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub at: f64,
    pub moved: [f64; 2],
}

/// Where a track's item is over the loop, as the CSS plays it (`moveRuns`).
pub fn move_runs(track: &Value, object: &str, duration: f64) -> Vec<Run> {
    let _ = (track, object, duration);
    todo!("U5: port motion.mjs moveRuns")
}

/// The move a run list gives at percentage `p` (`moveAtPercent`).
pub fn move_at_percent(runs: &[Run], p: f64) -> [f64; 2] {
    let _ = (runs, p);
    todo!("U5: port motion.mjs moveAtPercent")
}

/// The CSS that plays a scene's motion (`motionCss`); `copies` holds, by
/// item id in the order they were found, the copies of each item drawn in
/// several places.
pub fn motion_css(motion: &Value, ids: &OMap<String, Value>, copies: &OMap<String, Vec<Copy>>) -> String {
    let _ = (motion, ids, copies);
    todo!("U5: port motion.mjs motionCss")
}
