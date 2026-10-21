//! The command line → a command and its params, and the global flags (ports cli/lib/argv.mjs).
//!
//! ```text
//! pixellab <group> <verb> [positionals] [--flags] [--json] [--site <path>] [--out <file>]
//! ```
//!
//! The rules, the same for every command (the help prints them too):
//! - The leading words name the command: `scene render` is scene.render. The
//!   longest run of words that names a command wins, so `check hero-island`
//!   is `check` with one positional.
//! - Positionals fill the command's `args` in order (its usage line shows
//!   them). If the last arg is a list, it takes the rest.
//! - Every param is also a flag, its name in kebab-case: --name, --scale.
//!   `--flag value` and `--flag=value` both work.
//! - Booleans: `--flag` (true), `--no-flag` (false), or `--flag=true|false`.
//! - Lists: comma-separated (`--at 14,6,1`) or the flag repeated.
//! - Objects: JSON (`--data '{"a": 1}'`).
//! - `--` ends the flags; everything after it is positional.
//! - Global flags go anywhere: --json, --site <path>, --out <file>,
//!   --dry-run, --force, -h/--help.
//!
//! Values stay strings here; `check_params` coerces them to the schema's
//! types.

#![allow(clippy::result_large_err)]

use engine::serialize::keys;
use serde_json::{Map, Value};

use crate::errors::CommandError;
use crate::params::suggest;
use crate::registry::{Command, Registry};

/// A global flag: whether it takes a value, and its help line.
pub struct GlobalFlag {
    pub name: &'static str,
    pub value: bool,
    pub help: &'static str,
}

/// The global flags, in the order the help lists them.
pub static GLOBAL_FLAGS: &[GlobalFlag] = &[
    GlobalFlag { name: "json", value: false, help: "print the command's result as JSON" },
    GlobalFlag {
        name: "document",
        value: true,
        help: "the .pixellab document (else $PIXELLAB_DOC, ~/.config/pixellab/config.json, or the document in --site)",
    },
    GlobalFlag {
        name: "site",
        value: true,
        help: "a site checkout holding the document (else $PIXELLAB_SITE, the config file, ~/Projects/jlawcordova.github.io)",
    },
    GlobalFlag { name: "out", value: true, help: "write the PNG of a render command to this file" },
    GlobalFlag { name: "dry-run", value: false, help: "an edit command shows what it would write (a diff) and writes nothing" },
    GlobalFlag { name: "force", value: false, help: "an edit command writes despite validation problems or a file changed on disk" },
    GlobalFlag { name: "help", value: false, help: "help for the command or group" },
];

/// The global flags as given (`Globals`). `dry_run` and `force` are None
/// unless the flag was given.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Globals {
    pub json: bool,
    pub help: bool,
    pub document: Option<String>,
    pub site: Option<String>,
    pub out: Option<String>,
    pub dry_run: Option<bool>,
    pub force: Option<bool>,
}

/// What a command line asks for (`Parsed`).
pub enum Parsed<'a> {
    /// Help on a topic: `usage` is true when it was asked for by typing
    /// nothing or only a group (the short usage), not by `help` or `--help`.
    Help { topic: Vec<String>, globals: Globals, usage: bool },
    /// A command and its params (strings, or lists of strings).
    Command { command: &'a Command, params: Map<String, Value>, globals: Globals },
}

