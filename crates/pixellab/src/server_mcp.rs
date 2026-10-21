//! `pixellab serve --mcp`: the registry's commands as MCP tools, for agents
//! (ports cli/lib/server-mcp.mjs; docs/cli.md, "MCP"). Hand-written MCP over
//! stdio (protocol 2025-06-18): initialize, notifications/initialized, ping,
//! tools/list and tools/call, as newline-delimited JSON-RPC 2.0.
//!
//! Agents propose, people approve: edits stay in the server's session, and
//! doc_save writes a proposal to the state dir instead of the document,
//! unless the server was started with --direct. Every tools/call appends
//! { time, tool, args, ok, summary } to activity.jsonl there. Before each
//! call the session settles its proposals the owner applied or rejected and
//! reloads clean docs whose files changed (session.sync).

use std::io::{BufRead, Write};

use serde_json::{json, Map, Value};

use crate::activity_summary::activity_summary;
use crate::context::Ctx;
use crate::document::{Document, Env};
use crate::errors::Error;
use crate::jsonrpc::{serve_lines, Failure, Output, RpcError, INVALID_PARAMS, METHOD_NOT_FOUND};
use crate::registry::{Command, Registry};
use crate::session::{Mode, Options, Session};
use crate::state::{iso_time, now_ms};

pub const PROTOCOL_VERSION: &str = "2025-06-18";
const SUPPORTED: [&str; 3] = [PROTOCOL_VERSION, "2025-03-26", "2024-11-05"];

/// Commands that aren't tools, and why (`LEFT_OUT`).
pub const LEFT_OUT: [(&str, &str); 5] = [
    ("proposal.apply", "approving a proposal is for a person, in Pixel Lab or with pixellab proposal apply"),
    ("proposal.reject", "rejecting a proposal is for a person"),
    ("art.build", "it publishes the SVGs a site inlines; a person publishes after approving"),
    ("document.copy", "saving a copy of the document is for a person"),
    ("document.migrate", "migrating the document is for a person, in Pixel Lab or with pixellab document migrate"),
];

/// Tools that drop work: their MCP annotations say so.
const DESTRUCTIVE: [&str; 3] = ["scene.remove", "scene.track.remove", "doc.revert"];

/// scene.place → scene_place (`toolName`).
pub fn tool_name(command: &str) -> String {
    command.replace('.', "_")
}

pub const INSTRUCTIONS: &str = "Pixel Lab edits the pixel art in a .pixellab document: scenes (isometric, 32×16 tiles) and objects (sprites and blocks drawn from a fixed palette). The site that inlines the art publishes from the document.
Look first: document_info, scene_list, scene_items, scene_render (returns the PNG), object_list, object_show, object_render, palette_get, check (problems, and warnings such as off-grid motion).
Edit: scene_place (also into a group, a new group or a layer), scene_move, scene_set, scene_group_set, object_paint, object_fill, object_map_set (a whole map in one call) and the rest. Layers name parts of a scene (the Mine, the Railway, the Train inside it) and never change paint order or the published SVG; they are apart from { group } wrappers: scene_items lists them, scene_layer_new makes one from items and layers, scene_layer_rename, scene_layer_ungroup, scene_layer_assign (into a layer or the top level), scene_layer_move (by tiles, together). Depth: a scene's items always draw back to front by where each stands (column, row and level, and the size of its block or the footprint of its sprite; a pixel-placed item has a level). Every edit puts the items in that order, so scene_items reads back to front, a new item goes where it belongs and the paths it returns are after the sort (reordered says which other paths moved). There is no raise or lower: scene_set depth (front, back or auto) on an item or a group is the one manual control, to draw it over or under everything else in its list. object_footprint says how many tiles a large sprite covers; scene_sort puts a scene written before this (or by hand) in order and reports what moved. Motion: scene_track_add, scene_track_remove, scene_key_set, scene_key_move, scene_key_delete, scene_motion_set, scene_motion_path (on-grid keys along a polyline). Edits stay in this server's memory with undo (doc_undo, doc_redo, doc_status).
Save: doc_save validates the doc and, unless the server runs with --direct, writes a proposal for a person to approve in Pixel Lab instead of changing the document. Once the owner applies or rejects it, this server brings the doc back in line with the file on its own before your next call (doc_status shows how, as synced), so keep editing without a revert. doc_reload reloads a doc from disk when nothing unsaved would be lost. Before proposing, doc_status lists unsavedInApp: docs the owner has open in Pixel Lab with unsaved edits; say so if you propose against one. Say what you changed when you're done.
Ask: chat_ask puts a question with two to four answers in front of the owner in Pixel Lab's Agent chat, as buttons; their choice comes back as their next message, so end your turn after it. Without a chat attached (a terminal) it says so: ask in your own conversation.";

