//! Scene motion (ports cli/lib/engine/motion.mjs; island level up spec): a
//! scene's `motion` is one loop of `duration` ms, with a track of keys per
//! item it moves. Moves step a pixel at a time between keys. The engine
//! compiles the tracks into CSS inside the scene's SVG; `state_at` plays
//! them. Time 0 is the rest frame.

use serde_json::Value;

use serde_json::Map;

use crate::depth::Copy;
use crate::engine::{js_string, truthy, Sources};
use crate::jsnum;
use crate::omap::OMap;
use crate::serialize::keys;

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
pub const KEY_PROPS: [&str; 5] = ["t", "move", "show", "pose", "state"];

/// The frame a loop of `count` frames shows at `time` ms (`loopFrame`).
pub fn loop_frame(loop_name: &str, count: usize, time: f64) -> usize {
    let cycle = LOOP_CYCLES.iter().find(|(n, _)| *n == loop_name).map_or(200.0 * count as f64, |(_, c)| *c);
    let n = count as f64;
    let frame = ((((time % cycle) + cycle) % cycle) / cycle * n).floor() % n;
    if frame.is_nan() {
        0
    } else {
        frame as usize
    }
}

fn q(v: Option<&Value>) -> String {
    format!("'{}'", js_string(v))
}

fn is_int(v: Option<&Value>) -> bool {
    v.and_then(Value::as_f64).is_some_and(jsnum::is_integer)
}

/// Every object item with an id, by id, searching groups too (`itemsById`).
pub fn items_by_id(items: &Value) -> OMap<String, Value> {
    let mut out = OMap::new();
    fn walk(items: &Value, out: &mut OMap<String, Value>) {
        let Some(list) = items.as_array() else { return };
        for item in list {
            let Some(o) = item.as_object() else { continue };
            if o.contains_key("group") {
                walk(o.get("items").unwrap_or(&Value::Null), out);
            } else if let Some(id) = o.get("id").and_then(Value::as_str) {
                out.set(id.to_string(), item.clone());
            }
        }
    }
    walk(items, &mut out);
    out
}

