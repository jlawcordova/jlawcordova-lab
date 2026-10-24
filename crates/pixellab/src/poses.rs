//! Poses become states (docs/intents/2026-10-object-states/spec.md, R37 to R40, D8). A pose family
//! is a base object and the objects its items pose as in a scene's motion (the hero island's
//! `crane-hook`, `crane-hook-drop` and `crane-hook-low`). Merging it gives the base one frame per
//! member, and a state per frame; each key that posed an item of the base gets the matching `state`
//! instead, and with `fold` a run of keys that alternates between poses at one interval becomes one
//! key into a looping state. The plan is worked out here as pure functions over the sources; the
//! command checks that every scene renders the same before it writes anything.

use std::collections::{BTreeMap, HashSet};

use engine::engine::{is_key, is_name, resolve, Resolved, Sources, SpriteLayer, CAPS};
use engine::jsnum::num;
use engine::motion;
use engine::serialize::keys;
use serde_json::{json, Map, Value};

use crate::errors::CommandError;
use crate::render::ScenePixels;

/// The loop name and frame prefix of the merged sprite's frames: a loop name of their own, which
/// never plays by itself, since the sprite now has states.
pub const LOOP: &str = "pf";
pub const PREFIX: &str = "p";
/// The base's own drawing, as a state.
pub const REST: &str = "rest";
/// A state's frame length when it has one frame (R4's still sprite).
const STILL_MS: f64 = 200.0;
/// The fewest keys a run must have to fold (R39).
const FOLD_MIN: usize = 4;

fn refuse(message: impl Into<String>) -> CommandError {
    CommandError::new(message, "refused")
}

/// What `from_poses` does with the scenes.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// Only these scenes (all of them when empty).
    pub scenes: Vec<String>,
    pub fold: bool,
    /// The folded state's name.
    pub name: Option<String>,
}

/// A scene the merge rewrites: its new source, and how many keys it changed and folded.
#[derive(Clone, Debug)]
pub struct SceneChange {
    pub scene: String,
    pub doc: Value,
    pub keys: usize,
    pub folded: usize,
}

/// The whole merge, before it's written.
#[derive(Clone, Debug)]
pub struct Plan {
    pub object: String,
    /// The base's new source.
    pub base: Value,
    /// The family, the base first: frame `k` is member `k`'s drawing.
    pub members: Vec<String>,
    /// Each member's state, in the same order.
    pub states: Vec<String>,
    /// The looping states folding made (name, frames, ms).
    pub folded: Vec<(String, Vec<usize>, f64)>,
    pub scenes: Vec<SceneChange>,
    /// Objects that extended the base, now written out whole so they keep their drawing and don't take on its states.
    pub detached: Vec<(String, Value)>,
}

// ------------------------------------------------------------------ the family

fn scene_names(sources: &Sources, only: &[String]) -> Vec<String> {
    sources.scenes.keys().filter(|n| only.is_empty() || only.contains(n)).cloned().collect()
}

/// Each track of a scene whose item places `object`, by index into `motion.tracks`.
fn tracks_of_object(scene: &Value, object: &str) -> Vec<usize> {
    let ids = motion::items_by_id(scene.get("items").unwrap_or(&Value::Null));
    let Some(tracks) = scene.pointer("/motion/tracks").and_then(Value::as_array) else { return vec![] };
    tracks
        .iter()
        .enumerate()
        .filter(|(_, t)| {
            t.get("item").and_then(Value::as_str).and_then(|id| ids.get(id)).and_then(|i| i.get("object")).and_then(Value::as_str)
                == Some(object)
        })
        .map(|(i, _)| i)
        .collect()
}

/// The base and the objects a `pose` key names on an item of the base, in the order they're first
/// posed (scenes by name, tracks and keys in order).
pub fn family(sources: &Sources, base: &str, only: &[String]) -> Vec<String> {
    let mut out = vec![base.to_string()];
    for name in scene_names(sources, only) {
        let scene = &sources.scenes[&name];
        for i in tracks_of_object(scene, base) {
            for key in scene["motion"]["tracks"][i].get("keys").and_then(Value::as_array).into_iter().flatten() {
                if let Some(p) = key.get("pose").and_then(Value::as_str) {
                    if !out.iter().any(|o| o == p) {
                        out.push(p.to_string());
                    }
                }
            }
        }
    }
    out
}