/// The Agent chat a server works for (`chatOf`): PIXELLAB_CHAT and
/// PIXELLAB_CHAT_TITLE in its env.
pub fn chat_of(env: &Env) -> Option<Value> {
    let id = env.get("PIXELLAB_CHAT").map(|s| s.trim()).filter(|s| !s.is_empty())?;
    let title = env.get("PIXELLAB_CHAT_TITLE").map(|s| s.trim()).filter(|s| !s.is_empty()).unwrap_or("Chat");
    Some(json!({ "id": id, "title": title }))
}

fn log(text: &str) {
    eprintln!("pixellab: {text}");
}

/// A tool's description (`describe`).
fn describe(cmd: &Command, direct: bool) -> String {
    let mut text = format!("{}.{}", cmd.summary, cmd.description.map(|d| format!(" {d}")).unwrap_or_default());
    if cmd.name == "doc.save" {
        text += if direct {
            " This server runs with --direct: doc_save writes the site."
        } else {
            " In this server doc_save does not change the site: it writes a proposal (with the list of edits since the doc last matched its file) for a person to approve. Once it is applied or rejected, the doc is brought in line with the file before your next call."
        };
    }
    if cmd.mutates && !cmd.writes_direct && cmd.name.split('.').next() != Some("doc") {
        text += " The edit stays in memory until doc_save.";
    }
    text
}

/// The tools: every command but the left-out ones, by tool name, in command order.
fn tools<'a>(registry: &'a Registry, left: &[(&str, &str)]) -> Vec<(String, &'a Command)> {
    registry.commands.iter().filter(|c| !left.iter().any(|(n, _)| *n == c.name)).map(|c| (tool_name(c.name), c)).collect()
}

/// tools/list's tools (`listTools`).
pub fn list_tools(registry: &Registry, direct: bool) -> Value {
    let left: Vec<(&str, &str)> = left_out(direct);
    let list: Vec<Value> = tools(registry, &left)
        .into_iter()
        .map(|(name, cmd)| {
            let mut schema = Map::new();
            schema.insert("type".into(), json!("object"));
            schema.insert("properties".into(), cmd.params.get("properties").cloned().unwrap_or(json!({})));
            if let Some(req) = cmd.params.get("required").and_then(Value::as_array).filter(|r| !r.is_empty()) {
                schema.insert("required".into(), Value::Array(req.clone()));
            }
            schema.insert("additionalProperties".into(), json!(false));
            json!({
                "name": name,
                "title": cmd.summary,
                "description": describe(cmd, direct),
                "inputSchema": schema,
                "annotations": {
                    "readOnlyHint": !cmd.mutates && !cmd.writes_direct,
                    "destructiveHint": DESTRUCTIVE.contains(&cmd.name) || cmd.name.ends_with(".delete"),
                    "openWorldHint": false
                }
            })
        })
        .collect();
    Value::Array(list)
}

fn left_out(direct: bool) -> Vec<(&'static str, &'static str)> {
    if direct {
        LEFT_OUT.iter().copied().filter(|(n, _)| *n == "proposal.apply" || *n == "proposal.reject").collect()
    } else {
        LEFT_OUT.to_vec()
    }
}

/// The initialize result.
pub fn initialize_result(asked: Option<&str>) -> Value {
    let version = asked.filter(|a| SUPPORTED.contains(a)).unwrap_or(PROTOCOL_VERSION);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "pixellab", "title": "Pixel Lab", "version": crate::server_stdio::version() },
        "instructions": INSTRUCTIONS
    })
}

