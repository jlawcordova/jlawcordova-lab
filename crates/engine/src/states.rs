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

// ------------------------------------------------------------ items and tracks

/// How many frames a sprite draws: what its frame-loop layers share (the longest, if they differ),
/// else 1 for a still sprite.
pub fn frame_count(obj: &crate::engine::Resolved) -> usize {
    obj.layers
        .iter()
        .map(|l| match l {
            crate::engine::SpriteLayer::Frames { frames, .. } => frames.len(),
            crate::engine::SpriteLayer::Map { .. } => 1,
        })
        .max()
        .unwrap_or(1)
        .max(1)
}

/// A key of a scene track on the percentage scale of its loop: from `pct` of the loop on, a stated
/// item shows `frame` and stands `moved` from where it is placed. Written never later than the
/// time it stands for, so a render at a whole ms agrees with the CSS.
#[derive(Clone, Debug, PartialEq)]
pub struct Stop {
    pub pct: f64,
    pub frame: usize,
    pub moved: [f64; 2],
}

/// What a scene's track makes one object of an item show over the loop (R15, R16): the baked changes
/// of frame and move, which the compiled SVG plays frame for frame and a render at a time reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Keyed {
    pub duration: f64,
    pub stops: Vec<Stop>,
}

impl Keyed {
    /// What shows `t` ms into the scene (the loop repeats).
    pub fn at(&self, t: f64) -> Pose {
        let d = self.duration;
        let p = (((t % d) + d) % d) / d * 100.0;
        let stop = self.stops.iter().rev().find(|s| s.pct <= p + 1e-9).or(self.stops.first());
        stop.map_or(Pose { frame: 0, moved: [0.0, 0.0] }, |s| Pose { frame: s.frame, moved: s.moved })
    }
}

/// An item whose object has states: the states, the one it starts in, how many frames it draws and,
/// when its scene track keys states, what that track makes it show.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemStates {
    pub states: Rc<States>,
    pub start: String,
    /// How many ms before the scene's time 0 it entered `start`, for a render partway through a keyed track.
    pub since: f64,
    pub frames: usize,
    pub keyed: Option<Rc<Keyed>>,
}

impl ItemStates {
    /// Whether two items play the same, so they share a class and its rules.
    pub fn same_play(&self, other: &ItemStates) -> bool {
        self.states == other.states && self.start == other.start && self.frames == other.frames && self.keyed == other.keyed
    }

    /// What it shows `t` ms into the scene.
    pub fn pose_at(&self, t: f64) -> Pose {
        match &self.keyed {
            Some(k) => k.at(t),
            None => at(&self.states, &run(&self.states, &self.start), t - self.since),
        }
    }

    /// Every place its states can put it, relative to where it is drawn: for the box a scene fits to.
    pub fn moves(&self) -> Vec<[i32; 2]> {
        let mut out = vec![[0, 0]];
        out.extend(self.states.list.iter().flat_map(|s| s.moves.iter()).map(|k| [k.moved[0] as i32, k.moved[1] as i32]));
        out
    }
}

/// One stretch of a scene track: from `from` to `to` the item shows `pose` and is in `state` (None
/// when the object has no states), which it entered at `entered`.
#[derive(Clone, Debug, PartialEq)]
pub struct Seg {
    pub from: f64,
    pub to: f64,
    pub pose: String,
    pub state: Option<String>,
    pub entered: f64,
}

/// Whether a track has a key that changes the state.
pub fn keys_states(track: &Value) -> bool {
    track.get("keys").and_then(Value::as_array).is_some_and(|keys| keys.iter().any(|k| k.get("state").is_some_and(Value::is_string)))
}

/// The state an item is in over a scene's loop, from its track's keys (R15). A key with `state`
/// enters it from its first frame. A key that poses the item as another object, with no `state`,
/// keeps the state's name and timing if the new object has a state of that name, and otherwise
/// enters that object's start. The first key without a state leaves the item in the state it
/// starts in, `own` for the object it is placed as.
pub fn timeline(track: &Value, object: &str, own: Option<&str>, duration: f64, states_of: &dyn Fn(&str) -> Option<Rc<States>>) -> Vec<Seg> {
    let filled = crate::motion::filled_keys(track, object);
    let raw = track.get("keys").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut out: Vec<Seg> = Vec::new();
    let mut name: Option<String> = None;
    let mut entered = 0.0;
    let mut before: Option<&str> = None;
    for (i, f) in filled.iter().enumerate() {
        let states = states_of(&f.pose);
        let keyed = raw.get(i).and_then(|k| k.get("state")).and_then(Value::as_str);
        if let Some(s) = keyed {
            name = Some(s.to_string());
            entered = f.t;
        } else if i == 0 {
            name = states.as_ref().map(|s| {
                let start = if f.pose == object { own.filter(|o| s.has(o)).unwrap_or(&s.start) } else { &s.start };
                start.to_string()
            });
        } else if before != Some(f.pose.as_str()) {
            match (&name, &states) {
                (Some(n), Some(s)) if s.has(n) => {}
                (_, Some(s)) => {
                    name = Some(s.start.clone());
                    entered = f.t;
                }
                (_, None) => name = None,
            }
        }
        before = Some(&f.pose);
        let to = filled.get(i + 1).map_or(duration, |next| next.t);
        out.push(Seg { from: f.t, to, pose: f.pose.clone(), state: name.clone(), entered });
    }
    out
}

