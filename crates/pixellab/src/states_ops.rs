//! State edits: a sprite's states, their frames and moves, and the references to them (docs/intents/2026-10-object-states/spec.md,
//! D3). Each function changes an object source in place, in the shape a person would write it, and returns what the caller needs;
//! `states_refs` and `rename_in_scene` reach into the scenes that name a state. The checks here say what is wrong in the words of the
//! command; the engine's own (`states::validate`) still decides what may be saved.

use std::collections::HashSet;

use engine::engine::{is_name, resolve, Sources};
use engine::jsnum::{self, num};
use engine::motion;
use engine::states::{self, States, MAX_FRAMES, MAX_LENGTH, MAX_MS, MAX_STATES, MIN_MS};
use serde_json::{json, Map, Value};

use crate::errors::CommandError;

type Result<T> = std::result::Result<T, CommandError>;

/// Names the scene commands read as words: `scene.key.set --state clear` and `scene.set --state start`.
pub const RESERVED: [&str; 2] = ["clear", "start"];

fn err(message: impl Into<String>, code: &str) -> CommandError {
    CommandError::new(message, code)
}

fn usage(message: impl Into<String>) -> CommandError {
    CommandError::usage(message)
}

fn s(x: f64) -> String {
    jsnum::to_string(x)
}

// -------------------------------------------------------------------- reading

/// The sprite's frame count and the frame length its frame loop plays at today (R4): what a new state gets
/// without `--frames` and `--ms`. A still sprite has one frame, and 200 ms.
pub fn timing(doc: &Value) -> (usize, f64) {
    let mut count = 1;
    let mut ms = 200.0;
    let mut first = true;
    for layer in doc.get("layers").and_then(Value::as_array).into_iter().flatten() {
        let Some(frames) = layer.get("frames").and_then(Value::as_array) else { continue };
        count = count.max(frames.len());
        if first {
            first = false;
            let name = layer.get("loop").and_then(Value::as_str).unwrap_or("");
            let n = motion::SITE_LOOPS.iter().find(|l| l.name == name).map_or(frames.len(), |l| l.frames).max(1) as f64;
            let cycle = motion::LOOP_CYCLES.iter().find(|(l, _)| *l == name).map_or(200.0 * n, |(_, c)| *c);
            ms = cycle / n;
        }
    }
    (count, ms)
}

/// The object as a sprite that can have states: not a block, and one that doesn't extend another (R5).
pub fn check_sprite(name: &str, doc: &Value) -> Result<()> {
    if doc.get("kind").and_then(Value::as_str) == Some("block") {
        return Err(err(format!("{name} is a block; a block has no states"), "not_editable"));
    }
    if let Some(base) = doc.get("extends").and_then(Value::as_str) {
        return Err(err(format!("{name} extends {base} and has its states; change them on {base}"), "not_editable"));
    }
    Ok(())
}

fn states_map(doc: &Value) -> Option<&Map<String, Value>> {
    doc.get("states").and_then(Value::as_object)
}

fn names_of(doc: &Value) -> Vec<String> {
    states_map(doc).map(|m| engine::serialize::keys(m).into_iter().cloned().collect()).unwrap_or_default()
}

/// A state of the object, or a not_found that lists the ones it has.
pub fn need_state(object: &str, doc: &Value, state: &str) -> Result<()> {
    let names = names_of(doc);
    if names.is_empty() {
        return Err(err(format!("{object} has no states (object.state.add makes the first)"), "no_states"));
    }
    if !names.iter().any(|n| n == state) {
        return Err(err(format!("{object} has no state {state} (it has {})", names.join(", ")), "not_found"));
    }
    Ok(())
}

fn state_mut<'a>(doc: &'a mut Value, state: &str) -> &'a mut Value {
    &mut doc["states"][state]
}

/// A new state's name: lowercase words joined by hyphens, not taken, and not one the scene commands read as a word.
pub fn check_new_name(doc: &Value, name: &str) -> Result<()> {
    if !is_name(name) {
        return Err(usage(format!("a state's name is lowercase words joined by hyphens, like wave-twice, not {name:?}")));
    }
    if RESERVED.contains(&name) {
        return Err(usage(format!(
            "{name} is a word the scene commands read (--state clear, --state start): name the state something else"
        )));
    }
    if let Some(why) = engine::states::reserved(name) {
        return Err(usage(format!("{name}: {why}; name the state something else")));
    }
    if names_of(doc).iter().any(|n| n == name) {
        return Err(err(format!("there is already a state named {name}"), "state_exists"));
    }
    Ok(())
}

fn check_frames(frames: &[usize], count: usize) -> Result<()> {
    if frames.is_empty() || frames.len() > MAX_FRAMES {
        return Err(usage(format!("a state plays 1 to {MAX_FRAMES} frames")));
    }
    if let Some(bad) = frames.iter().find(|f| **f >= count) {
        return Err(usage(format!("there's no frame {bad}; the sprite has {count} (0 to {})", count - 1)));
    }
    Ok(())
}

