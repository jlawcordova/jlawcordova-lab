//! document migrate (R8, D3) and the old format's problem (R9), on temp
//! copies of the format-1 fixture (fixtures/format1/sample.pixellab).

mod common;

use std::path::{Path, PathBuf};

use pixellab::cli::{main, Io};
use pixellab::registry::registry;
use serde_json::Value;

/// Runs the program's CLI: (exit code, stdout, stderr).
fn cli(argv: &[&str], env: &std::collections::HashMap<String, String>, cwd: &Path) -> (i32, String, String) {
    let reg = registry();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let argv: Vec<String> = argv.iter().map(|s| s.to_string()).collect();
    let code = main(&argv, &mut Io { stdout: &mut out, stderr: &mut err, env: env.clone(), cwd: cwd.to_path_buf(), registry: &reg });
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// A temp copy of the format-1 fixture, and an env with a temp HOME and state dir.
fn old_document() -> (PathBuf, std::collections::HashMap<String, String>) {
    let base = common::temp_dir("pixellab-migrate-");
    let root = base.join("old.pixellab");
    common::copy_dir(&common::repo().join("fixtures/format1/sample.pixellab"), &root);
    let mut env = std::collections::HashMap::new();
    env.insert("HOME".to_string(), base.join("home").to_string_lossy().into_owned());
    env.insert("PIXELLAB_STATE".to_string(), base.join("state").to_string_lossy().into_owned());
    (root, env)
}

fn without_notes(text: &str) -> Value {
    let mut v: Value = serde_json::from_str(text).unwrap();
    if let Some(m) = v.as_object_mut() {
        m.shift_remove("notes");
        m.shift_remove("colorNotes");
    }
    v
}

#[test]
fn r9_a_format_1_document_is_the_old_formats_problem_until_it_is_migrated() {
    let (root, env) = old_document();
    let (code, _, err) = cli(&["scene", "list", "--document", root.to_str().unwrap()], &env, &root);
    assert_eq!(code, 1);
    assert_eq!(err, "pixellab: sample uses the old format. Migrate it with `pixellab document migrate`, or in the app.\n");
    let (code, out, err) = cli(&["document", "migrate", "--document", root.to_str().unwrap()], &env, &root);
    assert_eq!(code, 0, "{err}");
    assert!(
        out.starts_with(&format!("Migrated to format 2 in {}: 6 files written, 1 note kept, 5 .mjs files removed", root.display())),
        "{out}"
    );
    let (code, out, _) = cli(&["scene", "list", "--document", root.to_str().unwrap()], &env, &root);
    assert_eq!(code, 0);
    assert!(out.contains("sample"));
    assert!(!root.join("palette.mjs").exists() && root.join("palette.json").exists());
}

#[test]
fn r8_migrate_to_writes_the_format_2_copy_and_leaves_the_document_alone() {
    let (root, env) = old_document();
    let copy = root.parent().unwrap().join("new.pixellab");
    let (code, out, err) =
        cli(&["document", "migrate", "--to", copy.to_str().unwrap(), "--document", root.to_str().unwrap(), "--json"], &env, &root);
    assert_eq!(code, 0, "{err}");
    let r: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        r["files"],
        serde_json::json!([
            "manifest.json",
            "palette.json",
            "objects/block.json",
            "objects/pebble.json",
            "scenes/sample-preview.json",
            "scenes/sample.json"
        ])
    );
    assert_eq!(r["removed"], serde_json::json!([]));
    assert_eq!(
        r["notes"],
        serde_json::json!(["palette.json: The sample document's palette (fixtures/format1): a few world colors and one outfit color."])
    );
    assert!(root.join("palette.mjs").exists(), "the document is left as it is");
    // The copy is the committed format-2 fixture, apart from the palette's notes.
    for file in ["palette.json", "objects/block.json", "objects/pebble.json", "scenes/sample-preview.json", "scenes/sample.json"] {
        let want = std::fs::read_to_string(common::fixture().join(&file)).unwrap();
        let got = std::fs::read_to_string(copy.join(&file)).unwrap();
        if file == "palette.json" {
            assert_eq!(without_notes(&got), without_notes(&want));
            assert_eq!(engine::format::serialize_palette(&serde_json::from_str(&got).unwrap()), got, "canonical");
        } else {
            assert_eq!(got, want, "{file}");
        }
    }
    assert_eq!(
        std::fs::read_to_string(copy.join("manifest.json")).unwrap(),
        std::fs::read_to_string(common::fixture().join("manifest.json")).unwrap()
    );
    // A path that exists is refused.
    let (code, _, err) = cli(&["document", "migrate", "--to", copy.to_str().unwrap(), "--document", root.to_str().unwrap()], &env, &root);
    assert_eq!((code, err), (1, format!("pixellab: {} already exists; pick another path\n", copy.display())));
}

