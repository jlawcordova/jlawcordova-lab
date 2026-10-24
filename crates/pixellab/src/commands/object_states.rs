//! The state commands (docs/intents/2026-10-object-states/spec.md, D3): `object.state.*` and
//! `object.states.make` edit a sprite's states, and `object.state.run` reads what a state plays.

use engine::engine::{resolve, validate};
use engine::jsnum::num;
use engine::states::{self, Settle};
use serde_json::{json, Map, Value};

use super::common::{flag, format_change, obj, opt_num, opt_str, str_of};
use super::history::put_files;
use crate::context::{Ctx, DocKind, Loaded};
use crate::errors::{CommandError, Result};
use crate::registry::{Command, Params};
use crate::session::Edit;
use crate::states_ops as ops;

fn name_param() -> Value {
    json!({ "type": "string", "description": "object name (or objects/<name>)" })
}

fn state_param() -> Value {
    json!({ "type": "string", "description": "a state of the object" })
}

/// A time in ms as the commands report it: whole numbers stay whole, others keep 3 decimals.
fn ms(t: f64) -> Value {
    num((t * 1000.0).round() / 1000.0)
}

fn move_json(m: [f64; 2]) -> Value {
    json!([num(m[0]), num(m[1])])
}

/// The object's states, or a refusal that says it has none (or not this one).
fn states_of(ctx: &mut Ctx, query: &str, state: Option<&str>) -> Result<(String, std::rc::Rc<states::States>)> {
    let name = ctx.find(query, Some(DocKind::Object))?.name;
    ctx.doc(DocKind::Object, &name)?;
    let loaded = ctx.sources()?;
    let found = resolve(&loaded.sources, &name)
        .map_err(|e| CommandError::new(format!("object {name} doesn't resolve: {}; run pixellab check {name}", e.0), "resolve_error"))?
        .states
        .ok_or_else(|| CommandError::new(format!("object {name} has no states (object.state.add makes the first)"), "no_states"))?;
    if let Some(s) = state {
        if !found.has(s) {
            return Err(
                CommandError::new(format!("object {name} has no state {s} (it has {})", found.names().join(", ")), "not_found").into()
            );
        }
    }
    Ok((name, found))
}

fn run_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let state = opt_str(&params, "state").unwrap_or("");
    let (name, found) = states_of(ctx, opt_str(&params, "name").unwrap_or(""), Some(state))?;
    let run = states::run(&found, state);
    let horizon = opt_num(&params, "horizon");
    let changes = states::changes(&found, &run, horizon);
    let segments: Vec<Value> = run.segments.iter().map(|g| json!({ "state": g.state, "from": num(g.from), "to": num(g.to) })).collect();
    let settle = match &run.settle {
        Settle::Loop { from, period } => json!({ "kind": "loop", "from": num(*from), "period": num(*period) }),
        Settle::Hold { from, frame, moved } => json!({ "kind": "hold", "from": num(*from), "frame": frame, "move": move_json(*moved) }),
    };
    let changes: Vec<Value> = changes.iter().map(|c| json!({ "t": ms(c.t), "frame": c.frame, "move": move_json(c.moved) })).collect();
    Ok(Value::Object(obj(vec![
        ("object", json!(name)),
        ("state", json!(state)),
        ("segments", Value::Array(segments)),
        ("settle", settle),
        ("changes", Value::Array(changes)),
    ])))
}

fn run_format(r: &Value, _: &Params) -> String {
    let mut lines = vec![format!("{} in {}", str_of(r.get("object")), str_of(r.get("state")))];
    for g in r["segments"].as_array().into_iter().flatten() {
        lines.push(format!("  {} from {} to {} ms", str_of(g.get("state")), str_of(g.get("from")), str_of(g.get("to"))));
    }
    let s = &r["settle"];
    lines.push(match s["kind"].as_str() {
        Some("loop") => format!("  then loops from {} ms, every {} ms", str_of(s.get("from")), str_of(s.get("period"))),
        _ => format!("  then holds frame {} from {} ms", str_of(s.get("frame")), str_of(s.get("from"))),
    });
    for c in r["changes"].as_array().into_iter().flatten() {
        let m = c.get("move");
        lines.push(format!(
            "  {:>8} ms  frame {}  move {},{}",
            str_of(c.get("t")),
            str_of(c.get("frame")),
            str_of(m.and_then(|m| m.get(0))),
            str_of(m.and_then(|m| m.get(1)))
        ));
    }
    lines.join("\n")
}

