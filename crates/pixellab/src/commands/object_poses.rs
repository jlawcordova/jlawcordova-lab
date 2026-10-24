//! `object.states.from-poses` (docs/intents/2026-10-object-states/spec.md, R37 to R40, D8): a pose
//! family becomes one object with states, and the scenes' pose keys become state keys, with every
//! render checked to stay the same before anything is written.

use engine::engine::validate;
use serde_json::{json, Value};

use super::common::{flag, obj, opt_str, str_of};
use super::history::put_files;
use crate::context::{Ctx, DocKind};
use crate::errors::{CommandError, Result};
use crate::poses::{self, Options};
use crate::registry::{Command, Params};

fn strings(params: &Params, key: &str) -> Vec<String> {
    match params.get(key) {
        Some(Value::Array(list)) => list.iter().filter_map(Value::as_str).flat_map(|s| s.split(',')).map(str::to_string).collect(),
        Some(Value::String(s)) => s.split(',').map(str::to_string).collect(),
        _ => Vec::new(),
    }
}

fn ms(t: f64) -> String {
    engine::jsnum::to_string(t)
}

fn run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let object = ctx.find(opt_str(&params, "name").unwrap_or(""), Some(DocKind::Object))?.name;
    let options =
        Options { scenes: strings(&params, "scene"), fold: flag(&params, "fold"), name: opt_str(&params, "as").map(str::to_string) };
    let delete = flag(&params, "deleteUnused");
    let dry = flag(&params, "dryRun");
    // Every doc the merge could touch is read through the session, so a pending edit is what's merged.
    let names: Vec<String> = ctx.sources()?.sources.objects.keys().cloned().collect();
    for n in &names {
        ctx.doc(DocKind::Object, n)?;
    }
    let scenes: Vec<String> = ctx.sources()?.sources.scenes.keys().cloned().collect();
    for n in &scenes {
        ctx.doc(DocKind::Scene, n)?;
    }
    let loaded = ctx.sources()?;
    let before = &loaded.sources;
    let plan = poses::plan(before, &object, &options)?;
    let mut after = poses::applied(before, &plan);
    let unused = poses::unused(&after, &plan);
    let deleted: Vec<String> = if delete { unused.clone() } else { vec![] };
    for d in &deleted {
        after.objects.remove(d);
    }
    after.extents.borrow_mut().clear();

    // Nothing new may break: the files the merge writes must have no problems.
    let files: Vec<String> = std::iter::once(engine::format::object_file(&object))
        .chain(plan.detached.iter().map(|(n, _)| engine::format::object_file(n)))
        .chain(plan.scenes.iter().map(|s| engine::format::scene_file(&s.scene)))
        .collect();
    let was: Vec<String> = validate(before);
    let broken: Vec<String> =
        validate(&after).into_iter().filter(|p| !was.contains(p) && files.iter().any(|f| p.starts_with(&format!("{f}: ")))).collect();
    if !broken.is_empty() {
        return Err(CommandError::new(
            format!("making {object}'s states from its poses would leave problems, so nothing changed:\n{}", broken.join("\n")),
            "invalid",
        )
        .with_details(Value::from(broken))
        .into());
    }

    // Every scene that shows the family renders the same, before and after, at every 100 ms (R38).
    let mut differ: Vec<Value> = Vec::new();
    for scene in poses::affected(before, &plan) {
        let times = poses::differences(before, &after, &plan, &scene, 5)
            .map_err(|m| CommandError::new(format!("{scene} doesn't render: {m}"), "render_error"))?;
        if !times.is_empty() {
            differ.push(json!({ "scene": scene, "times": times }));
        }
    }
    if !differ.is_empty() {
        let list: Vec<String> = differ
            .iter()
            .map(|d| {
                format!(
                    "{} at {}",
                    str_of(d.get("scene")),
                    d["times"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_f64)
                        .map(|t| format!("{} ms", ms(t)))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect();
        return Err(CommandError::new(
            format!("the scenes wouldn't render the same with {object}'s poses as states, so nothing changed: {}", list.join("; ")),
            "differs",
        )
        .with_details(Value::Array(differ))
        .into());
    }

    let states = poses::state_names(&plan.base);
    let scene_results: Vec<Value> = plan.scenes.iter().map(|s| json!({ "scene": s.scene, "keys": s.keys, "folded": s.folded })).collect();
    let detached: Vec<&str> = plan.detached.iter().map(|(n, _)| n.as_str()).collect();
    let keys: usize = plan.scenes.iter().map(|s| s.keys).sum();
    let mut summary = format!(
        "{object} now has {} frames and {} states ({}); {keys} pose key{} became state keys in {} scene{}",
        plan.members.len(),
        states.len(),
        states.join(", "),
        if keys == 1 { "" } else { "s" },
        plan.scenes.len(),
        if plan.scenes.len() == 1 { "" } else { "s" },
    );
    if !plan.folded.is_empty() {
        summary.push_str(&format!("; folded into {}", plan.folded.iter().map(|(n, _, _)| n.as_str()).collect::<Vec<_>>().join(", ")));
    }
    if !deleted.is_empty() {
        summary.push_str(&format!("; deleted {}", deleted.join(", ")));
    } else if !unused.is_empty() {
        summary.push_str(&format!("; nothing uses {} now (--delete-unused removes them)", unused.join(", ")));
    }
    let mut out = obj(vec![
        ("doc", json!(format!("object:{object}"))),
        ("changed", json!(!dry)),
        ("summary", json!(if dry { format!("Would make: {summary}") } else { summary.clone() })),
        ("object", json!(object)),
        ("frames", json!(plan.members.len())),
        ("members", json!(plan.members)),
        ("start", json!(poses::REST)),
        ("states", json!(states)),
        (
            "folded",
            json!(plan.folded.iter().map(|(n, f, d)| json!({ "state": n, "frames": f, "ms": engine::jsnum::num(*d) })).collect::<Vec<_>>()),
        ),
        ("scenes", Value::Array(scene_results)),
        ("detached", json!(detached)),
        ("unused", json!(unused)),
        ("deleted", json!(deleted)),
    ]);
    if dry {
        out.insert("dryRun".into(), json!(true));
        return Ok(Value::Object(out));
    }

    // One version across every doc (as a library import is), undone whole by history undo.
    let label = format!("Made {object}'s states from its poses");
    let mut targets: Vec<(String, Option<String>)> =
        vec![(engine::format::object_file(&object), Some(engine::format::serialize_object(&plan.base)))];
    targets.extend(plan.detached.iter().map(|(n, d)| (engine::format::object_file(n), Some(engine::format::serialize_object(d)))));
    targets.extend(plan.scenes.iter().map(|s| (engine::format::scene_file(&s.scene), Some(engine::format::serialize_scene(&s.doc)))));
    targets.extend(deleted.iter().map(|d| (engine::format::object_file(d), None)));
    put_files(ctx.session()?, targets, &label, false, true, false)?;
    Ok(Value::Object(out))
}

fn format(r: &Value, _: &Params) -> String {
    str_of(r.get("summary"))
}

/// This file's command.
pub fn commands() -> Vec<Command> {
    let mut c = Command::new(
        "object.states.from-poses",
        "Turn a pose family into one object with states, and its scenes' pose keys into state keys",
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "the base object: the one its items are placed as (or objects/<name>)" },
                "scene": { "type": "array", "items": { "type": "string" }, "description": "only these scenes (default: every scene), comma-separated or repeated on the command line" },
                "fold": { "type": "boolean", "description": "also turn each run of keys that alternates between poses at one interval into one key into a looping state" },
                "as": { "type": "string", "description": "the folded state's name (default: the run's first state, with -loop)" },
                "deleteUnused": { "type": "boolean", "description": "delete the pose objects nothing uses afterwards, in the same version" },
                "dryRun": { "type": "boolean", "description": "say what it would do, and check the renders, without writing" }
            },
            "required": ["name"]
        }),
        true,
        run,
    );
    c.description = Some(
        "A pose family is a base object and the objects a scene's motion keys pose its items as (crane-hook, crane-hook-drop and crane-hook-low). Each member's drawing becomes a frame of the base, lined up on its anchor and padded to one box, with the color keys merged; each frame gets a state of its own that loops (rest for the base's own drawing, and each pose named without the base's name: drop, low). Every key that posed an item of the base gets the matching state instead. With --fold, a run of at least four keys that only change the pose, alternating at one interval, becomes one key into a looping state that plays those frames at that interval. Every scene that shows the family is rendered before and after at every 100 ms of its loop (and either side of each key), and if any pixel differs the command changes nothing and says where. Objects that extended the base are written out whole, so they keep their drawing. The pose objects stay unless --delete-unused is given and nothing uses them; the result lists them. It's one version across every file it changes. It refuses a pose that is a frame loop or has an ambient loop or actions, a base that extends another or already has states, and a merged sprite past the size or 12-color caps.",
    );
    c.args = vec!["name"];
    c.examples = vec![
        "pixellab object states from-poses crane-hook",
        "pixellab object states from-poses crossing-signal --fold --as flash --delete-unused",
    ];
    c.format = Some(format);
    // It saves every file it changes itself, as one version (put_files), and has its own dryRun.
    c.writes_direct = true;
    vec![c]
}
