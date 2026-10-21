//! The read commands against the Node CLI's results (plan step 6): with
//! PIXELLAB_GOLDENS set to the folder `node scripts/goldens.mjs --out <dir>`
//! wrote, every read command the corpus invoked is run on the format-2 copy
//! and its JSON compared with Node's, after R3's normalizing: the documents'
//! paths, `.mjs` → `.json`, no `ms` or `node`, and PNGs compared decoded.
//! Commands not ported yet are skipped and counted.

use std::path::{Path, PathBuf};

use pixellab::context::Ctx;
use pixellab::document::{open_document, OpenOptions};
use pixellab::registry::registry;
use pixellab::session::{Options, Session};
use serde_json::{Map, Value};

fn goldens() -> Option<PathBuf> {
    match std::env::var_os("PIXELLAB_GOLDENS") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("corpus: skipped (set PIXELLAB_GOLDENS to the folder `node scripts/goldens.mjs --out <dir>` wrote)");
            None
        }
    }
}

/// A value with R3's differences taken out.
fn normalize(v: &Value, roots: &[(String, &str)], node: bool) -> Value {
    match v {
        Value::String(s) => {
            let mut s = s.clone();
            for (root, to) in roots {
                s = s.replace(root.as_str(), to);
            }
            if node {
                s = s.replace(".mjs", ".json");
            }
            Value::String(s)
        }
        Value::Array(list) => Value::Array(list.iter().map(|e| normalize(e, roots, node)).collect()),
        Value::Object(m) => {
            let mut out = Map::new();
            for (k, e) in m {
                if k == "ms" || k == "node" {
                    continue;
                }
                if (k == "png" || k == "focusPng") && e.is_string() {
                    out.insert(k.clone(), Value::String(png_summary(e.as_str().unwrap())));
                    continue;
                }
                out.insert(k.clone(), normalize(e, roots, node));
            }
            Value::Object(out)
        }
        v => v.clone(),
    }
}

fn base64_decode(text: &str) -> Vec<u8> {
    let table = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        _ => 63,
    };
    let bytes: Vec<u8> = text.bytes().filter(|c| *c != b'=').collect();
    let mut out = Vec::new();
    for chunk in bytes.chunks(4) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, c)| n | (table(*c) as u32) << (18 - 6 * i));
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    out
}

/// A PNG as its decoded size and pixels' hash, so two encoders compare equal.
fn png_summary(b64: &str) -> String {
    match engine::png::decode(&base64_decode(b64)) {
        Ok(img) => format!("<png {}×{} {}>", img.width, img.height, &engine::sha256::hex(&img.rgba)[..16]),
        Err(e) => format!("<bad png: {e}>"),
    }
}

#[test]
fn every_read_command_gives_nodes_result() {
    let Some(dir) = goldens() else { return };
    let corpus: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("corpus.json")).unwrap()).unwrap();
    let reg = registry();
    let (mut compared, mut skipped) = (0, std::collections::BTreeMap::<String, usize>::new());
    let mut differ = Vec::new();
    for doc in corpus["docs"].as_array().unwrap() {
        let name = doc["name"].as_str().unwrap();
        let format1 = doc["format1"].as_str().unwrap();
        let format2 = doc["format2"].as_str().unwrap();
        let parent = |p: &str| Path::new(p).parent().unwrap().to_string_lossy().into_owned();
        let node_roots = [(format1.to_string(), "<root>"), (parent(format1), "<base>")];
        let rust_roots = [(format2.to_string(), "<root>"), (parent(format2), "<base>")];
        let state = std::env::temp_dir().join(format!("pixellab-corpus-state-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&state);
        let mut env = pixellab::document::process_env();
        env.insert("PIXELLAB_STATE".into(), state.to_string_lossy().into_owned());
        env.remove("PIXELLAB_DOC");
        let document =
            open_document(&OpenOptions { doc: Some(format2.into()), site: None, env: env.clone(), cwd: PathBuf::from(format2) }).unwrap();
        let mut session = Session::new(document, Options { env, ..Options::default() });
        let cases: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("goldens").join(name).join("commands.json")).unwrap()).unwrap();
        for case in cases.as_array().unwrap() {
            let command = case["command"].as_str().unwrap();
            if !reg.has(command) {
                *skipped.entry(command.to_string()).or_default() += 1;
                continue;
            }
            let want = if case["ok"] == Value::Bool(true) {
                normalize(&case["result"], &node_roots, true)
            } else {
                normalize(
                    &serde_json::json!({ "error": { "code": case["error"]["code"], "message": case["error"]["message"] } }),
                    &node_roots,
                    true,
                )
            };
            let got = match reg.call(&mut Ctx::new(&mut session), command, case["params"].clone(), false) {
                Ok(v) => normalize(&v, &rust_roots, false),
                Err(pixellab::errors::Error::Command(e)) => {
                    normalize(&serde_json::json!({ "error": { "code": e.code, "message": e.message } }), &rust_roots, false)
                }
                Err(e) => serde_json::json!({ "error": { "code": "internal", "message": e.message() } }),
            };
            compared += 1;
            let (w, g) = (engine::format::stringify(&want), engine::format::stringify(&got));
            if w != g {
                let at = w.bytes().zip(g.bytes()).position(|(a, b)| a != b).unwrap_or(w.len().min(g.len()));
                let from = at.saturating_sub(80);
                differ.push(format!(
                    "{name}: {command} {}\n  node: …{}…\n  rust: …{}…",
                    case["params"],
                    &w[from..w.len().min(at + 120)],
                    &g[from.min(g.len())..g.len().min(at + 120)]
                ));
            }
        }
        let _ = std::fs::remove_dir_all(&state);
    }
    eprintln!("corpus: {compared} command results compared; not ported yet: {skipped:?}");
    assert!(differ.is_empty(), "{} results differ:\n{}", differ.len(), differ.iter().take(15).cloned().collect::<Vec<_>>().join("\n"));
}