// ------------------------------------------------------------------ the edits

fn usage(message: impl Into<String>) -> crate::errors::Error {
    CommandError::usage(message).into()
}

/// An object's state edit as one undo step: `f` gets the object's name, its source and the sources as they were.
fn state_edit(ctx: &mut Ctx, query: &str, f: impl FnOnce(&str, &mut Value, &Loaded) -> Result<Edit>) -> Result<Value> {
    let name = ctx.find(query, Some(DocKind::Object))?.name;
    ctx.doc(DocKind::Object, &name)?;
    ctx.edit(DocKind::Object, &name, None, |doc, loaded| f(&name, doc, loaded))
}

fn op<T>(r: std::result::Result<T, CommandError>) -> Result<T> {
    r.map_err(Into::into)
}

fn names(doc: &Value) -> Vec<String> {
    doc.get("states").and_then(Value::as_object).map(|m| engine::serialize::keys(m).into_iter().cloned().collect()).unwrap_or_default()
}

fn str_param<'a>(params: &'a Params, key: &str) -> &'a str {
    opt_str(params, key).unwrap_or("")
}

fn ints(params: &Params, key: &str) -> Option<Vec<usize>> {
    params.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).map(|n| n as usize).collect())
}

fn index(params: &Params, key: &str) -> Option<usize> {
    params.get(key).and_then(Value::as_u64).map(|n| n as usize)
}

fn add_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let with = ops::New {
        frames: ints(&params, "frames"),
        ms: opt_num(&params, "ms"),
        once: flag(&params, "once"),
        then: opt_str(&params, "then").map(str::to_string),
        after: opt_str(&params, "after").map(str::to_string),
    };
    let state = str_param(&params, "state").to_string();
    state_edit(ctx, str_param(&params, "name"), |object, doc, _| {
        let message = op(ops::add(object, doc, &state, &with))?;
        let start = doc["start"].as_str() == Some(state.as_str());
        Ok(Edit::new(obj(vec![("state", json!(state)), ("start", json!(start)), ("states", json!(names(doc)))]), message))
    })
}

fn set_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let given = |k: &str| params.get(k).is_some_and(|v| !v.is_null() && v != &json!(false));
    if !["rename", "frames", "ms", "loop", "once", "then", "hold", "start"].iter().any(|k| given(k)) {
        return Err(usage("object.state.set: give --rename, --frames, --ms, --loop, --once, --then, --hold or --start"));
    }
    let change = ops::Change {
        rename: opt_str(&params, "rename").map(str::to_string),
        frames: ints(&params, "frames"),
        ms: opt_num(&params, "ms"),
        looping: flag(&params, "loop"),
        once: flag(&params, "once"),
        then: opt_str(&params, "then").map(str::to_string),
        hold: flag(&params, "hold"),
        start: flag(&params, "start"),
    };
    let state = str_param(&params, "state").to_string();
    let object = ctx.find(str_param(&params, "name"), Some(DocKind::Object))?.name;
    let mut working = ctx.doc(DocKind::Object, &object)?;
    // Checks the change first, on a copy, so a refusal changes nothing in any doc.
    let message = op(ops::set(&object, &mut working, &state, &change))?;
    let renamed = change.rename.as_deref().filter(|n| *n != state);
    let loaded = ctx.sources()?;
    // A rename reaches the scenes that name the state: every item and key, in every scene (D3).
    let mut scenes: Vec<(String, Value, usize)> = Vec::new();
    if let Some(new) = renamed {
        let sharing = ops::sharing(&loaded.sources, &object);
        for scene in loaded.sources.scenes.keys() {
            let mut doc = ctx.doc(DocKind::Scene, scene)?;
            let n = ops::rename_in_scene(&mut doc, &sharing, &state, new);
            if n > 0 {
                scenes.push((scene.clone(), doc, n));
            }
        }
    }
    let done = |extra: Vec<(&str, Value)>| {
        let mut pairs = vec![("state", json!(renamed.unwrap_or(state.as_str()))), ("states", json!(names(&working)))];
        pairs.extend(extra);
        obj(pairs)
    };
    if scenes.is_empty() {
        let (state2, change2) = (state.clone(), change.clone());
        return state_edit(ctx, &object, move |o, doc, _| {
            let message = op(ops::set(o, doc, &state2, &change2))?;
            Ok(Edit::new(done(vec![]), message))
        });
    }
    // Several docs are one version, as a library import is: the whole is checked before anything is written.
    let new = renamed.expect("a rename");
    let mut after = loaded.sources.clone();
    after.objects.insert(object.clone(), working.clone());
    for (scene, doc, _) in &scenes {
        after.scenes.insert(scene.clone(), doc.clone());
    }
    let files: Vec<String> =
        std::iter::once(engine::format::object_file(&object)).chain(scenes.iter().map(|(n, _, _)| engine::format::scene_file(n))).collect();
    let broken: Vec<String> = validate(&after).into_iter().filter(|p| files.iter().any(|f| p.starts_with(&format!("{f}: ")))).collect();
    if !broken.is_empty() {
        return Err(CommandError::new(
            format!("renaming {state} to {new} would leave problems, so nothing changed:\n{}", broken.join("\n")),
            "invalid",
        )
        .with_details(Value::from(broken))
        .into());
    }
    let label = format!("Renamed {object}'s state {state} to {new} ({} scene{})", scenes.len(), if scenes.len() == 1 { "" } else { "s" });
    let mut targets: Vec<(String, Option<String>)> =
        vec![(engine::format::object_file(&object), Some(engine::format::serialize_object(&working)))];
    targets.extend(scenes.iter().map(|(n, d, _)| (engine::format::scene_file(n), Some(engine::format::serialize_scene(d)))));
    put_files(ctx.session()?, targets, &label, false, true, false)?;
    let touched: Vec<Value> = scenes.iter().map(|(n, _, c)| json!({ "scene": n, "changed": c })).collect();
    let mut out =
        obj(vec![("doc", json!(format!("object:{object}"))), ("changed", json!(true)), ("summary", json!(format!("{message}; {label}")))]);
    out.extend(done(vec![("scenes", Value::Array(touched))]));
    Ok(Value::Object(out))
}

