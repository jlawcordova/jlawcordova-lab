//! Scene motion edits: tracks, keys, the duration and paths on the grid (ports cli/lib/motion-ops.mjs).
//!
//! A key is `{ t, move?, show?, pose?, state? }`: move is pixels from where the scene
//! places the item, show, pose and state hold from their key on (a state is entered from its first frame). Between two keys an
//! item steps `steps_for(dx, dy)` times; it's on the grid when each step is 1 px
//! straight, 1:1 or 2:1, so the art never lands between pixels of its lines.
//!
//! The Node functions threw `SceneOpError`s; here they are `CommandError`s with
//! the same code (exit code 1, as `SceneOpError` has).

use engine::jsnum::{self, num};
use engine::motion;
use serde_json::{json, Map, Value};

use crate::errors::CommandError;

/// Each step of an on-grid move, in absolute pixels: straight, 1:1 or 2:1.
pub const ON_GRID: [[f64; 2]; 4] = [[1.0, 0.0], [0.0, 1.0], [1.0, 1.0], [2.0, 1.0]];

const KEY_ORDER: [&str; 5] = ["t", "move", "show", "pose", "state"];

fn err(message: impl Into<String>, code: &str) -> CommandError {
    CommandError::new(message, code)
}

fn s(x: f64) -> String {
    jsnum::to_string(x)
}

fn ms(t: f64) -> String {
    format!("{} ms", s(t))
}

fn tnum(key: &Value) -> f64 {
    key.get("t").and_then(Value::as_f64).unwrap_or(f64::NAN)
}

/// Rebuilds a key's props in the canonical order: t, move, show, pose, state.
fn canonical_key(key: &Value) -> Value {
    let mut out = Map::new();
    for prop in KEY_ORDER {
        if let Some(v) = key.get(prop) {
            out.insert(prop.to_string(), v.clone());
        }
    }
    Value::Object(out)
}

/// Whether a move of (dx, dy) between two keys steps on the grid.
pub fn on_grid(dx: f64, dy: f64) -> bool {
    let n = motion::steps_for(dx, dy);
    if n == 0.0 {
        return true;
    }
    let (ux, uy) = (dx.abs() / n, dy.abs() / n);
    ON_GRID.iter().any(|[a, b]| *a == ux && *b == uy)
}

/// Every key-to-key move of a scene's motion that isn't on the grid, the loop
/// back from the last key to the first included. A move made while the item is
/// hidden (a reset to its start) isn't seen, so it isn't listed. Each entry is
/// `{ track, item, from, to, move: [dx, dy], steps, step: [x, y] }`.
pub fn off_grid_moves(scene: &Value) -> Vec<Value> {
    let Some(m) = scene.get("motion").filter(|m| m.is_object()) else { return vec![] };
    let Some(tracks) = m.get("tracks").and_then(Value::as_array) else { return vec![] };
    let Some(duration) = m.get("duration").and_then(Value::as_f64).filter(|d| jsnum::is_integer(*d)) else { return vec![] };
    let ids = motion::items_by_id(scene.get("items").unwrap_or(&Value::Null));
    let mut out = vec![];
    for (ti, track) in tracks.iter().enumerate() {
        let Some(item_id) = track.get("item").and_then(Value::as_str) else { continue };
        let Some(item) = ids.get(item_id) else { continue };
        let Some(raw) = track.get("keys").and_then(Value::as_array) else { continue };
        if raw.is_empty() {
            continue;
        }
        let object = item.get("object").and_then(Value::as_str).unwrap_or("");
        let keys = motion::filled_keys(track, object);
        for (i, from) in keys.iter().enumerate() {
            let (to_t, to_move) = match keys.get(i + 1) {
                Some(k) => (k.t, k.moved),
                None => (duration, keys[0].moved),
            };
            let (dx, dy) = (to_move[0] - from.moved[0], to_move[1] - from.moved[1]);
            if !from.show || on_grid(dx, dy) {
                continue;
            }
            let steps = motion::steps_for(dx, dy);
            out.push(json!({
                "track": ti,
                "item": item_id,
                "from": num(from.t),
                "to": num(to_t),
                "move": [num(dx), num(dy)],
                "steps": num(steps),
                "step": [num(dx / steps), num(dy / steps)],
            }));
        }
    }
    out
}

