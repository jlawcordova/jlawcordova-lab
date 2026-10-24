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

// -------------------------------------------------------------------- the run

/// One state of a run: it plays from `from` to `to` ms after entering the first.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub state: String,
    pub from: f64,
    pub to: f64,
}

/// Where a run ends up: it loops, or it holds.
#[derive(Clone, Debug, PartialEq)]
pub enum Settle {
    /// From `from` on, the segments that start there repeat every `period` ms. A looping state is one
    /// segment; a chain of one-shot states that leads back to itself is the whole cycle.
    Loop { from: f64, period: f64 },
    /// From `from` on, the last state's last frame shows and its move holds its last key.
    Hold { from: f64, frame: usize, moved: [f64; 2] },
}

impl Settle {
    pub fn from(&self) -> f64 {
        match self {
            Settle::Loop { from, .. } | Settle::Hold { from, .. } => *from,
        }
    }
}

/// What a state plays after it is entered (R6): the states `then` leads through, then where it
/// settles. It is the same every time.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub segments: Vec<Segment>,
    pub settle: Settle,
}

/// What an item shows `t` ms after entering a state: which frame of the sprite, and how far the
/// whole sprite has moved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub frame: usize,
    pub moved: [f64; 2],
}

/// A time the frame or the move changes, and what shows from then on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Change {
    pub t: f64,
    pub frame: usize,
    pub moved: [f64; 2],
}

/// The run of `state` (R6). A state that isn't one of these plays as if it held frame 0.
pub fn run(states: &States, state: &str) -> Run {
    let mut segments: Vec<Segment> = Vec::new();
    let mut at = 0.0;
    let mut next = Some(state.to_string());
    while let Some(name) = next.take() {
        let Some(s) = states.get(&name) else { break };
        // A state visited again closes the cycle: it repeats from where that state first started.
        if let Some(seen) = segments.iter().find(|g| g.state == name).map(|g| g.from) {
            return Run { segments, settle: Settle::Loop { from: seen, period: at - seen } };
        }
        segments.push(Segment { state: name, from: at, to: at + s.length() });
        at += s.length();
        match &s.end {
            End::Loop => {
                let from = at - s.length();
                return Run { segments, settle: Settle::Loop { from, period: s.length() } };
            }
            End::Hold => return Run { settle: Settle::Hold { from: at, frame: last_frame(s), moved: held_move(s) }, segments },
            End::Then(then) => next = Some(then.clone()),
        }
    }
    // A hand-over to a state that doesn't exist (validation names it): the last state holds.
    match segments.last().and_then(|g| states.get(&g.state)) {
        Some(s) => Run { settle: Settle::Hold { from: at, frame: last_frame(s), moved: held_move(s) }, segments },
        None => Run { segments, settle: Settle::Hold { from: 0.0, frame: 0, moved: [0.0, 0.0] } },
    }
}

fn last_frame(s: &State) -> usize {
    s.frames.last().copied().unwrap_or(0)
}

fn held_move(s: &State) -> [f64; 2] {
    s.moves.last().map_or([0.0, 0.0], |k| k.moved)
}

/// Where a state's move has the sprite from each ms on, a whole pixel at a time, as `ambient` steps
/// an action or a loop: the steps' times in ms after entering the state.
fn steps_of(s: &State) -> Vec<(f64, [f64; 2])> {
    if s.moves.is_empty() {
        return Vec::new();
    }
    let length = s.length();
    // A looping state steps back to its first key over the rest of the loop; a one-shot one holds its last.
    let end = if s.plays_once() { held_move(s) } else { s.moves[0].moved };
    ambient::runs(&s.moves, end, length).into_iter().map(|(percent, moved)| (length * percent / 100.0, moved)).collect()
}

fn move_in(steps: &[(f64, [f64; 2])], t: f64) -> [f64; 2] {
    let mut found = steps.first().map_or([0.0, 0.0], |s| s.1);
    for &(at, moved) in steps {
        if at > t {
            break;
        }
        found = moved;
    }
    found
}

/// What an item shows `t` ms after it enters the state `run` is the run of (R6).
pub fn at(states: &States, run: &Run, t: f64) -> Pose {
    let mut t = t.max(0.0);
    match &run.settle {
        Settle::Loop { from, period } if t >= *from && *period > 0.0 => t = from + ((t - from) % period),
        Settle::Hold { from, frame, moved } if t >= *from => return Pose { frame: *frame, moved: *moved },
        _ => {}
    }
    let Some(seg) = run.segments.iter().rev().find(|g| g.from <= t).or(run.segments.first()) else {
        return Pose { frame: 0, moved: [0.0, 0.0] };
    };
    let Some(s) = states.get(&seg.state) else { return Pose { frame: 0, moved: [0.0, 0.0] } };
    let inside = (t - seg.from).min(s.length());
    Pose { frame: s.frame_at(inside), moved: move_in(&steps_of(s), inside) }
}

/// The times a segment's frame or move can change, `shift` ms into the run.
fn marks(states: &States, seg: &Segment, shift: f64, out: &mut Vec<f64>) {
    let Some(s) = states.get(&seg.state) else { return };
    let base = seg.from + shift;
    out.push(base);
    out.extend((1..s.frames.len()).map(|i| base + i as f64 * s.ms));
    out.extend(steps_of(s).into_iter().map(|(at, _)| base + at));
}

/// Every time the frame or the move changes, from 0 up to `horizon` ms: the settle's first pass by
/// default, and a looping settle goes on repeating up to a later one. The first change is at 0.
pub fn changes(states: &States, run: &Run, horizon: Option<f64>) -> Vec<Change> {
    let from = run.settle.from();
    let horizon = horizon.unwrap_or(match run.settle {
        Settle::Loop { from, period } => from + period,
        Settle::Hold { from, .. } => from + 1.0,
    });
    let mut times: Vec<f64> = vec![0.0];
    // The part before the settle plays once.
    for seg in run.segments.iter().filter(|g| g.from < from || matches!(run.settle, Settle::Hold { .. })) {
        marks(states, seg, 0.0, &mut times);
    }
    match run.settle {
        Settle::Loop { period, .. } => {
            let mut shift = 0.0;
            while period > 0.0 && from + shift < horizon {
                for seg in run.segments.iter().filter(|g| g.from >= from) {
                    marks(states, seg, shift, &mut times);
                }
                shift += period;
            }
        }
        Settle::Hold { from, .. } => times.push(from),
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    times.dedup();
    let mut out: Vec<Change> = Vec::new();
    for t in times.into_iter().filter(|t| *t < horizon) {
        // Sampled a hair after the mark: the sum of a segment's start and a step's time isn't exact.
        let pose = at(states, run, t + 1e-7);
        if out.last().is_none_or(|last| last.frame != pose.frame || last.moved != pose.moved) {
            out.push(Change { t, frame: pose.frame, moved: pose.moved });
        }
    }
    out
}
