//! Proposals and the activity log (ports cli/lib/commands/proposal.mjs;
//! docs/intents/2026-10-mac-app/plan.md, "Agents propose, people approve"):
//! an MCP agent's doc.save writes a proposal to the state dir; a person
//! lists, shows (with a diff), applies or rejects it. activity.list tails
//! the log of MCP tool calls.

use engine::engine::{load_sources, Sources};
use serde_json::{json, Map, Value};

use super::common::{flag, obj, opt_num, opt_str, str_of};
use crate::context::{doc_file, Ctx, DocKind, Loaded};
use crate::diff::unified_diff;
use crate::errors::{CommandError, Error, Result};
use crate::registry::{Command, Params};
use crate::render::{render_object_image, render_scene_image, ObjectImageOptions, SceneImageOptions, Theme, MAX_SCALE};
use crate::session::{import_source, problems_in, read_source, Disk, Session};
use crate::state::{write_atomic, State};

fn id_param() -> Value {
    json!({ "type": "string", "description": "the proposal id (proposal.list)" })
}

/// The session of a ctx, or the error a ctx with no state gave.
fn session_of<'a, 'b>(ctx: &'a mut Ctx<'b>) -> Result<&'a mut Session> {
    match ctx.session.as_deref_mut() {
        Some(s) => Ok(s),
        None => Err(CommandError::new("no state dir in this ctx", "no_state").into()),
    }
}

fn state_of(ctx: &mut Ctx) -> Result<State> {
    Ok(session_of(ctx)?.state.clone())
}

/// A proposal's doc, checked: "scene:<name>" or "object:<name>" with a
/// kebab-case name, so a hand-made file can't point outside the sources
/// (`targetOf`).
fn target_of(proposal: &Value) -> Result<(DocKind, String)> {
    let doc = str_of(proposal.get("doc"));
    let parsed = doc
        .split_once(':')
        .and_then(|(k, n)| match k {
            "scene" => Some((DocKind::Scene, n)),
            "object" => Some((DocKind::Object, n)),
            _ => None,
        })
        .filter(|(_, n)| !n.is_empty() && !n.contains('\n') && engine::engine::is_name(n));
    match parsed {
        Some((kind, name)) if proposal.get("source").is_some_and(Value::is_string) => Ok((kind, name.to_string())),
        _ => Err(CommandError::new(
            format!("proposal {} is malformed: doc must be scene:<name> or object:<name> and source a string", str_of(proposal.get("id"))),
            "bad_proposal",
        )
        .into()),
    }
}

/// Whether the file still is what the proposal was made from (`baseMatches`).
fn base_matches(proposal: &Value, disk: &Disk) -> bool {
    match proposal.get("baseHash") {
        Some(Value::Null) => !disk.exists,
        Some(Value::String(h)) => disk.exists && disk.hash.as_deref() == Some(h.as_str()),
        _ => false,
    }
}

fn listed(p: &Value, matches: bool) -> Map<String, Value> {
    let mut m = Map::new();
    let mut put = |k: &str, v: Option<&Value>| {
        if let Some(v) = v {
            m.insert(k.to_string(), v.clone());
        }
    };
    put("id", p.get("id"));
    if p.get("number").and_then(Value::as_f64).is_some_and(|n| n.fract() == 0.0) {
        put("number", p.get("number"));
    }
    put("doc", p.get("doc"));
    put("summary", Some(&p.get("summary").filter(|v| !v.is_null()).cloned().unwrap_or_else(|| json!([]))));
    put("createdAt", p.get("createdAt"));
    put("agent", Some(&p.get("agent").filter(|v| !v.is_null()).cloned().unwrap_or(Value::Null)));
    if let Some(chat) = p.get("chat").filter(|c| c.get("id").is_some_and(Value::is_string)) {
        let title = chat.get("title").filter(|t| !t.is_null()).map_or("Chat".to_string(), |t| str_of(Some(t)));
        put("chat", Some(&json!({ "id": chat["id"], "title": title })));
    }
    put("baseMatches", Some(&json!(matches)));
    m
}

fn list_run(ctx: &mut Ctx, _: Params) -> Result<Value> {
    let session = session_of(ctx)?;
    let mut out = Vec::new();
    for p in session.state.list_proposals() {
        let matches = match target_of(&p) {
            Ok((kind, name)) => base_matches(&p, &read_source(&session.document, kind, &name)),
            Err(_) => false, // malformed: listed, never matching
        };
        out.push(Value::Object(listed(&p, matches)));
    }
    Ok(json!({ "proposals": out }))
}

