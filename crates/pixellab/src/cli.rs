//! The CLI door: argv → a command run → human text or exact JSON, and an exit
//! code (0 ok, 1 command error or failed check, 2 usage error) (ports
//! cli/lib/cli.mjs). `main` takes its streams and environment as arguments,
//! so tests call it directly.
//!
//! One-shot edits: each command runs in a fresh session. A mutating
//! command's changes are saved right after it runs, validated and checked for
//! conflicts first (--force writes anyway); --dry-run prints the diff instead
//! and writes nothing. `serve` starts the stdio or MCP server.

use std::io::Write;
use std::path::{Component, Path, PathBuf};

use engine::format::{stringify, stringify_pretty};
use engine::jsnum;
use serde_json::{json, Map, Value};

use crate::argv::{command_words, parse_command_line, Parsed, GLOBAL_FLAGS};
use crate::context::Ctx;
use crate::document::{open_document, Env, OpenOptions};
use crate::errors::{CommandError, Error};
use crate::help::{help_for, markdown};
use crate::params::check_params;
use crate::registry::Registry;
use crate::session::{spread, Options, Session};

/// The streams and environment a command line runs against.
pub struct Io<'a> {
    pub stdout: &'a mut dyn Write,
    pub stderr: &'a mut dyn Write,
    pub env: Env,
    pub cwd: PathBuf,
    pub registry: &'a Registry,
}

