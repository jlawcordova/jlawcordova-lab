//! The document (ports cli/test/document.test.mjs; format 2): resolution
//! (R3), the manifest (R2), making and copying one (R4), a site checkout,
//! R9's old format, and fresh source loading.
#![allow(clippy::result_large_err)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{make_document, make_site, temp_dir};
use pixellab::context::Ctx;
use pixellab::document::{
    check_document, check_manifest, find_document_in_site, home_dir, open_document, resolve_document_root, site_budgets, site_colors, Env,
    From, OpenOptions, Resolved, Via, DEFAULT_DARK_PAGE,
};
use pixellab::errors::CommandError;
use pixellab::registry::registry;
use pixellab::session::{Options, Session};
use serde_json::{json, Value};

fn env_of(pairs: &[(&str, &str)]) -> Env {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn with(home: &Path, pairs: &[(&str, &str)]) -> Env {
    let mut env = env_of(pairs);
    env.insert("HOME".into(), home.to_string_lossy().into_owned());
    env
}

fn opts(doc: Option<&Path>, site: Option<&Path>, env: Env) -> OpenOptions {
    OpenOptions {
        doc: doc.map(|p| p.to_string_lossy().into_owned()),
        site: site.map(|p| p.to_string_lossy().into_owned()),
        env,
        cwd: PathBuf::from("/"),
    }
}

fn resolved(root: &str, from: From, via: Via) -> Resolved {
    Resolved { root: PathBuf::from(root), from, via }
}

fn config_dir(home: &Path) -> PathBuf {
    home.join(".config").join("pixellab")
}

fn write_config(home: &Path, text: &str) {
    fs::create_dir_all(config_dir(home)).unwrap();
    fs::write(config_dir(home).join("config.json"), text).unwrap();
}

fn resolve(doc: Option<&str>, site: Option<&str>, env: &Env, cwd: &str) -> Result<Resolved, CommandError> {
    resolve_document_root(doc, site, env, Path::new(cwd))
}

fn err_of<T>(r: Result<T, CommandError>) -> CommandError {
    match r {
        Ok(_) => panic!("expected an error"),
        Err(e) => e,
    }
}

fn is_document_error(e: &CommandError) -> bool {
    e.code == "document" && e.exit_code == 1
}

// document resolution (R3)

#[test]
fn r3_document_wins_over_pixellab_doc_the_config_file_site_pixellab_site_and_the_default() {
    let doc = make_document();
    let home = &doc.home;
    let root = doc.root.to_string_lossy().into_owned();
    write_config(home, &json!({ "document": "/from/config", "site": "/site/from/config" }).to_string());
    let e = |pairs: &[(&str, &str)]| with(home, pairs);
    assert_eq!(
        resolve(Some(&root), Some("/s"), &e(&[("PIXELLAB_DOC", "/from/env"), ("PIXELLAB_SITE", "/s2")]), "/").unwrap(),
        resolved(&root, From::Flag, Via::Doc)
    );
    assert_eq!(resolve(None, Some("/s"), &e(&[("PIXELLAB_DOC", "/from/env")]), "/").unwrap(), resolved("/from/env", From::Env, Via::Doc));
    assert_eq!(resolve(None, Some("/s"), &e(&[]), "/").unwrap(), resolved("/from/config", From::Config, Via::Doc));
    write_config(home, &json!({ "site": "/site/from/config" }).to_string());
    assert_eq!(resolve(None, Some("/s"), &e(&[("PIXELLAB_SITE", "/s2")]), "/").unwrap(), resolved("/s", From::Flag, Via::Site));
    assert_eq!(resolve(None, None, &e(&[("PIXELLAB_SITE", "/s2")]), "/").unwrap(), resolved("/s2", From::Env, Via::Site));
    assert_eq!(resolve(None, None, &e(&[]), "/").unwrap(), resolved("/site/from/config", From::Config, Via::Site));
    fs::remove_dir_all(config_dir(home)).unwrap();
    let default = resolve(None, None, &e(&[]), "/").unwrap();
    assert_eq!(default, Resolved { root: home.join("Projects").join("jlawcordova.github.io"), from: From::Default, via: Via::Site });
}

#[test]
fn tilde_and_relative_paths_are_expanded() {
    let doc = make_document();
    let env = with(&doc.home, &[]);
    assert_eq!(resolve(Some("~/x.pixellab"), None, &env, "/").unwrap().root, doc.home.join("x.pixellab"));
    assert_eq!(resolve(Some("rel"), None, &env, "/tmp/base").unwrap().root, PathBuf::from("/tmp/base/rel"));
    assert_eq!(home_dir(&env), doc.home);
}

#[test]
fn a_config_file_that_is_not_json_or_with_a_non_path_value_is_a_clear_error() {
    let doc = make_document();
    let env = with(&doc.home, &[]);
    write_config(&doc.home, "{ nope");
    let e = err_of(resolve(None, None, &env, "/"));
    assert!(is_document_error(&e) && e.message.contains("config.json is not valid JSON"), "{}", e.message);
    write_config(&doc.home, &json!({ "document": 7 }).to_string());
    let e = err_of(resolve(None, None, &env, "/"));
    assert!(is_document_error(&e) && e.message.contains(r#""document" must be a path"#), "{}", e.message);
}

#[test]
fn r3_a_missing_folder_names_the_path_where_it_came_from_and_how_to_fix_it() {
    let doc = make_document();
    let nowhere = doc.base.join("nowhere");
    let e = err_of(check_document(&nowhere, From::Env, Via::Doc));
    assert!(is_document_error(&e));
    assert!(
        e.message.starts_with(&format!("document not found: {} (from PIXELLAB_DOC) is not a folder", nowhere.display())),
        "{}",
        e.message
    );
    assert!(e.message.contains("--document <path.pixellab>"));
    let e = err_of(check_document(&nowhere, From::Flag, Via::Site));
    assert!(is_document_error(&e));
    assert!(e.message.starts_with(&format!("site not found: {} (from --site) is not a folder", nowhere.display())), "{}", e.message);
}

#[test]
// Format 2 reads palette.json, so a folder without it is no document.
fn r3_a_folder_without_palette_mjs_is_not_a_document_a_checkout_without_art_is_not_a_site() {
    let doc = make_document();
    let e = err_of(check_document(&doc.home, From::Flag, Via::Doc));
    assert!(is_document_error(&e) && e.message.contains("isn't a Pixel Lab document: it has no palette.json"), "{}", e.message);
    let e = err_of(check_document(&doc.home, From::Flag, Via::Site));
    assert!(is_document_error(&e) && e.message.contains("doesn't look like the site checkout: no *.pixellab or source/"), "{}", e.message);
}

fn cli(args: &[&str], env: Env) -> (i32, String, String) {
    let reg = registry();
    let argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = {
        let mut io = pixellab::cli::Io { stdout: &mut out, stderr: &mut err, env, cwd: PathBuf::from("/"), registry: &reg };
        pixellab::cli::main(&argv, &mut io)
    };
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

#[test]
fn r3_the_cli_reports_a_bad_document_with_exit_code_1_and_code_document() {
    let doc = make_document();
    let nowhere = doc.base.join("nowhere");
    let (code, out, err) = cli(&["scene", "list", "--document", &nowhere.to_string_lossy()], doc.env());
    assert_eq!(code, 1);
    assert!(err.starts_with("pixellab: document not found: "), "{err}");
    assert!(err.contains("nowhere (from --document)"), "{err}");
    assert_eq!(out, "");
    let mut env = doc.env();
    env.insert("PIXELLAB_SITE".into(), doc.home.to_string_lossy().into_owned());
    let (code, _, err) = cli(&["scene", "list", "--json"], env);
    assert_eq!(code, 1);
    let e: Value = serde_json::from_str(&err).unwrap();
    assert_eq!(e["error"]["code"], "document");
}

#[test]
fn r3_the_default_site_under_home_is_tried_when_nothing_else_is_set() {
    let doc = make_document();
    let (code, _, err) = cli(&["document", "info"], doc.env());
    assert_eq!(code, 1);
    assert!(err.contains("Projects/jlawcordova.github.io (from the default site)"), "{err}");
}

#[test]
fn r1_r6_open_document_reads_the_manifest_and_carries_this_packages_engine() {
    let doc = make_document();
    let opened = open_document(&opts(Some(&doc.root), None, doc.env())).unwrap();
    assert_eq!(opened.root, doc.root);
    assert_eq!(opened.kind, "document");
    assert_eq!(opened.name, "sample");
    assert_eq!(opened.publish_dir, Some(doc.base.join("sample-out")));
    assert_eq!(opened.images_dir, None);
    assert!(opened.budgets.is_empty());
    assert_eq!(opened.preview_card.as_deref(), Some("#E9DCC6"));
    assert_eq!(opened.preview_grid.as_deref(), Some("rgba(90, 62, 43, 0.06)"));
    assert_eq!(opened.dark_page, DEFAULT_DARK_PAGE);
}

#[test]
fn r2_a_document_without_a_manifest_opens_with_defaults_and_no_publish_folder() {
    let doc = make_document();
    fs::remove_file(doc.root.join("manifest.json")).unwrap();
    let opened = open_document(&opts(Some(&doc.root), None, doc.env())).unwrap();
    assert_eq!(opened.publish_dir, None);
    assert_eq!(opened.name, "sample");
    assert_eq!((opened.preview_card, opened.preview_grid), (None, None));
}

// the manifest (R2)

fn ok(m: Value) {
    assert_eq!(check_manifest(&m, "manifest.json").unwrap(), m);
}

fn bad(m: Value, expect: &str) {
    let e = err_of(check_manifest(&m, "manifest.json"));
    assert!(is_document_error(&e) && e.message.contains(expect), "wanted {expect:?} in {:?}", e.message);
}

#[test]
// The current format is 2: required, and nothing else is.
fn r2_format_is_required_and_must_be_1_everything_else_is_optional() {
    ok(json!({ "format": 2 }));
    bad(json!({}), r#""format" must be 2 (missing)"#);
    bad(json!({ "format": 3 }), r#""format" must be 2 (got 3)"#);
    bad(json!([]), "must be a JSON object");
}

#[test]
fn r2_format_2_with_every_key_is_accepted() {
    ok(json!({
        "format": 2, "name": "x",
        "publish": { "dir": "..", "images": "../png", "budgets": { "a.svg": { "raw": 1, "gzip": 1 } } },
        "preview": { "card": "#000000", "grid": "#111111" },
        "theme": { "darkPage": "#1C1712" }
    }));
}

#[test]
fn r2_unknown_keys_and_bad_values_are_named() {
    bad(json!({ "format": 2, "colour": 1 }), r#"unknown key "colour""#);
    bad(json!({ "format": 2, "publish": { "folder": ".." } }), r#"unknown key publish."folder""#);
    bad(json!({ "format": 2, "publish": { "dir": "" } }), r#""publish.dir" must be a path"#);
    bad(json!({ "format": 2, "publish": { "images": "" } }), r#""publish.images" must be a path"#);
    bad(
        json!({ "format": 2, "publish": { "budgets": { "a.svg": { "raw": -1, "gzip": 1 } } } }),
        r#""publish.budgets.a.svg" must be { raw: <bytes>, gzip: <bytes> }"#,
    );
    bad(json!({ "format": 2, "preview": { "card": "red" } }), r#"preview.card "red" isn't a #RRGGBB color"#);
    bad(json!({ "format": 2, "preview": { "grid": "rgb(1, 2, 3)" } }), "isn't a #RRGGBB or rgba");
    bad(json!({ "format": 2, "theme": { "darkPage": "#12" } }), r#""theme.darkPage" must be a #RRGGBB color"#);
}

#[test]
fn r2_unknown_keys_and_bad_values_are_named_without_previews() {
    bad(json!({ "format": 2, "colour": 1 }), r#"unknown key "colour""#);
    bad(json!({ "format": 2, "publish": { "folder": ".." } }), r#"unknown key publish."folder""#);
    bad(json!({ "format": 2, "publish": { "dir": "" } }), r#""publish.dir" must be a path"#);
    bad(json!({ "format": 2, "publish": { "images": 3 } }), r#""publish.images" must be a path"#);
    bad(
        json!({ "format": 2, "publish": { "budgets": { "a.svg": { "raw": -1, "gzip": 1 } } } }),
        r#""publish.budgets.a.svg" must be { raw: <bytes>, gzip: <bytes> }"#,
    );
    bad(json!({ "format": 2, "theme": { "darkPage": "#12" } }), r#""theme.darkPage" must be a #RRGGBB color"#);
    ok(
        json!({ "format": 2, "name": "x", "publish": { "dir": "..", "images": "../png", "budgets": { "a.svg": { "raw": 1, "gzip": 1 } } }, "theme": { "darkPage": "#1C1712" } }),
    );
}

#[test]
fn r2_a_manifest_that_is_not_json_or_that_fails_the_checks_stops_the_document_from_opening() {
    let doc = make_document();
    doc.write("manifest.json", "{ nope");
    let e = err_of(open_document(&opts(Some(&doc.root), None, doc.env())));
    assert!(is_document_error(&e) && e.message.contains("manifest.json is not valid JSON"), "{}", e.message);
    doc.write("manifest.json", r#"{ "format": 3 }"#);
    let e = err_of(open_document(&opts(Some(&doc.root), None, doc.env())));
    assert!(e.message.contains(r#"manifest.json: "format" must be 2"#), "{}", e.message);
}

#[test]
fn r2_the_manifests_budgets_colors_and_theme_reach_the_document() {
    let doc = make_document();
    let manifest = json!({ "format": 2, "publish": { "dir": "/abs/out", "images": "png", "budgets": { "sample.svg": { "raw": 10, "gzip": 5 } } }, "theme": { "darkPage": "#000000" } });
    doc.write("manifest.json", &manifest.to_string());
    let opened = open_document(&opts(Some(&doc.root), None, doc.env())).unwrap();
    assert_eq!(opened.publish_dir, Some(PathBuf::from("/abs/out")));
    assert_eq!(opened.images_dir, Some(doc.root.join("png")), "publish.images is resolved like publish.dir");
    assert_eq!(opened.budgets.len(), 1);
    assert_eq!((opened.budgets["sample.svg"].raw, opened.budgets["sample.svg"].gzip), (10, 5));
    assert_eq!(opened.dark_page, "#000000");
    assert_eq!(opened.name, "sample", "the folder name when the manifest has none");
}

// document.new and document.copy (R4)

/// Runs a command on a session over the document at `root`.
fn call_on(root: &Path, home: &Path, name: &str, params: Value) -> Result<Value, pixellab::errors::Error> {
    let env = with(home, &[("PIXELLAB_STATE", &home.join("state").to_string_lossy())]);
    let document = open_document(&opts(Some(root), None, env.clone())).unwrap();
    let mut session = Session::new(document, Options { env, ..Options::default() });
    registry().call(&mut Ctx::new(&mut session), name, params, false)
}

#[test]
fn r4_document_new_needs_no_open_document_so_it_works_on_a_mac_with_no_site_checkout() {
    // An empty home: no config, no default site to fall back on (as in CI).
    let base = temp_dir("pixellab-test-");
    let fresh = base.join("fresh");
    let r = registry().call(&mut Ctx::none(), "document.new", json!({ "path": fresh.to_string_lossy() }), false).unwrap();
    assert_eq!(r["root"], json!(format!("{}.pixellab", fresh.display())));
    // A command that needs a document still says how to name one.
    let e = err_of(open_document(&opts(None, None, with(&base, &[]))));
    assert!(is_document_error(&e));
    assert!(e.message.contains("Pass --document <path.pixellab>"), "{}", e.message);
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn r4_document_new_makes_an_empty_document_with_the_starter_palette_and_a_manifest_and_refuses_an_existing_path() {
    let base = temp_dir("pixellab-test-");
    let path = base.join("meadow");
    let reg = registry();
    let out = reg.call(&mut Ctx::none(), "document.new", json!({ "path": path.to_string_lossy() }), false).unwrap();
    let root = PathBuf::from(format!("{}.pixellab", path.display()));
    assert_eq!(out["root"], json!(root.to_string_lossy()));
    assert_eq!(out["name"], "meadow");
    let manifest: Value = serde_json::from_str(&fs::read_to_string(root.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest, json!({ "format": 2, "name": "meadow" }));
    assert!(root.join("objects").is_dir() && root.join("scenes").is_dir());
    let opened = open_document(&opts(Some(&root), None, Env::new())).unwrap();
    let docs = opened.load_docs();
    assert!(docs.load_errors.is_empty(), "{:?}", docs.load_errors);
    let sources = engine::engine::load_sources(docs.palette.unwrap(), docs.objects, docs.scenes);
    assert_eq!(sources.colors.len(), 48, "the starter palette: 32 world and 16 outfit colors");
    assert!(engine::engine::validate(&sources).is_empty());
    // (scene new on it waits for the scene commands: scene.new is ported with them.)
    let again = err_of_call(reg.call(&mut Ctx::none(), "document.new", json!({ "path": path.to_string_lossy() }), false));
    assert_eq!((again.code.as_str(), again.exit_code), ("exists", 1));
    let other = base.join("other.pixellab");
    let named = reg.call(&mut Ctx::none(), "document.new", json!({ "path": other.to_string_lossy(), "name": "Other art" }), false).unwrap();
    assert_eq!(named["name"], "Other art");
    let _ = fs::remove_dir_all(&base);
}

fn err_of_call(r: Result<Value, pixellab::errors::Error>) -> CommandError {
    match r {
        Err(pixellab::errors::Error::Command(c)) => c,
        other => panic!("expected a command error, got {other:?}"),
    }
}

#[test]
fn r4_document_copy_saves_the_whole_document_elsewhere_save_as_and_refuses_an_existing_path() {
    let doc = make_document();
    let to = doc.base.join("copy");
    let copy = PathBuf::from(format!("{}.pixellab", to.display()));
    let r = call_on(&doc.root, &doc.home, "document.copy", json!({ "to": to.to_string_lossy() })).unwrap();
    let from = doc.root.to_string_lossy().into_owned();
    let dest = copy.to_string_lossy().into_owned();
    assert_eq!(r, json!({ "from": from, "to": dest, "wroteManifest": false, "summary": format!("Copied {from} to {dest}") }));
    assert_eq!(
        fs::read_to_string(copy.join("objects/pebble.json")).unwrap(),
        fs::read_to_string(doc.root.join("objects/pebble.json")).unwrap()
    );
    let opened = open_document(&opts(Some(&copy), None, Env::new())).unwrap();
    assert_eq!(opened.name, "sample", "the manifest comes along");
    assert_eq!(opened.publish_dir, Some(doc.base.join("sample-out")), "a relative publish.dir now points next to the copy");
    let again = err_of_call(call_on(&doc.root, &doc.home, "document.copy", json!({ "to": to.to_string_lossy() })));
    assert_eq!(again.exit_code, 1);
    assert!(again.message.contains("already exists"), "{}", again.message);
    let inside = err_of_call(call_on(&doc.root, &doc.home, "document.copy", json!({ "to": doc.root.join("inner").to_string_lossy() })));
    assert_eq!(inside.exit_code, 2);
}

// a site checkout (R3, legacy)

#[test]
fn r3_a_pixellab_under_src_assets_pixel_art_is_the_document_of_a_site() {
    let site = make_site();
    assert_eq!(find_document_in_site(&site.root), Some((site.document.clone(), false)));
    let opened = open_document(&opts(None, Some(&site.root), site.env())).unwrap();
    assert_eq!(opened.kind, "document");
    assert_eq!(opened.root, site.document);
    assert_eq!(opened.site_root, Some(site.root.clone()));
    assert_eq!(opened.name, "sample");
}

#[test]
fn r3_a_pixellab_under_src_assets_pixel_art_wins_over_source_and_the_first_by_name_wins() {
    let site = make_site();
    let art = site.root.join("src/assets/pixel-art");
    fs::create_dir_all(art.join("source")).unwrap();
    fs::create_dir_all(art.join("aaa.pixellab")).unwrap();
    assert_eq!(find_document_in_site(&site.root), Some((art.join("aaa.pixellab"), false)));
    fs::remove_dir_all(art.join("aaa.pixellab")).unwrap();
    fs::remove_dir_all(&site.document).unwrap();
    assert_eq!(find_document_in_site(&site.root), Some((art.join("source"), true)));
    fs::remove_dir_all(art.join("source")).unwrap();
    assert_eq!(find_document_in_site(&site.root), None);
}

#[test]
fn r3_the_sites_budgets_are_the_legacy_ones() {
    let b = site_budgets();
    assert_eq!((b["hero-island.svg"].raw, b["hero-island.svg"].gzip), (750 * 1024, 150 * 1024));
    assert_eq!((b["range-sprite.svg"].raw, b["range-sprite.svg"].gzip), (200 * 1024, 50 * 1024));
}

#[test]
#[ignore = "a legacy source folder is format 1 now (R9); document migrate covers it"]
fn r4_a_copy_of_a_legacy_source_folder_gets_a_manifest_with_the_sites_budgets_and_publish_folder() {
    todo!("U25 (and migrate): a legacy source folder is format 1 now (R9); document copy of it is covered by document migrate");
}

// R9: format 1 is migrated, not edited

const OLD: &str = " uses the old format. Migrate it with `pixellab document migrate`, or in the app.";

#[test]
fn r9_a_folder_with_palette_mjs_and_no_palette_json_gives_the_old_format_problem() {
    let base = temp_dir("pixellab-test-");
    let root = base.join("meadow.pixellab");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("palette.mjs"), "export default { world: {}, outfit: {} };\n").unwrap();
    let e = err_of(check_document(&root, From::Flag, Via::Doc));
    assert!(is_document_error(&e));
    assert_eq!(e.message, format!("meadow{OLD}"));
    // The manifest's name wins over the folder's.
    fs::write(root.join("manifest.json"), r#"{ "format": 1, "name": "Meadow art" }"#).unwrap();
    let e = err_of(check_document(&root, From::Flag, Via::Doc));
    assert_eq!(e.message, format!("Meadow art{OLD}"));
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn r9_a_manifest_with_format_1_gives_the_old_format_problem() {
    let doc = make_document();
    doc.write("manifest.json", r#"{ "format": 1 }"#);
    let e = err_of(open_document(&opts(Some(&doc.root), None, doc.env())));
    assert!(is_document_error(&e));
    assert_eq!(e.message, format!("sample{OLD}"));
    doc.write("manifest.json", r#"{ "format": 1, "name": "Named" }"#);
    let e = err_of(open_document(&opts(Some(&doc.root), None, doc.env())));
    assert_eq!(e.message, format!("Named{OLD}"));
}

#[test]
fn r9_a_site_with_a_legacy_source_folder_gives_the_old_format_problem_named_for_the_site() {
    let base = temp_dir("pixellab-test-");
    let site = base.join("my-site");
    let home = base.join("home");
    fs::create_dir_all(&home).unwrap();
    let source = site.join("src/assets/pixel-art/source");
    fs::create_dir_all(source.join("scenes")).unwrap();
    fs::write(source.join("palette.mjs"), "export default { world: {}, outfit: {} };\n").unwrap();
    let e = err_of(open_document(&opts(None, Some(&site), with(&home, &[]))));
    assert!(is_document_error(&e));
    assert_eq!(e.message, format!("my-site{OLD}"));
    // A site whose *.pixellab is still format 1 reports that document's name.
    let bundle = site.join("src/assets/pixel-art/art.pixellab");
    fs::create_dir_all(&bundle).unwrap();
    fs::write(bundle.join("palette.mjs"), "export default {};\n").unwrap();
    let e = err_of(open_document(&opts(None, Some(&site), with(&home, &[]))));
    assert_eq!(e.message, format!("art{OLD}"));
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn r9_publish_images_is_accepted_and_resolved() {
    let doc = make_document();
    doc.write("manifest.json", &json!({ "format": 2, "publish": { "dir": "../out", "images": "../png" } }).to_string());
    let opened = open_document(&opts(Some(&doc.root), None, doc.env())).unwrap();
    assert_eq!(opened.publish_dir, Some(doc.base.join("out")));
    assert_eq!(opened.images_dir, Some(doc.base.join("png")));
    doc.write("manifest.json", &json!({ "format": 2, "publish": { "images": "/abs/png" } }).to_string());
    let opened = open_document(&opts(Some(&doc.root), None, doc.env())).unwrap();
    assert_eq!(opened.images_dir, Some(PathBuf::from("/abs/png")));
    assert_eq!(opened.publish_dir, None);
}

// source loading

#[test]
fn loads_the_palette_objects_and_scenes() {
    let doc = make_document();
    doc.write("manifest.json", r#"{ "format": 2 }"#);
    let opened = open_document(&opts(Some(&doc.root), None, doc.env())).unwrap();
    let docs = opened.load_docs();
    assert!(docs.palette.is_some());
    assert!(docs.objects.contains_key("pebble") && docs.objects.contains_key("block"));
    assert!(docs.scenes.contains_key("sample"));
    assert!(docs.load_errors.is_empty(), "{:?}", docs.load_errors);
}

#[test]
fn an_edit_on_disk_is_picked_up_by_the_next_load() {
    let doc = make_document();
    doc.write("manifest.json", r#"{ "format": 2 }"#);
    let opened = open_document(&opts(Some(&doc.root), None, doc.env())).unwrap();
    doc.write("scenes/zz-fresh.json", r#"{ "viewBox": [0, 0, 10, 10], "items": [] }"#);
    assert_eq!(opened.load_docs().scenes["zz-fresh"]["viewBox"], json!([0, 0, 10, 10]));
    doc.write("scenes/zz-fresh.json", r#"{ "viewBox": [0, 0, 20, 10], "items": [] }"#);
    assert_eq!(opened.load_docs().scenes["zz-fresh"]["viewBox"], json!([0, 0, 20, 10]));
}

#[test]
fn docs_are_copies_changing_one_does_not_change_the_next_load() {
    let doc = make_document();
    doc.write("manifest.json", r#"{ "format": 2 }"#);
    let opened = open_document(&opts(Some(&doc.root), None, doc.env())).unwrap();
    let mut first = opened.load_docs();
    let before = first.objects["pebble"].clone();
    first.objects.get_mut("pebble").unwrap()["anchor"] = json!([99, 99]);
    assert_eq!(opened.load_docs().objects["pebble"], before);
}

#[test]
// Format 2 sources are JSON: a file that isn't JSON is the load error.
fn a_file_that_cant_be_imported_is_a_load_error_as_the_compiler_reports_it() {
    let doc = make_document();
    doc.write("manifest.json", r#"{ "format": 2 }"#);
    doc.write("objects/zz-broken.json", "{");
    let opened = open_document(&opts(Some(&doc.root), None, doc.env())).unwrap();
    let docs = opened.load_docs();
    assert_eq!(docs.load_errors.len(), 1);
    assert!(docs.load_errors[0].starts_with("objects/zz-broken.json: cannot be loaded: "), "{:?}", docs.load_errors);
    assert!(!docs.objects.contains_key("zz-broken"));
}

/// The session's working copies are the overlay now: a doc it holds replaces the disk's, a
/// new one is added. (The Node overlay's `null` for a removed doc has no Rust counterpart:
/// no session ever returned it.)
#[test]
fn an_overlay_replaces_adds_and_removes_docs_for_a_session() {
    use pixellab::context::DocKind;
    use pixellab::session::Edit;
    let doc = make_document();
    let mut env = doc.env();
    env.insert("PIXELLAB_STATE".into(), doc.base.join("state").to_string_lossy().into_owned());
    let document = open_document(&opts(Some(&doc.root), None, env.clone())).unwrap();
    let mut session = Session::new(document, Options { env, ..Options::default() });
    let mut ctx = Ctx::new(&mut session);
    let draft = json!({ "viewBox": [0, 0, 8, 8], "items": [{ "object": "pebble", "at": { "px": [4, 7] } }] });
    let draft2 = draft.clone();
    ctx.session().unwrap().create(DocKind::Scene, "draft", draft.clone(), "draft");
    assert_eq!(ctx.doc(DocKind::Scene, "draft").unwrap(), draft);
    assert!(ctx.names(DocKind::Scene).unwrap().contains(&"draft".to_string()));
    // Replaces: an edit of a doc on disk shows through the context.
    ctx.edit(DocKind::Scene, "sample", None, |d, _| {
        d["viewBox"] = json!([0, 0, 9, 9]);
        Ok(Edit::new(serde_json::Map::new(), "changed"))
    })
    .unwrap();
    assert_eq!(ctx.doc(DocKind::Scene, "sample").unwrap()["viewBox"], json!([0, 0, 9, 9]));
    assert_eq!(draft2["viewBox"], json!([0, 0, 8, 8]));
    let missing = ctx.doc(DocKind::Scene, "footer-dirt").unwrap_err();
    assert!(matches!(missing, pixellab::errors::Error::Command(c) if c.code == "not_found"));
}

#[test]
fn r3_a_sites_colors_come_from_its_variables_css() {
    let site = make_site();
    let (card, grid, dark) = site_colors(&site.root);
    assert_eq!(card.as_deref(), Some("#E9DCC6"));
    assert_eq!(grid.as_deref(), Some("rgba(90, 62, 43, 0.06)"));
    assert_eq!(dark.as_deref(), Some("#1C1712"));
    assert_eq!(site_colors(&site.base), (None, None, None));
}