/// What a state of a pose is called: the pose's name without the base's and its hyphen
/// (`crane-hook-drop` is `drop`), or the whole name when that isn't free.
fn state_name(base: &str, pose: &str, taken: &[String]) -> String {
    let short = pose.strip_prefix(base).and_then(|r| r.strip_prefix('-')).filter(|s| is_name(s));
    let free = |n: &str| n != "clear" && n != "start" && !taken.iter().any(|t| t == n);
    match short {
        Some(s) if free(s) => s.to_string(),
        _ if free(pose) => pose.to_string(),
        _ => (2..).map(|i| format!("{pose}-{i}")).find(|n| free(n)).expect("a free name"),
    }
}

// ------------------------------------------------------------------- the merge

/// One member's drawing: its layers on a grid, in its own map's coordinates.
struct Drawing {
    anchor: [i64; 2],
    width: i64,
    height: i64,
    /// Each layer's rows, and its class.
    layers: Vec<(Vec<Vec<char>>, Option<String>)>,
    keys: Vec<(char, String)>,
}

fn drawing(name: &str, r: &Resolved) -> Result<Drawing, CommandError> {
    if r.block.is_some() {
        return Err(refuse(format!("{name} is a block; only sprites have states")));
    }
    if r.layer_moves.iter().any(Option::is_some) {
        return Err(refuse(format!(
            "{name} has a layer with an ambient loop or actions; make those states first (object states make {name})"
        )));
    }
    let mut layers = Vec::new();
    for layer in &r.layers {
        match layer {
            SpriteLayer::Frames { .. } => {
                return Err(refuse(format!("{name} is a frame loop; a pose has to be one drawing to become one frame")));
            }
            SpriteLayer::Map { class, map } => layers.push((map.iter().map(|row| row.chars().collect()).collect(), class.clone())),
        }
    }
    let keys = r
        .keys
        .iter()
        .filter_map(|(k, c)| {
            let mut chars = k.chars();
            match (chars.next(), chars.next()) {
                (Some(ch), None) if !c.is_empty() => Some((ch, c.clone())),
                _ => None,
            }
        })
        .collect();
    Ok(Drawing { anchor: r.anchor, width: r.width, height: r.height, layers, keys })
}

/// The box every member fits in, around the shared anchor: (left, top, width, height), with the
/// anchor at (0, 0).
fn union_box(drawings: &[Drawing]) -> (i64, i64, i64, i64) {
    let left = drawings.iter().map(|d| -d.anchor[0]).min().unwrap_or(0);
    let top = drawings.iter().map(|d| -d.anchor[1]).min().unwrap_or(0);
    let right = drawings.iter().map(|d| d.width - d.anchor[0]).max().unwrap_or(0);
    let bottom = drawings.iter().map(|d| d.height - d.anchor[1]).max().unwrap_or(0);
    (left, top, right - left, bottom - top)
}

/// The characters a merged key may take, after the ones the members use.
const SPARE: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789#$%&*+=?@";

/// The merged color keys: the base's first, then each member's, renamed where a character already
/// means another color. Returns the keys and, per member, how its characters map.
fn merge_keys(drawings: &[Drawing]) -> Result<(Vec<(char, String)>, Vec<BTreeMap<char, char>>), CommandError> {
    let mut merged: Vec<(char, String)> = Vec::new();
    let mut maps = Vec::new();
    for d in drawings {
        let mut map = BTreeMap::new();
        for (ch, color) in &d.keys {
            let to = match merged.iter().find(|(c, _)| c == ch) {
                Some((_, same)) if same == color => *ch,
                Some(_) => match merged.iter().find(|(_, c)| c == color) {
                    Some((other, _)) => *other,
                    None => {
                        let spare = SPARE
                            .chars()
                            .find(|c| is_key(&c.to_string()) && !merged.iter().any(|(m, _)| m == c))
                            .ok_or_else(|| refuse("the family uses more key characters than a sprite can have"))?;
                        merged.push((spare, color.clone()));
                        spare
                    }
                },
                None => {
                    merged.push((*ch, color.clone()));
                    *ch
                }
            };
            map.insert(*ch, to);
        }
        maps.push(map);
    }
    Ok((merged, maps))
}