fn check_ms(ms: f64) -> Result<()> {
    if !jsnum::is_integer(ms) || !(MIN_MS..=MAX_MS).contains(&ms) {
        return Err(usage(format!("--ms is a whole number from {MIN_MS} to {MAX_MS}: how long each frame shows")));
    }
    Ok(())
}

fn check_length(state: &str, frames: usize, ms: f64) -> Result<()> {
    if frames as f64 * ms > MAX_LENGTH {
        return Err(usage(format!("{state} would last {} ms; a state lasts at most {MAX_LENGTH}", s(frames as f64 * ms))));
    }
    Ok(())
}

fn frames_value(frames: &[usize]) -> Value {
    Value::Array(frames.iter().map(|f| Value::from(*f)).collect())
}

fn frames_of(doc: &Value, state: &str) -> Vec<usize> {
    doc["states"][state]["frames"].as_array().map(|a| a.iter().filter_map(Value::as_u64).map(|n| n as usize).collect()).unwrap_or_default()
}

fn ms_of(doc: &Value, state: &str) -> f64 {
    doc["states"][state]["ms"].as_f64().unwrap_or(f64::NAN)
}

/// Rebuilds a state with its properties in the canonical order: frames, ms, loop, then, move.
fn canonical(state: &Value) -> Value {
    let mut out = Map::new();
    for key in ["frames", "ms", "loop", "then", "move"] {
        if let Some(v) = state.get(key) {
            out.insert(key.to_string(), v.clone());
        }
    }
    Value::Object(out)
}

/// The states, rebuilt in a list of names (a new order, a renamed state).
fn reorder(doc: &mut Value, order: &[String]) {
    let old = doc["states"].as_object().cloned().unwrap_or_default();
    let mut map = Map::new();
    for name in order {
        if let Some(v) = old.get(name) {
            map.insert(name.clone(), v.clone());
        }
    }
    doc["states"] = Value::Object(map);
}

// ------------------------------------------------------------------- the states

/// What `add` takes.
#[derive(Clone, Debug, Default)]
pub struct New {
    pub frames: Option<Vec<usize>>,
    pub ms: Option<f64>,
    pub once: bool,
    pub then: Option<String>,
    pub after: Option<String>,
}

/// Adds a state, after `after` or last; the first becomes the start (R4). Returns the message.
pub fn add(object: &str, doc: &mut Value, name: &str, with: &New) -> Result<String> {
    check_sprite(object, doc)?;
    check_new_name(doc, name)?;
    if names_of(doc).len() >= MAX_STATES {
        return Err(err(format!("a sprite has at most {MAX_STATES} states"), "too_many"));
    }
    let (count, default_ms) = timing(doc);
    let frames = with.frames.clone().unwrap_or_else(|| (0..count).collect());
    check_frames(&frames, count)?;
    let ms = with.ms.unwrap_or(default_ms);
    check_ms(ms)?;
    check_length(name, frames.len(), ms)?;
    if with.then.is_some() && !with.once {
        return Err(usage("--then goes with --once: only a state that plays once hands over"));
    }
    if let Some(then) = &with.then {
        if then != name {
            need_state(object, doc, then)?;
        }
    }
    if let Some(after) = &with.after {
        need_state(object, doc, after)?;
    }
    let mut state = Map::new();
    state.insert("frames".into(), frames_value(&frames));
    state.insert("ms".into(), num(ms));
    if with.once {
        state.insert("loop".into(), Value::Bool(false));
        if let Some(then) = &with.then {
            state.insert("then".into(), Value::String(then.clone()));
        }
    }
    let first = names_of(doc).is_empty();
    let mut order = names_of(doc);
    let at = with.after.as_ref().and_then(|a| order.iter().position(|n| n == a)).map_or(order.len(), |i| i + 1);
    if first {
        doc["states"] = json!({});
        doc["start"] = Value::String(name.to_string());
    }
    doc["states"][name] = Value::Object(state);
    order.insert(at, name.to_string());
    reorder(doc, &order);
    Ok(if first {
        format!("Added state {name}, the start: {} frame{} at {} ms", frames.len(), if frames.len() == 1 { "" } else { "s" }, s(ms))
    } else {
        format!("Added state {name}")
    })
}

/// What `set` takes.
#[derive(Clone, Debug, Default)]
pub struct Change {
    pub rename: Option<String>,
    pub frames: Option<Vec<usize>>,
    pub ms: Option<f64>,
    pub looping: bool,
    pub once: bool,
    pub then: Option<String>,
    pub hold: bool,
    pub start: bool,
}