fn list_format(r: &Value, _: &Params) -> String {
    let lines: Vec<String> = r["proposals"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| {
            let number = match p.get("number") {
                Some(n) if engine::engine::truthy(Some(n)) => format!("#{}  ", str_of(Some(n))),
                _ => String::new(),
            };
            let who = match p.get("chat") {
                Some(c) => format!("chat \"{}\"", str_of(c.get("title"))),
                None => match p.get("agent") {
                    Some(a) if !a.is_null() => str_of(Some(a)),
                    _ => "?".to_string(),
                },
            };
            let edits = p["summary"].as_array().map_or(0, Vec::len);
            format!(
                "{number}{}  {}  {who}  {edits} edit{}{}",
                str_of(p.get("id")),
                str_of(p.get("doc")),
                if edits == 1 { "" } else { "s" },
                if p["baseMatches"] == json!(true) { "" } else { "  (file changed since)" }
            )
        })
        .collect();
    if lines.is_empty() {
        "no proposals".to_string()
    } else {
        lines.join("\n")
    }
}

fn diff_of(before: &str, after: &str, file: &str) -> String {
    unified_diff(before, after, &format!("a/{file}"), &format!("b/{file}"), 3)
}

fn show_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let id = opt_str(&params, "id").unwrap_or("");
    let session = session_of(ctx)?;
    let p = session.state.read_proposal(id)?;
    let (kind, name) = target_of(&p)?;
    let disk = read_source(&session.document, kind, &name);
    let file = doc_file(kind, &name);
    let mut out = listed(&p, base_matches(&p, &disk));
    if let Some(h) = p.get("baseHash") {
        out.insert("baseHash".into(), h.clone());
    }
    out.insert("file".into(), json!(file));
    out.insert("exists".into(), json!(disk.exists));
    out.insert("currentHash".into(), disk.hash.as_ref().map_or(Value::Null, |h| json!(h)));
    out.insert("source".into(), p["source"].clone());
    out.insert("diff".into(), json!(diff_of(&disk.text, p["source"].as_str().unwrap_or(""), &file)));
    Ok(Value::Object(out))
}

fn show_format(r: &Value, _: &Params) -> String {
    let number = match r.get("number") {
        Some(n) if engine::engine::truthy(Some(n)) => format!("#{} ", str_of(Some(n))),
        _ => String::new(),
    };
    let agent = match r.get("agent") {
        Some(a) if !a.is_null() => str_of(Some(a)),
        _ => "an agent".to_string(),
    };
    let chat = r.get("chat").map(|c| format!(" in the chat \"{}\"", str_of(c.get("title")))).unwrap_or_default();
    let mut lines =
        vec![format!("{number}{}: {} by {agent}{chat}, {}", str_of(r.get("id")), str_of(r.get("doc")), str_of(r.get("createdAt")))];
    lines.extend(r["summary"].as_array().into_iter().flatten().map(|s| format!("  - {}", str_of(Some(s)))));
    if r["baseMatches"] != json!(true) {
        lines.push(format!("warning: {} changed since the proposal was made; apply needs --force", str_of(r.get("file"))));
    }
    let diff = str_of(r.get("diff"));
    lines.push(if diff.is_empty() { "(no change from the file as it is)".to_string() } else { diff });
    lines.into_iter().filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

/// The proposal's source loaded, or the `invalid` error both render and apply give.
fn load_source(id: &str, p: &Value) -> Result<Value> {
    import_source(p["source"].as_str().unwrap_or("")).map_err(|message| {
        CommandError::new(format!("proposal {id}: its source can't be loaded: {}", message.split('\n').next().unwrap_or("")), "invalid")
            .into()
    })
}

/// The proposed doc over the files on disk, not anyone's unsaved working copies.
fn overlay(session: &Session, kind: DocKind, name: &str, data: Value) -> Loaded {
    let docs = session.document.load_docs();
    let (mut objects, mut scenes) = (docs.objects, docs.scenes);
    if kind == DocKind::Scene {
        scenes.insert(name.to_string(), data);
    } else {
        objects.insert(name.to_string(), data);
    }
    let sources: Sources = load_sources(docs.palette.unwrap_or(Value::Null), objects, scenes);
    Loaded { sources, load_errors: docs.load_errors }
}

fn render_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let id = opt_str(&params, "id").unwrap_or("");
    let session = session_of(ctx)?;
    let p = session.state.read_proposal(id)?;
    let (kind, name) = target_of(&p)?;
    let data = load_source(id, &p)?;
    let view = overlay(session, kind, &name, data);
    let disk = read_source(&session.document, kind, &name);
    let scale = opt_num(&params, "scale").unwrap_or(1.0) as u32;
    let time = opt_num(&params, "time");
    let image = if kind == DocKind::Scene {
        let theme = if opt_str(&params, "theme") == Some("dark") { Theme::Dark } else { Theme::Light };
        let options = SceneImageOptions { scale, time: time.unwrap_or(0.0), theme, ..SceneImageOptions::default() };
        render_scene_image(&view.sources, &name, &session.document.dark_page, &options)?
    } else {
        let options = ObjectImageOptions {
            scale,
            time,
            frame: opt_num(&params, "frame").unwrap_or(0.0) as usize,
            layers: params
                .get("layers")
                .and_then(Value::as_array)
                .map(|l| l.iter().filter_map(Value::as_u64).map(|n| n as usize).collect()),
        };
        render_object_image(&view.sources, &name, &options)?
    };
    let mut out = obj(vec![
        ("id", json!(id)),
        ("doc", p.get("doc").cloned().unwrap_or(Value::Null)),
        ("kind", json!(kind.as_str())),
        ("baseMatches", json!(base_matches(&p, &disk))),
    ]);
    if p.get("doc").is_none() {
        out.shift_remove("doc");
    }
    if let Value::Object(m) = image {
        out.extend(m);
    }
    Ok(Value::Object(out))
}