#[test]
fn r8_migrate_refuses_code_already_migrated_documents_and_unsaved_edits() {
    let (root, env) = old_document();
    let doc = root.to_str().unwrap();
    // Code that isn't data, with its place.
    std::fs::write(root.join("objects/odd.mjs"), "import x from './y.mjs';\nexport default x;\n").unwrap();
    let (code, _, err) = cli(&["document", "migrate", "--document", doc], &env, &root);
    assert_eq!(code, 1);
    assert_eq!(err, "pixellab: 1 source can't be read as data; nothing written:\nobjects/odd.mjs:1:1: expected export default, found 'import' (imports, functions and other code aren't supported)\n");
    assert!(root.join("palette.mjs").exists() && !root.join("palette.json").exists(), "nothing written");
    std::fs::remove_file(root.join("objects/odd.mjs")).unwrap();
    // Unsaved edits open in Pixel Lab (a running process's open-docs file).
    let open = Path::new(&env["PIXELLAB_STATE"]).join("open");
    std::fs::create_dir_all(&open).unwrap();
    let file = open.join("1.json");
    std::fs::write(
        &file,
        serde_json::json!({ "pid": 1, "document": doc, "updatedAt": "x", "docs": [{ "doc": "scene:sample", "dirty": true }] }).to_string(),
    )
    .unwrap();
    let (code, _, err) = cli(&["document", "migrate", "--document", doc], &env, &root);
    assert_eq!((code, err.as_str()), (1, "pixellab: sample has unsaved edits open (scene:sample); save or revert them before migrating\n"));
    std::fs::remove_file(file).unwrap();
    assert_eq!(cli(&["document", "migrate", "--document", doc], &env, &root).0, 0);
    let (code, _, err) = cli(&["document", "migrate", "--document", doc], &env, &root);
    assert_eq!((code, err.as_str()), (1, "pixellab: sample is already in format 2; there's nothing to migrate\n"));
}

#[test]
fn r8_a_sites_legacy_source_folder_becomes_its_pixellab_bundle() {
    let base = common::temp_dir("pixellab-migrate-site-");
    let site = base.join("mysite");
    let source = site.join("src/assets/pixel-art/source");
    common::copy_dir(&common::repo().join("fixtures/format1/sample.pixellab"), &source);
    std::fs::remove_file(source.join("manifest.json")).unwrap();
    common::write(
        &site,
        "src/styles/variables.css",
        ":root { --color-card: #E9DCC6; --color-grid: rgba(90, 62, 43, 0.06); }\n[data-theme=\"dark\"] { --color-page: #1c1712; }\n",
    );
    let mut env = std::collections::HashMap::new();
    env.insert("HOME".to_string(), base.join("home").to_string_lossy().into_owned());
    env.insert("PIXELLAB_STATE".to_string(), base.join("state").to_string_lossy().into_owned());
    let (code, _, err) = cli(&["scene", "list", "--site", site.to_str().unwrap()], &env, &site);
    assert_eq!(
        (code, err.as_str()),
        (1, "pixellab: mysite uses the old format. Migrate it with `pixellab document migrate`, or in the app.\n")
    );
    let (code, out, err) = cli(&["document", "migrate", "--site", site.to_str().unwrap(), "--json"], &env, &site);
    assert_eq!(code, 0, "{err}");
    let r: Value = serde_json::from_str(&out).unwrap();
    let bundle = site.join("src/assets/pixel-art/mysite.pixellab");
    assert_eq!(r["root"], bundle.to_string_lossy().as_ref());
    assert!(!source.exists(), "the emptied source/ folder goes");
    let manifest: Value = serde_json::from_str(&std::fs::read_to_string(bundle.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["format"], 2);
    assert_eq!(manifest["publish"]["dir"], "..");
    assert_eq!(manifest["publish"]["budgets"]["hero-island.svg"]["raw"], 750 * 1024);
    assert_eq!(manifest["theme"]["darkPage"], "#1C1712");
    let (code, out, _) = cli(&["scene", "list", "--site", site.to_str().unwrap()], &env, &site);
    assert_eq!(code, 0);
    assert!(out.contains("sample"));
}

#[test]
fn r8_migrate_to_refuses_the_document_itself_and_a_folder_inside_it() {
    let (root, env) = old_document();
    let doc = root.to_str().unwrap();
    let before = list_files(&root);
    let (code, _, err) = cli(&["document", "migrate", "--document", doc, "--to", doc], &env, &root);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("already exists"), "{err}");
    let inside = root.join("copy.pixellab");
    let (code, _, err) = cli(&["document", "migrate", "--document", doc, "--to", inside.to_str().unwrap()], &env, &root);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("is inside the document itself"), "{err}");
    assert_eq!(list_files(&root), before, "nothing written");
}

