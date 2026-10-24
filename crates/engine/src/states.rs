//! States (docs/intents/2026-10-object-states/spec.md, R1 to R16, D1, D2). A sprite can have named
//! states: a run of its frames at one frame length, plus a move of the whole sprite, that either
//! loops, or plays once and then holds its last frame or hands over to another state. A placed
//! item is in one state; a page switches it with `data-state`, and a scene's motion key can
//! change it at a time. This module parses and checks a sprite's `start` and `states`, works out
//! what an item shows at a time after entering a state (the run, R6), and writes the CSS that
//! plays them. Everything an app, a render or a video shows comes from the one run.

use std::rc::Rc;

use serde_json::{Map, Value};

use crate::ambient::{self, Key};
use crate::engine::is_name;
use crate::jsnum;
use crate::serialize::keys;

/// The most states one sprite may have (R1).
pub const MAX_STATES: usize = 32;
/// The most frames one state may play.
pub const MAX_FRAMES: usize = 256;
/// The shortest and longest a frame shows, in ms.
pub const MIN_MS: f64 = 20.0;
pub const MAX_MS: f64 = 10_000.0;
/// The longest a state may last, in ms.
pub const MAX_LENGTH: f64 = 600_000.0;

const STATE_PROPS: [&str; 5] = ["frames", "ms", "loop", "then", "move"];

/// How a state ends: it loops, it plays once and holds its last frame, or it plays once and hands
/// over to another state.
#[derive(Clone, Debug, PartialEq)]
pub enum End {
    Loop,
    Hold,
    Then(String),
}

/// A state: the sprite's frames it plays, each shown for `ms`, and the move of the whole sprite
/// (ambient-style keys over the state's length, empty when it doesn't move).
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub name: String,
    pub frames: Vec<usize>,
    pub ms: f64,
    pub end: End,
    pub moves: Vec<Key>,
}

impl State {
    /// How long the state lasts (one pass over its frames), in ms.
    pub fn length(&self) -> f64 {
        self.frames.len() as f64 * self.ms
    }

    /// Whether it plays once (a hold or a hand-over), as against looping.
    pub fn plays_once(&self) -> bool {
        self.end != End::Loop
    }

    /// The frame it shows `t` ms into one pass, clamped to its last.
    pub fn frame_at(&self, t: f64) -> usize {
        let i = (t / self.ms).floor().max(0.0) as usize;
        self.frames[i.min(self.frames.len() - 1)]
    }
}

/// A sprite's states in the order the document lists them, and the one it starts in. Parsed once
/// per sprite, as `ambient::moves_of` is for moves.
#[derive(Clone, Debug, PartialEq)]
pub struct States {
    pub start: String,
    pub list: Vec<State>,
}

fn whole(v: Option<&Value>) -> Option<f64> {
    v.and_then(Value::as_f64).filter(|n| jsnum::is_integer(*n))
}

impl State {
    /// A state from its source; None when it is malformed (validation names why).
    fn parse(name: &str, v: &Value) -> Option<State> {
        let o = v.as_object()?;
        let frames: Vec<usize> =
            o.get("frames")?.as_array()?.iter().map(|f| whole(Some(f)).filter(|n| *n >= 0.0).map(|n| n as usize)).collect::<Option<_>>()?;
        if frames.is_empty() {
            return None;
        }
        let ms = whole(o.get("ms")).filter(|ms| *ms > 0.0)?;
        let end = match o.get("loop") {
            Some(Value::Bool(false)) => match o.get("then").and_then(Value::as_str) {
                Some(next) => End::Then(next.to_string()),
                None => End::Hold,
            },
            _ => End::Loop,
        };
        let moves = match o.get("move") {
            Some(m) => ambient::keys_of(Some(m))?,
            None => Vec::new(),
        };
        Some(State { name: name.to_string(), frames, ms, end, moves })
    }
}

impl States {
    /// A sprite's states from its source (`start`, `states`); None when it has none. A malformed
    /// state is left out, and a `start` that names nothing falls back to the first state.
    pub fn of(source: &Value) -> Option<Rc<States>> {
        let map = source.get("states")?.as_object()?;
        let list: Vec<State> = keys(map).into_iter().filter_map(|name| State::parse(name, &map[name])).collect();
        let first = list.first()?.name.clone();
        let start =
            source.get("start").and_then(Value::as_str).filter(|s| list.iter().any(|st| st.name == *s)).map_or(first, str::to_string);
        Some(Rc::new(States { start, list }))
    }

    pub fn get(&self, name: &str) -> Option<&State> {
        self.list.iter().find(|s| s.name == name)
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.list.iter().position(|s| s.name == name)
    }