/// A percentage of a loop, as the compiled CSS writes it: never later than the time it stands for.
fn pct_floor(t: f64, d: f64) -> f64 {
    (((t / d) * 100.0 * 1000.0 + 1e-6).floor() / 1000.0).clamp(0.0, 99.999)
}

fn pct_text(t: f64, d: f64) -> String {
    format!("{}%", jsnum::to_string(pct_floor(t, d)))
}

/// What a track makes `object` show over the scene's loop, baked: None when the track's keys don't
/// change a state, or the object has no states or is never shown (R16).
pub fn schedule(
    track: &Value,
    item_object: &str,
    own: Option<&str>,
    duration: f64,
    object: &str,
    states_of: &dyn Fn(&str) -> Option<Rc<States>>,
) -> Option<Keyed> {
    if !keys_states(track) {
        return None;
    }
    let states = states_of(object)?;
    let mut list: Vec<(f64, Pose)> = Vec::new();
    for seg in timeline(track, item_object, own, duration, states_of).iter().filter(|s| s.pose == object) {
        let Some(name) = seg.state.as_deref() else { continue };
        let r = run(&states, name);
        list.push((seg.from, at(&states, &r, seg.from - seg.entered)));
        for c in changes(&states, &r, Some(seg.to - seg.entered)) {
            let t = seg.entered + c.t;
            if t > seg.from && t < seg.to {
                list.push((t, Pose { frame: c.frame, moved: c.moved }));
            }
        }
    }
    let first = list.first()?.1;
    let mut stops: Vec<Stop> = vec![Stop { pct: 0.0, frame: first.frame, moved: first.moved }];
    for (t, p) in list {
        let pct = pct_floor(t, duration);
        let last = stops.last_mut().expect("a stop");
        if pct == last.pct {
            (last.frame, last.moved) = (p.frame, p.moved);
        } else if (last.frame, last.moved) != (p.frame, p.moved) {
            stops.push(Stop { pct, frame: p.frame, moved: p.moved });
        }
    }
    Some(Keyed { duration, stops })
}

// ----------------------------------------------------------------------- CSS

/// The CSS a scene's stated items play (R9 to R16, D2): the rules for the scene's style, and the
/// rules that go in its reduced-motion block.
pub struct Css {
    pub rules: Vec<String>,
    pub reduced: Vec<String>,
}

/// `opacity` stops showing frame `k`, over `d` ms, from a list of (time, frame) where the frame
/// changes. `end` adds a 100% stop (what an animation that fills forwards holds).
fn opacity_frames(stops: &[(f64, usize)], k: usize, d: f64, end: Option<usize>) -> String {
    let mut out: Vec<(String, bool)> = Vec::new();
    for &(t, frame) in stops {
        let on = frame == k;
        if out.last().is_none_or(|last| last.1 != on) {
            out.push((pct_text(t, d), on));
        }
    }
    if let Some(frame) = end {
        out.push(("100%".to_string(), frame == k));
    }
    out.iter().map(|(p, on)| format!("{p}{{opacity:{}}}", u8::from(*on))).collect()
}

/// `transform` stops from a list of (time, move), over `d` ms; `end` adds the 100% stop.
fn transform_frames(stops: &[(f64, [f64; 2])], d: f64, end: Option<[f64; 2]>) -> String {
    let mut out: Vec<(String, [f64; 2])> = Vec::new();
    for &(t, moved) in stops {
        if out.last().is_none_or(|last| last.1 != moved) {
            out.push((pct_text(t, d), moved));
        }
    }
    if let Some(moved) = end {
        out.push(("100%".to_string(), moved));
    }
    out.iter().map(|(p, m)| format!("{p}{{transform:translate({},{})}}", crate::motion::px(m[0]), crate::motion::px(m[1]))).collect()
}

/// What plays for one state of an item (the frames' and the move's animations), as CSS values.
struct StateAnimation {
    keyframes: Vec<String>,
    /// The `animation` value of each frame group; None when the state never shows it.
    frames: Vec<Option<String>>,
    /// The element's `animation` value: its move and the end event; None when it has neither.
    element: Option<String>,
    /// The frame it shows when it doesn't move (R14).
    settle: usize,
}