/// The warning line for an off-grid move (an entry of [`off_grid_moves`]), as check lists it.
pub fn off_grid_text(file: &str, w: &Value) -> String {
    let n = |k: &str| w.get(k).and_then(Value::as_f64).unwrap_or(f64::NAN);
    let pair = |k: &str| {
        let a = w.get(k).cloned().unwrap_or(Value::Null);
        let at = |i: usize| s(a.get(i).and_then(Value::as_f64).unwrap_or(f64::NAN));
        format!("{}, {}", at(0), at(1))
    };
    let steps = n("steps");
    let said = if steps == 1.0 { "1 step".to_string() } else { format!("{} steps", s(steps)) };
    format!(
        "{}: motion.tracks[{}] ({}): {} → {} ms moves ({}) in {} of ({}); on the grid each step is 1 px straight, 1:1 or 2:1 (scene_motion_path makes such keys)",
        file,
        s(n("track")),
        w.get("item").and_then(Value::as_str).unwrap_or(""),
        s(n("from")),
        s(n("to")),
        pair("move"),
        said,
        pair("step"),
    )
}

fn track_index(doc: &Value, id: &str) -> Option<usize> {
    doc.get("motion")?.get("tracks")?.as_array()?.iter().position(|t| t.get("item").and_then(Value::as_str) == Some(id))
}

/// The track of an item id, or `None`.
pub fn track_of<'a>(doc: &'a Value, id: &str) -> Option<&'a Value> {
    let i = track_index(doc, id)?;
    Some(&doc["motion"]["tracks"][i])
}

/// The track of an item id, or a `no_track` error.
pub fn need_track<'a>(doc: &'a mut Value, id: &str) -> Result<&'a mut Value, CommandError> {
    match track_index(doc, id) {
        Some(i) => Ok(&mut doc["motion"]["tracks"][i]),
        None => Err(err(format!("{id} has no motion track (scene.track.add makes one)"), "no_track")),
    }
}

fn keys_mut(track: &mut Value) -> &mut Vec<Value> {
    if !track.get("keys").is_some_and(Value::is_array) {
        track["keys"] = json!([]);
    }
    track["keys"].as_array_mut().expect("keys is an array")
}

fn duration_of(doc: &Value) -> f64 {
    doc.get("motion").and_then(|m| m.get("duration")).and_then(Value::as_f64).unwrap_or(f64::NAN)
}

fn has_motion(doc: &Value) -> bool {
    doc.get("motion").is_some_and(|m| !m.is_null())
}

/// Refuses a time outside the loop.
fn check_time(doc: &Value, t: f64) -> Result<(), CommandError> {
    if !jsnum::is_integer(t) || t < 0.0 {
        return Err(err("a key's time is a whole number of ms from 0", "usage"));
    }
    let d = duration_of(doc);
    if t >= d {
        return Err(err(format!("{} is past the loop, which is {} long (0 to {})", ms(t), ms(d), s(d - 1.0)), "out_of_range"));
    }
    Ok(())
}

fn key_times(track: &Value) -> String {
    track.get("keys").and_then(Value::as_array).map(|k| k.iter().map(|k| s(tnum(k))).collect::<Vec<_>>().join(", ")).unwrap_or_default()
}