/// Checks a scene's `motion` against its items and the objects
/// (`validateMotion`).
pub fn validate_motion(sources: &Sources, scene: &Value, add: &mut dyn FnMut(String)) {
    let Some(motion) = scene.get("motion").and_then(Value::as_object) else { return add("motion must be { duration, tracks }".into()) };
    for key in keys(motion) {
        if !MOTION_PROPS.contains(&key.as_str()) {
            add(format!("motion: unknown property '{key}'"));
        }
    }
    let duration = motion.get("duration");
    let whole_duration = is_int(duration);
    if !whole_duration || duration.and_then(Value::as_f64).unwrap_or(0.0) < 100.0 {
        add("motion: duration must be a whole number of ms, at least 100".into());
    }
    let Some(tracks) = motion.get("tracks").and_then(Value::as_array) else { return add("motion: tracks must be a list".into()) };
    let ids = items_by_id(scene.get("items").unwrap_or(&Value::Null));
    let mut seen: Vec<String> = Vec::new();
    for (i, track) in tracks.iter().enumerate() {
        let place = format!("motion.tracks[{i}]");
        let Some(t) = track.as_object() else {
            add(format!("{place}: must be {{ item, keys }}"));
            continue;
        };
        for key in keys(t) {
            if !TRACK_PROPS.contains(&key.as_str()) {
                add(format!("{place}: unknown property '{key}'"));
            }
        }
        let item = t.get("item");
        match item.and_then(Value::as_str) {
            Some(id) if ids.has(id) => {
                if seen.iter().any(|s| s == id) {
                    add(format!("{place}: item {} already has a track", q(item)));
                }
                seen.push(id.to_string());
            }
            _ => {
                add(format!("{place}: item {} is no item's id", q(item)));
                seen.push(js_string(item));
            }
        }
        let keys_list = match t.get("keys") {
            Some(Value::Array(list)) if !list.is_empty() => list,
            _ => {
                add(format!("{place}: keys must be a list of at least one key"));
                continue;
            }
        };
        let mut last = -1.0;
        // The object the item shows at each key (its pose, once a key poses it), for the keys' states.
        let object = ids.get(t.get("item").and_then(Value::as_str).unwrap_or("")).map(|item| js_string(item.get("object")));
        let shown = object.as_deref().map(|o| filled_keys(track, o));
        for (k, key) in keys_list.iter().enumerate() {
            let at = format!("{place}.keys[{k}]");
            let Some(kv) = key.as_object() else {
                add(format!("{at}: must be {{ t, move?, show?, pose?, state? }}"));
                continue;
            };
            for prop in keys(kv) {
                if !KEY_PROPS.contains(&prop.as_str()) {
                    add(format!("{at}: unknown property '{prop}'"));
                }
            }
            let t_value = kv.get("t");
            let tf = t_value.and_then(Value::as_f64).unwrap_or(f64::NAN);
            if !is_int(t_value) {
                add(format!("{at}: t must be a whole number of ms"));
            } else if k == 0 && tf != 0.0 {
                add(format!("{at}: the first key must be at t 0, the rest frame"));
            } else if tf <= last {
                add(format!("{at}: keys must be in time order, each later than the last"));
            } else if whole_duration && tf >= duration.and_then(Value::as_f64).unwrap_or(0.0) {
                add(format!("{at}: t must be less than the duration"));
            }
            if is_int(t_value) {
                last = tf;
            }
            if kv.contains_key("move") {
                let ok = matches!(kv.get("move"), Some(Value::Array(m)) if m.len() == 2 && m.iter().all(|v| is_int(Some(v))));
                if !ok {
                    add(format!("{at}: move must be two whole numbers"));
                }
            }
            if kv.contains_key("show") && !kv["show"].is_boolean() {
                add(format!("{at}: show must be true or false"));
            }
            if kv.contains_key("pose") && !kv["pose"].as_str().is_some_and(|p| sources.objects.contains_key(p)) {
                add(format!("{at}: pose {} does not exist", q(kv.get("pose"))));
            }
            if let Some(state) = kv.get("state") {
                // A state of the object shown at this key, which is the pose if the key or one before it poses.
                if let Some(pose) =
                    shown.as_ref().and_then(|s| s.get(k)).map(|f| f.pose.clone()).filter(|p| sources.objects.contains_key(p))
                {
                    if let Ok(r) = crate::engine::resolve(sources, &pose) {
                        if let Some(problem) = crate::states::state_problem(&pose, r.states.as_deref(), state) {
                            add(format!("{at}: {problem}"));
                        }
                    }
                }
            }
        }
    }
}

/// A key with its inherited values filled in (`filledKeys`).
#[derive(Clone, Debug, PartialEq)]
pub struct Filled {
    pub t: f64,
    pub moved: [f64; 2],
    pub show: bool,
    pub pose: String,
}

fn nullish(v: Option<&Value>) -> Option<&Value> {
    v.filter(|v| !v.is_null())
}

/// Each key of a track with its inherited values filled in (`filledKeys`):
/// move [0, 0], show true and the item's own object until a key says
/// otherwise.
pub fn filled_keys(track: &Value, object: &str) -> Vec<Filled> {
    let mut moved = [0.0, 0.0];
    let mut show = true;
    let mut pose = object.to_string();
    let Some(list) = track.get("keys").and_then(Value::as_array) else { return vec![] };
    list.iter()
        .map(|key| {
            if let Some(m) = key.get("move").filter(|m| truthy(Some(m))) {
                moved = [m.get(0).and_then(Value::as_f64).unwrap_or(f64::NAN), m.get(1).and_then(Value::as_f64).unwrap_or(f64::NAN)];
            }
            if let Some(s) = nullish(key.get("show")) {
                show = truthy(Some(s));
            }
            if let Some(p) = nullish(key.get("pose")) {
                pose = js_string(Some(p));
            }
            Filled { t: key.get("t").and_then(Value::as_f64).unwrap_or(f64::NAN), moved, show, pose: pose.clone() }
        })
        .collect()
}

fn gcd(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// How many pixel steps a move of (dx, dy) takes (`stepsFor`): one per
/// pixel along a straight or 2:1 line; 0 for no move.
pub fn steps_for(dx: f64, dy: f64) -> f64 {
    let (ax, ay) = (dx.abs(), dy.abs());
    if ax == 0.0 || ay == 0.0 {
        return ax.max(ay);
    }
    gcd(ax, ay)
}

/// Where a tracked item is, whether it shows and what it draws.
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub moved: [f64; 2],
    pub show: bool,
    pub pose: String,
}

