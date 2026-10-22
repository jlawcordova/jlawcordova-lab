//! The help text, from the registry (ports cli/lib/help.mjs), and the
//! generated command reference (ports scripts/docs-cli.mjs): `pixellab help
//! --markdown` prints exactly the text of docs/cli.md.

use engine::format::stringify;
use engine::jsnum;
use engine::serialize::entries;
use serde_json::{Map, Value};

use crate::argv::{command_words, kebab, GLOBAL_FLAGS};
use crate::registry::{Command, Registry};

const MAPPING: &str = "Arguments and flags (the same rules for every command):
  Positionals fill the command's arguments in the order its usage shows them.
  Every param is also a --flag, its name in kebab-case: --name hero-island.
  --flag value and --flag=value both work; -- ends the flags.
  Booleans: --flag, --no-flag, or --flag=true|false.
  Lists: comma-separated (--at 14,6,1) or the flag repeated.
  Objects: JSON (--data '{\"a\": 1}').
  Values are checked against the command's params schema (pixellab <command> --help).";

const EXIT: &str = "Exit codes: 0 ok, 1 command error (or check found problems), 2 usage error.";

/// `String(v)` for a JSON value (numbers as JavaScript prints them).
fn js_str(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => jsnum::number_string(n),
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(|v| if v.is_null() { String::new() } else { js_str(v) }).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}

/// A string's `.length`: UTF-16 code units.
fn ulen(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `s.padEnd(n)`.
fn pad(s: &str, n: usize) -> String {
    let len = ulen(s);
    if len >= n {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(n - len))
    }
}

fn type_of(schema: &Value) -> Option<&str> {
    schema.get("type").and_then(Value::as_str)
}

fn props_of(cmd: &Command) -> Map<String, Value> {
    cmd.params.get("properties").and_then(Value::as_object).cloned().unwrap_or_default()
}

fn required_of(cmd: &Command) -> Vec<&str> {
    cmd.params.get("required").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default()
}

/// The placeholder for a param's value.
fn placeholder(schema: &Value) -> String {
    if let Some(list) = schema.get("enum").and_then(Value::as_array) {
        return format!("<{}>", list.iter().map(js_str).collect::<Vec<_>>().join("|"));
    }
    match type_of(schema) {
        Some("integer") | Some("number") => "<n>".into(),
        Some("boolean") => String::new(),
        Some("array") => {
            let inner = placeholder(schema.get("items").unwrap_or(&Value::Object(Map::new()))).replace(['<', '>'], "");
            format!("<{},…>", if inner.is_empty() { "item".to_string() } else { inner })
        }
        Some("object") => "<json>".into(),
        _ => "<text>".into(),
    }
}

/// pixellab scene render <name> [--scale <n>] …
pub fn usage_line(cmd: &Command) -> String {
    let props = props_of(cmd);
    let required = required_of(cmd);
    let mut parts = vec![format!("pixellab {}", command_words(cmd.name))];
    for arg in &cmd.args {
        let many = if props.get(*arg).and_then(type_of) == Some("array") { "…" } else { "" };
        parts.push(if required.contains(arg) { format!("<{arg}{many}>") } else { format!("[{arg}{many}]") });
    }
    for (key, schema) in entries(&props) {
        if cmd.args.contains(&key.as_str()) {
            continue;
        }
        let ph = placeholder(schema);
        let flag = format!("--{}{}", kebab(key), if ph.is_empty() { String::new() } else { format!(" {ph}") });
        parts.push(if required.contains(&key.as_str()) { flag } else { format!("[{flag}]") });
    }
    if cmd.writes_png {
        parts.push("[--out <file.png>]".into());
    }
    parts.join(" ")
}

