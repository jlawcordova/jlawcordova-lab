//! Ambient loops and actions (docs/intents/2026-10-rust-engine/spec.md,
//! R12, R13, D6, D7). An ambient loop is a repeating move of a scene item or
//! an object's map layer; an action is a one-shot move played through
//! `data-play`. Both step a whole pixel at a time between keys, as scene
//! motion does, and both are placed here exactly as their CSS plays them.

use std::rc::Rc;

use serde_json::{Map, Value};

use crate::engine::is_name;
use crate::jsnum;
use crate::motion::{pct, pct_of, px, steps_for};
use crate::serialize::keys;

/// The longest an ambient loop or an action may last, in ms.
pub const MAX_DURATION: f64 = 600_000.0;
/// The most actions one element may carry.
pub const MAX_ACTIONS: usize = 16;

/// A key: `t` ms in, the element `moved` pixels from where its art is drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    pub t: f64,
    pub moved: [f64; 2],
}

/// A repeating move: after the last key it steps back to the first over the
/// rest of `duration`, and it starts `offset` ms in.
#[derive(Clone, Debug, PartialEq)]
pub struct Ambient {
    pub duration: f64,
    pub offset: f64,
    pub keys: Vec<Key>,
}

/// A one-shot move: it holds its last key until `duration`, then the
/// element is back at rest.
#[derive(Clone, Debug, PartialEq)]
pub struct Action {
    pub duration: f64,
    pub keys: Vec<Key>,
}

/// What one element (a scene item or an object's map layer) plays.
#[derive(Clone, Debug, PartialEq)]
pub struct Moves {
    pub ambient: Option<Ambient>,
    /// By name, in the document's order.
    pub actions: Vec<(String, Action)>,
}

fn num(v: Option<&Value>) -> Option<f64> {
    v.and_then(Value::as_f64)
}

pub(crate) fn keys_of(v: Option<&Value>) -> Option<Vec<Key>> {
    let list = v?.as_array()?;
    let keys: Option<Vec<Key>> = list
        .iter()
        .map(|k| {
            let m = k.get("move");
            let at = |i: usize| m.and_then(|m| m.get(i)).and_then(Value::as_f64);
            Some(Key { t: num(k.get("t"))?, moved: [at(0).unwrap_or(0.0), at(1).unwrap_or(0.0)] })
        })
        .collect();
    keys.filter(|k| !k.is_empty())
}

/// An element's moves, from its `ambient` and `actions`; None when it has
/// neither. Malformed ones are left out (validation names them).
pub fn moves_of(v: &Value) -> Option<Rc<Moves>> {
    let ambient = v.get("ambient").and_then(|a| {
        Some(Ambient { duration: num(a.get("duration"))?, offset: num(a.get("offset")).unwrap_or(0.0), keys: keys_of(a.get("keys"))? })
    });
    let actions: Vec<(String, Action)> = match v.get("actions") {
        Some(Value::Object(map)) => keys(map)
            .into_iter()
            .filter_map(|name| {
                let a = &map[name];
                Some((name.clone(), Action { duration: num(a.get("duration"))?, keys: keys_of(a.get("keys"))? }))
            })
            .collect(),
        _ => Vec::new(),
    };
    if ambient.is_none() && actions.is_empty() {
        return None;
    }
    Some(Rc::new(Moves { ambient, actions }))
}

/// Where the element is from each percentage of the loop on, as the CSS
/// steps it: the keys, then `end` at `duration`, whole pixels at a time.
pub(crate) fn runs(keys: &[Key], end: [f64; 2], duration: f64) -> Vec<(f64, [f64; 2])> {
    let mut all: Vec<Key> = keys.to_vec();
    all.push(Key { t: duration, moved: end });
    let mut out: Vec<(f64, [f64; 2])> = Vec::new();
    let mut start = |at: f64, moved: [f64; 2]| {
        if out.last().is_none_or(|last| last.1 != moved) {
            out.push((at, moved));
        }
    };
    for pair in all.windows(2) {
        let (key, next) = (&pair[0], &pair[1]);
        let (from, to) = (pct_of(key.t, duration), pct_of(next.t, duration));
        if to <= from {
            continue;
        }
        let (dx, dy) = (next.moved[0] - key.moved[0], next.moved[1] - key.moved[1]);
        let n = steps_for(dx, dy);
        if n == 0.0 {
            start(from, key.moved);
            continue;
        }
        let mut k = 0.0;
        while k < n {
            start(from + ((to - from) * k) / n, [key.moved[0] + (dx / n) * k, key.moved[1] + (dy / n) * k]);
            k += 1.0;
        }
    }
    out
}

