//! R4 (docs/intents/2026-10-rust-engine/spec.md): a scene over its raw or gzip
//! budget is still written, and `art build`, `art sizes` and `check` carry a
//! warning and exit 0 through every door (the CLI, `serve --stdio`,
//! `serve --mcp`). Problems in the sources still stop a build.
#![allow(clippy::result_large_err)]

mod common;

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use common::{make_document, temp_dir, TempDocument};
use pixellab::cli::{main, Io};
use pixellab::registry::registry;
use serde_json::{json, Value};

/// A temp copy whose manifest gives `sample.svg` this budget.
fn doc_with_budget(raw: u64, gzip: u64) -> TempDocument {
    let doc = make_document();
    let file = doc.root.join("manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    manifest["publish"]["budgets"] = json!({ "sample.svg": { "raw": raw, "gzip": gzip } });
    fs::write(&file, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
    doc
}

fn cli(doc: &TempDocument, args: &[&str]) -> (i32, String, String) {
    let reg = registry();
    let mut argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    argv.push("--document".into());
    argv.push(doc.root.to_string_lossy().into_owned());
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = {
        let mut io = Io { stdout: &mut out, stderr: &mut err, env: doc.env(), cwd: doc.root.clone(), registry: &reg };
        main(&argv, &mut io)
    };
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn out_dir(doc: &TempDocument) -> String {
    doc.base.join("out").to_string_lossy().into_owned()
}

#[test]
fn r4_over_budget_is_written_with_a_warning_and_ok_true() {
    let out = temp_dir("pixellab-budget-out-");
    let sources = {
        let doc = doc_with_budget(512, 25 * 1024);
        let (code, stdout, stderr) = cli(&doc, &["art", "build", "--dir", &out.to_string_lossy(), "--json"]);
        assert_eq!(code, 0, "{stdout}{stderr}");
        serde_json::from_str::<Value>(&stdout).unwrap()
    };
    assert_eq!(sources["ok"], true);
    assert!(out.join("sample.svg").exists(), "written even though it is over budget");
    let o = &sources["outputs"][0];
    assert_eq!(o["file"], "sample.svg");
    assert_eq!(o["kind"], "svg");
    assert_eq!(o["warnings"], json!(["over its 0.5 KB budget"]));
    assert_eq!(sources["warnings"], json!(["sample.svg: over its 0.5 KB budget"]));
    assert_eq!(
        o.as_object().unwrap().keys().cloned().collect::<Vec<_>>(),
        ["file", "scene", "kind", "raw", "gzip", "over", "report", "warnings"]
    );
    assert_eq!(sources["problems"], json!([]));
}

#[test]
fn r4_the_gzip_budget_warns_too_and_a_scene_within_budget_has_no_warnings() {
    let doc = doc_with_budget(100 * 1024, 300);
    let (code, stdout, _) = cli(&doc, &["art", "build", "--dir", &out_dir(&doc), "--json"]);
    assert_eq!(code, 0);
    let r: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(r["outputs"][0]["warnings"], json!(["over its 0.3 KB gzip budget"]));
    let fine = make_document();
    let (code, stdout, _) = cli(&fine, &["art", "build", "--dir", &out_dir(&fine), "--json"]);
    assert_eq!(code, 0);
    let r: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(r["outputs"][0]["warnings"], json!([]));
    assert_eq!(r["warnings"], json!([]));
}

#[test]
fn r4_art_build_prints_the_over_budget_tail_as_a_warning_and_exits_0() {
    let doc = doc_with_budget(512, 25 * 1024);
    let (code, stdout, stderr) = cli(&doc, &["art", "build", "--dir", &out_dir(&doc)]);
    assert_eq!(code, 0, "{stderr}");
    let line = stdout.lines().next().unwrap();
    assert!(
        line.starts_with("sample.svg: ") && line.contains(" gzip → ") && line.ends_with(", lossless — over its 0.5 KB budget (warning)"),
        "{line}"
    );
    assert!(!stdout.contains("OVER BUDGET"), "{stdout}");
    assert!(stdout.contains("published 1 file to "), "{stdout}");
}

#[test]
fn r4_art_sizes_prints_a_warning_line_and_exits_0() {
    let doc = doc_with_budget(512, 25 * 1024);
    let (code, stdout, stderr) = cli(&doc, &["art", "sizes", "sample"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stdout.lines().last().unwrap(), "warning: sample: over its 0.5 KB budget");
    let (code, stdout, _) = cli(&doc, &["art", "sizes", "sample", "--json"]);
    assert_eq!(code, 0);
    assert_eq!(serde_json::from_str::<Value>(&stdout).unwrap()["warnings"], json!(["over its 0.5 KB budget"]));
    let fine = make_document();
    let (_, stdout, _) = cli(&fine, &["art", "sizes", "sample"]);
    assert!(!stdout.contains("warning"), "{stdout}");
}

#[test]
fn r4_check_warns_about_an_over_budget_scene_and_exits_0() {
    let doc = doc_with_budget(512, 25 * 1024);
    let (code, stdout, stderr) = cli(&doc, &["check"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("warning: scenes/sample.json: sample.svg is over its 0.5 KB budget"), "{stdout}");
    // The sample's pebble stands partly outside its viewBox, which `check` now says too (scene-canvas spec R11).
    assert!(stdout.contains("outside its viewBox") && stdout.contains("2 warnings"), "{stdout}");
    let (code, stdout, _) = cli(&doc, &["check", "sample", "--json"]);
    assert_eq!(code, 0);
    let r: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(r["ok"], true);
    assert_eq!(r["problems"], json!([]));
    let w = r["warnings"].as_array().unwrap().iter().find(|w| w["code"] == "over_budget").expect("an over_budget warning");
    assert_eq!(w["scene"], "sample");
    assert_eq!(w["output"], "sample.svg");
    let fine = make_document();
    let (_, stdout, _) = cli(&fine, &["check", "--json"]);
    let r: Value = serde_json::from_str(&stdout).unwrap();
    assert!(r["warnings"].as_array().unwrap().iter().all(|w| w["code"] != "over_budget"));
}

#[test]
fn r4_a_validation_problem_still_stops_the_build_and_nothing_is_written() {
    let doc = doc_with_budget(512, 25 * 1024);
    let file = doc.root.join("objects/pebble.json");
    fs::write(&file, "{ not json").unwrap();
    let out = doc.base.join("out");
    let (code, stdout, _) = cli(&doc, &["art", "build", "--dir", &out.to_string_lossy(), "--json"]);
    assert_eq!(code, 1);
    let r: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(r["ok"], false);
    assert!(!r["problems"].as_array().unwrap().is_empty());
    assert_eq!(r["outputs"], json!([]));
    assert!(!out.exists(), "nothing written");
}

// ------------------------------------------------------------- servers

struct Rpc {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    id: u64,
}

fn start(mode: &str, doc: &TempDocument) -> Rpc {
    let state = temp_dir("pixellab-state-");
    let mut child = Command::new(env!("CARGO_BIN_EXE_pixellab"))
        .args(["serve", mode, "--document", &doc.root.to_string_lossy()])
        .args(if mode == "--mcp" { vec!["--direct"] } else { vec![] })
        .env_remove("PIXELLAB_SITE")
        .env_remove("PIXELLAB_DOC")
        .envs(doc.env())
        .env("PIXELLAB_STATE", &state)
        .current_dir(&state)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let stdin = child.stdin.take().unwrap();
    let stdout = BufReader::new(child.stdout.take().unwrap());
    Rpc { child, stdin, stdout, id: 0 }
}

impl Rpc {
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        let id = self.id;
        writeln!(self.stdin, "{}", json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })).unwrap();
        self.stdin.flush().unwrap();
        loop {
            let mut line = String::new();
            assert!(self.stdout.read_line(&mut line).unwrap() > 0, "the server closed");
            let m: Value = serde_json::from_str(&line).unwrap();
            if m.get("id") == Some(&json!(id)) && m.get("method").is_none() {
                return m["result"].clone();
            }
        }
    }
}

impl Drop for Rpc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn dir_has(dir: &Path, file: &str) -> bool {
    dir.join(file).exists()
}

#[test]
fn r4_over_stdio_the_result_carries_the_warnings_and_the_svg_is_written() {
    let doc = doc_with_budget(512, 25 * 1024);
    let out = doc.base.join("out");
    let mut s = start("--stdio", &doc);
    let r = s.request("art.build", json!({ "dir": out.to_string_lossy() }));
    assert_eq!(r["ok"], true);
    assert_eq!(r["outputs"][0]["warnings"], json!(["over its 0.5 KB budget"]));
    assert_eq!(r["outputs"][0]["kind"], "svg");
    assert!(dir_has(&out, "sample.svg"));
    let sizes = s.request("art.sizes", json!({ "name": "sample" }));
    assert_eq!(sizes["warnings"], json!(["over its 0.5 KB budget"]));
    let check = s.request("check", json!({}));
    assert_eq!(check["ok"], true);
    assert!(check["warnings"].as_array().unwrap().iter().any(|w| w["code"] == "over_budget"));
}

#[test]
fn r4_over_mcp_the_tool_result_carries_the_warnings() {
    let doc = doc_with_budget(512, 25 * 1024);
    let out = doc.base.join("out");
    let mut s = start("--mcp", &doc);
    s.request("initialize", json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "1" } }));
    let mut call = |name: &str, args: Value| s.request("tools/call", json!({ "name": name, "arguments": args }));
    let built = call("art_build", json!({ "dir": out.to_string_lossy() }));
    assert_ne!(built["isError"], true, "{built}");
    assert_eq!(built["structuredContent"]["ok"], true);
    assert_eq!(built["structuredContent"]["outputs"][0]["warnings"], json!(["over its 0.5 KB budget"]));
    let text = built["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("over its 0.5 KB budget (warning)"), "{text}");
    assert!(dir_has(&out, "sample.svg"));
    let sizes = call("art_sizes", json!({ "name": "sample" }));
    assert_eq!(sizes["structuredContent"]["warnings"], json!(["over its 0.5 KB budget"]));
    let check = call("check", json!({}));
    assert_ne!(check["isError"], true);
    assert!(check["structuredContent"]["warnings"].as_array().unwrap().iter().any(|w| w["code"] == "over_budget"));
}