#[test]
fn r8_in_place_the_manifest_is_written_after_the_sources() {
    // If writing a source fails, the manifest still says format 1 and the .mjs files stay.
    let (root, env) = old_document();
    let manifest = std::fs::read_to_string(root.join("manifest.json")).unwrap();
    std::fs::create_dir_all(root.join("scenes/sample.json")).unwrap(); // a folder where a source goes
    let (code, _, err) = cli(&["document", "migrate", "--document", root.to_str().unwrap()], &env, &root);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("writing ") && err.contains("failed"), "{err}");
    assert_eq!(std::fs::read_to_string(root.join("manifest.json")).unwrap(), manifest);
    assert!(root.join("palette.mjs").exists() && root.join("scenes/sample.mjs").exists());
}

/// Every file under a folder, relative and sorted.
fn list_files(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, rel: &str, out: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            if e.file_type().unwrap().is_dir() {
                walk(&e.path(), &path, out);
            } else {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, "", &mut out);
    out.sort();
    out
}

/// Runs `serve --stdio` over requests (one per line) until stdin ends; the responses by id.
fn stdio(args: &[&str], env: &std::collections::HashMap<String, String>, requests: &[(&str, Value)]) -> Vec<Value> {
    use std::io::Write as _;
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_pixellab"))
        .args(["serve", "--stdio"])
        .args(args)
        .env_clear()
        .envs(env)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for (id, (method, params)) in requests.iter().enumerate() {
        let line = serde_json::json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        writeln!(stdin, "{line}").unwrap();
    }
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    let mut replies: Vec<Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|m| m.get("id").is_some())
        .collect();
    replies.sort_by_key(|m| m["id"].as_u64());
    replies
}

#[test]
fn r8_r9_the_apps_server_opens_a_format_1_document_refuses_its_commands_and_migrates_it() {
    let (root, env) = old_document();
    let requests = [
        ("scene.list", serde_json::json!({})),
        ("session.info", serde_json::json!({})),
        ("document.migrate", serde_json::json!({})),
        ("scene.list", serde_json::json!({})),
    ];
    let r = stdio(&["--document", root.to_str().unwrap()], &env, &requests);
    assert_eq!(r[0]["error"]["message"], "sample uses the old format. Migrate it with `pixellab document migrate`, or in the app.");
    assert_eq!(r[0]["error"]["data"]["code"], "document");
    assert_eq!(r[1]["result"]["document"]["root"], root.to_string_lossy().as_ref());
    assert_eq!(r[2]["result"]["ok"], true, "{}", r[2]);
    assert_eq!(r[3]["result"]["scenes"][0]["file"], "scenes/sample.json", "{}", r[3]);
    assert!(root.join("palette.json").exists() && !root.join("palette.mjs").exists());
}

#[test]
fn r8_r9_the_apps_server_migrates_a_sites_legacy_source_folder_into_its_bundle_and_reopens_it() {
    let base = common::temp_dir("pixellab-migrate-site-");
    let site = base.join("mysite");
    let source = site.join("src/assets/pixel-art/source");
    common::copy_dir(&common::repo().join("fixtures/format1/sample.pixellab"), &source);
    std::fs::remove_file(source.join("manifest.json")).unwrap();
    let mut env = std::collections::HashMap::new();
    env.insert("HOME".to_string(), base.join("home").to_string_lossy().into_owned());
    env.insert("PIXELLAB_STATE".to_string(), base.join("state").to_string_lossy().into_owned());
    let requests =
        [("document.migrate", serde_json::json!({})), ("session.info", serde_json::json!({})), ("scene.list", serde_json::json!({}))];
    let r = stdio(&["--site", site.to_str().unwrap()], &env, &requests);
    let bundle = site.join("src/assets/pixel-art/mysite.pixellab");
    assert_eq!(r[0]["result"]["root"], bundle.to_string_lossy().as_ref(), "{}", r[0]);
    assert_eq!(r[1]["result"]["document"]["root"], bundle.to_string_lossy().as_ref());
    assert_eq!(r[1]["result"]["document"]["kind"], "document");
    assert_eq!(r[2]["result"]["scenes"][0]["name"], "sample", "{}", r[2]);
}

#[test]
fn r8_a_source_with_a_byte_order_mark_migrates_and_one_that_is_not_utf_8_is_refused_by_name() {
    let (root, env) = old_document();
    let palette = std::fs::read_to_string(root.join("palette.mjs")).unwrap();
    std::fs::write(root.join("palette.mjs"), format!("\u{feff}{palette}")).unwrap();
    std::fs::write(root.join("objects/bad.mjs"), b"export default { kind: '\xff' };\n").unwrap();
    let (code, _, err) = cli(&["document", "migrate", "--document", root.to_str().unwrap()], &env, &root);
    assert_eq!(code, 1);
    assert!(err.contains("1 source can't be read as data") && err.contains("objects/bad.mjs: not UTF-8 text"), "{err}");
    std::fs::remove_file(root.join("objects/bad.mjs")).unwrap();
    let (code, _, err) = cli(&["document", "migrate", "--document", root.to_str().unwrap()], &env, &root);
    assert_eq!(code, 0, "{err}");
}