pub(crate) fn at_percent(runs: &[(f64, [f64; 2])], p: f64) -> [f64; 2] {
    let mut found = runs.first().map_or([0.0, 0.0], |r| r.1);
    for &(at, moved) in runs {
        if at > p {
            break;
        }
        found = moved;
    }
    found
}

/// Where an ambient loop has its element at `time` ms (time 0 is `offset`
/// ms into the loop, as the CSS's negative delay plays it).
pub fn ambient_at(a: &Ambient, time: f64) -> [f64; 2] {
    let d = a.duration;
    let t = (((time + a.offset) % d) + d) % d;
    at_percent(&runs(&a.keys, a.keys[0].moved, d), (t / d) * 100.0)
}

/// Where an action has its element `at` ms after it started, or None once
/// it has ended (or before it starts).
pub fn action_at(a: &Action, at: f64) -> Option<[f64; 2]> {
    if !(0.0..a.duration).contains(&at) {
        return None;
    }
    let last = a.keys.last().map_or([0.0, 0.0], |k| k.moved);
    Some(at_percent(&runs(&a.keys, last, a.duration), (at / a.duration) * 100.0))
}

/// Where an element is at `time`: playing `play` (an action's name, and ms
/// into it) if it has that action and it hasn't ended, else on its ambient
/// loop, else at rest.
pub fn move_at(m: &Moves, time: f64, play: Option<(&str, f64)>) -> [f64; 2] {
    if let Some((name, at)) = play {
        if let Some(moved) = m.actions.iter().find(|(n, _)| n == name).and_then(|(_, a)| action_at(a, at)) {
            return moved;
        }
    }
    m.ambient.as_ref().map_or([0.0, 0.0], |a| ambient_at(a, time))
}

// ---------------------------------------------------------------- validation

fn is_whole(v: Option<&Value>) -> bool {
    v.and_then(Value::as_f64).is_some_and(jsnum::is_integer)
}

fn validate_keys(place: &str, what: &str, keys_value: Option<&Value>, duration: Option<f64>, add: &mut dyn FnMut(String)) {
    validate_key_list(place, &format!("{what}.keys"), keys_value, duration, add);
}

/// Checks a list of `{ t, move }` keys: the first at 0, rising, below `duration`. `list` names the
/// list in each problem (`ambient.keys`, or a state's `states.hop.move`).
pub(crate) fn validate_key_list(place: &str, list: &str, keys_value: Option<&Value>, duration: Option<f64>, add: &mut dyn FnMut(String)) {
    let Some(items) = keys_value.and_then(Value::as_array).filter(|l| !l.is_empty()) else {
        return add(format!("{place}: {list} must be a list of at least one key"));
    };
    let mut last: Option<f64> = None;
    for (i, key) in items.iter().enumerate() {
        let at = format!("{place}: {list}[{i}]");
        let Some(k) = key.as_object() else {
            add(format!("{at}: must be {{ t, move }}"));
            continue;
        };
        for name in keys(k) {
            if name != "t" && name != "move" {
                add(format!("{at}: unknown property '{name}' (a key has only t and move)"));
            }
        }
        let t = k.get("t").and_then(Value::as_f64).filter(|t| jsnum::is_integer(*t));
        match t {
            None => add(format!("{at}: t must be a whole number of ms")),
            Some(t) => {
                if i == 0 && t != 0.0 {
                    add(format!("{at}: the first key must be at t: 0"));
                }
                if last.is_some_and(|l| t <= l) {
                    add(format!("{at}: t must come after the key before it"));
                }
                if duration.is_some_and(|d| t >= d) || t < 0.0 {
                    add(format!("{at}: t must be from 0 to below the duration"));
                }
                last = Some(t);
            }
        }
        let moved = k.get("move");
        if !moved.and_then(Value::as_array).is_some_and(|m| m.len() == 2 && m.iter().all(|v| is_whole(Some(v)))) {
            add(format!("{at}: move must be two whole numbers"));
        }
    }
}

fn validate_duration(place: &str, what: &str, d: Option<&Value>, add: &mut dyn FnMut(String)) -> Option<f64> {
    match d.and_then(Value::as_f64).filter(|d| jsnum::is_integer(*d) && (1.0..=MAX_DURATION).contains(d)) {
        Some(d) => Some(d),
        None => {
            add(format!("{place}: {what}.duration must be a whole number of ms from 1 to 600000"));
            None
        }
    }
}