/// A layer's rows placed in the union box, with its characters renamed.
fn placed(rows: &[Vec<char>], d: &Drawing, bx: (i64, i64, i64, i64), map: &BTreeMap<char, char>) -> Vec<String> {
    let (left, top, w, h) = bx;
    let (dx, dy) = (-d.anchor[0] - left, -d.anchor[1] - top);
    (0..h)
        .map(|y| {
            (0..w)
                .map(|x| {
                    let (sx, sy) = (x - dx, y - dy);
                    if sx < 0 || sy < 0 {
                        return '.';
                    }
                    let c = rows.get(sy as usize).and_then(|r| r.get(sx as usize)).copied().unwrap_or('.');
                    if c == '.' {
                        '.'
                    } else {
                        map.get(&c).copied().unwrap_or(c)
                    }
                })
                .collect()
        })
        .collect()
}

/// Several layers' rows flattened to one, later layers over earlier ones.
fn flatten(layers: &[Vec<String>]) -> Vec<String> {
    let Some(first) = layers.first() else { return vec![] };
    let mut out: Vec<Vec<char>> = first.iter().map(|r| r.chars().collect()).collect();
    for layer in &layers[1..] {
        for (y, row) in layer.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                if c != '.' {
                    out[y][x] = c;
                }
            }
        }
    }
    out.into_iter().map(|r| r.into_iter().collect()).collect()
}

/// The base's new layers. The layers every member draws the same stay map layers (with their
/// class) when the ones that differ sit together; those become the one frame loop, at their place.
/// Otherwise every member is flattened into the frame loop.
fn merged_layers(drawings: &[Drawing], bx: (i64, i64, i64, i64), maps: &[BTreeMap<char, char>]) -> Result<Vec<Value>, CommandError> {
    let each: Vec<Vec<Vec<String>>> =
        drawings.iter().zip(maps).map(|(d, m)| d.layers.iter().map(|(rows, _)| placed(rows, d, bx, m)).collect()).collect();
    let classes: Vec<Option<String>> = drawings[0].layers.iter().map(|(_, c)| c.clone()).collect();
    let n = each[0].len();
    let same_count = each.iter().all(|layers| layers.len() == n);
    let differs: Vec<usize> =
        if same_count { (0..n).filter(|&i| each.iter().any(|layers| layers[i] != each[0][i])).collect() } else { vec![] };
    let together = !differs.is_empty() && differs.windows(2).all(|w| w[1] == w[0] + 1);
    let frames_layer = |frames: Vec<Vec<String>>| json!({ "loop": LOOP, "prefix": PREFIX, "frames": frames });
    if same_count && together {
        let (a, b) = (differs[0], differs[differs.len() - 1]);
        if let Some(class) = classes[a..=b].iter().flatten().next() {
            return Err(refuse(format!("the poses differ on a layer with class {class}, and a frame loop can't keep a class")));
        }
        let mut out = Vec::new();
        for (i, rows) in each[0].iter().enumerate() {
            if i == a {
                out.push(frames_layer(each.iter().map(|layers| flatten(&layers[a..=b])).collect()));
            } else if i < a || i > b {
                let mut layer = Map::new();
                if let Some(class) = &classes[i] {
                    layer.insert("class".into(), json!(class));
                }
                layer.insert("map".into(), json!(rows));
                out.push(Value::Object(layer));
            }
        }
        return Ok(out);
    }
    for d in drawings {
        if let Some(class) = d.layers.iter().filter_map(|(_, c)| c.as_ref()).next() {
            return Err(refuse(format!("a member has a layer with class {class}, which flattening into one frame loop would lose")));
        }
    }
    Ok(vec![frames_layer(each.iter().map(|layers| flatten(layers)).collect())])
}

fn used_colors(layers: &[Value], keys: &[(char, String)]) -> usize {
    let mut chars: HashSet<char> = HashSet::new();
    for layer in layers {
        let maps: Vec<&Value> = match layer.get("frames").and_then(Value::as_array) {
            Some(frames) => frames.iter().collect(),
            None => layer.get("map").into_iter().collect(),
        };
        for map in maps {
            for row in map.as_array().into_iter().flatten().filter_map(Value::as_str) {
                chars.extend(row.chars().filter(|c| *c != '.'));
            }
        }
    }
    keys.iter().filter(|(c, _)| chars.contains(c)).map(|(_, color)| color.as_str()).collect::<HashSet<_>>().len()
}