fn delete_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let state = str_param(&params, "state").to_string();
    state_edit(ctx, str_param(&params, "name"), |object, doc, loaded| {
        // A scene that names it keeps the state in the document: say where, and change nothing.
        let used: Vec<String> = ops::refs(&loaded.sources, object)
            .into_iter()
            .filter(|r| r.state == state)
            .map(|r| format!("{}: {}", engine::format::scene_file(&r.scene), r.place))
            .collect();
        if !used.is_empty() {
            return Err(CommandError::new(
                format!(
                    "{object}'s state {state} is named by {}; change them first (scene.set --state start, scene.key.set --state clear)",
                    used.join("; ")
                ),
                "in_use",
            )
            .with_details(json!(used))
            .into());
        }
        let message = op(ops::delete(object, doc, &state))?;
        Ok(Edit::new(obj(vec![("state", json!(state)), ("states", json!(names(doc)))]), message))
    })
}

fn duplicate_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let (state, as_) = (str_param(&params, "state").to_string(), opt_str(&params, "as").map(str::to_string));
    state_edit(ctx, str_param(&params, "name"), |object, doc, _| {
        let (new, message) = op(ops::duplicate(object, doc, &state, as_.as_deref()))?;
        Ok(Edit::new(obj(vec![("state", json!(new)), ("states", json!(names(doc)))]), message))
    })
}

fn move_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let (state, by) = (str_param(&params, "state").to_string(), params.get("by").and_then(Value::as_i64).unwrap_or(0));
    state_edit(ctx, str_param(&params, "name"), |object, doc, _| {
        let (place, message) = op(ops::move_by(object, doc, &state, by))?;
        Ok(Edit::new(obj(vec![("state", json!(state)), ("place", json!(place)), ("states", json!(names(doc)))]), message))
    })
}

fn frame_add_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let state = str_param(&params, "state").to_string();
    let (frame, at) = (index(&params, "frame"), index(&params, "at"));
    state_edit(ctx, str_param(&params, "name"), |object, doc, _| {
        // Without --frame, the last frame the state plays (a copy of what it ends on), else frame 0.
        let frame = frame.unwrap_or_else(|| {
            doc["states"][&state]["frames"].as_array().and_then(|f| f.last()).and_then(Value::as_u64).unwrap_or(0) as usize
        });
        let message = op(ops::frame_add(object, doc, &state, frame, at))?;
        Ok(Edit::new(obj(vec![("state", json!(state)), ("frames", doc["states"][&state]["frames"].clone())]), message))
    })
}

