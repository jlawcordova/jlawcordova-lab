//! The scene commands (ports the scene parts of cli/test/edit-commands.test.mjs):
//! scene.place, move, set, duplicate, snap, remove, the override, new and the
//! read commands, called through the registry on a session over a temp copy of
//! the sample document.
//!
//! The Node suite ran every command as a one-shot command line with
//! --dry-run and a real run, on a shared site. Here each test has its own
//! document and checks the session's working copy; the parts that go through
//! the CLI door (the dry-run diff, `wrote …`, exit codes) are written as
//! ignored tests until the CLI (U21) exists, and so is object.new (U24).

mod common;

use pixellab::context::{Ctx, DocKind};
use pixellab::document::{open_document, OpenOptions};
use pixellab::errors::{CommandError, Error};
use pixellab::registry::registry;
use pixellab::session::{Mode, Options, Session};
use pixellab::sorting::sort_scene;
use serde_json::{json, Value};

fn sprite() -> Value {
    json!({
        "kind": "sprite",
        "anchor": [2, 3],
        "keys": { "a": "grass-4", "b": "grass-2", "c": "ink" },
        "layers": [{ "map": [".bb.", "baab", "caac", ".cc."] }],
    })
}

fn zz_test() -> Value {
    json!({
        "viewBox": [-64, -64, 128, 128],
        "origin": [0, 0],
        "items": [
            { "object": "pine", "at": { "tile": [0, 0, 0] } },
            { "object": "pine", "at": { "tile": [1, 0, 0] } },
            { "object": "pine", "at": { "px": [10, 10] } },
            { "object": "house", "id": "home", "at": { "tile": [2, 2, 0] } },
        ],
        "motion": { "duration": 1000, "tracks": [{ "item": "home", "keys": [{ "t": 0 }, { "t": 500, "move": [2, 1] }] }] },
    })
}

/// A session over a temp copy of the sample document with the pine, the house and
/// the tree sprites, and the scene zz-test.
struct Lab {
    session: Session,
    _doc: common::TempDocument,
}

fn lab() -> Lab {
    let doc = common::make_document();
    for name in ["pine", "house", "tree"] {
        doc.write(&format!("objects/{name}.json"), &engine::format::stringify_pretty(&sprite()));
    }
    doc.write("scenes/zz-test.json", &engine::format::stringify_pretty(&zz_test()));
    let state = common::temp_dir("pixellab-state-");
    let mut env = doc.env();
    env.insert("PIXELLAB_STATE".into(), state.to_string_lossy().into_owned());
    let document = open_document(&OpenOptions {
        doc: Some(doc.root.to_string_lossy().into_owned()),
        site: None,
        env: env.clone(),
        cwd: doc.root.clone(),
    })
    .unwrap();
    Lab { session: Session::new(document, Options { mode: Mode::Direct, env, ..Options::default() }), _doc: doc }
}

impl Lab {
    /// `registry.call(ctx, name, { name: 'zz-test', ...params })`.
    fn call(&mut self, name: &str, params: Value) -> Result<Value, CommandError> {
        let mut merged = serde_json::Map::new();
        merged.insert("name".into(), json!("zz-test"));
        merged.extend(params.as_object().unwrap().clone());
        registry().call(&mut Ctx::new(&mut self.session), name, Value::Object(merged), false).map_err(|e| match e {
            Error::Command(c) => c,
            Error::Internal(m) => CommandError::new(m, "internal"),
        })
    }

    fn ok(&mut self, name: &str, params: Value) -> Value {
        self.call(name, params).unwrap_or_else(|e| panic!("{name}: {}", e.message))
    }

    /// The working copy of the scene.
    fn doc(&mut self) -> Value {
        Ctx::new(&mut self.session).doc(DocKind::Scene, "zz-test").unwrap()
    }

    fn items(&mut self) -> Vec<Value> {
        self.doc()["items"].as_array().unwrap().clone()
    }

    /// The index of the first item that `matches`, as a path string.
    fn index_of(&mut self, matches: impl Fn(&Value) -> bool) -> String {
        self.items().iter().position(matches).map_or("-1".to_string(), |i| i.to_string())
    }

    /// `sceneIsSorted`: the sort has nothing to move.
    fn is_sorted(&mut self) -> bool {
        let mut doc = self.doc();
        let loaded = Ctx::new(&mut self.session).sources().unwrap();
        sort_scene(&loaded.sources, &mut doc).moved.is_empty()
    }
}

fn tile_is(tile: &'static str) -> impl Fn(&Value) -> bool {
    move |item| join(&item["at"]["tile"]) == tile
}

fn px_is(px: &'static str) -> impl Fn(&Value) -> bool {
    move |item| join(&item["at"]["px"]) == px
}