/// Changes a state (R7's rules hold after it). A rename updates `start` and every `then` here; the caller updates the scenes.
/// Returns the message.
pub fn set(object: &str, doc: &mut Value, state: &str, change: &Change) -> Result<String> {
    check_sprite(object, doc)?;
    need_state(object, doc, state)?;
    if change.looping && change.once {
        return Err(usage("give only one of --loop, --once"));
    }
    if change.then.is_some() && change.hold {
        return Err(usage("give only one of --then, --hold"));
    }
    let (count, _) = timing(doc);
    let mut said: Vec<String> = Vec::new();
    let mut current = state.to_string();
    if let Some(new) = &change.rename {
        if new != state {
            check_new_name(doc, new)?;
        }
    }
    let loops_now = doc["states"][state].get("loop").is_none();
    let plays_once = change.once || (!loops_now && !change.looping);
    if change.then.is_some() && !plays_once {
        return Err(usage("--then goes with a state that plays once (--once)"));
    }
    if let Some(then) = &change.then {
        let target = if change.rename.as_deref() == Some(then.as_str()) { state } else { then.as_str() };
        need_state(object, doc, target)?;
    }
    let frames = match &change.frames {
        Some(f) => {
            check_frames(f, count)?;
            f.clone()
        }
        None => frames_of(doc, state),
    };
    let ms = match change.ms {
        Some(ms) => {
            check_ms(ms)?;
            ms
        }
        None => ms_of(doc, state),
    };
    check_length(state, frames.len(), ms)?;
    // What the state's move can hold: its keys must stay below the new length.
    if let Some(keys) = doc["states"][state].get("move").and_then(Value::as_array) {
        let length = frames.len() as f64 * ms;
        if let Some(late) = keys.iter().find(|k| k.get("t").and_then(Value::as_f64).is_some_and(|t| t >= length)) {
            return Err(err(
                format!(
                    "{state} would last {} ms, which cuts off its move key at {} ms; move or delete the key first (object.state.key.move)",
                    s(length),
                    s(late.get("t").and_then(Value::as_f64).unwrap_or(0.0))
                ),
                "out_of_range",
            ));
        }
    }
    {
        let st = state_mut(doc, state);
        if change.frames.is_some() {
            st["frames"] = frames_value(&frames);
            said.push(format!("frames {}", frames.iter().map(usize::to_string).collect::<Vec<_>>().join(",")));
        }
        if change.ms.is_some() {
            st["ms"] = num(ms);
            said.push(format!("{} ms a frame", s(ms)));
        }
        if change.looping {
            st.as_object_mut().expect("a state").shift_remove("loop");
            st.as_object_mut().expect("a state").shift_remove("then");
            said.push("loops".into());
        }
        if change.once {
            st["loop"] = Value::Bool(false);
            said.push("plays once".into());
        }
        if change.hold {
            st.as_object_mut().expect("a state").shift_remove("then");
            said.push("holds its last frame".into());
        }
        if let Some(then) = &change.then {
            st["then"] = Value::String(then.clone());
            said.push(format!("then {then}"));
        }
        *st = canonical(st);
    }
    if change.start {
        doc["start"] = Value::String(state.to_string());
        said.push("is the start".into());
    }
    if let Some(new) = change.rename.as_ref().filter(|n| n.as_str() != state) {
        let order: Vec<String> = names_of(doc).into_iter().map(|n| if n == state { new.clone() } else { n }).collect();
        let old = doc["states"][state].clone();
        doc["states"][new] = old;
        reorder(doc, &order);
        if doc["start"].as_str() == Some(state) {
            doc["start"] = Value::String(new.clone());
        }
        for name in names_of(doc) {
            if doc["states"][&name]["then"].as_str() == Some(state) {
                doc["states"][&name]["then"] = Value::String(new.clone());
            }
        }
        current = new.clone();
        said.insert(0, format!("renamed to {new}"));
    }
    Ok(if said.is_empty() { format!("{current} is as it was") } else { format!("State {state}: {}", said.join(", ")) })
}

/// Deletes a state. The caller has checked that nothing in the scenes names it. Refused while `start` or a `then` does; deleting the
/// last state takes `states` and `start` off, and the frame loops play on their own again. Returns the message.
pub fn delete(object: &str, doc: &mut Value, state: &str) -> Result<String> {
    check_sprite(object, doc)?;
    need_state(object, doc, state)?;
    let mut by: Vec<String> = Vec::new();
    if doc["start"].as_str() == Some(state) && names_of(doc).len() > 1 {
        by.push("it is the start (set another with object.state.set --start)".into());
    }
    for other in names_of(doc).iter().filter(|n| n.as_str() != state) {
        if doc["states"][other]["then"].as_str() == Some(state) {
            by.push(format!("{other} hands over to it"));
        }
    }
    if !by.is_empty() {
        return Err(err(format!("{object}'s state {state} stays: {}", by.join("; ")), "in_use"));
    }
    let remaining: Vec<String> = names_of(doc).into_iter().filter(|n| n != state).collect();
    if remaining.is_empty() {
        let o = doc.as_object_mut().expect("an object source");
        o.shift_remove("states");
        o.shift_remove("start");
        return Ok(format!("Deleted state {state}, the last: the frame loops play on their own again"));
    }
    reorder(doc, &remaining);
    Ok(format!("Deleted state {state}"))
}