/// An object written out whole, without `extends`: its resolved layers and keys, and its base's
/// layer moves, so it draws exactly as before.
fn standalone(sources: &Sources, name: &str) -> Result<Value, CommandError> {
    let r = resolve(sources, name).map_err(|e| refuse(format!("{name} doesn't resolve: {}", e.0)))?;
    let own = &sources.objects[name];
    let mut chain = vec![own];
    let mut at = own;
    while let Some(b) = at.get("extends").and_then(Value::as_str).and_then(|b| sources.objects.get(b)) {
        chain.push(b);
        at = b;
    }
    let root_layers = chain.last().and_then(|o| o.get("layers")).and_then(Value::as_array).cloned().unwrap_or_default();
    let mut out = Map::new();
    for key in ["notes", "category"] {
        if let Some(v) = own.get(key) {
            out.insert(key.into(), v.clone());
        }
    }
    out.insert("kind".into(), json!("sprite"));
    out.insert("anchor".into(), json!([r.anchor[0], r.anchor[1]]));
    if let Some(f) = chain.iter().find_map(|o| o.get("footprint")) {
        out.insert("footprint".into(), f.clone());
    }
    out.insert("keys".into(), Value::Object(r.keys.iter().map(|(k, c)| (k.clone(), json!(c))).collect()));
    let layers: Vec<Value> = r
        .layers
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let mut layer = Map::new();
            match l {
                SpriteLayer::Map { class, map } => {
                    if let Some(c) = class {
                        layer.insert("class".into(), json!(c));
                    }
                    layer.insert("map".into(), json!(map));
                }
                SpriteLayer::Frames { loop_name, prefix, frames } => {
                    layer.insert("loop".into(), json!(loop_name));
                    layer.insert("prefix".into(), json!(prefix));
                    layer.insert("frames".into(), json!(frames));
                }
            }
            for key in ["ambient", "actions"] {
                if let Some(v) = root_layers.get(i).and_then(|rl| rl.get(key)) {
                    layer.insert(key.into(), v.clone());
                }
            }
            Value::Object(layer)
        })
        .collect();
    out.insert("layers".into(), Value::Array(layers));
    Ok(Value::Object(out))
}

/// Every object that extends `base`, directly or through another.
fn extenders(sources: &Sources, base: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    loop {
        let more: Vec<String> = sources
            .objects
            .iter()
            .filter(|(n, o)| {
                !out.contains(n) && o.get("extends").and_then(Value::as_str).is_some_and(|b| b == base || out.iter().any(|x| x == b))
            })
            .map(|(n, _)| n.clone())
            .collect();
        if more.is_empty() {
            return out;
        }
        out.extend(more);
    }
}

// -------------------------------------------------------------------- the keys

fn t_of(key: &Value) -> f64 {
    key.get("t").and_then(Value::as_f64).unwrap_or(0.0)
}

/// A track's keys with each pose of the family as the matching state (R38).
fn rewrite_keys(keys: &mut [Value], members: &[String], states: &[String]) -> usize {
    let mut changed = 0;
    for key in keys.iter_mut() {
        let Some(o) = key.as_object_mut() else { continue };
        let Some(i) = o.get("pose").and_then(Value::as_str).and_then(|p| members.iter().position(|m| m == p)) else { continue };
        o.shift_remove("pose");
        o.insert("state".into(), json!(states[i]));
        changed += 1;
    }
    changed
}

