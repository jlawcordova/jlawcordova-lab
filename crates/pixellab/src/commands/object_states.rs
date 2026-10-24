//! The state commands (docs/intents/2026-10-object-states/spec.md, D3): `object.state.*` and
//! `object.states.make` edit a sprite's states, and `object.state.run` reads what a state plays.

use engine::engine::resolve;
use engine::jsnum::num;
use engine::states::{self, Settle};
use serde_json::{json, Value};

use super::common::{obj, opt_num, opt_str, str_of};
use crate::context::{Ctx, DocKind};
use crate::errors::{CommandError, Result};
use crate::registry::{Command, Params};

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
    vec![run]
}