/// Copies a state just after itself, as `name` (else `<state>-2`, made unique). Returns the new name and the message.
pub fn duplicate(object: &str, doc: &mut Value, state: &str, name: Option<&str>) -> Result<(String, String)> {
    check_sprite(object, doc)?;
    need_state(object, doc, state)?;
    if names_of(doc).len() >= MAX_STATES {
        return Err(err(format!("a sprite has at most {MAX_STATES} states"), "too_many"));
    }
    let new = match name {
        Some(n) => {
            check_new_name(doc, n)?;
            n.to_string()
        }
        None => {
            let taken = names_of(doc);
            (2..).map(|i| format!("{state}-{i}")).find(|n| !taken.contains(n)).expect("a free name")
        }
    };
    let copy = doc["states"][state].clone();
    doc["states"][&new] = copy;
    let mut order = names_of(doc);
    order.retain(|n| *n != new);
    let at = order.iter().position(|n| n == state).map_or(order.len(), |i| i + 1);
    order.insert(at, new.clone());
    reorder(doc, &order);
    Ok((new.clone(), format!("Duplicated state {state} as {new}")))
}

/// Moves a state in the list by `by` places (the list's order is what a later state wins by, and the pane's order).
pub fn move_by(object: &str, doc: &mut Value, state: &str, by: i64) -> Result<(usize, String)> {
    check_sprite(object, doc)?;
    need_state(object, doc, state)?;
    if by == 0 {
        return Err(usage("--by must not be 0"));
    }
    let mut order = names_of(doc);
    let from = order.iter().position(|n| n == state).expect("a state");
    let to = from as i64 + by;
    if to < 0 || to >= order.len() as i64 {
        return Err(err(
            format!("{state} can't move {} {}: there are {} states", if by > 0 { "down" } else { "up" }, by.abs(), order.len()),
            "out_of_range",
        ));
    }
    let name = order.remove(from);
    order.insert(to as usize, name);
    reorder(doc, &order);
    Ok((to as usize, format!("Moved state {state} to place {}", to + 1)))
}

// ----------------------------------------------------------------------- frames

fn put_frames(doc: &mut Value, state: &str, frames: &[usize]) -> Result<()> {
    let ms = ms_of(doc, state);
    check_length(state, frames.len(), ms)?;
    state_mut(doc, state)["frames"] = frames_value(frames);
    Ok(())
}

/// Adds frame `frame` of the sprite to a state at position `at` (last by default).
pub fn frame_add(object: &str, doc: &mut Value, state: &str, frame: usize, at: Option<usize>) -> Result<String> {
    check_sprite(object, doc)?;
    need_state(object, doc, state)?;
    let (count, _) = timing(doc);
    check_frames(&[frame], count)?;
    let mut frames = frames_of(doc, state);
    if frames.len() >= MAX_FRAMES {
        return Err(err(format!("{state} plays {MAX_FRAMES} frames, the most a state can"), "too_many"));
    }
    let at = at.unwrap_or(frames.len());
    if at > frames.len() {
        return Err(usage(format!("there's no position {at}; {state} plays {} frames (0 to {})", frames.len(), frames.len())));
    }
    frames.insert(at, frame);
    put_frames(doc, state, &frames)?;
    Ok(format!("{state} plays frame {frame} at position {at}"))
}

fn position(frames: &[usize], state: &str, at: usize) -> Result<()> {
    if at >= frames.len() {
        return Err(usage(format!("there's no position {at}; {state} plays {} frames (0 to {})", frames.len(), frames.len() as i64 - 1)));
    }
    Ok(())
}

/// Takes the frame at position `at` out of a state (not its only one).
pub fn frame_delete(object: &str, doc: &mut Value, state: &str, at: usize) -> Result<String> {
    check_sprite(object, doc)?;
    need_state(object, doc, state)?;
    let mut frames = frames_of(doc, state);
    position(&frames, state, at)?;
    if frames.len() == 1 {
        return Err(err(format!("{state} keeps at least one frame"), "last_frame"));
    }
    let gone = frames.remove(at);
    put_frames(doc, state, &frames)?;
    Ok(format!("{state} no longer plays frame {gone} at position {at}"))
}