/// The overview: every command, the global flags, the mapping rules.
pub fn overview_help(registry: &Registry) -> String {
    let rows: Vec<(String, &str)> = registry
        .commands
        .iter()
        .map(|cmd| {
            let required = required_of(cmd);
            let args = cmd
                .args
                .iter()
                .map(|a| if required.contains(a) { format!("<{a}>") } else { format!("[{a}]") })
                .collect::<Vec<_>>()
                .join(" ");
            let words = command_words(cmd.name);
            (if args.is_empty() { words } else { format!("{words} {args}") }, cmd.summary)
        })
        .collect();
    let w = rows.iter().map(|(a, _)| ulen(a)).max().unwrap_or(0);
    let mut lines: Vec<String> = vec![
        "pixellab: read and edit the pixel art in a .pixellab document, and publish it for a site".into(),
        String::new(),
        "Usage: pixellab <group> <verb> [args] [--flags] [--json] [--document <path.pixellab>]".into(),
        String::new(),
        "Commands:".into(),
    ];
    lines.extend(rows.iter().map(|(a, b)| format!("  {}  {b}", pad(a, w))));
    lines.extend([
        String::new(),
        "Servers (pixellab serve --help):".into(),
        "  serve --stdio            the Mac app's JSON-RPC server: a session with undo, saved by doc.save".into(),
        "  serve --mcp [--direct]   an MCP server for agents; doc_save writes a proposal (--direct: the document)".into(),
        String::new(),
        "Edit commands save as soon as they run, after validating (--force writes despite problems or a".into(),
        "changed file); --dry-run shows the diff instead. doc.* commands need a session (pixellab serve).".into(),
        String::new(),
        global_flags_help(),
        String::new(),
        MAPPING.into(),
        String::new(),
        EXIT.into(),
        String::new(),
        "Run pixellab <group> --help or pixellab <group> <verb> --help for details.".into(),
    ]);
    lines.join("\n")
}

fn global_flags_help() -> String {
    let rows: Vec<(String, &str)> = GLOBAL_FLAGS
        .iter()
        .map(|f| {
            let name = if f.name == "help" {
                "-h, --help".to_string()
            } else {
                let value = if f.value {
                    if f.name == "out" {
                        " <file.png>"
                    } else {
                        " <path>"
                    }
                } else {
                    ""
                };
                format!("--{}{value}", f.name)
            };
            (name, f.help)
        })
        .collect();
    let w = rows.iter().map(|(a, _)| ulen(a)).max().unwrap_or(0);
    let mut lines = vec!["Global flags:".to_string()];
    lines.extend(rows.iter().map(|(a, b)| format!("  {}  {b}", pad(a, w))));
    lines.join("\n")
}

/// One group's commands with their usage lines.
pub fn group_help(registry: &Registry, group: &str) -> String {
    let groups = registry.groups();
    let cmds: &[&Command] = groups.iter().find(|(g, _)| g == group).map(|(_, c)| c.as_slice()).unwrap_or(&[]);
    let mut lines = vec![format!("pixellab {group}: {} command{}", cmds.len(), if cmds.len() == 1 { "" } else { "s" }), String::new()];
    for cmd in cmds {
        lines.push(format!("  {}", usage_line(cmd)));
        lines.push(format!("      {}", cmd.summary));
    }
    lines.push(String::new());
    lines.push(format!(
        "Run pixellab {group} <verb> --help for a command's params. Global flags: --json, --document <path>, --site <checkout>, -h."
    ));
    lines.join("\n")
}

/// `(minimum, maximum)` facts as the help words them.
fn range_fact(s: &Value) -> String {
    match (s.get("minimum"), s.get("maximum")) {
        (Some(min), Some(max)) => format!("{} to {}", js_str(min), js_str(max)),
        (Some(min), None) => format!("at least {}", js_str(min)),
        (None, Some(max)) => format!("at most {}", js_str(max)),
        (None, None) => String::new(),
    }
}

