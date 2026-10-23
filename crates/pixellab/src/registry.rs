//! The command registry (ports cli/lib/registry.mjs;
//! docs/intents/2026-10-mac-app/plan.md, "One command registry, three
//! doors"). Every action is a command: a dotted name, a summary, its params
//! as a JSON Schema object, whether it mutates, and `run(ctx, params)` →
//! JSON. The CLI, the app server and the MCP server all list and call
//! commands from here, and all check params with the same `check_params`.

#![allow(clippy::result_large_err)]

use serde_json::{Map, Value};

use crate::commands;
use crate::context::Ctx;
use crate::errors::{CommandError, Error, Result};
use crate::params::{check_params, suggest};

/// A command's params, checked and with defaults filled in.
pub type Params = Map<String, Value>;

/// A command (`Command` in registry.mjs). The door-specific fields are
/// optional there; here they have defaults (`Command::new`).
#[derive(Clone)]
pub struct Command {
    pub name: &'static str,
    pub summary: &'static str,
    /// Longer help text.
    pub description: Option<&'static str>,
    /// A JSON Schema object, keys in the order the Node CLI wrote them (the
    /// MCP tool list prints it).
    pub params: Value,
    /// Param names the CLI fills from positionals, in order.
    pub args: Vec<&'static str>,
    pub mutates: bool,
    pub run: fn(&mut Ctx, Params) -> Result<Value>,
    /// The CLI's human-readable output.
    pub format: Option<fn(&Value, &Params) -> String>,
    /// Command lines for the help.
    pub examples: Vec<&'static str>,
    /// The result has a base64 `png` the CLI can write with --out.
    pub writes_png: bool,
    /// Runs only in a session (pixellab serve).
    pub session: bool,
    /// Writes files itself (`writes: 'direct'`), so --dry-run doesn't apply.
    pub writes_direct: bool,
    /// Runs without an open document (`document: false`).
    pub needs_document: bool,
}

impl Command {
    /// A command with the door-specific fields at their defaults: no
    /// description, args, format or examples; not a PNG, a session-only or a
    /// direct-writing command; it needs a document.
    pub fn new(
        name: &'static str,
        summary: &'static str,
        params: Value,
        mutates: bool,
        run: fn(&mut Ctx, Params) -> Result<Value>,
    ) -> Command {
        Command {
            name,
            summary,
            description: None,
            params,
            args: Vec::new(),
            mutates,
            run,
            format: None,
            examples: Vec::new(),
            writes_png: false,
            session: false,
            writes_direct: false,
            needs_document: true,
        }
    }
}

/// `NAME`: `/^[a-z][a-z0-9]*(?:\.[a-z][a-z0-9]*)*$/`.
fn is_command_name(name: &str) -> bool {
    name.split('.').all(|word| {
        let mut chars = word.chars();
        chars.next().is_some_and(|c| c.is_ascii_lowercase()) && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    })
}

/// Panics if a command definition is malformed: caught by the tests, never
/// at a user (`checkDefinition`).
fn check_definition(cmd: &Command) {
    let fail = |msg: String| -> ! { panic!("command {}: {msg}", cmd.name) };
    if !is_command_name(cmd.name) {
        fail("name must be dotted lowercase words, like scene.place".into());
    }
    if cmd.summary.is_empty() {
        fail("summary is required".into());
    }
    let props = cmd.params.get("properties").and_then(Value::as_object);
    if cmd.params.get("type").and_then(Value::as_str) != Some("object") || props.is_none() {
        fail("params must be a JSON Schema object with properties".into());
    }
    let props = props.unwrap();
    for arg in &cmd.args {
        if !props.contains_key(*arg) {
            fail(format!("args names {arg}, which isn't a param"));
        }
    }
    for req in cmd.params.get("required").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
        if !props.contains_key(req) {
            fail(format!("required names {req}, which isn't a param"));
        }
    }
}

/// The registry: every command, by name, in name order.
pub struct Registry {
    pub commands: Vec<Command>,
}

impl Registry {
    /// Builds a registry; panics on a malformed definition
    /// (`checkDefinition`), which the tests catch.
    pub fn new(list: Vec<Command>) -> Registry {
        let mut commands: Vec<Command> = Vec::new();
        for cmd in list {
            check_definition(&cmd);
            if commands.iter().any(|c| c.name == cmd.name) {
                panic!("command {} is defined twice", cmd.name);
            }
            commands.push(cmd);
        }
        commands.sort_by(|a, b| a.name.cmp(b.name));
        Registry { commands }
    }

    pub fn get(&self, name: &str) -> Option<&Command> {
        self.commands.iter().find(|c| c.name == name)
    }

    pub fn has(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// Commands grouped by their first word, in name order.
    pub fn groups(&self) -> Vec<(String, Vec<&Command>)> {
        let mut out: Vec<(String, Vec<&Command>)> = Vec::new();
        for cmd in &self.commands {
            let group = cmd.name.split('.').next().unwrap_or("");
            match out.iter_mut().find(|(g, _)| g == group) {
                Some((_, list)) => list.push(cmd),
                None => out.push((group.to_string(), vec![cmd])),
            }
        }
        out
    }

    /// Checks params against the command's schema, then runs it (`call`).
    /// `coerce`: params are strings from a command line.
    pub fn call(&self, ctx: &mut Ctx, name: &str, params: Value, coerce: bool) -> Result<Value> {
        let Some(cmd) = self.get(name) else {
            let names: Vec<&str> = self.commands.iter().map(|c| c.name).collect();
            return Err(CommandError::not_found(format!("no command {name}{}", suggest(name, &names))).into());
        };
        let checked = check_params(&cmd.params, &params, coerce, Some(name))?;
        // A panic (a bug, or a document the checks let through) is an internal
        // error for this call, as a thrown `Error` was in Node: the servers keep
        // running and keep the docs open in them.
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (cmd.run)(ctx, checked))) {
            Ok(result) => result,
            Err(payload) => {
                let what = payload
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "a panic".to_string());
                Err(Error::Internal(format!("{name} failed: {what}")))
            }
        }
    }
}

/// The built-in commands, from every file in `commands/`.
pub fn registry() -> Registry {
    let mut list = Vec::new();
    list.extend(commands::document::commands());
    list.extend(commands::migrate::commands());
    list.extend(commands::scene::commands());
    list.extend(commands::scene_edit::commands());
    list.extend(commands::scene_layers::commands());
    list.extend(commands::scene_motion::commands());
    list.extend(commands::ambient::commands());
    list.extend(commands::object::commands());
    list.extend(commands::object_edit::commands());
    list.extend(commands::check::commands());
    list.extend(commands::art::commands());
    list.extend(commands::doc::commands());
    list.extend(commands::history::commands());
    list.extend(commands::activity::commands());
    list.extend(commands::chat::commands());
    Registry::new(list)
}