/// Moves the position `at` of a state later (`by` 1) or earlier (−1). Returns where it went.
pub fn frame_move(object: &str, doc: &mut Value, state: &str, at: usize, by: i64) -> Result<(usize, String)> {
    check_sprite(object, doc)?;
    need_state(object, doc, state)?;
    let mut frames = frames_of(doc, state);
    position(&frames, state, at)?;
    if by == 0 {
        return Err(usage("--by must not be 0"));
    }
    let to = at as i64 + by;
    if to < 0 || to >= frames.len() as i64 {
        return Err(err(
            format!(
                "position {at} can't move {} {}: {state} plays {} frames",
                if by > 0 { "later" } else { "earlier" },
                by.abs(),
                frames.len()
            ),
            "out_of_range",
        ));
    }
    let f = frames.remove(at);
    frames.insert(to as usize, f);
    put_frames(doc, state, &frames)?;
    Ok((to as usize, format!("Moved position {at} of {state} to {to}")))
}

/// Renumbers the frames every state plays after the sprite's frames change (R30): `map` gives a frame its new number, or None
/// when it is gone. Refused, naming the state, when one would be left with no frames; nothing changes then. Returns the states
/// whose frames changed, in order.
pub fn remap_frames(doc: &mut Value, map: &dyn Fn(usize) -> Option<usize>) -> Result<Vec<String>> {
    let mut changed: Vec<(String, Vec<usize>)> = Vec::new();
    for name in names_of(doc) {
        let old = frames_of(doc, &name);
        let new: Vec<usize> = old.iter().filter_map(|f| map(*f)).collect();
        if new.is_empty() {
            return Err(err(
                format!("state {name} plays only that frame, so it would be left with none; give {name} another frame first (object.state.frame.add)"),
                "state_needs_frame",
            ));
        }
        if new != old {
            changed.push((name, new));
        }
    }
    let names = changed.iter().map(|(n, _)| n.clone()).collect();
    for (name, frames) in changed {
        state_mut(doc, &name)["frames"] = frames_value(&frames);
    }
    Ok(names)
}

// ------------------------------------------------------------------------ moves

fn keys_of(doc: &Value, state: &str) -> Vec<Value> {
    doc["states"][state].get("move").and_then(Value::as_array).cloned().unwrap_or_default()
}

fn tnum(key: &Value) -> f64 {
    key.get("t").and_then(Value::as_f64).unwrap_or(f64::NAN)
}

fn key_times(keys: &[Value]) -> String {
    keys.iter().map(|k| s(tnum(k))).collect::<Vec<_>>().join(", ")
}

fn ms(t: f64) -> String {
    format!("{} ms", s(t))
}

fn put_keys(doc: &mut Value, state: &str, keys: Vec<Value>) {
    let st = state_mut(doc, state);
    if keys.is_empty() {
        st.as_object_mut().expect("a state").shift_remove("move");
    } else {
        st["move"] = Value::Array(keys);
    }
    *st = canonical(st);
}

fn length_of(doc: &Value, state: &str) -> f64 {
    frames_of(doc, state).len() as f64 * ms_of(doc, state)
}

/// Where the whole sprite stands `t` ms into a state that plays once or loops, as its move says (for pinning a key).
pub fn move_at(doc: &Value, state: &str, t: f64) -> [f64; 2] {
    // Inside the state's own length, its run is the state's.
    States::of(doc).map_or([0.0, 0.0], |st| states::at(&st, &states::run(&st, state), t).moved)
}

/// Sets a key of a state's move, making it (in time order) if there isn't one: `moved` x,y, or, with None, where the sprite is then
/// (pinned). The first key is at 0 and made with the state's move. Returns whether it pinned, the key and the message.
pub fn key_set(object: &str, doc: &mut Value, state: &str, t: f64, moved: Option<[f64; 2]>) -> Result<(bool, Value, String)> {
    check_sprite(object, doc)?;
    need_state(object, doc, state)?;
    let length = length_of(doc, state);
    if !jsnum::is_integer(t) || t < 0.0 {
        return Err(usage("a key's time is a whole number of ms from 0"));
    }
    if t >= length {
        return Err(err(
            format!("{} is past the end of {state}, which lasts {} (0 to {})", ms(t), ms(length), s(length - 1.0)),
            "out_of_range",
        ));
    }
    let mut keys = keys_of(doc, state);
    let existing = keys.iter().position(|k| tnum(k) == t);
    let pinned = moved.is_none();
    if let (true, Some(i)) = (pinned, existing) {
        return Ok((false, keys[i].clone(), format!("{state} already has a key at {}", ms(t))));
    }
    let value = moved.unwrap_or_else(|| move_at(doc, state, t));
    let key = json!({ "t": num(t), "move": [num(value[0]), num(value[1])] });
    match existing {
        Some(i) => keys[i] = key.clone(),
        None => {
            // The first key is the rest frame, at 0.
            if keys.is_empty() && t > 0.0 {
                keys.push(json!({ "t": 0, "move": [0, 0] }));
            }
            let at = keys.iter().filter(|k| tnum(k) < t).count();
            keys.insert(at, key.clone());
        }
    }
    put_keys(doc, state, keys);
    let verb = if existing.is_some() { "Set" } else { "Made" };
    Ok((pinned, key, format!("{verb} {state}'s key at {}: move {}, {}", ms(t), s(value[0]), s(value[1]))))
}

