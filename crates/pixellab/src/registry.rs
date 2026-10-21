//! The command registry (ports cli/lib/registry.mjs;
//! docs/intents/2026-10-mac-app/plan.md, "One command registry, three
//! doors"). Every action is a command: a dotted name, a summary, its params
//! as a JSON Schema object, whether it mutates, and `run(ctx, params)` →
//! JSON. The CLI, the app server and the MCP server all list and call
//! commands from here, and all check params with the same `check_params`.

use serde_json::{Map, Value};

use crate::commands;
use crate::context::Ctx;
use crate::errors::Result;

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

/// The registry: every command, by name, in name order.
pub struct Registry {
    pub commands: Vec<Command>,
}

impl Registry {
    /// Builds a registry; panics on a malformed definition
    /// (`checkDefinition`), which the tests catch.
    pub fn new(list: Vec<Command>) -> Registry {
        let _ = list;
        todo!("U21: port registry.mjs createRegistry")
    }

    pub fn get(&self, name: &str) -> Option<&Command> {
        self.commands.iter().find(|c| c.name == name)
    }

    pub fn has(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// Commands grouped by their first word, in name order.
    pub fn groups(&self) -> Vec<(String, Vec<&Command>)> {
        todo!("U21: port registry.mjs groups")
    }

    /// Checks params against the command's schema, then runs it (`call`).
    /// `coerce`: params are strings from a command line.
    pub fn call(&self, ctx: &mut Ctx, name: &str, params: Value, coerce: bool) -> Result<Value> {
        let _ = (ctx, name, params, coerce);
        todo!("U21: port registry.mjs call")
    }
}

/// The built-in commands, from every file in `commands/`.
pub fn registry() -> Registry {
    let mut list = Vec::new();
    list.extend(commands::document::commands());
    list.extend(commands::scene::commands());
    list.extend(commands::scene_edit::commands());
    list.extend(commands::scene_layers::commands());
    list.extend(commands::scene_motion::commands());
    list.extend(commands::object::commands());
    list.extend(commands::object_edit::commands());
    list.extend(commands::check::commands());
    list.extend(commands::art::commands());
    list.extend(commands::doc::commands());
    list.extend(commands::proposal::commands());
    list.extend(commands::chat::commands());
    Registry::new(list)
}