/// Adds a track for an item id: one key at 0, the rest frame. Makes the
/// scene's motion with `duration` when it has none. Returns `{ track, message }`.
pub fn add_track(doc: &mut Value, id: &str, duration: Option<f64>) -> Result<Value, CommandError> {
    if !has_motion(doc) {
        let Some(d) = duration else {
            return Err(err("the scene has no motion yet; give its loop length with --duration (ms)", "no_motion"));
        };
        doc["motion"] = json!({ "duration": num(d), "tracks": [] });
    } else if let Some(d) = duration {
        let current = duration_of(doc);
        if d != current {
            return Err(err(format!("the scene's motion is {} long; scene.motion.set changes that", ms(current)), "usage"));
        }
    }
    if track_of(doc, id).is_some() {
        return Err(err(format!("{id} already has a track"), "track_exists"));
    }
    let track = json!({ "item": id, "keys": [{ "t": 0 }] });
    if !doc["motion"].get("tracks").is_some_and(Value::is_array) {
        doc["motion"]["tracks"] = json!([]);
    }
    doc["motion"]["tracks"].as_array_mut().expect("tracks is an array").push(track.clone());
    Ok(json!({ "track": track, "message": format!("Added a track for {id}") }))
}

/// Removes an item's track. Returns `{ message }`.
pub fn remove_track(doc: &mut Value, id: &str) -> Result<Value, CommandError> {
    need_track(doc, id)?;
    if let Some(tracks) = doc["motion"]["tracks"].as_array_mut() {
        tracks.retain(|t| t.get("item").and_then(Value::as_str) != Some(id));
    }
    Ok(json!({ "message": format!("Removed {id}'s track") }))
}

/// The values `set_key` writes.
#[derive(Clone, Debug, Default)]
pub struct KeyValues {
    pub move_: Option<[f64; 2]>,
    pub show: Option<bool>,
    pub pose: Option<String>,
    pub state: Option<String>,
    pub clear: Vec<String>,
}

/// Sets a key's values, making the key (in time order) if there isn't one.
/// Returns `{ key, message }`.
pub fn set_key(doc: &mut Value, id: &str, t: f64, values: &KeyValues) -> Result<Value, CommandError> {
    need_track(doc, id)?;
    check_time(doc, t)?;
    let track = need_track(doc, id)?;
    let keys = keys_mut(track);
    let mut i = keys.iter().position(|k| tnum(k) == t);
    let made = i.is_none();
    let idx = match i.take() {
        Some(i) => i,
        None => {
            let at = keys.iter().filter(|k| tnum(k) < t).count();
            keys.insert(at, json!({ "t": num(t) }));
            at
        }
    };
    let mut key = keys[idx].clone();
    for prop in &values.clear {
        if !["move", "show", "pose", "state"].contains(&prop.as_str()) {
            return Err(err(format!("clear takes move, show, pose or state, not {prop}"), "usage"));
        }
        if let Some(o) = key.as_object_mut() {
            o.shift_remove(prop.as_str());
        }
    }
    if let Some(m) = values.move_ {
        key["move"] = json!([num(m[0]), num(m[1])]);
    }
    if let Some(sh) = values.show {
        key["show"] = Value::Bool(sh);
    }
    if let Some(p) = &values.pose {
        key["pose"] = Value::String(p.clone());
    }
    if let Some(st) = &values.state {
        key["state"] = Value::String(st.clone());
    }
    keys[idx] = canonical_key(&key);
    let mut said = vec![];
    if let Some(m) = values.move_ {
        said.push(format!("move {}, {}", s(m[0]), s(m[1])));
    }
    if let Some(sh) = values.show {
        said.push(format!("show {sh}"));
    }
    if let Some(p) = values.pose.as_deref().filter(|p| !p.is_empty()) {
        said.push(format!("pose {p}"));
    }
    if let Some(st) = &values.state {
        said.push(format!("state {st}"));
    }
    if !values.clear.is_empty() {
        said.push(format!("cleared {}", values.clear.join(", ")));
    }
    let tail = if said.is_empty() { String::new() } else { format!(": {}", said.join(", ")) };
    Ok(json!({
        "key": keys[idx].clone(),
        "message": format!("{} {id}'s key at {}{tail}", if made { "Made" } else { "Set" }, ms(t)),
    }))
}