/// Deletes a key of a state's move; deleting the only one takes the move off. The key at 0 stays while others follow it.
pub fn key_delete(object: &str, doc: &mut Value, state: &str, t: f64) -> Result<String> {
    check_sprite(object, doc)?;
    need_state(object, doc, state)?;
    let mut keys = keys_of(doc, state);
    let Some(i) = keys.iter().position(|k| tnum(k) == t) else {
        return Err(err(format!("{state} has no key at {}; its keys are at {} ms", ms(t), key_times(&keys)), "not_found"));
    };
    if i == 0 && keys.len() > 1 {
        return Err(err("the key at 0 is the rest frame; delete the others first, or set its values instead", "first_key"));
    }
    keys.remove(i);
    let message = if keys.is_empty() { format!("Took {state}'s move off") } else { format!("Deleted {state}'s key at {}", ms(t)) };
    put_keys(doc, state, keys);
    Ok(message)
}

/// Moves a key of a state's move to another time (not the first, not onto another, not past the state's end).
pub fn key_move(object: &str, doc: &mut Value, state: &str, t: f64, to: f64) -> Result<(Value, String)> {
    check_sprite(object, doc)?;
    need_state(object, doc, state)?;
    let mut keys = keys_of(doc, state);
    let Some(i) = keys.iter().position(|k| tnum(k) == t) else {
        return Err(err(format!("{state} has no key at {}; its keys are at {} ms", ms(t), key_times(&keys)), "not_found"));
    };
    if t == 0.0 || to == 0.0 {
        return Err(err("the key at 0 is the rest frame and stays there; set its values instead", "first_key"));
    }
    let length = length_of(doc, state);
    if !jsnum::is_integer(to) || to < 0.0 || to >= length {
        return Err(err(format!("{} is outside {state}, which lasts {} (0 to {})", ms(to), ms(length), s(length - 1.0)), "out_of_range"));
    }
    if to != t && keys.iter().any(|k| tnum(k) == to) {
        return Err(err(format!("{state} already has a key at {}", ms(to)), "key_exists"));
    }
    keys[i]["t"] = num(to);
    let key = keys[i].clone();
    keys.sort_by(|a, b| tnum(a).partial_cmp(&tnum(b)).unwrap_or(std::cmp::Ordering::Equal));
    put_keys(doc, state, keys);
    Ok((key, format!("Moved {state}'s key from {} to {}", ms(t), ms(to))))
}

// -------------------------------------------------------------------------- make

/// What `make` did: the states it made.
pub struct Made {
    pub states: Vec<String>,
    pub message: String,
}

fn has_pixels(layer: &Value) -> bool {
    let maps: Vec<&Value> = match layer.get("frames").and_then(Value::as_array) {
        Some(frames) => frames.iter().collect(),
        None => layer.get("map").into_iter().collect(),
    };
    maps.iter().any(|m| m.as_array().into_iter().flatten().filter_map(Value::as_str).any(|row| row.chars().any(|c| c != '.')))
}

fn move_keys(keys: Option<&Value>) -> Value {
    keys.cloned().unwrap_or_else(|| json!([]))
}