/// Where a tracked item is at `time` ms (`stateAt`): its keys in order, the
/// last one looping back to the first at `duration`.
pub fn state_at(track: &Value, object: &str, duration: f64, time: f64) -> State {
    let keys = filled_keys(track, object);
    let t = ((time % duration) + duration) % duration;
    let mut i = keys.len() - 1;
    while i > 0 && keys[i].t > t {
        i -= 1;
    }
    let from = &keys[i];
    let wrap = Filled { t: duration, ..keys[0].clone() };
    let to = keys.get(i + 1).unwrap_or(&wrap);
    let (dx, dy) = (to.moved[0] - from.moved[0], to.moved[1] - from.moved[1]);
    let n = steps_for(dx, dy);
    let done = if n == 0.0 { 0.0 } else { ((t - from.t) / (to.t - from.t) * n).floor() };
    let step = if n == 0.0 { 1.0 } else { n };
    State { moved: [from.moved[0] + (dx / step) * done, from.moved[1] + (dy / step) * done], show: from.show, pose: from.pose.clone() }
}

/// The key a track is found by, as the engine's `Map`s tell ids apart.
fn id_key(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(format!("#n:{}", jsnum::number_string(n))),
        Value::Bool(b) => Some(format!("#b:{b}")),
        Value::Null => Some("#null".into()),
        _ => None,
    }
}

/// The scene's items as they stand at `time` (`poseItems`): each tracked
/// item placed by px at its moved point, drawing its pose, and marked
/// `hidden` while it doesn't show.
pub fn pose_items(items: &[Value], motion: Option<&Value>, time: f64, point_of: &dyn Fn(&Value) -> [f64; 2]) -> Vec<Value> {
    let Some(tracks) = motion.and_then(|m| m.get("tracks")).and_then(Value::as_array).filter(|t| !t.is_empty()) else {
        return items.to_vec();
    };
    let duration = motion.and_then(|m| m.get("duration")).and_then(Value::as_f64).unwrap_or(f64::NAN);
    let mut by_item: OMap<String, &Value> = OMap::new();
    for track in tracks {
        if let Some(k) = id_key(track.get("item")) {
            by_item.set(k, track);
        }
    }
    fn walk(list: &[Value], by_item: &OMap<String, &Value>, duration: f64, time: f64, point_of: &dyn Fn(&Value) -> [f64; 2]) -> Vec<Value> {
        list.iter()
            .map(|item| {
                if item.as_object().is_some_and(|o| o.contains_key("group")) {
                    let inner = item.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
                    let mut out = item.clone();
                    out["items"] = Value::Array(walk(&inner, by_item, duration, time, point_of));
                    return out;
                }
                let Some(track) = id_key(item.get("id")).and_then(|k| by_item.get(&k).copied()) else { return item.clone() };
                let state = state_at(track, &js_string(item.get("object")), duration, time);
                let [x, y] = point_of(item);
                let level = match item.pointer("/at/tile").filter(|t| truthy(Some(t))) {
                    Some(tile) => tile.get(2).cloned().unwrap_or(Value::Null),
                    None => item.pointer("/at/level").cloned().unwrap_or(Value::from(0)),
                };
                let mut at = Map::new();
                at.insert("px".into(), Value::Array(vec![jsnum::num(x + state.moved[0]), jsnum::num(y + state.moved[1])]));
                if truthy(Some(&level)) {
                    at.insert("level".into(), level);
                }
                let mut out = item.as_object().cloned().unwrap_or_default();
                out.insert("object".into(), Value::String(state.pose));
                out.insert("at".into(), Value::Object(at));
                if !state.show {
                    out.insert("hidden".into(), Value::Bool(true));
                }
                Value::Object(out)
            })
            .collect()
    }
    walk(items, &by_item, duration, time, point_of)
}