/// How long until a one-shot state says it has ended, in ms (R12): when it hands over, or when its
/// last frame starts. At least 1, so the animation has a length.
fn end_ms(s: &State) -> f64 {
    match s.end {
        End::Then(_) => s.length(),
        _ => (s.length() - s.ms).max(1.0),
    }
}

fn state_animation(scope: &str, n: usize, states: &States, state: &State, count: usize) -> StateAnimation {
    let r = run(states, &state.name);
    let from = r.settle.from();
    let all = changes(states, &r, None);
    let finite: Vec<Change> = all.iter().filter(|c| c.t < from).copied().collect();
    let after = at(states, &r, from);
    let (looping, period) = match r.settle {
        Settle::Loop { period, .. } => (true, period),
        Settle::Hold { .. } => (false, 0.0),
    };
    // The settle loop on its own clock: where it starts, then each change inside its first pass.
    let mut cycle: Vec<Change> = Vec::new();
    if looping {
        cycle.push(Change { t: 0.0, frame: after.frame, moved: after.moved });
        cycle.extend(all.iter().filter(|c| c.t > from).map(|c| Change { t: c.t - from, ..*c }));
    }
    let name = &state.name;
    let ms = |v: f64| jsnum::to_string(v);
    let delay = if from > 0.0 { format!(" {}ms", ms(from)) } else { String::new() };
    let mut keyframes = Vec::new();
    let mut frames: Vec<Option<String>> = vec![None; count];
    if count > 1 {
        for (k, slot) in frames.iter_mut().enumerate() {
            let mut parts: Vec<String> = Vec::new();
            if from > 0.0 && (finite.iter().any(|c| c.frame == k) || (!looping && after.frame == k)) {
                let id = format!("{scope}--s{n}-{name}-f{k}");
                let stops: Vec<(f64, usize)> = finite.iter().map(|c| (c.t, c.frame)).collect();
                keyframes.push(format!("@keyframes {id}{{{}}}", opacity_frames(&stops, k, from, Some(after.frame))));
                parts.push(format!("{id} {}ms step-end 1 forwards", ms(from)));
            }
            if looping && cycle.iter().any(|c| c.frame == k) {
                let id = format!("{scope}--s{n}-{name}-f{k}-l");
                let stops: Vec<(f64, usize)> = cycle.iter().map(|c| (c.t, c.frame)).collect();
                keyframes.push(format!("@keyframes {id}{{{}}}", opacity_frames(&stops, k, period, None)));
                parts.push(format!("{id} {}ms step-end{delay} infinite", ms(period)));
            }
            if !parts.is_empty() {
                *slot = Some(parts.join(","));
            }
        }
    }
    let mut element: Vec<String> = Vec::new();
    if from > 0.0 && (finite.iter().any(|c| c.moved != [0.0, 0.0]) || (!looping && after.moved != [0.0, 0.0])) {
        let id = format!("{scope}--s{n}-{name}-m");
        let stops: Vec<(f64, [f64; 2])> = finite.iter().map(|c| (c.t, c.moved)).collect();
        keyframes.push(format!("@keyframes {id}{{{}}}", transform_frames(&stops, from, Some(after.moved))));
        element.push(format!("{id} {}ms step-end 1 forwards", ms(from)));
    }
    if looping && cycle.iter().any(|c| c.moved != [0.0, 0.0]) {
        let id = format!("{scope}--s{n}-{name}-ml");
        let stops: Vec<(f64, [f64; 2])> = cycle.iter().map(|c| (c.t, c.moved)).collect();
        keyframes.push(format!("@keyframes {id}{{{}}}", transform_frames(&stops, period, None)));
        element.push(format!("{id} {}ms step-end{delay} infinite", ms(period)));
    }
    if state.plays_once() {
        element.push(format!("{scope}--{name} {}ms linear 1", ms(end_ms(state))));
    }
    StateAnimation { keyframes, frames, element: Some(element.join(",")).filter(|e| !e.is_empty()), settle: after.frame }
}

/// The selectors that make state `s` apply to the element with `class`: under a group, or on the
/// root, that has it in `data-state` (R10) or, for a state that plays once, in `data-play` (R9).
fn selectors(at: &str, s: &State, class: &str) -> String {
    let attrs: &[&str] = if s.plays_once() { &["data-state", "data-play"] } else { &["data-state"] };
    attrs
        .iter()
        .flat_map(|a| [format!("{at} [{a}~=\"{}\"] .{class}", s.name), format!("{at}[{a}~=\"{}\"] .{class}", s.name)])
        .collect::<Vec<_>>()
        .join(",")
}