/// Turns the layers' ambient loop and actions into states (R8). The ambient loop becomes the start state, `idle`: it loops, the
/// layer's move is the sprite's move and its length the loop's duration. Each action becomes a state of its name that plays once with
/// the action's move and then hands over to `idle`. The frames carry over (R4). Refuses, saying why, when the art couldn't stay the
/// same.
pub fn make(object: &str, doc: &mut Value) -> Result<Made> {
    check_sprite(object, doc)?;
    if states_map(doc).is_some() {
        return Err(err(format!("{object} has states already"), "has_states"));
    }
    let layers = doc.get("layers").and_then(Value::as_array).cloned().unwrap_or_default();
    let moving: Vec<usize> =
        layers.iter().enumerate().filter(|(_, l)| l.get("ambient").is_some() || l.get("actions").is_some()).map(|(i, _)| i).collect();
    if moving.is_empty() {
        return Err(err(
            format!("{object} has no ambient loop or actions on its layers to make states of (object.state.add makes a state)"),
            "nothing_to_make",
        ));
    }
    if moving.len() > 1 {
        let list = moving.iter().map(|i| format!("layer {i}")).collect::<Vec<_>>().join(", ");
        return Err(err(
            format!("{object} moves more than one layer ({list}); a state moves the whole sprite, so the art would change. Give the moves to one layer first"),
            "moves_on_many_layers",
        ));
    }
    let at = moving[0];
    if let Some(other) = layers.iter().enumerate().find(|(i, l)| *i != at && has_pixels(l)).map(|(i, _)| i) {
        return Err(err(
            format!("{object}'s layer {at} moves while layer {other} has pixels; a state moves the whole sprite, so layer {other} would move too and the art would change"),
            "moves_other_layers",
        ));
    }
    let layer = &layers[at];
    if layer.get("ambient").and_then(|a| a.get("offset")).and_then(Value::as_f64).is_some_and(|o| o != 0.0) {
        return Err(err(
            format!(
                "{object}'s ambient loop starts {} ms in; a state starts at 0, so it would not move the same",
                s(layer["ambient"]["offset"].as_f64().unwrap_or(0.0))
            ),
            "ambient_offset",
        ));
    }
    let (count, step) = timing(doc);
    let cycle = count as f64 * step;
    // The frames to play so that the state lasts as long as its move: one pass, or as many passes of the sprite's frames as fit.
    let frames_for = |duration: f64, name: &str| -> Result<(Vec<usize>, f64)> {
        if count == 1 {
            if (MIN_MS..=MAX_MS).contains(&duration) {
                return Ok((vec![0], duration));
            }
            return Err(err(
                format!(
                    "{name} lasts {} ms; a state's frame shows for {MIN_MS} to {MAX_MS} ms, so the art couldn't stay the same",
                    s(duration)
                ),
                "duration",
            ));
        }
        let passes = duration / cycle;
        if passes.fract() != 0.0 || passes < 1.0 || passes * count as f64 > MAX_FRAMES as f64 {
            return Err(err(
                format!("{name} lasts {} ms, which isn't a whole number of the frame loop's {} ms cycles, so it could not play in step with the frames", s(duration), s(cycle)),
                "duration",
            ));
        }
        Ok(((0..passes as usize).flat_map(|_| 0..count).collect(), step))
    };
    let mut made: Vec<(String, Value)> = Vec::new();
    let ambient = layer.get("ambient");
    // The start state: the ambient loop, or a still one when only actions move.
    let (frames, step_ms) = match ambient.and_then(|a| a.get("duration")).and_then(Value::as_f64) {
        Some(d) => frames_for(d, "the ambient loop")?,
        None => ((0..count).collect(), step),
    };
    let mut idle = json!({ "frames": frames_value(&frames), "ms": num(step_ms) });
    if let Some(a) = ambient {
        idle["move"] = move_keys(a.get("keys"));
    }
    made.push(("idle".into(), idle));
    for (name, action) in layer.get("actions").and_then(Value::as_object).into_iter().flat_map(|a| engine::serialize::entries(a)) {
        if made.iter().any(|(n, _)| n == name) {
            return Err(err(
                format!("an action is named {name}, which the ambient loop's state is called; rename the action first"),
                "name_taken",
            ));
        }
        if RESERVED.contains(&name.as_str()) {
            return Err(err(format!("an action is named {name}, a word the scene commands read; rename the action first"), "name_taken"));
        }
        if let Some(why) = engine::states::reserved(name) {
            return Err(err(format!("an action is named {name}, which can't name a state ({why}); rename the action first"), "name_taken"));
        }
        let (frames, step_ms) =
            frames_for(action.get("duration").and_then(Value::as_f64).unwrap_or(f64::NAN), &format!("the action {name}"))?;
        let mut state = json!({ "frames": frames_value(&frames), "ms": num(step_ms), "loop": false, "then": "idle" });
        state["move"] = move_keys(action.get("keys"));
        made.push((name.clone(), state));
    }
    // The layer's moves are the states' now.
    if let Some(l) = doc["layers"][at].as_object_mut() {
        l.shift_remove("ambient");
        l.shift_remove("actions");
    }
    let mut map = Map::new();
    for (name, state) in &made {
        map.insert(name.clone(), canonical(state));
    }
    doc["start"] = json!("idle");
    doc["states"] = Value::Object(map);
    let names: Vec<String> = made.into_iter().map(|(n, _)| n).collect();
    let message = format!(
        "Made {} of {object}'s ambient loop and actions: {}",
        if names.len() == 1 { "a state" } else { "states" },
        names.join(", ")
    );
    Ok(Made { states: names, message })
}

// -------------------------------------------------------------- the scenes' states

/// Every object whose states are `object`'s: it, and the sprites that extend it (R5).
pub fn sharing(sources: &Sources, object: &str) -> HashSet<String> {
    let mut out: HashSet<String> = HashSet::from([object.to_string()]);
    loop {
        let more: Vec<String> = sources
            .objects
            .iter()
            .filter(|(n, o)| !out.contains(*n) && o.get("extends").and_then(Value::as_str).is_some_and(|b| out.contains(b)))
            .map(|(n, _)| n.clone())
            .collect();
        if more.is_empty() {
            return out;
        }
        out.extend(more);
    }
}