/// camelCase → kebab-case, for flag names.
pub fn kebab(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            out.push('-');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// The words a command is typed as: scene.render → "scene render".
pub fn command_words(name: &str) -> String {
    name.split('.').collect::<Vec<_>>().join(" ")
}

/// `/^--([a-z][a-z0-9-]*)(?:=(.*))?$/s`: the flag's name and its `=value`.
fn global_flag_shape(token: &str) -> Option<(&str, Option<&str>)> {
    let body = token.strip_prefix("--")?;
    let name_len = body.bytes().position(|b| !(b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')).unwrap_or(body.len());
    let name = &body[..name_len];
    if !name.starts_with(|c: char| c.is_ascii_lowercase()) {
        return None;
    }
    match &body[name_len..] {
        "" => Some((name, None)),
        rest => rest.strip_prefix('=').map(|v| (name, Some(v))),
    }
}

/// Pulls the global flags out of argv, wherever they are.
fn take_globals(argv: &[String]) -> Result<(Globals, Vec<String>), CommandError> {
    let mut globals = Globals::default();
    let mut rest: Vec<String> = Vec::new();
    let mut i = 0;
    while i < argv.len() {
        let token = &argv[i];
        if token == "--" {
            rest.extend(argv[i..].iter().cloned());
            break;
        }
        if token == "-h" {
            globals.help = true;
            i += 1;
            continue;
        }
        let shape =
            global_flag_shape(token).and_then(|(name, value)| GLOBAL_FLAGS.iter().find(|f| f.name == name).map(|spec| (spec, value)));
        let Some((spec, inline)) = shape else {
            rest.push(token.clone());
            i += 1;
            continue;
        };
        let name = spec.name;
        if !spec.value {
            if let Some(v) = inline {
                if v != "true" && v != "false" {
                    return Err(CommandError::usage(format!("--{name} takes no value")));
                }
            }
            let on = inline != Some("false");
            match name {
                "json" => globals.json = on,
                "help" => globals.help = on,
                "dry-run" => globals.dry_run = Some(on),
                _ => globals.force = Some(on),
            }
            i += 1;
            continue;
        }
        let value = match inline {
            Some(v) => v.to_string(),
            None => {
                if i + 1 >= argv.len() {
                    return Err(CommandError::usage(format!("--{name} needs a value")));
                }
                i += 1;
                argv[i].clone()
            }
        };
        let slot = match name {
            "document" => &mut globals.document,
            "site" => &mut globals.site,
            _ => &mut globals.out,
        };
        if slot.is_some() {
            return Err(CommandError::usage(format!("--{name} is given twice")));
        }
        *slot = Some(value);
        i += 1;
    }
    Ok((globals, rest))
}

/// Parses a command line against the registry. `argv` is the arguments after
/// `pixellab`.
pub fn parse_command_line<'a>(argv: &[String], registry: &'a Registry) -> Result<Parsed<'a>, CommandError> {
    let (globals, rest) = take_globals(argv)?;
    let mut words: Vec<String> = Vec::new();
    let mut i = 0;
    while i < rest.len() && !rest[i].starts_with('-') {
        words.push(rest[i].clone());
        i += 1;
    }
    if words.first().map(String::as_str) == Some("help") {
        return Ok(Parsed::Help { topic: words[1..].to_vec(), globals, usage: false });
    }
    if words.is_empty() {
        if i < rest.len() && rest[i] != "--" {
            return Err(CommandError::usage(format!("a command comes before {}; run pixellab help", rest[i])));
        }
        let usage = !globals.help;
        return Ok(Parsed::Help { topic: Vec::new(), globals, usage });
    }
    let mut found: Option<(&Command, usize)> = None;
    for n in (1..=words.len()).rev() {
        let name = words[..n].join(".");
        if let Some(command) = registry.get(&name) {
            found = Some((command, n));
            break;
        }
    }
    let Some((command, used)) = found else {
        let group = &words[0];
        let groups = registry.groups();
        let is_group = groups.iter().any(|(g, _)| g == group);
        if is_group && (globals.help || words.len() == 1) {
            let usage = !globals.help;
            return Ok(Parsed::Help { topic: vec![group.clone()], globals, usage });
        }
        let typed = words[..words.len().min(if is_group { 2 } else { 1 })].join(" ");
        let names: Vec<String> = if is_group {
            registry.commands.iter().map(|c| command_words(c.name)).collect()
        } else {
            groups.iter().map(|(g, _)| g.clone()).collect()
        };
        return Err(CommandError::usage(format!("unknown command {typed}{}; run pixellab help", suggest(&typed, &names))));
    };
    if globals.help {
        return Ok(Parsed::Help { topic: command.name.split('.').map(String::from).collect(), globals, usage: false });
    }
    let mut tokens: Vec<String> = words[used..].to_vec();
    tokens.extend(rest[i..].iter().cloned());
    let params = parse_params(command, &tokens)?;
    Ok(Parsed::Command { command, params, globals })
}

/// `(flag name, param)` pairs in insertion order, a repeat keeping its place.
struct Flags(Vec<(String, String)>);

impl Flags {
    fn set(&mut self, flag: String, key: &str) {
        match self.0.iter_mut().find(|(f, _)| *f == flag) {
            Some(entry) => entry.1 = key.to_string(),
            None => self.0.push((flag, key.to_string())),
        }
    }

    fn get(&self, flag: &str) -> Option<&str> {
        self.0.iter().find(|(f, _)| f == flag).map(|(_, k)| k.as_str())
    }
}

/// Sets a param from a flag or positional: a list collects, anything else
/// may be given once.
fn set_param(
    params: &mut Map<String, Value>,
    props: &Map<String, Value>,
    words: &str,
    key: &str,
    value: &str,
    how: &str,
) -> Result<(), CommandError> {
    let is_list = props.get(key).and_then(|p| p.get("type")).and_then(Value::as_str) == Some("array");
    let value = Value::String(value.to_string());
    if let Some(existing) = params.get_mut(key) {
        if !is_list {
            return Err(CommandError::usage(format!("{words}: {how} is given twice")));
        }
        match existing {
            Value::Array(list) => list.push(value),
            other => *other = Value::Array(vec![other.clone(), value]),
        }
    } else {
        params.insert(key.to_string(), if is_list { Value::Array(vec![value]) } else { value });
    }
    Ok(())
}

/// Positionals and flags → params (as strings), for one command.
pub fn parse_params(command: &Command, tokens: &[String]) -> Result<Map<String, Value>, CommandError> {
    let empty = Map::new();
    let props = command.params.get("properties").and_then(Value::as_object).unwrap_or(&empty);
    let type_of = |key: &str| props.get(key).and_then(|p| p.get("type")).and_then(Value::as_str);
    let mut flags = Flags(Vec::new());
    for key in keys(props) {
        flags.set(kebab(key), key);
        flags.set(key.clone(), key);
    }
    let mut params = Map::new();
    let words = command_words(command.name);
    let mut positionals: Vec<String> = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];
        i += 1;
        if token == "--" {
            positionals.extend(tokens[i..].iter().cloned());
            break;
        }
        if !token.starts_with("--") {
            let mut chars = token.chars();
            if chars.next() == Some('-') && chars.next().is_some_and(|c| c != '-' && !c.is_ascii_digit()) {
                return Err(CommandError::usage(format!("{words}: unknown flag {token}; run pixellab {words} --help")));
            }
            positionals.push(token.clone());
            continue;
        }
        // /^--([^=]+)(?:=(.*))?$/s
        let body = &token[2..];
        let (raw, mut value) = match body.split_once('=') {
            Some((r, v)) => (r, Some(v.to_string())),
            None => (body, None),
        };
        if raw.is_empty() {
            return Err(CommandError::new("Cannot read properties of null (reading '1')", "internal"));
        }
        let key = flags.get(raw).map(String::from);
        if key.is_none() {
            if let Some(base) = raw.strip_prefix("no-") {
                if let Some(base_key) = flags.get(base).filter(|k| type_of(k) == Some("boolean")).map(String::from) {
                    if value.is_some() {
                        return Err(CommandError::usage(format!("{words}: --{raw} takes no value")));
                    }
                    set_param(&mut params, props, &words, &base_key, "false", &format!("--{base}"))?;
                    continue;
                }
            }
        }
        let Some(key) = key else {
            let mut known: Vec<String> = Vec::new();
            for (flag, _) in &flags.0 {
                let shown = format!("--{}", kebab(flag));
                if !known.contains(&shown) {
                    known.push(shown);
                }
            }
            return Err(CommandError::usage(format!(
                "{words}: unknown flag --{raw}{}; run pixellab {words} --help",
                suggest(&format!("--{raw}"), &known)
            )));
        };
        if type_of(&key) == Some("boolean") {
            let v = value.unwrap_or_else(|| "true".to_string());
            set_param(&mut params, props, &words, &key, &v, &format!("--{}", kebab(&key)))?;
            continue;
        }
        if value.is_none() {
            if i >= tokens.len() {
                return Err(CommandError::usage(format!("{words}: --{} needs a value", kebab(&key))));
            }
            value = Some(tokens[i].clone());
            i += 1;
        }
        let v = value.unwrap_or_default();
        set_param(&mut params, props, &words, &key, &v, &format!("--{}", kebab(&key)))?;
    }
    let args = &command.args;
    for (n, value) in positionals.iter().enumerate() {
        let key = if n < args.len() { Some(args[n]) } else { args.last().copied().filter(|last| type_of(last) == Some("array")) };
        let Some(key) = key else {
            let extra = positionals[args.len()..].join(" ");
            return Err(CommandError::usage(format!("{words}: unexpected {extra}; run pixellab {words} --help")));
        };
        if params.contains_key(key) && !(type_of(key) == Some("array") && n >= args.len()) {
            return Err(CommandError::usage(format!("{words}: {key} is given twice")));
        }
        set_param(&mut params, props, &words, key, value, key)?;
    }
    Ok(params)
}