/// A run of keys that only change the state, at one interval, cycling through two or more states:
/// (first key, keys in the run, the cycle of states, the interval).
fn find_run(keys: &[Value], from: usize, single: &[String], duration: f64) -> Option<(usize, usize, Vec<String>, f64)> {
    let only_state = |k: &Value| {
        k.as_object().is_some_and(|o| o.len() == 2 && o.contains_key("t"))
            && k.get("state").and_then(Value::as_str).is_some_and(|s| single.iter().any(|x| x == s))
    };
    for i in from..keys.len() {
        if !only_state(&keys[i]) || i + 1 >= keys.len() || !only_state(&keys[i + 1]) {
            continue;
        }
        let d = t_of(&keys[i + 1]) - t_of(&keys[i]);
        if !(20.0..=10_000.0).contains(&d) || d.fract() != 0.0 {
            continue;
        }
        let mut j = i + 1;
        while j + 1 < keys.len() && only_state(&keys[j + 1]) && t_of(&keys[j + 1]) - t_of(&keys[j]) == d {
            j += 1;
        }
        let names: Vec<String> = keys[i..=j].iter().map(|k| k["state"].as_str().unwrap_or("").to_string()).collect();
        // The run's last state shows until the next key (or the loop's end); a cycle may only fold over
        // what it plays the same, so a last key held longer than the interval stays a key of its own.
        let next = keys.get(j + 1).map_or(duration, t_of);
        let mut end = j;
        if next > t_of(&keys[j]) + d {
            end -= 1;
        }
        let run = &names[..=end - i];
        if run.len() < FOLD_MIN {
            continue;
        }
        let cycle = (2..=run.len() / 2)
            .find(|&p| run.iter().enumerate().all(|(m, s)| *s == run[m % p]) && (0..p).all(|m| run[m] != run[(m + 1) % p]));
        if let Some(cycle) = cycle {
            return Some((i, end - i + 1, run[..cycle].to_vec(), d));
        }
    }
    None
}

/// Folds every run in a track's keys (R39), naming new looping states as it goes. Returns how many
/// runs it folded.
fn fold_keys(
    keys: &mut Vec<Value>,
    frame_of: &dyn Fn(&str) -> usize,
    single: &[String],
    duration: f64,
    folded: &mut Vec<(String, Vec<usize>, f64)>,
    taken: &mut Vec<String>,
    wanted: Option<&str>,
) -> Result<usize, CommandError> {
    let mut count = 0;
    let mut from = 0;
    while let Some((i, len, cycle, d)) = find_run(keys, from, single, duration) {
        let frames: Vec<usize> = cycle.iter().map(|s| frame_of(s)).collect();
        let name = match folded.iter().find(|(_, f, ms)| *f == frames && *ms == d) {
            Some((n, _, _)) => n.clone(),
            None => {
                let n = match wanted.filter(|_| folded.is_empty()) {
                    Some(w) if taken.iter().any(|t| t == w) => {
                        return Err(refuse(format!("--name {w}: the object already has a state {w}")))
                    }
                    Some(w) => w.to_string(),
                    None => state_name("", &format!("{}-loop", cycle[0]), taken),
                };
                taken.push(n.clone());
                folded.push((n.clone(), frames, d));
                n
            }
        };
        let t = t_of(&keys[i]);
        keys.splice(i..i + len, [json!({ "t": num(t), "state": name })]);
        count += 1;
        from = i + 1;
    }
    Ok(count)
}

// ------------------------------------------------------------------- the plan

