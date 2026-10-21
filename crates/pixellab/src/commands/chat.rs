//! chat.ask and chat.context (ports cli/lib/commands/chat.mjs).
//!
//! Pixel Lab's Agent chat (docs/intents/2026-10-agent-chat/spec.md): the app
//! runs the owner's Claude Code with this CLI's MCP server, and these
//! commands are the chat's own. chat.context builds the selection block a
//! message carries (D5): what's selected, as the saved document has it.
//! chat.ask puts a question with answer buttons in front of the owner (D8):
//! it returns at once, and the owner's choice comes back as their next message.

use serde_json::{json, Value};

use super::common::{opt_str, str_of};
use crate::chat_context::{object_context, selection_context, SelectionInput};
use crate::context::{Ctx, DocKind};
use crate::errors::{CommandError, Error, Result};
use crate::registry::{Command, Params};
use crate::session::{import_source, read_source};

fn name_param() -> Value {
    json!({ "type": "string", "description": "scene name (or scenes/<name>)" })
}

/// The doc's saved file: its data and the version the block names (the first
/// 8 hex digits of its hash) (`savedDoc`).
fn saved_doc(ctx: &mut Ctx, kind: DocKind, name: &str) -> Result<(Value, String)> {
    let disk = read_source(ctx.document()?, kind, name);
    if !disk.exists {
        return Ok((json!({ "items": [] }), "unsaved".into()));
    }
    let data = import_source(&disk.text).map_err(Error::Internal)?;
    let version: String = disk.hash.unwrap_or_default().chars().take(8).collect();
    Ok((data, version))
}

/// The document's name for the block: its manifest's, else its folder's.
fn document_name(ctx: &Ctx) -> Result<String> {
    Ok(ctx.document()?.name.clone())
}

fn string_list(params: &Params, key: &str) -> Vec<String> {
    params.get(key).and_then(Value::as_array).map(|l| l.iter().map(|v| str_of(Some(v))).collect()).unwrap_or_default()
}

fn context_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    if params.get("object").is_some_and(|o| engine::engine::truthy(Some(o))) {
        let found = ctx.find(opt_str(&params, "object").unwrap_or(""), Some(DocKind::Object))?.name;
        let (_, version) = saved_doc(ctx, DocKind::Object, &found)?;
        return Ok(object_context(&document_name(ctx)?, &found, &version));
    }
    let name = match params.get("name") {
        Some(n) if engine::engine::truthy(Some(n)) => str_of(Some(n)),
        _ => return Err(CommandError::usage("chat.context needs a scene name, or --object").into()),
    };
    let scene = ctx.find(&name, Some(DocKind::Scene))?.name;
    let working = ctx.doc(DocKind::Scene, &scene)?;
    let (saved, version) = saved_doc(ctx, DocKind::Scene, &scene)?;
    let document = document_name(ctx)?;
    selection_context(&SelectionInput {
        document: &document,
        scene: &scene,
        version: &version,
        working: &working,
        saved: &saved,
        layers: &string_list(&params, "layers"),
        items: &string_list(&params, "items"),
        pinned: &string_list(&params, "pinned"),
    })
}

fn context_format(r: &Value, _: &Params) -> String {
    let unsaved = r["unsaved"].as_array().cloned().unwrap_or_default();
    let tail = if unsaved.is_empty() {
        String::new()
    } else {
        format!("\nleft out (not saved): {}", unsaved.iter().map(|u| str_of(Some(u))).collect::<Vec<_>>().join(", "))
    };
    format!("{}{tail}", str_of(r.get("text")))
}

fn ask_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let q = str_of(params.get("question")).trim().to_string();
    let answers: Vec<String> = params
        .get("options")
        .and_then(Value::as_array)
        .map(|l| l.iter().map(|o| str_of(Some(o)).trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();
    if q.is_empty() {
        return Err(CommandError::usage("chat.ask needs a question").into());
    }
    if answers.len() < 2 || answers.len() > 4 {
        return Err(CommandError::usage(format!("chat.ask needs two to four answers (got {})", answers.len())).into());
    }
    if answers.iter().any(|a| engine::engine::js_len(a) > 60) {
        return Err(CommandError::usage("each answer is a button label: 60 characters at most").into());
    }
    let chat = ctx.session.as_deref().and_then(|s| s.chat.clone());
    Ok(match chat {
        Some(chat) => json!({
            "asked": true,
            "question": q,
            "options": answers,
            "chat": chat,
            "note": "Pixel Lab shows your question with these answers as buttons. Their choice arrives as their next message. End your turn now without doing more.",
        }),
        None => json!({
            "asked": false,
            "question": q,
            "options": answers,
            "note": "No Pixel Lab chat is attached to this connection: ask in your own conversation.",
        }),
    })
}

fn ask_format(r: &Value, _: &Params) -> String {
    let options = r["options"].as_array().cloned().unwrap_or_default();
    let list = options.iter().enumerate().map(|(i, o)| format!("  {}. {}", i + 1, str_of(Some(o)))).collect::<Vec<_>>().join("\n");
    let tail = if r["asked"].as_bool().unwrap_or(false) { String::new() } else { format!("\n({})", str_of(r.get("note"))) };
    format!("{}\n{list}{tail}", str_of(r.get("question")))
}

/// This file's commands.
pub fn commands() -> Vec<Command> {
    let mut context = Command::new(
        "chat.context",
        "Build the selection block an Agent chat message carries: what's selected, as the saved document has it",
        json!({
            "type": "object",
            "properties": {
                "name": name_param(),
                "object": { "type": "string", "description": "an object: the block names the whole object (object mode) instead of a scene" },
                "layers": { "type": "array", "items": { "type": "string" }, "description": "selected layer ids, comma-separated on the command line" },
                "items": { "type": "array", "items": { "type": "string" }, "description": "selected item paths (12, 5.2), comma-separated on the command line" },
                "pinned": { "type": "array", "items": { "type": "string" }, "description": "which of those are pinned to the chat: layer:<id> or a path" }
            }
        }),
        false,
        context_run,
    );
    context.description = Some(
        "Pixel Lab sends this block with each chat message. Layers and items are named as the app has them (its working copy, which may have unsaved edits); the block names them by their paths in the saved file. An item placed, moved or changed since the last save, or a layer made or changed since, isn't in the saved file: it's left out and listed in unsaved. With --object, the block names a whole object instead.",
    );
    context.args = vec!["name"];
    context.format = Some(context_format);
    context.examples = vec!["pixellab chat context harbor --items 45.0,46.0 --layers boats"];

    let mut ask = Command::new(
        "chat.ask",
        "Ask the owner a question with two to four answers, shown as buttons in Pixel Lab's Agent chat",
        json!({
            "type": "object",
            "properties": {
                "question": { "type": "string", "description": "the question, in one or two sentences" },
                "options": { "type": "array", "items": { "type": "string" }, "description": "two to four short answers, as button labels" }
            },
            "required": ["question", "options"]
        }),
        false,
        ask_run,
    );
    ask.description = Some(
        "Use it when the owner should choose (every use of an object or only these, one way or another). It returns at once: the question shows in the chat with a button per answer, and the owner's choice arrives as their next message, so end your turn after asking. With no Pixel Lab chat on this connection (a terminal), it says so: ask in your own conversation.",
    );
    ask.args = vec!["question"];
    ask.format = Some(ask_format);
    ask.examples = vec!["pixellab chat ask \"Make every pine taller, or only these 2?\" --options \"Only these 2,Every pine (8)\""];

    vec![context, ask]
}