fn frame_delete_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let state = str_param(&params, "state").to_string();
    let at = index(&params, "at").unwrap_or(0);
    state_edit(ctx, str_param(&params, "name"), |object, doc, _| {
        let message = op(ops::frame_delete(object, doc, &state, at))?;
        Ok(Edit::new(obj(vec![("state", json!(state)), ("frames", doc["states"][&state]["frames"].clone())]), message))
    })
}

fn frame_move_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let state = str_param(&params, "state").to_string();
    let (at, by) = (index(&params, "at").unwrap_or(0), params.get("by").and_then(Value::as_i64).unwrap_or(0));
    state_edit(ctx, str_param(&params, "name"), |object, doc, _| {
        let (to, message) = op(ops::frame_move(object, doc, &state, at, by))?;
        Ok(Edit::new(obj(vec![("state", json!(state)), ("at", json!(to)), ("frames", doc["states"][&state]["frames"].clone())]), message))
    })
}

fn move_value(params: &Params) -> Option<[f64; 2]> {
    params.get("move").and_then(Value::as_array).map(|a| [a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(0.0)])
}

fn key_set_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let state = str_param(&params, "state").to_string();
    let (t, moved) = (opt_num(&params, "time").unwrap_or(f64::NAN), move_value(&params));
    state_edit(ctx, str_param(&params, "name"), |object, doc, _| {
        let (pinned, key, message) = op(ops::key_set(object, doc, &state, t, moved))?;
        Ok(Edit::new(obj(vec![("state", json!(state)), ("key", key), ("pinned", json!(pinned))]), message))
    })
}

fn key_delete_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let state = str_param(&params, "state").to_string();
    let t = opt_num(&params, "time").unwrap_or(f64::NAN);
    state_edit(ctx, str_param(&params, "name"), |object, doc, _| {
        let message = op(ops::key_delete(object, doc, &state, t))?;
        Ok(Edit::new(obj(vec![("state", json!(state))]), message))
    })
}

fn key_move_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let state = str_param(&params, "state").to_string();
    let (t, to) = (opt_num(&params, "time").unwrap_or(f64::NAN), opt_num(&params, "to").unwrap_or(f64::NAN));
    state_edit(ctx, str_param(&params, "name"), |object, doc, _| {
        let (key, message) = op(ops::key_move(object, doc, &state, t, to))?;
        Ok(Edit::new(obj(vec![("state", json!(state)), ("key", key)]), message))
    })
}

fn make_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    state_edit(ctx, str_param(&params, "name"), |object, doc, _| {
        let made = op(ops::make(object, doc))?;
        Ok(Edit::new(obj(vec![("states", json!(made.states)), ("start", json!("idle"))]), made.message))
    })
}