fn render_format(r: &Value, _: &Params) -> String {
    let n = r["items"].as_array().map_or(0, Vec::len);
    format!(
        "{}: {} as proposed, {}×{} PNG at {}×, {n} item{}",
        str_of(r.get("id")),
        str_of(r.get("doc")),
        str_of(r.get("width")),
        str_of(r.get("height")),
        str_of(r.get("scale")),
        if n == 1 { "" } else { "s" }
    )
}

fn apply_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let id = opt_str(&params, "id").unwrap_or("");
    let force = flag(&params, "force");
    let session = session_of(ctx)?;
    let state = session.state.clone();
    let p = state.read_proposal(id)?;
    let (kind, name) = target_of(&p)?;
    let file = doc_file(kind, &name);
    let disk = read_source(&session.document, kind, &name);
    if !base_matches(&p, &disk) && !force {
        return Err(CommandError::new(
            format!("{file} changed since proposal {id} was made; nothing written. Show it to compare, then reject it or force the apply."),
            "conflict",
        )
        .into());
    }
    let data = load_source(id, &p)?;
    // Validated against the files on disk, not anyone's unsaved working copies.
    let check = overlay(session, kind, &name, data);
    let problems = problems_in(&check, &file);
    if !problems.is_empty() && !force {
        return Err(CommandError::new(
            format!(
                "proposal {id} has {} problem{}; nothing written:\n{}",
                problems.len(),
                if problems.len() == 1 { "" } else { "s" },
                problems.join("\n")
            ),
            "invalid",
        )
        .with_details(json!(problems))
        .into());
    }
    let source = p["source"].as_str().unwrap_or("").to_string();
    write_atomic(&session.document.source_dir.join(&file), &source)?;
    state.remove_proposal(id)?;
    let ref_ = crate::session::doc_ref(kind.as_str(), &name);
    let reloaded = session.reload_if_clean(&ref_);
    let (mut merged, mut conflict) = (false, false);
    // A working copy with unsaved edits takes the proposal through a merge (spec D4), not a stale copy.
    if !reloaded && session.entries.has(&ref_) && session.is_dirty(&ref_) {
        match session.merge(&ref_) {
            Ok(r) => merged = r["merged"] == json!(true),
            Err(Error::Command(e)) if e.code == "merge_conflict" || e.code == "merge_invalid" => conflict = true,
            Err(e) => return Err(e),
        }
    }
    ctx.invalidate();
    let mut out = obj(vec![("id", json!(id))]);
    if let Some(d) = p.get("doc") {
        out.insert("doc".into(), d.clone());
    }
    out.insert("file".into(), json!(file));
    out.insert("wrote".into(), json!(true));
    out.insert("reloaded".into(), json!(reloaded));
    if merged {
        out.insert("merged".into(), json!(true));
    }
    if conflict {
        out.insert("conflict".into(), json!(true));
    }
    out.insert("diff".into(), json!(diff_of(&disk.text, &source, &file)));
    if !problems.is_empty() {
        out.insert("forcedProblems".into(), json!(problems));
    }
    let tail = if merged {
        "; merged into the open copy"
    } else if conflict {
        "; the open copy's unsaved edits clash with it, so it was left as it is"
    } else {
        ""
    };
    out.insert("summary".into(), json!(format!("Applied {id}: wrote {file}{tail}")));
    Ok(Value::Object(out))
}

fn summary_format(r: &Value, _: &Params) -> String {
    str_of(r.get("summary"))
}

fn reject_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let id = opt_str(&params, "id").unwrap_or("");
    let state = state_of(ctx)?;
    let p = state.read_proposal(id)?;
    state.remove_proposal(id)?;
    let doc = p.get("doc").filter(|d| !d.is_null()).cloned().unwrap_or(Value::Null);
    Ok(Value::Object(obj(vec![("id", json!(id)), ("doc", doc), ("rejected", json!(true)), ("summary", json!(format!("Rejected {id}")))])))
}