fn join(v: &Value) -> String {
    v.as_array().map(|a| a.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(",")).unwrap_or_default()
}

#[test]
fn scene_place() {
    let mut lab = lab();
    let r = lab.ok("scene.place", json!({ "object": "pine", "at": [0, 1, 0], "class": "sway" }));
    // It goes where it stands, not where it was added: the edit leaves the scene in depth order.
    let items = lab.items();
    let swayed = items.iter().find(|i| i["class"] == "sway").unwrap();
    assert_eq!(swayed, &json!({ "object": "pine", "class": "sway", "at": { "tile": [0, 1, 0] } }));
    assert!(lab.is_sorted());
    assert_eq!(r["doc"], "scene:zz-test");
    let r = lab.ok("scene.place", json!({ "object": "tree", "px": [3, 4], "id": "tall" }));
    // The path it returns is the one after the sort.
    let at: usize = r["path"].as_str().unwrap().parse().unwrap();
    assert_eq!(lab.items()[at], json!({ "object": "tree", "id": "tall", "at": { "px": [3, 4] } }));
    assert!(lab.is_sorted());
    // With no --at or --px: the level-0 tile at the middle of the view, where the lab's cursor starts.
    let middle = lab.ok("scene.place", json!({ "object": "pine" }));
    assert_eq!(middle["item"]["at"], json!({ "tile": [0, 0, 0] }));
    let bad = lab.call("scene.place", json!({ "object": "no-such-thing" })).unwrap_err();
    assert_eq!(bad.exit_code, 1);
    assert!(bad.message.contains("no object named no-such-thing"), "{}", bad.message);
}

#[test]
#[ignore = "needs U21"]
fn scene_place_cli_door() {
    // pixellab scene place zz-test pine --at 0,1,0 --class sway --dry-run prints
    // "--- a/scenes/zz-test.json\n+++ b/scenes/zz-test.json\n@@ " and "dry run: nothing written" and writes nothing;
    // the real run prints "wrote scenes/zz-test.json"; with --json, saved is
    // [{ doc: 'scene:zz-test', file, wrote: true, hash }]; a --dry-run --json has dryRun[0].file;
    // an unknown object exits 1 with "no object named no-such-thing".
    unreachable!("needs the CLI (U21)");
}

#[test]
fn scene_move_by_tiles_by_pixels_and_a_key_past_time_0() {
    let mut lab = lab();
    let first = lab.index_of(tile_is("0,0,0"));
    lab.ok("scene.move", json!({ "item": first, "by": [0, 2, 1] }));
    assert!(lab.items().iter().any(tile_is("0,2,1")), "it moved, to wherever it now sorts");
    let px = lab.index_of(px_is("10,10"));
    lab.ok("scene.move", json!({ "item": px, "nudge": [-1, 2] }));
    assert!(lab.items().iter().any(px_is("9,12")));
    lab.ok("scene.move", json!({ "item": "home", "by": [1, 0], "time": 500 }));
    assert_eq!(lab.doc()["motion"]["tracks"][0]["keys"][1], json!({ "t": 500, "move": [18, 9] }));
    assert!(lab.is_sorted());
}

#[test]
fn scene_set_tile_px_a_field_class_and_id() {
    let mut lab = lab();
    let second = lab.index_of(tile_is("1,0,0"));
    lab.ok("scene.set", json!({ "item": second, "tile": [4, 4, 0] }));
    assert!(lab.items().iter().any(tile_is("4,4,0")));
    let at = lab.index_of(tile_is("4,4,0"));
    lab.ok("scene.set", json!({ "item": at, "level": 2 }));
    assert!(lab.items().iter().any(tile_is("4,4,2")));
    let at = lab.index_of(tile_is("4,4,2"));
    lab.ok("scene.set", json!({ "item": at, "px": [7, 8] }));
    assert!(lab.items().iter().any(px_is("7,8")));
    lab.ok("scene.set", json!({ "item": "home", "id": "cabin", "class": "warm glow" }));
    let doc = lab.doc();
    let cabin = doc["items"].as_array().unwrap().iter().find(|i| i["id"] == "cabin").unwrap();
    assert_eq!(cabin.as_object().unwrap().keys().collect::<Vec<_>>(), ["object", "id", "class", "at"]);
    assert_eq!(doc["motion"]["tracks"][0]["item"], "cabin");
    let px = lab.index_of(px_is("7,8"));
    let wrong = lab.call("scene.set", json!({ "item": px, "col": 1 })).unwrap_err();
    assert_eq!(wrong.exit_code, 1);
    assert!(wrong.message.contains("placed by pixel"), "{}", wrong.message);
}