/// Works out the merge of `base`'s pose family (R37 to R39), or why it can't be done. Nothing is
/// written; the command checks the renders, then writes the plan as one version.
pub fn plan(sources: &Sources, base: &str, options: &Options) -> Result<Plan, CommandError> {
    let doc = sources.objects.get(base).ok_or_else(|| CommandError::new(format!("no object named {base}"), "not_found"))?;
    for scene in &options.scenes {
        if !sources.scenes.contains_key(scene) {
            return Err(CommandError::new(format!("no scene named {scene}"), "not_found"));
        }
    }
    if doc.get("extends").is_some() {
        return Err(refuse(format!("{base} extends another object, and a sprite that extends another can't have states of its own")));
    }
    if doc.get("states").is_some() {
        return Err(refuse(format!("{base} already has states")));
    }
    if let Some(w) = options.name.as_deref() {
        if !is_name(w) || w == "clear" || w == "start" {
            return Err(CommandError::usage(format!(
                "--name {w}: a state's name is lowercase words joined by hyphens, and not clear or start"
            )));
        }
    }
    let members = family(sources, base, &options.scenes);
    if members.len() < 2 {
        return Err(refuse(format!("no scene poses an item of {base} as another object, so it has no pose family")));
    }
    let resolved: Vec<Resolved> = members
        .iter()
        .map(|m| resolve(sources, m).map_err(|e| refuse(format!("{m} doesn't resolve: {}", e.0))))
        .collect::<Result<_, _>>()?;
    let drawings: Vec<Drawing> = members.iter().zip(&resolved).map(|(m, r)| drawing(m, r)).collect::<Result<_, _>>()?;
    let bx = union_box(&drawings);
    let character = resolved[0].character;
    let (max_w, max_h) = if character { (CAPS.character_width, CAPS.character_height) } else { (CAPS.size, CAPS.size) };
    if bx.2 > max_w || bx.3 > max_h {
        return Err(refuse(format!("the merged sprite would be {}×{}, past the {max_w}×{max_h} a sprite can be", bx.2, bx.3)));
    }
    let (merged, maps) = merge_keys(&drawings)?;
    let layers = merged_layers(&drawings, bx, &maps)?;
    let colors = used_colors(&layers, &merged);
    if colors > CAPS.colors {
        return Err(refuse(format!("the merged sprite would use {colors} colors, past the {} a sprite can use", CAPS.colors)));
    }

    // A state per member: rest for the base's own drawing, then each pose's.
    let mut names: Vec<String> = vec![REST.to_string()];
    for m in &members[1..] {
        let n = state_name(base, m, &names);
        names.push(n);
    }
    let mut taken = names.clone();
    let frame_of = |s: &str| names.iter().position(|n| n == s).unwrap_or(0);

    // The keys, in every scene that poses an item of the base.
    let mut folded: Vec<(String, Vec<usize>, f64)> = Vec::new();
    let mut scenes = Vec::new();
    for name in scene_names(sources, &options.scenes) {
        let scene = &sources.scenes[&name];
        let tracks = tracks_of_object(scene, base);
        if tracks.is_empty() {
            continue;
        }
        let duration = scene.pointer("/motion/duration").and_then(Value::as_f64).unwrap_or(0.0);
        let mut doc = scene.clone();
        let (mut changed, mut folds) = (0, 0);
        for i in tracks {
            let Some(keys) = doc["motion"]["tracks"][i].get_mut("keys").and_then(Value::as_array_mut) else { continue };
            changed += rewrite_keys(keys, &members, &names);
            if options.fold {
                folds += fold_keys(keys, &frame_of, &names, duration, &mut folded, &mut taken, options.name.as_deref())?;
            }
        }
        if changed > 0 {
            scenes.push(SceneChange { scene: name, doc, keys: changed, folded: folds });
        }
    }
    if options.fold && options.name.is_some() && folded.is_empty() {
        return Err(refuse("--name names the folded state, but no run of keys alternates at one interval to fold"));
    }

    // The base: the union box's anchor, the merged keys and layers, and its states.
    let mut out = Map::new();
    for (k, v) in doc.as_object().into_iter().flatten() {
        match k.as_str() {
            "anchor" => {
                out.insert(k.clone(), json!([-bx.0, -bx.1]));
            }
            "keys" => {
                out.insert(k.clone(), Value::Object(merged.iter().map(|(c, color)| (c.to_string(), json!(color))).collect()));
            }
            "layers" => {
                out.insert(k.clone(), Value::Array(layers.clone()));
            }
            "rows" => {}
            _ => {
                out.insert(k.clone(), v.clone());
            }
        }
    }
    out.insert("start".into(), json!(REST));
    let mut states = Map::new();
    for (i, n) in names.iter().enumerate() {
        states.insert(n.clone(), json!({ "frames": [i], "ms": STILL_MS }));
    }
    for (n, frames, ms) in &folded {
        states.insert(n.clone(), json!({ "frames": frames, "ms": num(*ms) }));
    }
    out.insert("states".into(), Value::Object(states));

    let detached: Vec<(String, Value)> =
        extenders(sources, base).into_iter().map(|n| standalone(sources, &n).map(|v| (n, v))).collect::<Result<_, _>>()?;
    Ok(Plan { object: base.to_string(), base: Value::Object(out), members, states: names, folded, scenes, detached })
}

/// The sources with the plan applied.
pub fn applied(sources: &Sources, plan: &Plan) -> Sources {
    let mut after = sources.clone();
    after.extents.borrow_mut().clear();
    after.objects.insert(plan.object.clone(), plan.base.clone());
    for (name, doc) in &plan.detached {
        after.objects.insert(name.clone(), doc.clone());
    }
    for s in &plan.scenes {
        after.scenes.insert(s.scene.clone(), s.doc.clone());
    }
    after
}