/// One command: usage, description, arguments, flags, examples.
pub fn command_help(cmd: &Command) -> String {
    let props = props_of(cmd);
    let required = required_of(cmd);
    let mut rows: Vec<(String, String)> = Vec::new();
    for (key, s) in entries(&props) {
        let ph = placeholder(s);
        let is_req = required.contains(&key.as_str());
        let name = if cmd.args.contains(&key.as_str()) {
            format!("{}, --{}", if is_req { format!("<{key}>") } else { format!("[{key}]") }, kebab(key))
        } else {
            format!("--{}{}", kebab(key), if ph.is_empty() { String::new() } else { format!(" {ph}") })
        };
        let kind = if type_of(s) == Some("array") {
            let item = s.get("items").and_then(type_of).unwrap_or("values");
            Some(format!("list of {item}"))
        } else {
            type_of(s).map(String::from)
        };
        let facts: Vec<String> = [
            kind.unwrap_or_default(),
            if is_req { "required".into() } else { String::new() },
            s.get("default").map(|d| format!("default {}", stringify(d))).unwrap_or_default(),
            range_fact(s),
        ]
        .into_iter()
        .filter(|f| !f.is_empty())
        .collect();
        let desc = s.get("description").and_then(Value::as_str).unwrap_or("");
        rows.push((name, format!("{desc}{}({})", if desc.is_empty() { "" } else { " " }, facts.join(", "))));
    }
    if cmd.writes_png {
        rows.push(("--out <file.png>".into(), "write the PNG to this file (CLI only)".into()));
    }
    let w = rows.iter().map(|(a, _)| ulen(a)).max().unwrap_or(0);
    let mut lines = vec![format!("Usage: {}", usage_line(cmd)), String::new(), format!("{}.", cmd.summary)];
    if let Some(description) = cmd.description.filter(|d| !d.is_empty()) {
        lines.push(String::new());
        lines.extend(wrap(description, 78));
    }
    lines.push(String::new());
    lines.push(format!("Command: {}{}", cmd.name, if cmd.mutates { " (mutates)" } else { " (read-only)" }));
    if !rows.is_empty() {
        lines.push(String::new());
        lines.push("Params:".into());
        lines.extend(rows.iter().map(|(a, b)| format!("  {}  {b}", pad(a, w))));
    }
    if !cmd.examples.is_empty() {
        lines.push(String::new());
        lines.push("Examples:".into());
        lines.extend(cmd.examples.iter().map(|e| format!("  {e}")));
    }
    lines.push(String::new());
    lines.push(
        "Global flags: --json (the result as JSON), --document <path>, --site <checkout>, -h. Run pixellab help for how args and flags map to params."
            .into(),
    );
    lines.join("\n")
}

/// JavaScript's `\s`.
fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'
    )
}