#[test]
fn scene_duplicate_scene_snap_scene_remove() {
    let mut lab = lab();
    lab.ok("scene.set", json!({ "item": "home", "id": "cabin" }));
    let second = lab.index_of(tile_is("1,0,0"));
    lab.ok("scene.set", json!({ "item": second, "px": [7, 8] }));
    lab.ok("scene.duplicate", json!({ "item": "cabin" }));
    // The copy has no id and stands one column over; it is listed where it sorts.
    let items = lab.items();
    let cabin = items.iter().find(|i| i["id"] == "cabin").unwrap();
    let copy = items.iter().find(|i| i["object"] == "house" && i.get("id").is_none()).unwrap();
    let t = &cabin["at"]["tile"];
    assert_eq!(copy["at"]["tile"], json!([t[0].as_i64().unwrap() + 1, t[1], t[2]]));
    assert!(lab.is_sorted());
    let px = lab.index_of(px_is("7,8"));
    lab.ok("scene.snap", json!({ "item": px }));
    assert!(!lab.items().iter().any(px_is("7,8")), "it snapped to a tile");
    lab.ok("scene.remove", json!({ "item": "cabin" }));
    let doc = lab.doc();
    assert!(!doc["items"].as_array().unwrap().iter().any(|i| i["id"] == "cabin"));
    assert_eq!(doc["motion"]["tracks"], json!([]));
}

#[test]
fn r6_scene_raise_and_scene_lower_are_gone_the_override_is_scene_set_depth() {
    let mut lab = lab();
    for verb in ["raise", "lower"] {
        let err = lab.call(&format!("scene.{verb}"), json!({ "item": "0" })).unwrap_err();
        assert!(err.message.contains(&format!("no command scene.{verb}")), "{}", err.message);
    }
    let first = lab.index_of(|_| true);
    lab.ok("scene.set", json!({ "item": first, "depth": "front" }));
    let items = lab.items();
    let last = items.last().unwrap();
    assert_eq!(last["depth"], "front", "a front item is the last of its list");
    let order = ["object", "id", "class", "layer", "depth", "at"];
    let keys: Vec<&String> = last.as_object().unwrap().keys().collect();
    let mut sorted = keys.clone();
    sorted.sort_by_key(|k| order.iter().position(|o| o == k));
    assert_eq!(keys, sorted);
}

#[test]
#[ignore = "needs U21"]
fn r6_scene_raise_and_scene_lower_are_gone_cli_door() {
    // pixellab scene raise zz-test 0: exit 2, stderr "unknown command scene raise" (and lower).
    unreachable!("needs the CLI (U21)");
}

#[test]
fn scene_new() {
    let mut lab = lab();
    let made = lab.call("scene.new", json!({ "name": "zz-made" })).unwrap();
    assert_eq!(made["file"], "scenes/zz-made.json");
    assert_eq!(made["changed"], true);
    let doc = Ctx::new(&mut lab.session).doc(DocKind::Scene, "zz-made").unwrap();
    assert_eq!(doc, json!({ "viewBox": [-64, -64, 128, 128], "origin": [0, 0], "items": [] }));
    let again = lab.call("scene.new", json!({ "name": "zz-made" })).unwrap_err();
    assert_eq!(again.exit_code, 1);
    assert!(again.message.contains("already an object or scene called zz-made"), "{}", again.message);
    assert_eq!(lab.call("scene.new", json!({ "name": "Bad_Name" })).unwrap_err().exit_code, 2);
}

#[test]
#[ignore = "needs U21"]
fn scene_new_and_object_new_cli_door() {
    // scene new zz-made --dry-run prints "+++ b/scenes/zz-made.json\n@@ -0,0 +1,… @@" and writes nothing;
    // object new zz-rock --width 5 --height 3 has anchor [2, 2] and a map of three '.....' rows;
    // object new zz-cube --kind block is a block (U24).
    unreachable!("needs the CLI (U21) and object.new (U24)");
}

#[test]
fn scene_list_scene_show_and_scene_items_read_the_working_copy() {
    let mut lab = lab();
    let list = registry().call(&mut Ctx::new(&mut lab.session), "scene.list", json!({}), false).unwrap();
    let zz = list["scenes"].as_array().unwrap().iter().find(|s| s["name"] == "zz-test").unwrap().clone();
    assert_eq!(zz["items"], 4);
    assert_eq!(zz["motion"], true);
    assert_eq!(zz["file"], "scenes/zz-test.json");
    let show = lab.ok("scene.show", json!({}));
    assert_eq!(show["objects"], json!({ "pine": 3, "house": 1 }));
    assert_eq!(show["motion"], json!({ "duration": 1000, "tracks": [{ "item": "home", "keys": 2 }] }));
    let items = lab.ok("scene.items", json!({}));
    assert_eq!(items["scene"], "zz-test");
    assert_eq!(items["items"].as_array().unwrap().len(), 4);
}