/// The objects a scene places or poses as.
fn scene_uses(scene: &Value) -> HashSet<String> {
    let mut out: HashSet<String> = HashSet::new();
    fn walk(list: &[Value], out: &mut HashSet<String>) {
        for item in list {
            if let Some(inner) = item.get("items").filter(|_| item.get("group").is_some()).and_then(Value::as_array) {
                walk(inner, out);
            } else if let Some(o) = item.get("object").and_then(Value::as_str) {
                out.insert(o.to_string());
            }
        }
    }
    walk(scene.get("items").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]), &mut out);
    for track in scene.pointer("/motion/tracks").and_then(Value::as_array).into_iter().flatten() {
        for key in track.get("keys").and_then(Value::as_array).into_iter().flatten() {
            if let Some(p) = key.get("pose").and_then(Value::as_str) {
                out.insert(p.to_string());
            }
        }
    }
    out
}

/// The pose objects nothing uses once the plan is applied (R40): no scene places or poses as it,
/// and no object extends it.
pub fn unused(after: &Sources, plan: &Plan) -> Vec<String> {
    let mut used: HashSet<String> = after.scenes.values().flat_map(scene_uses).collect();
    used.extend(after.objects.values().filter_map(|o| o.get("extends").and_then(Value::as_str)).map(str::to_string));
    plan.members[1..].iter().filter(|m| !used.contains(*m)).cloned().collect()
}

/// The scenes whose renders the merge could change: every scene that places or poses the base, a
/// member or a detached object.
pub fn affected(sources: &Sources, plan: &Plan) -> Vec<String> {
    let watched: HashSet<&str> = plan.members.iter().map(String::as_str).chain(plan.detached.iter().map(|(n, _)| n.as_str())).collect();
    sources
        .scenes
        .iter()
        .filter(|(_, scene)| scene_uses(scene).iter().any(|o| watched.contains(o.as_str())))
        .map(|(n, _)| n.clone())
        .collect()
}

/// The times a scene's render is compared at: every 100 ms of its loop, and each of `extra` (the
/// keys of the tracks the merge rewrites, which can fall between), or `0` alone for a scene without
/// motion.
pub fn check_times(scene: &Value, extra: &[f64]) -> Vec<f64> {
    let duration = scene.pointer("/motion/duration").and_then(Value::as_f64).unwrap_or(0.0);
    if duration <= 0.0 {
        return vec![0.0];
    }
    let mut times: Vec<f64> = (0..).map(|i| i as f64 * 100.0).take_while(|t| *t < duration).collect();
    times.extend(extra.iter().copied().filter(|t| (0.0..duration).contains(t)));
    times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    times.dedup();
    times
}

/// The key times of a scene's tracks whose item places `base`.
pub fn family_key_times(scene: &Value, base: &str) -> Vec<f64> {
    tracks_of_object(scene, base)
        .into_iter()
        .flat_map(|i| scene["motion"]["tracks"][i].get("keys").and_then(Value::as_array).cloned().unwrap_or_default())
        .map(|k| t_of(&k))
        .collect()
}

/// The names of a doc's states, in order.
pub fn state_names(doc: &Value) -> Vec<String> {
    doc.get("states").and_then(Value::as_object).map(|m| keys(m).into_iter().cloned().collect()).unwrap_or_default()
}

/// The times `scene` renders differently with the plan applied (`after`), at most `limit` of them
/// (R38): every 100 ms of its loop and each key of the base's tracks, pixel by pixel.
pub fn differences(before: &Sources, after: &Sources, plan: &Plan, scene: &str, limit: usize) -> Result<Vec<f64>, String> {
    let extra = family_key_times(&before.scenes[scene], &plan.object);
    let mut out = Vec::new();
    let (mut was, mut now) = (ScenePixels::new(before, scene), ScenePixels::new(after, scene));
    for t in check_times(&after.scenes[scene], &extra) {
        let (a, b) = (was.at(t)?, now.at(t)?);
        if a.len() != b.len() || a.iter().any(|(k, v)| b.get(k) != Some(v)) {
            out.push(t);
            if out.len() >= limit {
                break;
            }
        }
    }
    Ok(out)
}
