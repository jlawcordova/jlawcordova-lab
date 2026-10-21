//! Help output and exit codes, through the CLI door (ports cli/test/help.test.mjs).
//! The tests that need the real commands (U22 to U26) are ignored until they
//! are ported; the `test registry` tests run the same behavior on a small
//! registry built here.

#![allow(clippy::result_large_err)]

mod common;

use std::collections::HashMap;

use pixellab::argv::command_words;
use pixellab::cli::{main, Io};
use pixellab::context::{Ctx, DocKind};
use pixellab::errors::{CommandError, Result};
use pixellab::help::{help_for, markdown, usage_line};
use pixellab::registry::{registry, Command, Params, Registry};
use pixellab::session::Edit;
use serde_json::{json, Map, Value};

use common::make_site;

fn exec(reg: &Registry, args: &[&str], env: HashMap<String, String>) -> (i32, String, String) {
    let argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = {
        let mut io = Io { stdout: &mut out, stderr: &mut err, env, cwd: std::env::temp_dir(), registry: reg };
        main(&argv, &mut io)
    };
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn spawn_cli(args: &[&str]) -> (i32, String, String) {
    exec(&registry(), args, HashMap::new())
}

// ---- a small registry ----

fn ok(_: &mut Ctx, _: Params) -> Result<Value> {
    Ok(json!({ "ok": true }))
}

fn render(_: &mut Ctx, _: Params) -> Result<Value> {
    Ok(json!({ "png": "iVBORw0KGgo=", "width": 2, "height": 3 }))
}

fn show(_: &mut Ctx, p: Params) -> Result<Value> {
    Err(CommandError::new(format!("no scene named {}", p["name"].as_str().unwrap()), "not_found").into())
}

fn bad(_: &mut Ctx, _: Params) -> Result<Value> {
    Ok(json!({ "ok": false, "problems": 1 }))
}

fn place(ctx: &mut Ctx, _: Params) -> Result<Value> {
    ctx.edit(DocKind::Scene, "sample", None, |doc, _| {
        doc["items"].as_array_mut().unwrap().push(json!({ "object": "pebble", "at": { "tile": [3, 2, 0] } }));
        Ok(Edit::new(Map::new(), "pebble placed"))
    })
}

fn schema(props: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": required })
}

fn test_registry() -> Registry {
    let mut render_cmd = Command::new(
        "scene.render",
        "Render a scene to a PNG",
        schema(
            json!({
                "name": { "type": "string", "description": "scene name (or scenes/<name>)" },
                "scale": { "type": "integer", "description": "whole-pixel zoom of the PNG", "default": 1, "minimum": 1, "maximum": 16 },
                "theme": { "type": "string", "enum": ["light", "dark"] },
                "hide": { "type": "array", "items": { "type": "string" } }
            }),
            &["name"],
        ),
        false,
        render,
    );
    render_cmd.args = vec!["name"];
    render_cmd.writes_png = true;
    render_cmd.examples = vec!["pixellab scene render hero-island --scale 2 --out hero.png"];
    render_cmd.description =
        Some("Renders the scene as a PNG at a whole-pixel zoom, with a transparent background, so you can see the art.");
    render_cmd.format = Some(|r, _| format!("{}×{} PNG", r["width"], r["height"]));
    let mut show_cmd = Command::new("scene.show", "Show a scene", schema(json!({ "name": { "type": "string" } }), &["name"]), false, show);
    show_cmd.args = vec!["name"];
    let place_cmd = Command::new("scene.place", "Place an item", schema(json!({}), &[]), true, place);
    let bad_cmd = Command::new("scene.bad", "Fail a check", schema(json!({}), &[]), false, bad);
    let mut undo = Command::new("doc.undo", "Undo the last edit", schema(json!({}), &[]), true, ok);
    undo.session = true;
    let mut build = Command::new("art.build", "Publish", schema(json!({}), &[]), true, ok);
    build.writes_direct = true;
    let mut make = Command::new("thing.make", "Make a thing with no document", schema(json!({}), &[]), true, bad);
    make.needs_document = false;
    let mut forced =
        Command::new("thing.force", "Edit with a force param", schema(json!({ "force": { "type": "boolean" } }), &[]), true, ok);
    forced.writes_direct = true;
    let mut check = Command::new("check", "Validate the sources", schema(json!({ "name": { "type": "string" } }), &[]), false, ok);
    check.args = vec!["name"];
    Registry::new(vec![
        render_cmd,
        show_cmd,
        place_cmd,
        bad_cmd,
        Command::new("scene.list", "List the scenes", schema(json!({}), &[]), false, ok),
        undo,
        build,
        make,
        forced,
        check,
    ])
}

// ---- help ----

#[test]
fn pixellab_help_lists_every_command_the_global_flags_the_mapping_and_exit_codes() {
    let reg = registry();
    let (code, out, err) = spawn_cli(&["help"]);
    assert_eq!(code, 0);
    assert_eq!(err, "");
    for cmd in &reg.commands {
        let line = format!("  {}", command_words(cmd.name));
        assert!(out.lines().any(|l| l.starts_with(&line) && l.ends_with(cmd.summary)), "{}", cmd.name);
    }
    for part in [
        "--json",
        "--site <path>",
        "--out <file.png>",
        "Positionals fill",
        "kebab-case",
        "--no-flag",
        "comma-separated",
        "Exit codes: 0 ok, 1 command error",
        "2 usage error",
    ] {
        assert!(out.contains(part), "{part}");
    }
    assert_eq!(spawn_cli(&["--help"]).1, out);
}

#[test]
fn pixellab_help_lists_every_command_the_global_flags_the_mapping_and_exit_codes_test_registry() {
    let reg = test_registry();
    let (code, out, err) = exec(&reg, &["help"], HashMap::new());
    assert_eq!((code, err.as_str()), (0, ""));
    for cmd in &reg.commands {
        let line = format!("  {}", command_words(cmd.name));
        assert!(out.lines().any(|l| l.starts_with(&line) && l.ends_with(cmd.summary)), "{}", cmd.name);
    }
    assert!(out.contains("  scene render <name>  Render a scene to a PNG"), "{out}");
    for part in [
        "--json",
        "--site <path>",
        "--out <file.png>",
        "Positionals fill",
        "kebab-case",
        "--no-flag",
        "comma-separated",
        "Exit codes: 0 ok, 1 command error",
        "2 usage error",
    ] {
        assert!(out.contains(part), "{part}");
    }
    assert_eq!(exec(&reg, &["--help"], HashMap::new()).1, out);
}

#[test]
fn bare_pixellab_prints_help_to_stderr_with_exit_code_2() {
    let (code, out, err) = exec(&test_registry(), &[], HashMap::new());
    assert_eq!(code, 2);
    assert_eq!(out, "");
    assert!(err.starts_with("pixellab: read and edit"));
}

#[test]
fn group_help_pixellab_scene_help_pixellab_help_scene() {
    let (code, out, _) = spawn_cli(&["scene", "--help"]);
    assert_eq!(code, 0);
    assert!(out.lines().any(|l| l == "pixellab scene: 27 commands"), "{out}");
    assert!(out.lines().any(|l| l
        == "  pixellab scene render <name> [--scale <n>] [--time <n>] [--theme <light|dark>] [--play <text>] [--show <text>] [--hide <text,…>] [--focus <text>] [--out <file.png>]"));
    assert!(out.lines().any(|l| l == "  pixellab scene list"));
    assert_eq!(spawn_cli(&["help", "scene"]).1, out);
    let bare = spawn_cli(&["scene"]);
    assert_eq!(bare.0, 2);
    assert_eq!(bare.2, out);
}

#[test]
fn group_help_pixellab_scene_help_pixellab_help_scene_test_registry() {
    let reg = test_registry();
    let (code, out, _) = exec(&reg, &["scene", "--help"], HashMap::new());
    assert_eq!(code, 0);
    assert!(out.lines().any(|l| l == "pixellab scene: 5 commands"), "{out}");
    assert!(
        out.lines()
            .any(|l| l == "  pixellab scene render <name> [--scale <n>] [--theme <light|dark>] [--hide <text,…>] [--out <file.png>]"),
        "{out}"
    );
    assert!(out.lines().any(|l| l == "  pixellab scene list"));
    assert!(out.contains("      List the scenes\n"));
    assert_eq!(exec(&reg, &["help", "scene"], HashMap::new()).1, out);
    let bare = exec(&reg, &["scene"], HashMap::new());
    assert_eq!(bare.0, 2);
    assert_eq!(bare.1, "");
    assert_eq!(bare.2, out);
}

#[test]
fn command_help_pixellab_scene_render_help() {
    let (code, out, _) = spawn_cli(&["scene", "render", "--help"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("Usage: pixellab scene render <name> [--scale <n>]"));
    assert!(out.lines().any(|l| l == "Command: scene.render (read-only)"));
    assert!(out.lines().any(|l| l.starts_with("  <name>, --name") && l.ends_with("scene name (or scenes/<name>) (string, required)")));
    assert!(out
        .lines()
        .any(|l| l.starts_with("  --scale <n>") && l.ends_with("whole-pixel zoom of the PNG (integer, default 1, 1 to 16)")));
    assert!(out.contains("\nExamples:\n  pixellab scene render hero-island --scale 2 --out"));
    assert_eq!(spawn_cli(&["help", "scene", "render"]).1, out);
    assert_eq!(spawn_cli(&["scene", "render", "hero-island", "-h"]).1, out);
}

#[test]
fn command_help_pixellab_scene_render_help_test_registry() {
    let reg = test_registry();
    let (code, out, _) = exec(&reg, &["scene", "render", "--help"], HashMap::new());
    assert_eq!(code, 0);
    assert!(out.starts_with("Usage: pixellab scene render <name> [--scale <n>]"), "{out}");
    assert!(out.lines().any(|l| l == "Command: scene.render (read-only)"));
    assert!(out.contains("\n  <name>, --name        scene name (or scenes/<name>) (string, required)\n"), "{out}");
    assert!(out.contains("\n  --scale <n>           whole-pixel zoom of the PNG (integer, default 1, 1 to 16)\n"), "{out}");
    assert!(out.contains("\n  --theme <light|dark>  (string)\n"), "{out}");
    assert!(out.contains("\n  --hide <text,…>       (list of string)\n"), "{out}");
    assert!(out.contains("\n  --out <file.png>      write the PNG to this file (CLI only)\n"), "{out}");
    assert!(out.contains("\nExamples:\n  pixellab scene render hero-island --scale 2 --out"));
    assert!(
        out.contains("\nRenders the scene as a PNG at a whole-pixel zoom, with a transparent\nbackground, so you can see the art.\n"),
        "{out}"
    );
    assert_eq!(exec(&reg, &["help", "scene", "render"], HashMap::new()).1, out);
    assert_eq!(exec(&reg, &["scene", "render", "hero-island", "-h"], HashMap::new()).1, out);
}

#[test]
fn help_for_every_command_and_group_renders() {
    for cmd in &registry().commands {
        let mut args: Vec<&str> = cmd.name.split('.').collect();
        args.push("--help");
        let (code, out, _) = spawn_cli(&args);
        assert_eq!(code, 0, "{}", cmd.name);
        assert!(out.starts_with(&format!("Usage: pixellab {}", command_words(cmd.name))), "{}", cmd.name);
    }
    let (_, out, _) = spawn_cli(&["check", "--help"]);
    assert!(out.starts_with("Usage: pixellab check [name] [--kind <object|scene>]"));
}

#[test]
fn help_for_every_command_and_group_renders_test_registry() {
    let reg = test_registry();
    for cmd in &reg.commands {
        let mut args: Vec<&str> = cmd.name.split('.').collect();
        args.push("--help");
        let (code, out, _) = exec(&reg, &args, HashMap::new());
        assert_eq!(code, 0, "{}", cmd.name);
        assert!(out.starts_with(&format!("Usage: pixellab {}", command_words(cmd.name))), "{}", cmd.name);
        let topic: Vec<String> = cmd.name.split('.').map(String::from).collect();
        assert_eq!(help_for(&reg, &topic).as_deref(), Some(out.trim_end_matches('\n')));
    }
    // A group of one command with the group's own name is that command.
    let (_, check, _) = exec(&reg, &["check", "--help"], HashMap::new());
    assert!(check.starts_with("Usage: pixellab check [name]"));
    assert!(help_for(&reg, &["nope".to_string()]).is_none());
    assert!(help_for(&reg, &["scene".to_string(), "nope".to_string()]).is_none());
    assert!(usage_line(reg.get("scene.render").unwrap()).ends_with("[--out <file.png>]"));
}

#[test]
fn usage_errors_exit_2_with_a_pointer_to_help() {
    let reg = test_registry();
    for args in [&["scene", "frobnicate"][..], &["nope"], &["scene", "render", "x", "--bogus"], &["help", "nope"], &["scene", "render"]] {
        let (code, out, err) = exec(&reg, args, HashMap::new());
        assert_eq!(code, 2, "{args:?}");
        assert_eq!(out, "", "{args:?}");
        assert!(err.starts_with("pixellab: "), "{args:?}: {err}");
    }
    let (code, _, err) = exec(&reg, &["scene", "frobnicate", "--json"], HashMap::new());
    assert_eq!(code, 2);
    let parsed: Value = serde_json::from_str(&err).unwrap();
    assert_eq!(parsed["error"]["code"], "usage");
    let (_, _, err) = exec(&reg, &["help", "nope"], HashMap::new());
    assert_eq!(err, "pixellab: no help for nope; run pixellab help\n");
    let (_, _, err) = exec(&reg, &["scene", "render"], HashMap::new());
    assert_eq!(err, "pixellab: scene.render: name is required\n");
    // --version is not a command line flag of the Node CLI either.
    let (code, _, err) = exec(&reg, &["--version"], HashMap::new());
    assert_eq!((code, err.as_str()), (2, "pixellab: a command comes before --version; run pixellab help\n"));
}

#[test]
fn global_flag_usage_checks_refuse_before_anything_runs() {
    let reg = test_registry();
    let cases: [(&[&str], &str); 5] = [
        (&["scene", "list", "--out", "x.png"], "pixellab: --out goes with a render command, not scene list\n"),
        (&["scene", "list", "--dry-run"], "pixellab: --dry-run goes with an edit command, not scene list\n"),
        (&["art", "build", "--dry-run"], "pixellab: --dry-run goes with an edit command, not art build\n"),
        (&["scene", "list", "--force"], "pixellab: --force goes with an edit command, not scene list\n"),
        (
            &["doc", "undo"],
            "pixellab: doc undo works on a session's open docs: run it through pixellab serve --stdio or --mcp. On the command line, edits are saved as they're made.\n",
        ),
    ];
    for (args, message) in cases {
        let (code, out, err) = exec(&reg, args, HashMap::new());
        assert_eq!((code, out.as_str(), err.as_str()), (2, "", message), "{args:?}");
    }
    // A command with its own force param takes --force.
    let (code, ..) = exec(&reg, &["thing", "force", "--force"], HashMap::new());
    assert_ne!(code, 2);
}

#[test]
fn a_param_that_clashes_with_a_global_flag_is_an_internal_error() {
    let clash = Registry::new(vec![Command::new("a.b", "x", schema(json!({ "json": { "type": "boolean" } }), &[]), false, ok)]);
    let (code, _, err) = exec(&clash, &["help"], HashMap::new());
    assert_eq!(code, 1);
    assert_eq!(err, "pixellab: internal error: command a.b: param json clashes with the global --json\n");
    let (code, _, err) = exec(&clash, &["help", "--json"], HashMap::new());
    assert_eq!(code, 1);
    assert_eq!(err, "{\"error\":{\"code\":\"internal\",\"message\":\"command a.b: param json clashes with the global --json\"}}\n");
}

// ---- exit codes ----

#[test]
fn exit_codes_0_ok_1_command_error_2_usage_error() {
    let site = make_site();
    let mut env = site.env();
    env.insert("PIXELLAB_SITE".into(), site.root.to_string_lossy().into_owned());
    let reg = registry();
    assert_eq!(exec(&reg, &["scene", "list"], env.clone()).0, 0);
    assert_eq!(exec(&reg, &["scene", "show", "no-such-scene"], env.clone()).0, 1);
    assert_eq!(exec(&reg, &["scene", "render", "sample", "--scale", "big"], env.clone()).0, 2);
    // A missing required positional is a usage error, before the site is read.
    let missing = exec(&reg, &["scene", "show"], env);
    assert_eq!(missing.0, 2);
    assert!(missing.2.contains("scene.show: name is required"));
}

#[test]
fn exit_codes_0_ok_1_command_error_2_usage_error_test_registry() {
    let site = make_site();
    let mut env = site.env();
    env.insert("PIXELLAB_SITE".into(), site.root.to_string_lossy().into_owned());
    let reg = test_registry();
    assert_eq!(exec(&reg, &["scene", "list"], env.clone()).0, 0);
    let (code, out, err) = exec(&reg, &["scene", "show", "no-such-scene"], env.clone());
    assert_eq!((code, out.as_str(), err.as_str()), (1, "", "pixellab: no scene named no-such-scene\n"));
    let (code, _, err) = exec(&reg, &["scene", "show", "nope", "--json"], env.clone());
    assert_eq!(code, 1);
    assert_eq!(err, "{\"error\":{\"code\":\"not_found\",\"message\":\"no scene named nope\"}}\n");
    assert_eq!(exec(&reg, &["scene", "render", "sample", "--scale", "big"], env.clone()).0, 2);
    // A missing required positional is a usage error, before the site is read.
    let missing = exec(&reg, &["scene", "show"], HashMap::new());
    assert_eq!(missing.0, 2);
    assert!(missing.2.contains("scene.show: name is required"));
    // A result with ok: false exits 1, and still prints.
    let (code, out, _) = exec(&reg, &["scene", "bad"], env.clone());
    assert_eq!(code, 1);
    assert_eq!(out, "{\n  \"ok\": false,\n  \"problems\": 1\n}\n");
    // A command that needs no document runs without one, and exits 0.
    let (code, out, _) = exec(&reg, &["thing", "make", "--json"], HashMap::new());
    assert_eq!((code, out.as_str()), (0, "{\n  \"ok\": false,\n  \"problems\": 1\n}\n"));
    // No document to open is a command error.
    let (code, _, err) = exec(&reg, &["scene", "list", "--document", "/no/such/dir.pixellab"], env);
    assert_eq!(code, 1);
    assert!(err.starts_with("pixellab: "), "{err}");
}

#[test]
fn stdout_piped_into_head_closes_quietly() {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let site = make_site();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_pixellab"));
    cmd.args(["palette", "get", "--document"]).arg(&site.document).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (k, v) in site.env() {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().unwrap();
    // Read the first chunk, then close the pipe, as `| head` does.
    let mut stdout = child.stdout.take().unwrap();
    let mut first = [0u8; 16];
    let _ = stdout.read(&mut first).unwrap();
    drop(stdout);
    let mut stderr = String::new();
    child.stderr.take().unwrap().read_to_string(&mut stderr).unwrap();
    child.wait().unwrap();
    assert_eq!(stderr, "");
}

#[test]
fn out_writes_the_png_and_says_so() {
    let site = make_site();
    let reg = test_registry();
    let dir = common::temp_dir("pixellab-out-");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let argv: Vec<String> = ["scene", "render", "sample", "--out", "shot.png", "--document"]
        .iter()
        .map(|s| s.to_string())
        .chain([site.document.to_string_lossy().into_owned()])
        .collect();
    let code = {
        let mut io = Io { stdout: &mut out, stderr: &mut err, env: site.env(), cwd: dir.clone(), registry: &reg };
        main(&argv, &mut io)
    };
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    assert_eq!(std::fs::read(dir.join("shot.png")).unwrap(), [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    assert_eq!(String::from_utf8(out).unwrap(), format!("2×3 PNG\nwrote {} (2×3)\n", dir.join("shot.png").display()));
    // Without --out the human text says how to get the PNG.
    let (code, out, _) = exec(&reg, &["scene", "render", "sample", "--document", &site.document.to_string_lossy()], site.env());
    assert_eq!((code, out.as_str()), (0, "2×3 PNG\n(add --out <file.png> to write the PNG, or --json for it as base64)\n"));
}

#[test]
fn an_edit_command_saves_as_it_runs_and_dry_run_shows_the_diff_and_writes_nothing() {
    let site = make_site();
    let doc = site.document.to_string_lossy().into_owned();
    let reg = test_registry();
    let file = site.document.join("scenes/sample.json");
    let before = std::fs::read_to_string(&file).unwrap();
    let (code, out, err) = exec(&reg, &["scene", "place", "--dry-run", "--document", &doc], site.env());
    assert_eq!((code, err.as_str()), (0, ""));
    assert!(out.contains("--- a/scenes/sample.json\n+++ b/scenes/sample.json\n"), "{out}");
    assert!(out.ends_with("dry run: nothing written (scenes/sample.json would change)\n"), "{out}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
    let (code, out, _) = exec(&reg, &["scene", "place", "--dry-run", "--json", "--document", &doc], site.env());
    assert_eq!(code, 0);
    let parsed: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed["dryRun"][0]["doc"], "scene:sample");
    assert_eq!(parsed["dryRun"][0]["file"], "scenes/sample.json");
    assert_eq!(parsed["dryRun"][0]["conflict"], Value::Null);
    let (code, out, _) = exec(&reg, &["scene", "place", "--document", &doc], site.env());
    assert_eq!(code, 0);
    let hash = pixellab::session::hash_text(&std::fs::read_to_string(&file).unwrap());
    let expected = format!(
        "{{\n  \"doc\": \"scene:sample\",\n  \"changed\": true,\n  \"summary\": \"pebble placed\",\n  \"saved\": [\n    {{\n      \"doc\": \"scene:sample\",\n      \"file\": \"scenes/sample.json\",\n      \"wrote\": true,\n      \"hash\": \"{hash}\"\n    }}\n  ]\n}}\nwrote scenes/sample.json\n"
    );
    assert_eq!(out, expected);
    assert_ne!(std::fs::read_to_string(&file).unwrap(), before);
}

#[test]
fn help_markdown_is_the_cli_reference() {
    let reg = test_registry();
    let (code, out, _) = exec(&reg, &["help", "--markdown"], HashMap::new());
    assert_eq!(code, 0);
    assert_eq!(out, markdown(&reg));
    // Everything between the commands table and the reference is fixed text: it is docs/cli.md's.
    let docs = std::fs::read_to_string(common::repo().join("docs/cli.md")).unwrap();
    let between = |text: &str| {
        let start = text.find("## The command line").unwrap();
        let end = text.find("## Command reference").unwrap();
        text[start..end].to_string()
    };
    assert_eq!(between(&out), between(&docs));
    assert!(out.ends_with("\n") && !out.ends_with("\n\n"));
    assert!(out.contains("#### scene.render\n\nRender a scene to a PNG.\n"), "{out}");
    assert!(out.contains("| `name` (positional) | string | required | scene name (or scenes/<name>) |"), "{out}");
    assert!(out.contains("| `--scale` | integer | default `1` | whole-pixel zoom of the PNG (1 to 16) |"), "{out}");
    assert!(out.contains("| `--theme` | `light` \\| `dark` |  |  |"), "{out}");
    assert!(out.contains("read-only · MCP tool `scene_render`"), "{out}");
    assert!(out.contains("edits docs · session only (`serve`) · MCP tool `doc_undo`"), "{out}");
}

#[test]
fn docs_cli_md_is_generated_from_the_registry_and_up_to_date() {
    // scripts/docs-cli.mjs writes docs/cli.md from the program's help --markdown (plan.md, PR 3).
    let docs = std::fs::read_to_string(common::repo().join("docs/cli.md")).unwrap();
    assert_eq!(markdown(&registry()), docs, "docs/cli.md is out of date: run npm run docs:cli");
}
