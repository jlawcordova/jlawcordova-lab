//! The document (ports cli/test/document.test.mjs; format 2): resolution
//! (R3), the manifest (R2), making and copying one (R4), a site checkout,
//! R9's old format, and fresh source loading.
#![allow(clippy::result_large_err)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{make_document, make_site, temp_dir};
use pixellab::document::{
    check_document, check_manifest, find_document_in_site, home_dir, open_document, resolve_document_root, site_budgets, site_colors, Env,
    From, OpenOptions, Resolved, Via, DEFAULT_DARK_PAGE,
};
use pixellab::errors::CommandError;
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
fn r3_a_folder_without_palette_json_is_not_a_document_a_checkout_without_art_is_not_a_site() {
    let doc = make_document();
    let e = err_of(check_document(&doc.home, From::Flag, Via::Doc));
    assert!(is_document_error(&e) && e.message.contains("isn't a Pixel Lab document: it has no palette.json"), "{}", e.message);
    let e = err_of(check_document(&doc.home, From::Flag, Via::Site));
    assert!(is_document_error(&e) && e.message.contains("doesn't look like the site checkout: no *.pixellab or source/"), "{}", e.message);
}

#[test]
#[ignore = "needs U21"]
fn r3_the_cli_reports_a_bad_document_with_exit_code_1_and_code_document() {
    todo!("U21: run `scene list --document <nowhere>`: exit 1, stderr starts `pixellab: document not found: .*nowhere (from --document)`, empty stdout; with --json the error code is `document`");
}

#[test]
#[ignore = "needs U21"]
fn r3_the_default_site_under_home_is_tried_when_nothing_else_is_set() {
    todo!("U21: run `document info` with an empty HOME: exit 1, stderr names Projects/jlawcordova.github.io (from the default site)");
}

#[test]
#[ignore = "needs U8"]
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
#[ignore = "needs U8"]
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
fn r2_format_is_required_and_must_be_2_everything_else_is_optional() {
    ok(json!({ "format": 2 }));
    bad(json!({}), r#""format" must be 2 (missing)"#);
    bad(json!({ "format": 3 }), r#""format" must be 2 (got 3)"#);
    bad(json!([]), "must be a JSON object");
}

#[test]
#[ignore = "needs U8"]
fn r2_format_2_with_every_key_is_accepted() {
    ok(json!({
        "format": 2, "name": "x",
        "publish": { "dir": "..", "images": "../png", "budgets": { "a.svg": { "raw": 1, "gzip": 1 } } },
        "preview": { "card": "#000000", "grid": "#111111" },
        "theme": { "darkPage": "#1C1712" }
    }));
}

#[test]
#[ignore = "needs U8"]
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

#[test]
#[ignore = "needs U25"]
fn r4_document_new_needs_no_open_document_so_it_works_on_a_mac_with_no_site_checkout() {
    todo!("U25: `document new <base>/fresh --json` with an empty HOME exits 0 and names <base>/fresh.pixellab; `scene list` exits 1 with `Pass --document <path.pixellab>`");
}

#[test]
#[ignore = "needs U25"]
fn r4_document_new_makes_an_empty_document_with_the_starter_palette_and_a_manifest_and_refuses_an_existing_path() {
    todo!("U25: manifest is {{ format: 2, name }}, objects/ and scenes/ exist, the starter palette has 48 colors, `exists` code on a second run, --name");
}

#[test]
#[ignore = "needs U25"]
fn r4_document_copy_saves_the_whole_document_elsewhere_save_as_and_refuses_an_existing_path() {
    todo!("U25: copy to <base>/copy.pixellab; objects/pebble.json is identical; the copy opens with name `sample` and publish_dir next to the copy; second copy exits 1 `already exists`; copy into itself exits 2");
}

// a site checkout (R3, legacy)

#[test]
#[ignore = "needs U8"]
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
#[ignore = "needs U25"]
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
fn a_file_that_cant_be_read_is_a_load_error_as_the_compiler_reports_it() {
    let doc = make_document();
    doc.write("manifest.json", r#"{ "format": 2 }"#);
    doc.write("objects/zz-broken.json", "{");
    let opened = open_document(&opts(Some(&doc.root), None, doc.env())).unwrap();
    let docs = opened.load_docs();
    assert_eq!(docs.load_errors.len(), 1);
    assert!(docs.load_errors[0].starts_with("objects/zz-broken.json: cannot be loaded: "), "{:?}", docs.load_errors);
    assert!(!docs.objects.contains_key("zz-broken"));
}

#[test]
#[ignore = "needs U14"]
fn an_overlay_replaces_adds_and_removes_docs_for_a_session() {
    todo!("U14: a context over the open document with an overlay: draft scene added, footer-dirt removed, not_found for it");
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