fn activity_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let limit = opt_num(&params, "limit").unwrap_or(50.0) as usize;
    let state = state_of(ctx)?;
    Ok(json!({ "entries": state.tail_activity(limit) }))
}

fn activity_format(r: &Value, _: &Params) -> String {
    let lines: Vec<String> = r["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|e| {
            let summary = match e.get("summary") {
                Some(s) if !s.is_null() => str_of(Some(s)),
                _ => String::new(),
            };
            format!(
                "{}  {}  {}  {summary}",
                str_of(e.get("time")),
                if engine::engine::truthy(e.get("ok")) { "ok " } else { "err" },
                str_of(e.get("tool"))
            )
        })
        .collect();
    if lines.is_empty() {
        "no activity".to_string()
    } else {
        lines.join("\n")
    }
}

/// This file's commands.
pub fn commands() -> Vec<Command> {
    let mut list = Command::new(
        "proposal.list",
        "List the proposals agents made, oldest first",
        json!({ "type": "object", "properties": {} }),
        false,
        list_run,
    );
    list.description = Some(concat!(
        "Each is { id, number, doc, summary: [edits], createdAt, agent, chat?, baseMatches }; number is its #n (older proposals have none), chat ({ id, title }) is the Agent chat it came from, ",
        "and baseMatches is false when the file changed since the agent opened it."
    ));
    list.format = Some(list_format);

    let mut show = Command::new(
        "proposal.show",
        "Show a proposal: its edits, the source, and a diff from the file as it is now",
        json!({ "type": "object", "properties": { "id": id_param() }, "required": ["id"] }),
        false,
        show_run,
    );
    show.args = vec!["id"];
    show.format = Some(show_format);

    let mut render = Command::new(
        "proposal.render",
        "Render a proposal's doc as it would be once applied, without writing anything",
        json!({
            "type": "object",
            "properties": {
                "id": id_param(),
                "scale": { "type": "integer", "minimum": 1, "maximum": MAX_SCALE, "default": 1, "description": "whole-pixel zoom of the PNG" },
                "time": { "type": "integer", "minimum": 0, "description": "ms into the motion and loops (a scene: default 0; an object: overrides frame)" },
                "frame": { "type": "integer", "minimum": 0, "default": 0, "description": "an object's frame" },
                "layers": { "type": "array", "items": { "type": "integer", "minimum": 0 }, "description": "only these layers of a sprite, as object.render's --layers" },
                "theme": {
                    "type": "string",
                    "enum": ["light", "dark"],
                    "default": "light",
                    "description": "a scene's theme, as scene.render's --theme"
                }
            },
            "required": ["id"]
        }),
        false,
        render_run,
    );
    render.description = Some(concat!(
        "The proposed source over the files on disk, rendered as scene.render or object.render renders the doc (same params, same result shape), ",
        "plus { id, doc, kind, baseMatches }. The Agent window draws it over the stage as a ghost."
    ));
    render.args = vec!["id"];
    render.writes_png = true;
    render.examples = vec!["pixellab proposal render 20261019-142301-a1b2c3 --scale 2 --out /tmp/proposal.png"];
    render.format = Some(render_format);

    let mut apply = Command::new(
        "proposal.apply",
        "Apply a proposal: check its base, validate it, write the doc's source and remove the proposal",
        json!({
            "type": "object",
            "properties": {
                "id": id_param(),
                "force": { "type": "boolean", "default": false, "description": "apply despite a changed base or problems" }
            },
            "required": ["id"]
        }),
        true,
        apply_run,
    );
    apply.description = Some(concat!(
        "Refuses (code conflict) if the file changed since the proposal was made, and (code invalid) if the proposed doc has validation problems, unless --force. ",
        "An open working copy of the doc with no unsaved edits reloads from the new file (reloaded: true); one with unsaved edits has the proposal merged into it, ",
        "keeping them (merged: true), or, when both changed the same lines, stays as it is and is reported stale (conflict: true). The file is written either way."
    ));
    apply.args = vec!["id"];
    apply.writes_direct = true;
    apply.format = Some(summary_format);

    let mut reject = Command::new(
        "proposal.reject",
        "Reject a proposal: delete it, leaving the site as it is",
        json!({ "type": "object", "properties": { "id": id_param() }, "required": ["id"] }),
        true,
        reject_run,
    );
    reject.args = vec!["id"];
    reject.writes_direct = true;
    reject.format = Some(summary_format);

    let mut activity = Command::new(
        "activity.list",
        "The last MCP tool calls agents made, oldest first",
        json!({
            "type": "object",
            "properties": {
                "limit": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 50, "description": "how many entries" }
            }
        }),
        false,
        activity_run,
    );
    activity.description = Some("Each entry is { time, tool, args, ok, summary }, from activity.jsonl in the state dir.");
    activity.format = Some(activity_format);

    vec![list, show, render, apply, reject, activity]
}