/// `text.split(/\s+/)`.
fn split_words(text: &str) -> Vec<&str> {
    let mut words = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if is_js_space(c) {
            words.push(&text[start..i]);
            let mut end = i + c.len_utf8();
            while let Some(&(j, d)) = chars.peek() {
                if is_js_space(d) {
                    end = j + d.len_utf8();
                    chars.next();
                } else {
                    break;
                }
            }
            start = end;
        }
    }
    words.push(&text[start..]);
    words
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in split_words(text) {
        if !line.is_empty() && ulen(&line) + 1 + ulen(word) > width {
            lines.push(std::mem::take(&mut line));
            line = word.to_string();
        } else if line.is_empty() {
            line = word.to_string();
        } else {
            line = format!("{line} {word}");
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// Help for a topic: [] (overview), [group], or a command's words. None if
/// the topic names nothing.
pub fn help_for(registry: &Registry, topic: &[String]) -> Option<String> {
    if topic.is_empty() {
        return Some(overview_help(registry));
    }
    let groups = registry.groups();
    let group = groups.iter().find(|(g, _)| *g == topic[0]);
    if let Some(cmd) = registry.get(&topic.join(".")) {
        // A group of one command with the group's own name (check) is that command.
        if topic.len() > 1 || group.is_some_and(|(_, c)| c.len() == 1) {
            return Some(command_help(cmd));
        }
    }
    if topic.len() == 1 && group.is_some() {
        return Some(group_help(registry, &topic[0]));
    }
    None
}

// ---- the command reference (scripts/docs-cli.mjs) ----

/// The servers' protocol versions and the commands MCP leaves out, from the servers.
const MCP_PROTOCOL_VERSION: &str = crate::server_mcp::PROTOCOL_VERSION;
use crate::server_mcp::LEFT_OUT;

const REPO: &str = "/Users/jlawcordova/Projects/jlawcordova-lab";

fn tool_name(command: &str) -> String {
    command.replace('.', "_")
}

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn doc_type_of(s: &Value) -> String {
    if let Some(list) = s.get("enum").and_then(Value::as_array) {
        return list.iter().map(|v| format!("`{}`", js_str(v))).collect::<Vec<_>>().join(" \\| ");
    }
    let items = s.get("items");
    if type_of(s) == Some("array") {
        if items.and_then(type_of) == Some("array") {
            let inner = items.and_then(|i| i.get("items")).and_then(type_of).unwrap_or("value");
            return format!("list of [{inner}, …]");
        }
        return format!("list of {}", items.and_then(type_of).unwrap_or("values"));
    }
    type_of(s).unwrap_or("any").to_string()
}

fn command_section(cmd: &Command) -> String {
    let props = props_of(cmd);
    let required = required_of(cmd);
    let left_out = LEFT_OUT.iter().any(|(n, _)| *n == cmd.name);
    let facts: Vec<String> = [
        if cmd.mutates { "edits docs".to_string() } else { "read-only".to_string() },
        if cmd.session { "session only (`serve`)".to_string() } else { String::new() },
        if cmd.writes_direct { "writes files directly".to_string() } else { String::new() },
        if left_out { "not an MCP tool".to_string() } else { format!("MCP tool `{}`", tool_name(cmd.name)) },
    ]
    .into_iter()
    .filter(|f| !f.is_empty())
    .collect();
    let mut lines: Vec<String> = vec![
        format!("#### {}", cmd.name),
        String::new(),
        format!("{}.", cmd.summary),
        String::new(),
        "```".into(),
        usage_line(cmd),
        "```".into(),
        String::new(),
    ];
    if let Some(description) = cmd.description.filter(|d| !d.is_empty()) {
        lines.push(description.to_string());
        lines.push(String::new());
    }
    lines.push(facts.join(" · "));
    lines.push(String::new());
    if !props.is_empty() {
        lines.push("| param | type | | description |".into());
        lines.push("| --- | --- | --- | --- |".into());
        for (key, s) in entries(&props) {
            let req = if required.contains(&key.as_str()) {
                "required".to_string()
            } else if let Some(d) = s.get("default") {
                format!("default `{}`", stringify(d))
            } else {
                String::new()
            };
            let range = match (s.get("minimum"), s.get("maximum")) {
                (Some(min), Some(max)) => format!(" ({} to {})", js_str(min), js_str(max)),
                (Some(min), None) => format!(" (at least {})", js_str(min)),
                _ => String::new(),
            };
            let flag = if cmd.args.contains(&key.as_str()) { format!("`{key}` (positional)") } else { format!("`--{}`", kebab(key)) };
            let desc = s.get("description").and_then(Value::as_str).unwrap_or("");
            lines.push(format!("| {flag} | {} | {req} | {} |", doc_type_of(s), cell(&format!("{desc}{range}"))));
        }
        lines.push(String::new());
    }
    if !cmd.examples.is_empty() {
        lines.push("```sh".into());
        lines.extend(cmd.examples.iter().map(|e| e.to_string()));
        lines.push("```".into());
        lines.push(String::new());
    }
    lines.join("\n")
}

/// docs/cli.md, generated from the registry: what `pixellab help --markdown` prints.
pub fn markdown(registry: &Registry) -> String {
    let index: Vec<String> = registry
        .commands
        .iter()
        .map(|c| {
            let kinds: Vec<&str> = [
                if c.mutates && !c.writes_direct { "edit" } else { "" },
                if c.writes_direct { "writes files" } else { "" },
                if c.session { "session" } else { "" },
            ]
            .into_iter()
            .filter(|k| !k.is_empty())
            .collect();
            format!(
                "| [`{}`](#{}) | {} | {} |",
                command_words(c.name),
                c.name.replace('.', ""),
                cell(c.summary),
                if kinds.is_empty() { "read".to_string() } else { kinds.join(", ") }
            )
        })
        .collect();
    let globals: Vec<String> = GLOBAL_FLAGS
        .iter()
        .map(|f| format!("| `{}` | {} |", if f.name == "help" { "-h, --help".to_string() } else { format!("--{}", f.name) }, cell(f.help)))
        .collect();
    let left_out = LEFT_OUT.iter().map(|(name, why)| format!("`{}` ({why})", tool_name(name))).collect::<Vec<_>>().join("; ");
    let reference = registry
        .groups()
        .iter()
        .map(|(group, cmds)| format!("### {group}\n\n{}", cmds.iter().map(|c| command_section(c)).collect::<Vec<_>>().join("\n")))
        .collect::<Vec<_>>()
        .join("\n");
    let text = TEMPLATE
        .replace("@@INDEX@@", &index.join("\n"))
        .replace("@@GLOBALS@@", &globals.join("\n"))
        .replace("@@STDIO_VERSION@@", &crate::server_stdio::PROTOCOL_VERSION.to_string())
        .replace("@@MCP_VERSION@@", MCP_PROTOCOL_VERSION)
        .replace("@@LEFT_OUT@@", &left_out)
        .replace("@@REPO@@", REPO)
        .replace("@@REFERENCE@@", &reference);
    let mut text = text;
    while text.contains("\n\n\n") {
        text = text.replace("\n\n\n", "\n\n");
    }
    format!("{}\n", text.trim_end_matches('\n'))
}

const TEMPLATE: &str = r####"# pixellab: commands, the stdio protocol and MCP

<!-- Generated by scripts/docs-cli.mjs from the command registry: run `npm run docs:cli` after changing a command. Don't edit by hand. -->

`pixellab` reads and edits the pixel art in a `.pixellab` document and publishes it as the SVGs a site inlines. It has three doors onto one command registry (`crates/pixellab/src/registry.rs`): the command line, `pixellab serve --stdio` for the Mac app, and `pixellab serve --mcp` for agents. Every command takes the same params (a JSON Schema object) and returns the same JSON through each door.

- [Commands](#commands)
- [The command line](#the-command-line)
- [The stdio protocol](#the-stdio-protocol) (`serve --stdio`, for the Mac app)
- [MCP](#mcp) (`serve --mcp`, for agents) and [setup for Claude Code](#setup-for-claude-code)
- [Proposals and activity](#proposals-and-activity)
- [Command reference](#command-reference)

## Commands

| command | summary | kind |
| --- | --- | --- |
@@INDEX@@

## The command line

```
pixellab <group> <verb> [args] [--flags] [--json] [--document <path.pixellab>]
```

The leading words name the command (`scene place` is `scene.place`; the longest match wins). Positionals fill the command's arguments in order; every param is also a `--kebab-case` flag. Booleans: `--flag`, `--no-flag`. Lists: comma-separated (`--at 14,6,1`) or the flag repeated; a list of pairs (pixels) is `3,4 5,6` or the flag repeated. Exit codes: 0 ok, 1 command error (or `check` found problems), 2 usage error. With `--json`, errors are `{"error": {"code", "message", "details?"}}` on stderr.

| global flag | |
| --- | --- |
@@GLOBALS@@

**Edits are one-shot.** A command that edits a doc loads it, applies the edit, validates the doc with the engine, serializes it with the engine and writes it atomically (temp file and rename), then prints what changed and `wrote <file>`. It refuses, writing nothing, when the doc has validation problems (code `invalid`, the problems in `details`) or the file changed on disk while it ran (code `conflict`); `--force` writes anyway. `--dry-run` prints the unified diff it would write, any problems, and writes nothing. With `--json` the result gains `saved: [{ doc, file, wrote, hash }]` or `dryRun: [{ doc, file, diff, problems, conflict }]`. The `doc.*` commands (undo, redo, save…) only make sense in a session, so they run through `pixellab serve`.

The document comes from `--document`, else `PIXELLAB_DOC`, else `~/.config/pixellab/config.json` (`{"document": "…"}`), else `--site <checkout>` / `PIXELLAB_SITE` / `{"site": "…"}` (the `*.pixellab` under the site's `src/assets/pixel-art/`, or its legacy `source/` folder), else `~/Projects/jlawcordova.github.io`.

## The stdio protocol

`pixellab serve --stdio [--document <path> | --site <checkout>]` is the Mac app's server: newline-delimited [JSON-RPC 2.0](https://www.jsonrpc.org/specification) on stdin and stdout, one message per line. stdout carries nothing but JSON-RPC; logs go to stderr. Requests are answered one at a time, in order. Batches are not supported. Protocol version: @@STDIO_VERSION@@ (`session.info` reports it).

The server holds a **session**: docs open as in-memory working copies with undo and redo (at most 200 steps; rapid repeats of the same edit within a second, such as a held arrow key or one paint stroke sent as many `object.paint` calls, merge into one step). Every command reads the working copies, so `scene.render` shows unsaved edits. Nothing is written until `doc.save`.

**Methods** are the command names (`scene.place`, `doc.undo`, … see the reference), plus:

| method | result |
| --- | --- |
| `session.info` | `{ name: "pixellab", version, protocol, pid, document: { root, name, kind, publishDir, siteRoot }, site, sourceDir, stateDir, saves: "direct", docs: [DocStatus] }` |
| `shutdown` | `{ ok: true }`, then the server stops reading and exits 0. It also exits when stdin closes. |

```json
→ {"jsonrpc":"2.0","id":1,"method":"scene.place","params":{"name":"hero-island","object":"pine","at":[14,6,1]}}
← {"jsonrpc":"2.0","method":"doc.changed","params":{"doc":"scene:hero-island","dirty":true,"canUndo":true,"canRedo":false}}
← {"jsonrpc":"2.0","id":1,"result":{"doc":"scene:hero-island","changed":true,"summary":"pine placed at column 14, row 6, level 1","path":"198","item":{"object":"pine","at":{"tile":[14,6,1]}}}}
```

**Results** are the command's JSON. The shapes the app needs most:

- `scene.render` / `object.render`: `{ png, scale, viewBox: [x, y, w, h], frame?, width, height, time | frame, items: [{ path, object, at, id?, pose?, bounds: [x, y, w, h] }] }`. `png` is a base64 PNG with a transparent background at an integer `scale`; `bounds` are in output pixels (scale included), for hit-testing; `path` is the item's dotted path (`"5.2"`). An object render's item has `path: null` and the result has its `anchor`. A scene render's `viewBox` is the box the scene publishes at (its own, or the fit to what it paints: `frame` says `"fixed"` or `"fit"`), or the `view` window asked for; a scene without a `viewBox` has no edge, so the app asks for the window it shows. `scene.show` and `scene.list` give the resolved `viewBox` and `frame` too.
- Edits: `{ doc: "scene:<name>" | "object:<name>", changed, summary, ... }`; scene item edits add `path` (the item's path after the edit) and `item`; paint edits add `pixels` (how many changed).
- `DocStatus` (in `doc.changed`, `session.info` and `doc.status` without a doc): `{ doc, dirty, canUndo, canRedo }`. `doc.status` with a doc adds `file, exists, stale` (the file changed on disk since the doc was opened or last matched it), `undo: [edit summaries]`, `redo: [...]`, `edits` (the edits since the doc last matched its file: what a proposal would list), `proposal?`, `synced?` and `unsavedInApp` (see MCP).
- `doc.save`: `{ doc, file, wrote, hash, bytes, diff, dirty, canUndo, canRedo }` (`wrote` is false when the file already held exactly this text).
- `doc.undo` / `doc.redo` / `doc.revert`: `{ doc, changed, summary, dirty, canUndo, canRedo }`; `doc.reload` adds `reloaded`, `doc.merge` adds `merged`.
- `proposal.apply`: `{ id, doc, file, wrote, reloaded, merged?, conflict?, diff, summary }`: an open copy with no unsaved edits reloads; one with unsaved edits has the proposal merged in (`merged`), or, when both changed the same lines, is left as it is and reported with `doc.stale` (`conflict`).

**Errors** are JSON-RPC errors with a stable `data.code`:

```json
{"jsonrpc":"2.0","id":7,"error":{"code":-32602,"message":"scene.move: item is required","data":{"code":"usage","details":["item is required"]}}}
```

| `error.code` | when | `data.code` |
| --- | --- | --- |
| -32700 | a line that isn't JSON (`id` is null) | |
| -32600 | not a JSON-RPC 2.0 request | |
| -32601 | no such method | `not_found` |
| -32602 | bad params | `usage` (with `details`: every problem) |
| -32000 | the command ran and failed | `not_found`, `invalid` (validation problems in `details`), `conflict`, `dirty`, `exists`, `ambiguous`, `is_group`, `at_front`, `paint_blocked`, `color_cap`, `load_error`, … |
| -32603 | a bug | `internal` |

**Notifications** from the server (no `id`):

| method | params | when |
| --- | --- | --- |
| `doc.changed` | `{ doc, dirty, canUndo, canRedo }` | after each edit, undo, redo, save, revert, merge or reload, and when a clean open doc reloads because its file changed. Sent before the request's response. |
| `doc.stale` | `{ doc, file }` | an open doc with unsaved edits whose file changed on disk (the app shows Merge, Reload, Keep Mine), and after an applied proposal whose merge into the open copy clashed |
| `files.changed` | `{ files: ["scenes/hero-island.json", …] }` | sources changed on disk (another editor, git, an applied proposal, a save), debounced 150 ms |
| `proposals.changed` | `{ proposals: [ProposalSummary] }` (as `proposal.list`) | a proposal was written, applied or rejected |
| `activity.appended` | `{ entries: [{ time, tool, args, ok, summary }] }` | an MCP server logged tool calls |

An open doc without unsaved changes reloads from disk when its file changes; one with unsaved changes stays as it is, the server sends `doc.stale`, `doc.status` reports it `stale`, and its `doc.save` fails with `conflict` until it's merged (`doc.merge`: a three-way line merge of the file into the working copy, keeping its edits; it refuses with `merge_conflict` when both changed the same lines), reverted, or saved with `force`. The server also keeps `open/<pid>.json` in the state dir (see below), so agents can see what the app has open with unsaved edits.

## MCP

`pixellab serve --mcp [--direct] [--document <path> | --site <checkout>]` is an MCP server over stdio (protocol `@@MCP_VERSION@@`, hand-written: `initialize`, `notifications/initialized`, `ping`, `tools/list`, `tools/call`). Tools are the commands with `.` → `_` (`scene.place` → `scene_place`), each with its params as `inputSchema`. A tool's result is `content: [{ type: "text", text: <the result as JSON> }]` plus `structuredContent` (the result); render tools return the PNG as an `image` content block first, so agents can see the art. A failed command is a result with `isError: true` and `structuredContent: { error: { code, message, details? } }`; an unknown tool is a JSON-RPC error.

**Agents propose, people approve.** Edits stay in the server's session. `doc_save` validates the doc (refusing one with problems unless `force`) and writes a **proposal** to the state dir instead of the document; a person approves it in Pixel Lab's Agent window or with `pixellab proposal apply`. With `--direct`, `doc_save` writes the document instead.

**The session follows disk.** Before every `tools/call`, the session brings its open docs in line with disk where nothing is lost. A doc whose proposal was applied or rejected is settled: marked clean if it matches the file; reloaded if it has no edits since the proposal; rebased onto the file, keeping the later edits, if the proposal was applied; left as it is only when a rejected proposal has edits on top. `doc_status` reports it as `synced: { proposal, outcome: applied | rejected, action: clean | reloaded | rebased | kept }` (`settled`, without a doc, lists the latest, including a new doc discarded after its proposal was rejected). A clean doc whose file changed reloads. So the next edit starts from the file, with no revert. `doc_reload` reloads a doc only when nothing unsaved is lost (code `dirty` with the edits otherwise, `proposed` while its proposal waits); `doc_revert`, which drops edits, is annotated destructive.

**What the app has open.** `doc_status` lists `unsavedInApp`: the docs Pixel Lab has open with unsaved edits (from `open/<pid>.json`). A proposal for one of them comes back with `warnings`: approving it will merge it into the owner's unsaved edits, and lines both changed will clash. Left out of the tools: @@LEFT_OUT@@. With `--direct`, `art_build` is a tool.

### Setup for Claude Code

With the app installed by `scripts/install.sh`, point Claude Code at the program inside it (the Agent window's Connect row shows the same command):

```sh
claude mcp add pixellab -- "$HOME/Applications/Pixel Lab.app/Contents/Helpers/pixellab" serve --mcp
```

Or at a release's `pixellab` (`pixellab-darwin-arm64.tar.gz`, `pixellab-linux-x64.tar.gz`), or this repo's build for development (`cargo build --release`):

```sh
claude mcp add pixellab -- @@REPO@@/target/release/pixellab serve --mcp
```

Add `--document <path.pixellab>` (or `--site <checkout>`) after `--mcp` for another document, or set `PIXELLAB_DOC`. Proposals land in the state dir for review; run `pixellab proposal list` and `pixellab proposal show <id>` to see them from a terminal.

## Proposals and activity

The state dir is `$PIXELLAB_STATE`, else `~/Library/Application Support/Pixel Lab/`.

- `proposals/<id>.json`: `{ id, doc: "scene:<name>" | "object:<name>", baseHash, source, summary, createdAt, agent }`. `id` is `YYYYMMDD-HHMMSS-xxxxxx` (UTC, sortable). `baseHash` is the sha256 (hex) of the file the agent opened, or null for a new doc. `source` is the full new file, serialized by the engine. `summary` lists the edits since the doc last matched its file (opened, saved, reloaded, or its last proposal settled), so a proposal made after an earlier one was applied lists only its own edits. `agent` is the MCP client's name and version. Saving the same doc again in one MCP session rewrites its proposal.
- `activity.jsonl`: one line per MCP `tools/call`: `{ time, tool, args, ok, summary }` (`tool` is the MCP tool name).
- `open/<pid>.json`: written by Pixel Lab's server (`serve --stdio`) whenever its open docs change, removed when it stops: `{ pid, document, updatedAt, docs: [{ doc, dirty }] }`. Readers skip a file whose process is gone or whose `document` isn't theirs.

`proposal.apply` refuses (`conflict`) when the file no longer has `baseHash`, and (`invalid`) when the proposed doc has validation problems, unless forced; then it writes `source`, deletes the proposal, and reloads an open, unchanged working copy. `proposal.reject` deletes it. `proposal.render` renders the proposed doc over the files on disk (the shape of `scene.render` or `object.render`) without writing, for the Agent window's ghost preview.

## Command reference

@@REFERENCE@@"####;
