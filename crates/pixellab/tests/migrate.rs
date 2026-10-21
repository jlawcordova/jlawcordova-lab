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
        serde_json::json!(["palette.json: The sample document's palette (cli/test/fixtures): a few world colors and one outfit color."])
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