/// The items with every hidden one taken out (`shownItems`).
pub fn shown_items(items: &[Value]) -> Vec<Value> {
    items
        .iter()
        .filter(|item| !truthy(item.get("hidden")))
        .map(|item| {
            if item.as_object().is_some_and(|o| o.contains_key("group")) {
                let inner = item.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
                let mut out = item.clone();
                out["items"] = Value::Array(shown_items(&inner));
                out
            } else {
                item.clone()
            }
        })
        .collect()
}

pub(crate) fn px(v: f64) -> String {
    if v == 0.0 {
        "0".into()
    } else {
        format!("{}px", jsnum::to_string(v))
    }
}

/// A key's place in the loop as a percentage, rounded to 3 decimals as the
/// CSS writes it (`pctOf`).
pub fn pct_of(t: f64, duration: f64) -> f64 {
    jsnum::fixed((t / duration) * 100.0, 3)
}

pub(crate) fn pct(t: f64, duration: f64) -> String {
    format!("{}%", jsnum::to_string(pct_of(t, duration)))
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
    let mut out = vec![object.to_string()];
    for key in track.get("keys").and_then(Value::as_array).into_iter().flatten() {
        if let Some(p) = key.get("pose").filter(|p| truthy(Some(p))) {
            let p = js_string(Some(p));
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// Whether a track moves its item (`isMoving`).
pub fn is_moving(track: &Value, object: &str) -> bool {
    let keys = filled_keys(track, object);
    keys.iter().any(|k| k.moved != keys[0].moved)
}

/// A move a track's item takes from a percentage of the loop on.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub at: f64,
    pub moved: [f64; 2],
}

/// Where a track's item is over the loop, as the CSS plays it (`moveRuns`).
pub fn move_runs(track: &Value, object: &str, duration: f64) -> Vec<Run> {
    let keys = filled_keys(track, object);
    let mut all = keys.clone();
    if let Some(first) = keys.first() {
        all.push(Filled { t: duration, ..first.clone() });
    }
    let mut runs: Vec<Run> = Vec::new();
    let start = |at: f64, moved: [f64; 2], runs: &mut Vec<Run>| {
        if runs.last().is_none_or(|last| last.moved != moved) {
            runs.push(Run { at, moved });
        }
    };
    for i in 0..all.len().saturating_sub(1) {
        let (key, next) = (&all[i], &all[i + 1]);
        let (from, to) = (pct_of(key.t, duration), pct_of(next.t, duration));
        if to <= from {
            continue;
        }
        let (dx, dy) = (next.moved[0] - key.moved[0], next.moved[1] - key.moved[1]);
        let n = steps_for(dx, dy);
        if n == 0.0 {
            start(from, key.moved, &mut runs);
            continue;
        }
        let mut k = 0.0;
        while k < n {
            start(from + ((to - from) * k) / n, [key.moved[0] + (dx / n) * k, key.moved[1] + (dy / n) * k], &mut runs);
            k += 1.0;
        }
    }
    runs
}

/// The move a run list gives at percentage `p` (`moveAtPercent`).
pub fn move_at_percent(runs: &[Run], p: f64) -> [f64; 2] {
    let mut found = &runs[0];
    for run in runs {
        if run.at > p {
            break;
        }
        found = run;
    }
    found.moved
}

/// The opacity stops that show a copy in its windows (`copyStops`).
fn copy_stops(windows: &[(f64, f64)]) -> Vec<(f64, bool)> {
    let round = |v: f64| jsnum::fixed(v, 4);
    let mut stops = vec![(0.0, windows[0].0 <= 0.0)];
    let stop = |at: f64, on: bool, stops: &mut Vec<(f64, bool)>| {
        let last = stops.len() - 1;
        if stops[last].0 == at {
            stops[last].1 = on;
        } else if stops[last].1 != on {
            stops.push((at, on));
        }
        let n = stops.len();
        if n > 1 && stops[n - 1].1 == stops[n - 2].1 {
            stops.pop();
        }
    };
    for &(from, to) in windows {
        if round(from) > 0.0 {
            stop(round(from), true, &mut stops);
        }
        if round(to) < 100.0 {
            stop(round(to), false, &mut stops);
        }
    }
    stops
}

/// The CSS that plays a scene's motion (`motionCss`); `copies` holds, by
/// item id in the order they were found, the copies of each item drawn in
/// several places.
pub fn motion_css(motion: &Value, ids: &OMap<String, Value>, copies: &OMap<String, Vec<Copy>>) -> String {
    let rules = motion_rules(motion, ids, copies, None);
    if rules.is_empty() {
        return String::new();
    }
    rules.concat() + "@media (prefers-reduced-motion:reduce){[class^=\"mv-\"],[class*=\" mv-\"]{animation:none}}"
}

/// A frame loop a scene plays: its class, its frames' class prefix, and how
/// many frames its art has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameLoop {
    pub name: String,
    pub prefix: String,
    pub frames: usize,
}

/// The rules that play a frame loop in a compiled SVG (spec R11, D7), as
/// pixel-art.css plays the site's loops: its cycle (`LOOP_CYCLES`, else 200
/// ms a frame) over its frame count (`SITE_LOOPS`, else the art's), frame
/// `k` shown from `k / n` of the cycle.
fn frame_loop_rules(scope: &str, l: &FrameLoop) -> Vec<String> {
    let n = SITE_LOOPS.iter().find(|s| s.name == l.name).map_or(l.frames, |s| s.frames).max(1);
    let cycle = LOOP_CYCLES.iter().find(|(name, _)| *name == l.name).map_or(200.0 * n as f64, |(_, c)| *c);
    let at = format!("[data-pixellab=\"{scope}\"] .{}", l.name);
    let mut rules = vec![
        format!("@keyframes {scope}--{}{{0%{{opacity:1}}{}{{opacity:0}}100%{{opacity:0}}}}", l.name, pct(cycle / n as f64, cycle)),
        format!("{at}{{opacity:0;animation:{scope}--{} {}ms step-end infinite}}", l.name, jsnum::to_string(cycle)),
    ];
    for k in 1..l.frames {
        let delay = cycle * (n as f64 - k as f64) / n as f64;
        rules.push(format!("{at}.{}{k}{{animation-delay:-{}ms}}", l.prefix, jsnum::to_string(delay)));
    }
    rules
}

/// A compiled SVG's whole `<style>` (spec D7): the scene's motion with its
/// names scoped to `scope`, its elements' ambient loops and actions, its
/// frame loops, and one reduced-motion rule that stops them all and shows
/// each loop's first frame. Empty when the scene plays nothing.
pub fn scene_css(
    scope: &str,
    motion: Option<(&Value, &OMap<String, Value>, &OMap<String, Vec<Copy>>)>,
    loops: &[FrameLoop],
    elements: &[std::rc::Rc<crate::ambient::Moves>],
    stated: &[std::rc::Rc<crate::states::ItemStates>],
) -> String {
    let mut rules = motion.map(|(m, ids, copies)| motion_rules(m, ids, copies, Some(scope))).unwrap_or_default();
    rules.extend(crate::ambient::elements_css(scope, elements));
    let states = crate::states::css(scope, stated);
    rules.extend(states.rules);
    let generated = !rules.is_empty();
    for l in loops {
        rules.extend(frame_loop_rules(scope, l));
    }
    if rules.is_empty() {
        return String::new();
    }
    let at = format!("[data-pixellab=\"{scope}\"]");
    let mut stop: Vec<String> = Vec::new();
    if generated {
        stop.push(format!("{at} [class*=\"{scope}--\"]"));
    }
    stop.extend(loops.iter().map(|l| format!("{at} .{}", l.name)));
    let mut reduced = format!("{}{{animation:none}}", stop.join(","));
    // Each stated item shows the frame its state settles in (R14).
    reduced.push_str(&states.reduced.concat());
    if !loops.is_empty() {
        let first: Vec<String> = loops.iter().map(|l| format!("{at} .{}.{}0", l.name, l.prefix)).collect();
        reduced.push_str(&format!("{}{{opacity:1}}", first.join(",")));
    }
    rules.push(format!("@media (prefers-reduced-motion:reduce){{{reduced}}}"));
    rules.concat()
}

/// The motion rules, unscoped as the Node CLI wrote them, or with every
/// generated name `<scope>--…` and every selector under `[data-pixellab]`.
fn motion_rules(motion: &Value, ids: &OMap<String, Value>, copies: &OMap<String, Vec<Copy>>, scope: Option<&str>) -> Vec<String> {
    let named = |name: &str| match scope {
        Some(s) => format!("{s}--{name}"),
        None => name.to_string(),
    };
    let sel = |name: &str| match scope {
        Some(s) => format!("[data-pixellab=\"{s}\"] .{s}--{name}"),
        None => format!(".{name}"),
    };
    let duration = motion.get("duration").and_then(Value::as_f64).unwrap_or(f64::NAN);
    let ms = jsnum::to_string(duration);
    let run = |name: &str, timing: &str| format!("animation:{} {ms}ms {timing} infinite", named(name));
    let mut rules: Vec<String> = Vec::new();
    for track in motion.get("tracks").and_then(Value::as_array).into_iter().flatten() {
        let id = js_string(track.get("item"));
        let object = ids.get(&id).map(|item| js_string(item.get("object"))).unwrap_or_else(|| "undefined".into());
        let keys = filled_keys(track, &object);
        if keys.is_empty() {
            continue;
        }
        let cls = motion_class(&id);
        let [rx, ry] = keys[0].moved;
        let moves = keys.iter().any(|k| k.moved != [rx, ry]);
        let shows = keys.iter().any(|k| !k.show);
        let mut animations = Vec::new();
        if moves {
            let mut all = keys.clone();
            all.push(Filled { t: duration, ..keys[0].clone() });
            let frames: Vec<String> = (0..all.len())
                .map(|i| {
                    let key = &all[i];
                    let n = all.get(i + 1).map_or(0.0, |next| steps_for(next.moved[0] - key.moved[0], next.moved[1] - key.moved[1]));
                    let timing =
                        if n != 0.0 { format!(";animation-timing-function:steps({},end)", jsnum::to_string(n)) } else { String::new() };
                    format!("{}{{transform:translate({},{}){timing}}}", pct(key.t, duration), px(key.moved[0] - rx), px(key.moved[1] - ry))
                })
                .collect();
            rules.push(format!("@keyframes {}{{{}}}", named(&cls), frames.concat()));
            animations.push(format!("{} {ms}ms step-end infinite", named(&cls)));
        }
        if shows {
            let frames: Vec<String> =
                keys.iter().map(|k| format!("{}{{opacity:{}}}", pct(k.t, duration), if k.show { 1 } else { 0 })).collect();
            rules.push(format!("@keyframes {}-s{{{}}}", named(&cls), frames.concat()));
            animations.push(format!("{}-s {ms}ms step-end infinite", named(&cls)));
        }
        let base = if keys[0].show { "" } else { "opacity:0;" };
        if !animations.is_empty() || !base.is_empty() {
            let anim = if animations.is_empty() { String::new() } else { format!("animation:{}", animations.join(",")) };
            rules.push(format!("{}{{{base}{anim}}}", sel(&cls)));
        }
        let poses = poses_of(track, &object);
        if poses.len() > 1 {
            for (k, pose) in poses.iter().enumerate() {
                let name = pose_class(&id, k);
                let frames: Vec<String> = keys
                    .iter()
                    .map(|key| format!("{}{{opacity:{}}}", pct(key.t, duration), if key.pose == *pose { 1 } else { 0 }))
                    .collect();
                rules.push(format!("@keyframes {}{{{}}}", named(&name), frames.concat()));
                rules.push(format!(
                    "{}{{{}{}}}",
                    sel(&name),
                    if keys[0].pose == *pose { "" } else { "opacity:0;" },
                    run(&name, "step-end")
                ));
            }
        }
        if let Some(list) = copies.get(&id) {
            for (k, copy) in list.iter().enumerate() {
                let name = copy_class(&id, k);
                let frames: Vec<String> = copy_stops(&copy.windows)
                    .iter()
                    .map(|(at, on)| format!("{}%{{opacity:{}}}", jsnum::to_string(*at), if *on { 1 } else { 0 }))
                    .collect();
                rules.push(format!("@keyframes {}{{{}}}", named(&name), frames.concat()));
                rules.push(format!("{}{{{}{}}}", sel(&name), if k == 0 { "" } else { "opacity:0;" }, run(&name, "step-end")));
            }
        }
    }
    rules
}