/// Checks an element's `ambient` and `actions` (D6). `place` names the item
/// or layer in each problem.
pub fn validate(place: &str, element: &Map<String, Value>, add: &mut dyn FnMut(String)) {
    if let Some(a) = element.get("ambient") {
        match a.as_object() {
            None => add(format!("{place}: ambient must be {{ duration, offset?, keys }}")),
            Some(m) => {
                for name in keys(m) {
                    if !["duration", "offset", "keys"].contains(&name.as_str()) {
                        add(format!("{place}: ambient: unknown property '{name}'"));
                    }
                }
                let d = validate_duration(place, "ambient", m.get("duration"), add);
                if let Some(o) = m.get("offset") {
                    if !(is_whole(Some(o)) && o.as_f64().is_some_and(|o| o >= 0.0 && d.is_none_or(|d| o < d))) {
                        add(format!("{place}: ambient.offset must be a whole number of ms from 0 to below the duration"));
                    }
                }
                validate_keys(place, "ambient", m.get("keys"), d, add);
            }
        }
    }
    if let Some(a) = element.get("actions") {
        match a.as_object() {
            None => add(format!("{place}: actions must be an object of named actions")),
            Some(m) => {
                if m.len() > MAX_ACTIONS {
                    add(format!("{place}: at most {MAX_ACTIONS} actions"));
                }
                for name in keys(m) {
                    let what = format!("actions.{name}");
                    if !is_name(name) {
                        add(format!("{place}: action '{name}': names are lowercase words joined by hyphens"));
                    }
                    let Some(action) = m[name].as_object() else {
                        add(format!("{place}: {what} must be {{ duration, keys }}"));
                        continue;
                    };
                    for key in keys(action) {
                        if key != "duration" && key != "keys" {
                            add(format!("{place}: {what}: unknown property '{key}'"));
                        }
                    }
                    let d = validate_duration(place, &what, action.get("duration"), add);
                    validate_keys(place, &what, action.get("keys"), d, add);
                }
            }
        }
    }
}

// ----------------------------------------------------------------------- CSS

pub(crate) fn frames(keys: &[Key], end: [f64; 2], duration: f64) -> String {
    let mut all: Vec<Key> = keys.to_vec();
    all.push(Key { t: duration, moved: end });
    (0..all.len())
        .map(|i| {
            let key = &all[i];
            let n = all.get(i + 1).map_or(0.0, |next| steps_for(next.moved[0] - key.moved[0], next.moved[1] - key.moved[1]));
            let timing = if n != 0.0 { format!(";animation-timing-function:steps({},end)", jsnum::to_string(n)) } else { String::new() };
            format!("{}{{transform:translate({},{}){timing}}}", pct(key.t, duration), px(key.moved[0]), px(key.moved[1]))
        })
        .collect()
}

/// The CSS that plays a scene's elements (D7): element `n` (from 1) has the
/// class `<scope>--a<n>`. An ambient loop is keyframes `<scope>--am-<n>`,
/// its offset a negative delay. An action is keyframes `<scope>--<action>`
/// (its documented `animationName`), played while `data-play` on the
/// `<svg>` or a group around the element names it; it outranks the ambient
/// loop by specificity, and only plays without reduced motion, so under it
/// no `animationend` fires.
pub fn elements_css(scope: &str, elements: &[Rc<Moves>]) -> Vec<String> {
    let at = format!("[data-pixellab=\"{scope}\"]");
    let mut rules = Vec::new();
    let mut plays = Vec::new();
    let mut written: Vec<&str> = Vec::new();
    for (i, m) in elements.iter().enumerate() {
        let class = format!("{scope}--a{}", i + 1);
        if let Some(a) = &m.ambient {
            let name = format!("{scope}--am-{}", i + 1);
            rules.push(format!("@keyframes {name}{{{}}}", frames(&a.keys, a.keys[0].moved, a.duration)));
            let delay = if a.offset != 0.0 { format!(" -{}ms", jsnum::to_string(a.offset)) } else { String::new() };
            rules.push(format!("{at} .{class}{{animation:{name} {}ms step-end{delay} infinite}}", jsnum::to_string(a.duration)));
        }
        for (action, a) in &m.actions {
            let name = format!("{scope}--{action}");
            if !written.contains(&action.as_str()) {
                written.push(action);
                let last = a.keys.last().map_or([0.0, 0.0], |k| k.moved);
                rules.push(format!("@keyframes {name}{{{}}}", frames(&a.keys, last, a.duration)));
            }
            plays.push(format!(
                "{at} [data-play~=\"{action}\"] .{class},{at}[data-play~=\"{action}\"] .{class}{{animation:{name} {}ms step-end 1}}",
                jsnum::to_string(a.duration)
            ));
        }
    }
    if !plays.is_empty() {
        rules.push(format!("@media (prefers-reduced-motion:no-preference){{{}}}", plays.concat()));
    }
    rules
}