/// This file's commands.
pub fn commands() -> Vec<Command> {
    let mut run = Command::new(
        "object.state.run",
        "What a state plays: the states it leads through, where it settles, and when its frame or move changes",
        json!({
            "type": "object",
            "properties": {
                "name": name_param(),
                "state": state_param(),
                "horizon": { "type": "integer", "minimum": 1, "description": "list changes up to this many ms after entering (default: the settle plus one period)" }
            },
            "required": ["name", "state"]
        }),
        false,
        run_run,
    );
    run.description = Some(
        "The run is what an item shows after it enters the state, and it is the same every time: segments are the states that then leads through (each from and to, in ms after entering), settle says where it ends up (a loop from some ms with its period, or a hold of one frame), and changes lists every time the frame or the whole sprite's move (x,y pixels) changes, from 0 up to horizon. The app, the renders and the published SVG all play this one run.",
    );
    run.args = vec!["name", "state"];
    run.examples = vec!["pixellab object state run character hop", "pixellab object state run lamp flicker --horizon 3000"];
    run.format = Some(run_format);

    type Run = fn(&mut Ctx, Params) -> Result<Value>;
    let edit =
        |name: &'static str, summary: &'static str, props: Vec<(&str, Value)>, required: Vec<&str>, run: Run, args: Vec<&'static str>| {
            let mut properties = Map::new();
            properties.insert("name".into(), name_param());
            for (k, v) in props {
                properties.insert(k.into(), v);
            }
            let mut c = Command::new(name, summary, json!({ "type": "object", "properties": properties, "required": required }), true, run);
            c.args = args;
            c.format = Some(format_change);
            c
        };
    let frames_param = json!({
        "type": "array", "items": { "type": "integer", "minimum": 0 }, "minItems": 1, "maxItems": 256,
        "description": "the frames it plays, in order: frame numbers of the sprite, repeats allowed (on the command line 0,1,2)"
    });
    let ms_param = json!({ "type": "integer", "minimum": 20, "maximum": 10000, "description": "how long each frame shows, in ms" });
    let at_param = json!({ "type": "integer", "minimum": 0, "description": "a position in the state's frames, from 0" });
    let time_param = json!({ "type": "integer", "minimum": 0, "description": "ms after entering the state" });
    let move_param = json!({ "type": "array", "items": { "type": "integer" }, "minItems": 2, "maxItems": 2, "description": "pixels the whole sprite is moved: x,y" });

    let mut add = edit(
        "object.state.add",
        "Add a state to a sprite: frames played at one frame length, that loops or plays once",
        vec![
            ("state", json!({ "type": "string", "description": "the new state's name: lowercase words joined by hyphens" })),
            ("frames", frames_param.clone()),
            ("ms", ms_param.clone()),
            (
                "once",
                json!({ "type": "boolean", "default": false, "description": "play once, then hold the last frame (or hand over, with --then)" }),
            ),
            ("then", json!({ "type": "string", "description": "with --once: the state that follows" })),
            ("after", json!({ "type": "string", "description": "the state it goes after in the list (default: last)" })),
        ],
        vec!["name", "state"],
        add_run,
        vec!["name", "state"],
    );
    add.description = Some(
        "A state is the sprite's frames played at one frame length; it loops, or plays once and then holds its last frame or hands over to another state (--once --then). Without --frames it plays every frame, and without --ms it uses the frame length the sprite's frame loop plays at today (150 ms for ff, 200 ms for wf, 500 ms for hf, 200 ms for any other loop and for a still sprite), so the first state looks and plays as the sprite did. The first state is the start: every placement plays it unless an item says otherwise. A sprite has at most 32 states, and a block or a sprite that extends another has none of its own. The names clear and start are the scene commands' words and can't be state names, and neither can names that start as the compiler's own do (mv-, am-, a or s and a number, like s2).",
    );
    add.examples =
        vec!["pixellab object state add lamp idle", "pixellab object state add lamp flicker --frames 0,1,0,1 --ms 80 --once --then idle"];

    let mut set = edit(
        "object.state.set",
        "Change a state: rename it, its frames, frame length, how it ends, or make it the start",
        vec![
            ("state", json!({ "type": "string", "description": "the state" })),
            ("rename", json!({ "type": "string", "description": "its new name" })),
            ("frames", frames_param),
            ("ms", ms_param),
            ("loop", json!({ "type": "boolean", "default": false, "description": "make it loop" })),
            ("once", json!({ "type": "boolean", "default": false, "description": "make it play once" })),
            ("then", json!({ "type": "string", "description": "the state that follows a state that plays once" })),
            (
                "hold",
                json!({ "type": "boolean", "default": false, "description": "a state that plays once holds its last frame (takes the then off)" }),
            ),
            ("start", json!({ "type": "boolean", "default": false, "description": "make it the state items start in" })),
        ],
        vec!["name", "state"],
        set_run,
        vec!["name", "state"],
    );
    set.description = Some(
        "Renaming updates the start, every then, and every scene item and motion key that names it, in every scene: all of it is one version (and one undo step for each doc it touches). Changing the frames or the length is refused when the state's move would be cut off. The result lists the scenes it changed.",
    );
    set.examples = vec!["pixellab object state set lamp flicker --ms 60", "pixellab object state set character hop --rename jump"];

    let mut delete = edit(
        "object.state.delete",
        "Delete a state",
        vec![("state", json!({ "type": "string", "description": "the state" }))],
        vec!["name", "state"],
        delete_run,
        vec!["name", "state"],
    );
    delete.description = Some(
        "Refused, with the list, while it is the start, a then names it, or a scene item or key names it. Deleting the last state takes states and start off, and the frame loops play on their own again.",
    );

    let duplicate = edit(
        "object.state.duplicate",
        "Copy a state, just after it",
        vec![
            ("state", json!({ "type": "string", "description": "the state" })),
            ("as", json!({ "type": "string", "description": "the copy's name (default: <state>-2)" })),
        ],
        vec!["name", "state"],
        duplicate_run,
        vec!["name", "state"],
    );

    let mut move_ = edit(
        "object.state.move",
        "Move a state later (--by 1) or earlier (--by -1) in the list",
        vec![
            ("state", json!({ "type": "string", "description": "the state" })),
            ("by", json!({ "type": "integer", "description": "places later (positive) or earlier (negative)" })),
        ],
        vec!["name", "state", "by"],
        move_run,
        vec!["name", "state"],
    );
    move_.description =
        Some("The list's order is the order the states are shown in, and a page that names two states gets the one listed later.");

    let frame_add = edit(
        "object.state.frame.add",
        "Add a frame of the sprite to a state's frames",
        vec![
            ("state", json!({ "type": "string", "description": "the state" })),
            ("frame", json!({ "type": "integer", "minimum": 0, "description": "the sprite's frame (default: the one the state ends on)" })),
            ("at", at_param.clone()),
        ],
        vec!["name", "state"],
        frame_add_run,
        vec!["name", "state"],
    );
    let frame_delete = edit(
        "object.state.frame.delete",
        "Take a position out of a state's frames (not the only one)",
        vec![("state", json!({ "type": "string", "description": "the state" })), ("at", at_param.clone())],
        vec!["name", "state", "at"],
        frame_delete_run,
        vec!["name", "state"],
    );
    let frame_move = edit(
        "object.state.frame.move",
        "Move a position of a state's frames later (--by 1) or earlier (--by -1)",
        vec![
            ("state", json!({ "type": "string", "description": "the state" })),
            ("at", at_param),
            ("by", json!({ "type": "integer", "description": "places later (positive) or earlier (negative)" })),
        ],
        vec!["name", "state", "at", "by"],
        frame_move_run,
        vec!["name", "state"],
    );

    let mut key_set = edit(
        "object.state.key.set",
        "Set a key of a state's move: where the whole sprite stands at a time (made if there's no key there)",
        vec![("state", json!({ "type": "string", "description": "the state" })), ("time", time_param.clone()), ("move", move_param)],
        vec!["name", "state", "time"],
        key_set_run,
        vec!["name", "state"],
    );
    key_set.description = Some(
        "The move is one for the whole sprite, in pixels from where it is drawn, a whole pixel at a time as an action's is. The first key is at 0 and is made with the move. With no --move, the key pins the sprite where it is at that time (pinned: true), and a time that already has a key is left alone (changed: false).",
    );
    let key_delete = edit(
        "object.state.key.delete",
        "Delete a key of a state's move (the last one takes the move off)",
        vec![("state", json!({ "type": "string", "description": "the state" })), ("time", time_param.clone())],
        vec!["name", "state", "time"],
        key_delete_run,
        vec!["name", "state"],
    );
    let key_move = edit(
        "object.state.key.move",
        "Move a key of a state's move to another time",
        vec![
            ("state", json!({ "type": "string", "description": "the state" })),
            ("time", time_param),
            ("to", json!({ "type": "integer", "minimum": 0, "description": "its new time, ms after entering the state" })),
        ],
        vec!["name", "state", "time", "to"],
        key_move_run,
        vec!["name", "state"],
    );

    let mut make = edit(
        "object.states.make",
        "Turn a sprite's layer ambient loop and actions into states",
        vec![],
        vec!["name"],
        make_run,
        vec!["name"],
    );
    make.description = Some(
        "For moving a sprite over to states (the site's character, for one). The ambient loop becomes the start state, idle: it loops, the layer's move is the sprite's move and its length the loop's duration. Each action becomes a state of the same name that plays once with the action's move and then hands over to idle. The sprite's frames carry over, and the layer's ambient and actions are taken off. It refuses, and says why, when it couldn't keep the art the same: an ambient loop that starts part-way in (an offset), moves on more than one layer, a move on a layer while another has pixels, or a duration that isn't a whole number of the frame loop's cycles. A scene item's own ambient and actions stay as they are.",
    );

    vec![run, add, set, delete, duplicate, move_, frame_add, frame_delete, frame_move, key_set, key_delete, key_move, make]
}