/// `path.resolve(cwd, p)`: absolute, with `.` and `..` folded away.
fn resolve_path(cwd: &Path, p: &str) -> PathBuf {
    let joined = cwd.join(p);
    let mut out = PathBuf::new();
    for part in joined.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// `Buffer.from(text, 'base64')`: skips what isn't base64, stops at `=`.
pub fn base64_decode(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for b in text.bytes() {
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            _ => continue,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    out
}

fn js_str(v: Option<&Value>) -> String {
    match v {
        None => "undefined".into(),
        Some(Value::Null) => "null".into(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => jsnum::number_string(n),
        Some(Value::String(s)) => s.clone(),
        Some(_) => "[object Object]".into(),
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// Writes to a stream; a closed pipe stops quietly, as the Node CLI's EPIPE handler did.
fn put(stream: &mut dyn Write, text: &str) {
    let _ = stream.write_all(text.as_bytes());
    let _ = stream.flush();
}

/// Runs one command line. Never panics on bad input; returns the exit code.
/// `argv` is the arguments after `pixellab`.
pub fn main(argv: &[String], io: &mut Io) -> i32 {
    let json = argv.iter().any(|a| a == "--json");
    match run(argv, io) {
        Ok(code) => code,
        Err(Error::Command(err)) => {
            if json {
                put(io.stderr, &format!("{}\n", stringify(&json!({ "error": err.to_json() }))));
            } else {
                put(io.stderr, &format!("pixellab: {}\n", err.message));
            }
            err.exit_code
        }
        Err(Error::Internal(message)) => {
            if json {
                put(io.stderr, &format!("{}\n", stringify(&json!({ "error": { "code": "internal", "message": message } }))));
            } else {
                put(io.stderr, &format!("pixellab: internal error: {message}\n"));
            }
            1
        }
    }
}

fn run(argv: &[String], io: &mut Io) -> Result<i32, Error> {
    // `pixellab --version`: the release's version (spec R27), or 0.0.0-dev.
    if argv.len() == 1 && (argv[0] == "--version" || argv[0] == "-V") {
        put(io.stdout, &format!("pixellab {}\n", crate::server_stdio::version()));
        return Ok(0);
    }
    let registry = io.registry;
    for cmd in &registry.commands {
        let props = cmd.params.get("properties").and_then(Value::as_object);
        for key in engine::serialize::keys(props.unwrap_or(&Map::new())) {
            if GLOBAL_FLAGS.iter().any(|f| f.name == key) && key != "force" {
                return Err(Error::Internal(format!("command {}: param {key} clashes with the global --{key}", cmd.name)));
            }
        }
    }
    if argv.first().map(String::as_str) == Some("serve") {
        return Ok(crate::serve::serve(&argv[1..], io));
    }
    let parsed = parse_command_line(argv, registry)?;
    let (command, mut params, globals) = match parsed {
        Parsed::Help { topic, usage, .. } => {
            if topic.is_empty() && !usage && argv.first().map(String::as_str) == Some("help") && argv.iter().any(|a| a == "--markdown") {
                put(io.stdout, &markdown(registry));
                return Ok(0);
            }
            let Some(text) = help_for(registry, &topic) else {
                return Err(CommandError::usage(format!("no help for {}; run pixellab help", topic.join(" "))).into());
            };
            // A bare `pixellab` or `pixellab scene` is a usage error that shows help.
            put(if usage { io.stderr } else { io.stdout }, &format!("{text}\n"));
            return Ok(if usage { 2 } else { 0 });
        }
        Parsed::Command { command, params, globals } => (command, params, globals),
    };
    let dry_run = globals.dry_run.unwrap_or(false);
    let force = globals.force.unwrap_or(false);
    if globals.out.is_some() && !command.writes_png {
        return Err(CommandError::usage(format!("--out goes with a render command, not {}", command_words(command.name))).into());
    }
    if command.session {
        return Err(CommandError::usage(format!(
            "{} works on a session's open docs: run it through pixellab serve --stdio or --mcp. On the command line, edits are saved as they're made.",
            command_words(command.name)
        ))
        .into());
    }
    if dry_run && (!command.mutates || command.writes_direct) {
        return Err(CommandError::usage(format!("--dry-run goes with an edit command, not {}", command_words(command.name))).into());
    }
    let has_force_param = command.params.get("properties").and_then(Value::as_object).is_some_and(|p| p.contains_key("force"));
    if force && !command.mutates && !has_force_param {
        return Err(CommandError::usage(format!("--force goes with an edit command, not {}", command_words(command.name))).into());
    }
    if force && has_force_param {
        params.insert("force".into(), Value::String("true".into()));
    }
    // Params are checked before the site is read, so a usage error is always exit 2.
    let checked = check_params(&command.params, &Value::Object(params), true, Some(command.name))?;
    if !command.needs_document {
        // It makes a document, so there's none to open first (a Mac with no site checkout has no default).
        let mut ctx = Ctx::none();
        ctx.open =
            Some(OpenOptions { doc: globals.document.clone(), site: globals.site.clone(), env: io.env.clone(), cwd: io.cwd.clone() });
        let result = registry.call(&mut ctx, command.name, Value::Object(checked.clone()), false)?;
        if globals.json {
            put(io.stdout, &format!("{}\n", stringify_pretty(&result)));
        } else {
            let text = command.format.map_or_else(|| stringify_pretty(&result), |f| f(&result, &checked));
            put(io.stdout, &format!("{text}\n"));
        }
        return Ok(0);
    }
    let document = open_document(&OpenOptions {
        doc: globals.document.clone(),
        site: globals.site.clone(),
        env: io.env.clone(),
        cwd: io.cwd.clone(),
    })?;
    let mut session = Session::new(document, Options { env: io.env.clone(), ..Options::default() });
    let mut result = {
        let mut ctx = Ctx::new(&mut session);
        registry.call(&mut ctx, command.name, Value::Object(checked.clone()), false)?
    };
    let mut notes: Vec<String> = Vec::new();
    if command.mutates && !command.writes_direct {
        let touched = session.dirty_refs();
        let mut files: Vec<String> = Vec::new();
        for r in &touched {
            files.push(session.entry(r)?.file.clone());
        }
        let extra = if dry_run {
            let mut plans: Vec<Value> = Vec::new();
            for (r, file) in touched.iter().zip(&files) {
                let plan = session.plan(r)?;
                plans.push(json!({
                    "doc": r,
                    "file": file,
                    "diff": plan.diff,
                    "problems": plan.problems,
                    "conflict": plan.conflict,
                }));
                notes.push(plan.diff.trim_end().to_string());
                if !plan.problems.is_empty() {
                    let n = plan.problems.len();
                    notes.push(format!("{file} has {n} problem{}; saving it would be refused (--force writes anyway):", plural(n)));
                    notes.extend(plan.problems.iter().cloned());
                }
                if plan.conflict.is_some() {
                    notes.push(format!("{file} changed on disk; saving would be refused as a conflict"));
                }
            }
            notes.push(if touched.is_empty() {
                "dry run: nothing would change".to_string()
            } else {
                format!("dry run: nothing written ({} would change)", files.join(", "))
            });
            ("dryRun", Value::Array(plans))
        } else {
            let mut saved: Vec<Value> = Vec::new();
            for r in &touched {
                let saved_one = session.save(r, force)?;
                let get = |k: &str| saved_one.get(k).cloned().unwrap_or(Value::Null);
                let file = js_str(saved_one.get("file"));
                let wrote = saved_one.get("wrote").and_then(Value::as_bool).unwrap_or(false);
                let mut entry = Map::new();
                entry.insert("doc".into(), get("doc"));
                entry.insert("file".into(), get("file"));
                entry.insert("wrote".into(), get("wrote"));
                entry.insert("hash".into(), get("hash"));
                notes.push(if wrote { format!("wrote {file}") } else { format!("{file} unchanged") });
                if let Some(forced) = saved_one.get("forcedProblems").and_then(Value::as_array) {
                    entry.insert("forcedProblems".into(), Value::Array(forced.clone()));
                    notes.push(format!("(forced: {} problem{} written)", forced.len(), plural(forced.len())));
                }
                saved.push(Value::Object(entry));
            }
            if touched.is_empty() {
                notes.push("nothing written".into());
            }
            ("saved", Value::Array(saved))
        };
        if let Value::Object(map) = result {
            let mut add = Map::new();
            add.insert(extra.0.into(), extra.1);
            result = Value::Object(spread(map, add));
        }
    }
    let mut wrote: Option<String> = None;
    if let Some(out) = &globals.out {
        let file = resolve_path(&io.cwd, out);
        let png = result.get("png").and_then(Value::as_str).ok_or_else(|| {
            Error::Internal("The first argument must be of type string or an instance of Buffer, ArrayBuffer, or Array or an Array-like Object. Received undefined".into())
        })?;
        if let Err(e) = std::fs::write(&file, base64_decode(png)) {
            let message = if e.kind() == std::io::ErrorKind::NotFound {
                format!("ENOENT: no such file or directory, open '{}'", file.display())
            } else {
                e.to_string()
            };
            return Err(Error::Internal(message));
        }
        wrote = Some(format!("wrote {} ({}×{})", file.display(), js_str(result.get("width")), js_str(result.get("height"))));
    }
    if globals.json {
        put(io.stdout, &format!("{}\n", stringify_pretty(&result)));
    } else {
        let text = command.format.map_or_else(|| stringify_pretty(&result), |f| f(&result, &checked));
        let hint =
            if command.writes_png && wrote.is_none() { "(add --out <file.png> to write the PNG, or --json for it as base64)" } else { "" };
        let mut parts: Vec<String> = vec![text];
        parts.extend(wrote);
        parts.extend(notes);
        parts.push(hint.to_string());
        let lines: Vec<String> = parts.into_iter().filter(|p| !p.is_empty()).collect();
        put(io.stdout, &format!("{}\n", lines.join("\n")));
    }
    Ok(if result.get("ok") == Some(&Value::Bool(false)) { 1 } else { 0 })
}