/// The CSS for the scene's stated items, element `n` (from 1) being `items[n - 1]`, with the
/// class `<scope>--s<n>` and a frame group `<scope>--s<n>-f<k>` for each frame it draws (R9 to R16).
pub fn css(scope: &str, items: &[Rc<ItemStates>]) -> Css {
    let at = format!("[data-pixellab=\"{scope}\"]");
    let mut defs: Vec<String> = Vec::new();
    let mut play: Vec<String> = Vec::new();
    let mut reduced: Vec<String> = Vec::new();
    // The end event's keyframes are the same for every state of a name, so each is written once.
    let mut ended: Vec<&str> = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let n = i + 1;
        let class = format!("{scope}--s{n}");
        let group = |k: usize| format!("{scope}--s{n}-f{k}");
        let count = item.frames;
        if count > 1 {
            defs.push(format!("{}{{opacity:0}}", (0..count).map(|k| format!("{at} .{}", group(k))).collect::<Vec<_>>().join(",")));
        }
        if let Some(keyed) = &item.keyed {
            // R16: the scene's keys, baked over its loop, frame for frame; no `data-state` reaches this item (R17).
            let d = keyed.duration;
            let run_name = |id: &str| format!("{id} {}ms step-end infinite", jsnum::to_string(d));
            if count > 1 {
                for k in 0..count {
                    let stops: Vec<String> = {
                        let mut out: Vec<(String, bool)> = Vec::new();
                        for s in &keyed.stops {
                            let on = s.frame == k;
                            if out.last().is_none_or(|l| l.1 != on) {
                                out.push((format!("{}%", jsnum::to_string(s.pct)), on));
                            }
                        }
                        out.iter().map(|(p, on)| format!("{p}{{opacity:{}}}", u8::from(*on))).collect()
                    };
                    if keyed.stops.iter().any(|s| s.frame == k) {
                        defs.push(format!("@keyframes {}{{{}}}", group(k), stops.concat()));
                        play.push(format!("{at} .{}{{animation:{}}}", group(k), run_name(&group(k))));
                    }
                }
            }
            if keyed.stops.iter().any(|s| s.moved != [0.0, 0.0]) {
                let id = format!("{class}-m");
                let frames: String = keyed
                    .stops
                    .iter()
                    .map(|s| {
                        format!(
                            "{}%{{transform:translate({},{})}}",
                            jsnum::to_string(s.pct),
                            crate::motion::px(s.moved[0]),
                            crate::motion::px(s.moved[1])
                        )
                    })
                    .collect();
                defs.push(format!("@keyframes {id}{{{frames}}}"));
                play.push(format!("{at} .{class}{{animation:{}}}", run_name(&id)));
            }
            if count > 1 {
                reduced.push(format!("{at} .{}{{opacity:1}}", group(keyed.stops[0].frame)));
            }
            continue;
        }
        let animations: Vec<StateAnimation> = item.states.list.iter().map(|s| state_animation(scope, n, &item.states, s, count)).collect();
        for (s, a) in item.states.list.iter().zip(&animations) {
            defs.extend(a.keyframes.iter().cloned());
            if s.plays_once() && !ended.contains(&s.name.as_str()) {
                ended.push(&s.name);
                defs.push(format!("@keyframes {scope}--{}{{from{{--pl-end:0}}to{{--pl-end:1}}}}", s.name));
            }
            // Every frame group and the element are set under each state, so one state's animations never carry into another's.
            let mut stopped: Vec<String> = Vec::new();
            if count > 1 {
                for (k, frame) in a.frames.iter().enumerate() {
                    match frame {
                        Some(value) => play.push(format!("{}{{animation:{value}}}", selectors(&at, s, &group(k)))),
                        None => stopped.push(selectors(&at, s, &group(k))),
                    }
                }
            }
            if !stopped.is_empty() {
                play.push(format!("{}{{animation:none}}", stopped.join(",")));
            }
            play.push(format!("{}{{animation:{}}}", selectors(&at, s, &class), a.element.as_deref().unwrap_or("none")));
            if count > 1 {
                for k in 0..count {
                    reduced.push(format!("{}{{opacity:{}}}", selectors(&at, s, &group(k)), u8::from(k == a.settle)));
                }
            }
        }
        // With no attribute, an item plays the state it starts in.
        if let Some(index) = item.states.index_of(&item.start) {
            let a = &animations[index];
            if count > 1 {
                for (k, frame) in a.frames.iter().enumerate() {
                    if let Some(value) = frame {
                        play.push(format!("{at} .{}{{animation:{value}}}", group(k)));
                    }
                }
            }
            if let Some(value) = &a.element {
                play.push(format!("{at} .{class}{{animation:{value}}}"));
            }
            if count > 1 {
                reduced.push(format!("{at} .{}{{opacity:1}}", group(a.settle)));
            }
        }
    }
    let mut rules = defs;
    if !play.is_empty() {
        rules.push(format!("@media (prefers-reduced-motion:no-preference){{{}}}", play.concat()));
    }
    Css { rules, reduced }
}
