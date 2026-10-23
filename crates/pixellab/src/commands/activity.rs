//! The activity log of agents' MCP tool calls (ports the activity half of
//! cli/lib/commands/proposal.mjs): activity.list tails it for the Agent window.

use serde_json::{json, Value};

use super::common::{opt_num, str_of};
use crate::context::Ctx;
use crate::errors::{CommandError, Result};
use crate::registry::{Command, Params};
use crate::state::State;

/// The state dir of a ctx's session.
fn state_of(ctx: &mut Ctx) -> Result<State> {
    match ctx.session.as_deref() {
        Some(s) => Ok(s.state.clone()),
        None => Err(CommandError::new("no state dir in this ctx", "no_state").into()),
    }
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

    vec![activity]
}