/// Keys an item where it is at `t` (a key `{ t, move }` with the move the track gives it then), so its motion doesn't change until
/// the key is edited. A time that already has a key is left alone. Returns `{ key, pinned, message }`; `object` is the item's own object.
pub fn pin_key(doc: &mut Value, id: &str, object: &str, t: f64) -> Result<Value, CommandError> {
    need_track(doc, id)?;
    check_time(doc, t)?;
    let track = track_of(doc, id).expect("a track");
    if let Some(existing) = track.get("keys").and_then(Value::as_array).and_then(|k| k.iter().find(|k| tnum(k) == t)) {
        return Ok(json!({ "key": existing.clone(), "pinned": false, "message": format!("{id} already has a key at {}", ms(t)) }));
    }
    let moved = motion::state_at(track, object, duration_of(doc), t).moved;
    let r = set_key(doc, id, t, &KeyValues { move_: Some(moved), ..KeyValues::default() })?;
    Ok(
        json!({ "key": r["key"].clone(), "pinned": true, "message": format!("Pinned {id} at {}: move {}, {}", ms(t), s(moved[0]), s(moved[1])) }),
    )
}

/// Moves a key to another time (not the first key, not onto another). Returns `{ key, message }`.
pub fn move_key(doc: &mut Value, id: &str, t: f64, to: f64) -> Result<Value, CommandError> {
    let track = need_track(doc, id)?;
    if !keys_mut(track).iter().any(|k| tnum(k) == t) {
        return Err(err(format!("{id} has no key at {}; its keys are at {} ms", ms(t), key_times(track)), "not_found"));
    }
    if t == 0.0 {
        return Err(err("the key at 0 is the rest frame and stays there; set its values instead", "first_key"));
    }
    if to == 0.0 {
        return Err(err("0 is the rest frame, which the first key holds", "first_key"));
    }
    check_time(doc, to)?;
    let track = need_track(doc, id)?;
    let keys = keys_mut(track);
    if to != t && keys.iter().any(|k| tnum(k) == to) {
        return Err(err(format!("{id} already has a key at {}", ms(to)), "key_exists"));
    }
    let at = keys.iter().position(|k| tnum(k) == t).expect("the key was found");
    keys[at]["t"] = num(to);
    let key = keys[at].clone();
    keys.sort_by(|a, b| tnum(a).partial_cmp(&tnum(b)).unwrap_or(std::cmp::Ordering::Equal));
    Ok(json!({ "key": key, "message": format!("Moved {id}'s key from {} to {}", ms(t), ms(to)) }))
}

/// Deletes a key (not the first: remove the track for that). Returns `{ message }`.
pub fn delete_key(doc: &mut Value, id: &str, t: f64) -> Result<Value, CommandError> {
    let track = need_track(doc, id)?;
    let keys = keys_mut(track);
    let Some(i) = keys.iter().position(|k| tnum(k) == t) else {
        return Err(err(format!("{id} has no key at {}; its keys are at {} ms", ms(t), key_times(track)), "not_found"));
    };
    if i == 0 {
        return Err(err("the key at 0 is the rest frame; remove the track to drop it (scene.track.remove)", "first_key"));
    }
    keys_mut(track).remove(i);
    Ok(json!({ "message": format!("Deleted {id}'s key at {}", ms(t)) }))
}

/// Sets (or makes) the loop's length; refuses one that would cut off keys. Returns `{ message }`.
pub fn set_duration(doc: &mut Value, duration: f64) -> Result<Value, CommandError> {
    if !has_motion(doc) {
        doc["motion"] = json!({ "duration": num(duration), "tracks": [] });
        return Ok(json!({ "message": format!("Made the scene's motion, {} long", ms(duration)) }));
    }
    let mut late = vec![];
    for tr in doc["motion"].get("tracks").and_then(Value::as_array).into_iter().flatten() {
        let item = tr.get("item").and_then(Value::as_str).unwrap_or("");
        for k in tr.get("keys").and_then(Value::as_array).into_iter().flatten() {
            if tnum(k) >= duration {
                late.push(format!("{item} at {}", ms(tnum(k))));
            }
        }
    }
    if !late.is_empty() {
        let one = late.len() == 1;
        return Err(err(
            format!(
                "a {} loop would cut off key{}: {}; move or delete {} first",
                ms(duration),
                if one { "" } else { "s" },
                late.join(", "),
                if one { "it" } else { "them" }
            ),
            "out_of_range",
        ));
    }
    doc["motion"]["duration"] = num(duration);
    Ok(json!({ "message": format!("The loop is {} long", ms(duration)) }))
}

fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// A move split into on-grid runs: for |dx| ≥ 2|dy| a 2:1 run then a straight
/// one; between, a 2:1 run and a 1:1 run; steeper, a 1:1 run then a straight
/// vertical one.
pub fn grid_runs(dx: f64, dy: f64) -> Vec<[f64; 2]> {
    let (a, b) = (dx.abs(), dy.abs());
    let (sx, sy) = (sign(dx), sign(dy));
    let mut runs = vec![];
    let mut add = |x: f64, y: f64| {
        if x != 0.0 || y != 0.0 {
            runs.push([x * sx + 0.0, y * sy + 0.0]);
        }
    };
    if a >= 2.0 * b {
        add(2.0 * b, b);
        add(a - 2.0 * b, 0.0);
    } else if a >= b {
        add(2.0 * (a - b), a - b);
        add(2.0 * b - a, 2.0 * b - a);
    } else {
        add(a, a);
        add(0.0, b - a);
    }
    runs
}

/// `Math.hypot(x, y)` as V8 computes it (scaled, with compensated summing).
fn hypot(x: f64, y: f64) -> f64 {
    let max = x.abs().max(y.abs());
    if max == 0.0 {
        return 0.0;
    }
    let (mut sum, mut compensation) = (0.0_f64, 0.0_f64);
    for v in [x, y] {
        let n = v.abs() / max;
        let summand = n * n - compensation;
        let preliminary = sum + summand;
        compensation = (preliminary - sum) - summand;
        sum = preliminary;
    }
    sum.sqrt() * max
}

/// How `motion_path` times its keys.
#[derive(Clone, Copy, Debug, Default)]
pub struct PathTiming {
    pub start: f64,
    /// Pixels per second.
    pub speed: Option<f64>,
    pub end: Option<f64>,
}

struct PathRun {
    from: [f64; 2],
    to: [f64; 2],
    length: f64,
}

/// Sets `t`'s entry in an insertion-ordered list (a JS `Map` keyed by number).
fn put(map: &mut Vec<(f64, Value)>, t: f64, key: Value) {
    match map.iter_mut().find(|(k, _)| *k == t) {
        Some(e) => e.1 = key,
        None => map.push((t, key)),
    }
}

fn with_move(key: &Value, mv: [f64; 2]) -> Value {
    let mut k = key.clone();
    k["move"] = json!([num(mv[0]), num(mv[1])]);
    canonical_key(&k)
}