/// Where a scene names a state of an object: an item's `state`, or a key's.
#[derive(Clone, Debug, PartialEq)]
pub struct Ref {
    pub scene: String,
    pub place: String,
    pub state: String,
}

/// The items and keys of every scene that name a state of `object` (or of a sprite sharing its states).
pub fn refs(sources: &Sources, object: &str) -> Vec<Ref> {
    let sharing = sharing(sources, object);
    let mut out = Vec::new();
    for (scene, doc) in &sources.scenes {
        fn walk(list: &[Value], path: &str, sharing: &HashSet<String>, scene: &str, out: &mut Vec<Ref>) {
            for (i, item) in list.iter().enumerate() {
                let here = if path.is_empty() { i.to_string() } else { format!("{path}.{i}") };
                if let Some(inner) = item.get("items").filter(|_| item.get("group").is_some()).and_then(Value::as_array) {
                    walk(inner, &here, sharing, scene, out);
                } else if let (Some(o), Some(state)) =
                    (item.get("object").and_then(Value::as_str), item.get("state").and_then(Value::as_str))
                {
                    if sharing.contains(o) {
                        out.push(Ref { scene: scene.to_string(), place: format!("item {here} ({o})"), state: state.to_string() });
                    }
                }
            }
        }
        walk(doc.get("items").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]), "", &sharing, scene, &mut out);
        let ids = motion::items_by_id(doc.get("items").unwrap_or(&Value::Null));
        for track in doc.pointer("/motion/tracks").and_then(Value::as_array).into_iter().flatten() {
            let Some(item) = track.get("item").and_then(Value::as_str).and_then(|id| ids.get(id)) else { continue };
            let base = item.get("object").and_then(Value::as_str).unwrap_or("");
            let filled = motion::filled_keys(track, base);
            for (k, key) in track.get("keys").and_then(Value::as_array).into_iter().flatten().enumerate() {
                if let (Some(state), Some(f)) = (key.get("state").and_then(Value::as_str), filled.get(k)) {
                    if sharing.contains(&f.pose) {
                        out.push(Ref {
                            scene: scene.to_string(),
                            place: format!("{}'s key at {} ms", track.get("item").and_then(Value::as_str).unwrap_or(""), s(f.t)),
                            state: state.to_string(),
                        });
                    }
                }
            }
        }
    }
    out
}

/// A scene with every item and key that names `old` (on an object sharing these states) naming `new`. Returns how many it changed.
pub fn rename_in_scene(scene: &mut Value, sharing: &HashSet<String>, old: &str, new: &str) -> usize {
    fn walk(list: &mut [Value], sharing: &HashSet<String>, old: &str, new: &str) -> usize {
        let mut n = 0;
        for item in list.iter_mut() {
            if item.get("group").is_some() {
                if let Some(inner) = item.get_mut("items").and_then(Value::as_array_mut) {
                    n += walk(inner, sharing, old, new);
                }
            } else if item.get("state").and_then(Value::as_str) == Some(old)
                && item.get("object").and_then(Value::as_str).is_some_and(|o| sharing.contains(o))
            {
                item["state"] = Value::String(new.to_string());
                n += 1;
            }
        }
        n
    }
    let mut changed = 0;
    if let Some(list) = scene.get_mut("items").and_then(Value::as_array_mut) {
        changed += walk(list, sharing, old, new);
    }
    let ids = motion::items_by_id(scene.get("items").unwrap_or(&Value::Null));
    let mut edits: Vec<(usize, usize)> = Vec::new();
    for (ti, track) in scene.pointer("/motion/tracks").and_then(Value::as_array).into_iter().flatten().enumerate() {
        let Some(item) = track.get("item").and_then(Value::as_str).and_then(|id| ids.get(id)) else { continue };
        let filled = motion::filled_keys(track, item.get("object").and_then(Value::as_str).unwrap_or(""));
        for (k, key) in track.get("keys").and_then(Value::as_array).into_iter().flatten().enumerate() {
            if key.get("state").and_then(Value::as_str) == Some(old) && filled.get(k).is_some_and(|f| sharing.contains(&f.pose)) {
                edits.push((ti, k));
            }
        }
    }
    for (ti, k) in edits {
        scene["motion"]["tracks"][ti]["keys"][k]["state"] = Value::String(new.to_string());
        changed += 1;
    }
    changed
}

/// The states an object has, resolved (through `extends`), for the commands that read them.
pub fn states_of(sources: &Sources, object: &str) -> Option<std::rc::Rc<States>> {
    resolve(sources, object).ok().and_then(|r| r.states)
}