    pub fn has(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// The state names, in order.
    pub fn names(&self) -> Vec<&str> {
        self.list.iter().map(|s| s.name.as_str()).collect()
    }

    /// The state an item starts in: its own `state` if it names one of these, else `start`.
    pub fn first<'a>(&'a self, own: Option<&str>) -> &'a State {
        own.and_then(|n| self.get(n)).or_else(|| self.get(&self.start)).unwrap_or(&self.list[0])
    }
}

// ---------------------------------------------------------------- validation

/// Checks a sprite's `start` and `states` (R7). `frame_count` is how many frames the sprite has:
/// what its frame-loop layers share, else 1. `add` gets each problem, with the file put in front.
pub fn validate(o: &Map<String, Value>, frame_count: usize, add: &mut dyn FnMut(String)) {
    let Some(states) = o.get("states") else {
        if o.contains_key("start") {
            add("start names a state, but there are no states".into());
        }
        return;
    };
    let Some(map) = states.as_object().filter(|m| !m.is_empty()) else {
        return add("states must be an object of named states (leave it out for none)".into());
    };
    if map.len() > MAX_STATES {
        add(format!("at most {MAX_STATES} states, not {}", map.len()));
    }
    match o.get("start") {
        None => add(format!("start is missing: name the state it starts in ({})", state_list(map))),
        Some(start) => match start.as_str() {
            Some(s) if map.contains_key(s) => {}
            _ => add(format!("start {} is not one of its states ({})", crate::engine::q(Some(start)), state_list(map))),
        },
    }
    // Names are the object's keys, so one can't appear twice: a second would replace the first.
    for name in keys(map) {
        let place = format!("states.{name}");
        if !is_name(name) {
            add(format!("{place}: names are lowercase words joined by hyphens"));
        }
        let Some(s) = map[name].as_object() else {
            add(format!("{place}: must be {{ frames, ms, loop?, then?, move? }}"));
            continue;
        };
        for key in keys(s) {
            if !STATE_PROPS.contains(&key.as_str()) {
                add(format!("{place}: unknown property '{key}'"));
            }
        }
        match s.get("frames").and_then(Value::as_array).filter(|f| !f.is_empty() && f.len() <= MAX_FRAMES) {
            None => add(format!("{place}: frames must be a list of 1 to {MAX_FRAMES} frame numbers")),
            Some(list) => {
                for f in list {
                    match whole(Some(f)) {
                        Some(n) if n >= 0.0 && (n as usize) < frame_count => {}
                        _ => add(format!(
                            "{place}: frame {} is not a frame of the sprite (it has {}, from 0 to {})",
                            crate::engine::js_string(Some(f)),
                            frame_count,
                            frame_count - 1
                        )),
                    }
                }
            }
        }
        let ms = whole(s.get("ms")).filter(|ms| (MIN_MS..=MAX_MS).contains(ms));
        if ms.is_none() {
            add(format!("{place}: ms must be a whole number from {MIN_MS} to {MAX_MS} (how long each frame shows)"));
        }
        let frames = s.get("frames").and_then(Value::as_array).map_or(0, Vec::len);
        let length = ms.map(|ms| ms * frames as f64);
        if length.is_some_and(|l| l > MAX_LENGTH) {
            add(format!("{place}: lasts {} ms; at most {MAX_LENGTH}", jsnum::to_string(length.unwrap_or(0.0))));
        }
        let looping = match s.get("loop") {
            None => true,
            Some(Value::Bool(false)) => false,
            Some(_) => {
                add(format!("{place}: loop must be false (leave it out to loop)"));
                true
            }
        };
        if let Some(then) = s.get("then") {
            match then.as_str() {
                _ if looping => add(format!("{place}: a looping state has no then; set loop to false to play once and hand over")),
                Some(next) if map.contains_key(next) => {}
                _ => add(format!("{place}: then {} is not one of the states ({})", crate::engine::q(Some(then)), state_list(map))),
            }
        }
        if let Some(moves) = s.get("move") {
            ambient::validate_key_list(&place, "move", Some(moves), length.map(|l| l.min(MAX_LENGTH)), add);
        }
    }
}

fn state_list(map: &Map<String, Value>) -> String {
    keys(map).into_iter().map(String::as_str).collect::<Vec<_>>().join(", ")
}

/// A scene item's or key's `state` against the states of the object it shows (R7): says what is
/// wrong, or None.
pub fn state_problem(object: &str, states: Option<&States>, name: &Value) -> Option<String> {
    let Some(wanted) = name.as_str() else {
        return Some("state must be the name of one of the object's states".into());
    };
    match states {
        None => Some(format!("state '{wanted}' is no state of {object}, which has no states")),
        Some(s) if !s.has(wanted) => Some(format!("state '{wanted}' is no state of {object} (it has {})", s.names().join(", "))),
        Some(_) => None,
    }
}