/// Keys along a polyline, on the grid, at a steady speed (px per second) or
/// spread from `start` to `end`. Existing keys in (start, end] lose their move;
/// those left empty go, and those with show or pose get the point on the path
/// at their time, so the path stays on the grid through them. `points` are
/// moves (pixels from the item's placement), the first at `start`. Returns
/// `{ keys, end, message }`.
pub fn motion_path(doc: &mut Value, id: &str, points: &[[f64; 2]], timing: PathTiming) -> Result<Value, CommandError> {
    let PathTiming { start, speed, end } = timing;
    need_track(doc, id)?;
    check_time(doc, start)?;
    if points.len() < 2 {
        return Err(err("a path needs at least two points", "usage"));
    }
    let mut runs: Vec<PathRun> = vec![];
    for i in 1..points.len() {
        let mut at = points[i - 1];
        for [rx, ry] in grid_runs(points[i][0] - points[i - 1][0], points[i][1] - points[i - 1][1]) {
            let to = [at[0] + rx, at[1] + ry];
            runs.push(PathRun { from: at, to, length: hypot(rx, ry) });
            at = to;
        }
    }
    if runs.is_empty() {
        return Err(err("the path goes nowhere: every point is the same", "usage"));
    }
    let total: f64 = runs.iter().fold(0.0, |n, r| n + r.length);
    if let Some(e) = end {
        if e <= start {
            return Err(err(format!("end ({}) must be after start ({})", ms(e), ms(start)), "usage"));
        }
    }
    if speed.is_none() && end.is_none() {
        return Err(err("scene.motion.path: give --speed or --end", "usage"));
    }
    let mut times: Vec<f64> = vec![];
    let mut cum = 0.0;
    for r in &runs {
        cum += r.length;
        let t = start
            + match speed {
                Some(sp) => jsnum::round((cum / sp) * 1000.0),
                None => jsnum::round((cum / total) * (end.expect("an end") - start)),
            };
        let floor = times.last().copied().unwrap_or(start) + 1.0;
        times.push(t.max(floor));
    }
    let last = *times.last().expect("a time per run");
    if last >= duration_of(doc) || end.is_some_and(|e| last > e) {
        let takes = match speed {
            Some(sp) => format!("at {} px/s the path takes {} ms", s(sp), s(last - start)),
            None => format!("the path needs {} ms", s(last - start)),
        };
        let past = match end {
            Some(e) if last > e => ms(e),
            _ => format!("the {} loop", ms(duration_of(doc))),
        };
        return Err(err(format!("{takes} from {}, ending at {}, past {past}", ms(start), ms(last)), "too_long"));
    }
    // Keys inside the path: they lose their move, and keep a point on it if they hold show or pose.
    let old: Vec<Value> = keys_mut(need_track(doc, id)?).clone();
    let mut kept: Vec<Value> = vec![];
    for key in old {
        let kt = tnum(&key);
        if kt <= start || kt > last {
            kept.push(key);
            continue;
        }
        let mut rest = key.clone();
        if let Some(o) = rest.as_object_mut() {
            o.shift_remove("move");
        }
        if rest.get("show").is_none() && rest.get("pose").is_none() {
            continue;
        }
        let k = times.iter().position(|t| *t >= kt).expect("the key is within the path");
        let run = &runs[k];
        let t0 = if k == 0 { start } else { times[k - 1] };
        let n = motion::steps_for(run.to[0] - run.from[0], run.to[1] - run.from[1]);
        let done = (((kt - t0) / (times[k] - t0)) * n).floor();
        let mv = [run.from[0] + ((run.to[0] - run.from[0]) / n) * done, run.from[1] + ((run.to[1] - run.from[1]) / n) * done];
        kept.push(with_move(&rest, mv));
    }
    let mut by_t: Vec<(f64, Value)> = vec![];
    for k in kept {
        put(&mut by_t, tnum(&k), k);
    }
    let start_key = by_t.iter().find(|(k, _)| *k == start).map(|(_, v)| v.clone()).unwrap_or_else(|| json!({ "t": num(start) }));
    put(&mut by_t, start, with_move(&start_key, points[0]));
    let mut written = vec![json!({ "t": num(start), "move": [num(points[0][0]), num(points[0][1])] })];
    for (i, r) in runs.iter().enumerate() {
        let t = times[i];
        let base = by_t.iter().find(|(k, _)| *k == t).map(|(_, v)| v.clone()).unwrap_or_else(|| json!({ "t": num(t) }));
        put(&mut by_t, t, with_move(&base, r.to));
        written.push(json!({ "t": num(t), "move": [num(r.to[0]), num(r.to[1])] }));
    }
    let mut keys: Vec<Value> = by_t.into_iter().map(|(_, v)| v).collect();
    keys.sort_by(|a, b| tnum(a).partial_cmp(&tnum(b)).unwrap_or(std::cmp::Ordering::Equal));
    *keys_mut(need_track(doc, id)?) = keys;
    let plural = if runs.len() == 1 { "" } else { "s" };
    Ok(json!({
        "keys": written,
        "end": num(last),
        "message": format!("Keyed {id} along {} points: {} on-grid run{plural}, {} to {}", points.len(), runs.len(), ms(start), ms(last)),
    }))
}