/// Runs the MCP server until its input ends; returns the exit code.
pub fn start(
    document: Document,
    env: Env,
    registry: &Registry,
    direct: bool,
    input: Box<dyn BufRead + Send>,
    output: Box<dyn Write + Send>,
) -> i32 {
    let chat = chat_of(&env);
    let mut session =
        Session::new(document, Options { env, mode: if direct { Mode::Direct } else { Mode::Propose }, chat, ..Options::default() });
    let left = left_out(direct);
    let rpc = Output::new(output);
    let mut handle = |method: &str, params: Value, _: &Value| -> Result<Value, Failure> {
        match method {
            "initialize" => {
                let client = params.get("clientInfo");
                session.agent =
                    client.and_then(|c| c.get("name")).and_then(Value::as_str).filter(|n| !n.is_empty()).map(|name| {
                        match client.and_then(|c| c.get("version")).and_then(Value::as_str).filter(|v| !v.is_empty()) {
                            Some(v) => format!("{name} {v}"),
                            None => name.to_string(),
                        }
                    });
                Ok(initialize_result(params.get("protocolVersion").and_then(Value::as_str)))
            }
            "notifications/initialized" | "notifications/cancelled" => Ok(Value::Null),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": list_tools(registry, direct) })),
            "tools/call" => call_tool(&mut session, registry, &left, &params),
            m if m.starts_with("notifications/") => Ok(Value::Null),
            m => Err(RpcError::new(METHOD_NOT_FOUND, format!("no method {m}")).into()),
        }
    };
    serve_lines(input, &rpc, &mut handle, &mut |t| log(t));
    0
}

/// tools/call (`callTool`).
fn call_tool(session: &mut Session, registry: &Registry, left: &[(&str, &str)], params: &Value) -> Result<Value, Failure> {
    let name = params.get("name");
    let args = params.get("arguments").filter(|a| !a.is_null()).cloned().unwrap_or(json!({}));
    let name_text = engine::engine::js_string(name);
    let cmd = name
        .and_then(Value::as_str)
        .and_then(|n| registry.commands.iter().find(|c| !left.iter().any(|(l, _)| *l == c.name) && tool_name(c.name) == n));
    let time = iso_time(now_ms());
    let append = |session: &Session, ok: bool, summary: &str| {
        let mut entry = Map::new();
        entry.insert("time".into(), json!(time));
        entry.insert("tool".into(), json!(name_text));
        entry.insert("args".into(), args.clone());
        entry.insert("ok".into(), json!(ok));
        entry.insert("summary".into(), json!(summary));
        if let Some(a) = &session.agent {
            entry.insert("agent".into(), json!(a));
        }
        if let Some(c) = &session.chat {
            entry.insert("chat".into(), c.clone());
        }
        if let Err(e) = session.state.append_activity(&Value::Object(entry)) {
            log(&format!("can't write the activity log: {}", e.message()));
        }
    };
    let Some(cmd) = cmd else {
        append(session, false, &format!("unknown tool {name_text}"));
        let why = name.and_then(Value::as_str).and_then(|n| left.iter().find(|(l, _)| *l == n.replace('_', ".")).map(|(_, w)| *w));
        return Err(RpcError::new(
            INVALID_PARAMS,
            format!("unknown tool {name_text}{}", why.map(|w| format!(": {w}")).unwrap_or_default()),
        )
        .into());
    };
    session.sync();
    match registry.call(&mut Ctx::new(session), cmd.name, args.clone(), false) {
        Ok(result) => {
            let summary = activity_summary(cmd, &result, args.as_object().unwrap_or(&Map::new()));
            append(session, true, &summary);
            if let Some(png) = result.get("png").and_then(Value::as_str) {
                let mut rest = result.as_object().cloned().unwrap_or_default();
                rest.remove("png");
                let rest = Value::Object(rest);
                return Ok(json!({
                    "content": [
                        { "type": "image", "data": png, "mimeType": "image/png" },
                        { "type": "text", "text": engine::format::stringify(&rest) }
                    ],
                    "structuredContent": rest
                }));
            }
            let structured = if result.is_object() { result } else { json!({ "result": result }) };
            Ok(json!({ "content": [{ "type": "text", "text": engine::format::stringify(&structured) }], "structuredContent": structured }))
        }
        Err(err) => {
            let message = err.message().to_string();
            append(session, false, message.split('\n').next().unwrap_or(""));
            let error = match &err {
                Error::Command(e) => e.to_json(),
                Error::Internal(m) => {
                    log(&format!("internal error in {name_text}: {m}"));
                    json!({ "code": "internal", "message": message })
                }
            };
            Ok(json!({ "content": [{ "type": "text", "text": message }], "structuredContent": { "error": error }, "isError": true }))
        }
    }
}
