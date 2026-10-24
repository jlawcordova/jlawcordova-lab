//! Poses become states (docs/intents/2026-10-object-states/spec.md, R37 to R40, D8), through the
//! registry on a temp copy of fixtures/poses.pixellab: a hook that poses as a dropped and a low
//! hook, and a crossing signal that alternates its lamps. Nothing here writes to the fixture.

#![allow(clippy::result_large_err)]

mod common;

use std::fs;
use std::path::PathBuf;

use pixellab::context::{Ctx, DocKind};
use pixellab::document::{open_document, OpenOptions};
use pixellab::errors::Error;
use pixellab::poses;
use pixellab::registry::registry;
use pixellab::session::{Options, Session};
use serde_json::{json, Value};

/// A temp copy of the poses fixture, with a state folder and an empty HOME.
struct Fx {
    base: PathBuf,
    root: PathBuf,
    home: PathBuf,
    state: PathBuf,
}

impl Fx {
    fn new() -> Fx {
        let base = common::temp_dir("pixellab-poses-");
        let (root, home, state) = (base.join("poses.pixellab"), base.join("home"), base.join("state"));
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&state).unwrap();
        common::copy_dir(&common::repo().join("fixtures/poses.pixellab"), &root);
        Fx { base, root, home, state }
    }

    fn write(&self, file: &str, value: &Value) {
        common::write(&self.root, file, &engine::format::serialize_file(file, value));
    }

    fn read(&self, file: &str) -> Value {
        serde_json::from_str(&fs::read_to_string(self.root.join(file)).unwrap()).unwrap()
    }

    fn exists(&self, file: &str) -> bool {
        self.root.join(file).exists()
    }

    fn session(&self) -> Session {
        let mut env = std::collections::HashMap::from([("HOME".to_string(), self.home.to_string_lossy().into_owned())]);
        env.insert("PIXELLAB_STATE".into(), self.state.to_string_lossy().into_owned());
        let document = open_document(&OpenOptions {
            doc: Some(self.root.to_string_lossy().into_owned()),
            site: None,
            env: env.clone(),
            cwd: self.root.clone(),
        })
        .unwrap();
        Session::new(document, Options { env, agent: Some("test".into()), ..Options::default() })
    }
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn call(s: &mut Session, name: &str, params: Value) -> Result<Value, Error> {
    let mut ctx = Ctx::new(s);
    registry().call(&mut ctx, name, params, false)
}

fn ok(s: &mut Session, name: &str, params: Value) -> Value {
    call(s, name, params).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn refused(r: Result<Value, Error>) -> (String, String) {
    match r.expect_err("a refusal") {
        Error::Command(c) => (c.code, c.message),
        Error::Internal(m) => ("internal".into(), m),
    }
}

fn problems(s: &mut Session) -> Vec<String> {
    let r = ok(s, "check", json!({}));
    r["problems"].as_array().unwrap().iter().map(|p| p["text"].as_str().unwrap().to_string()).collect()
}

/// The scene's PNG at every 100 ms of its 3 s loop, and at the fixture's odd key times.
fn renders(s: &mut Session) -> Vec<String> {
    let times = (0..30).map(|i| i * 100).chain([2050, 2150]);
    times.map(|t| ok(s, "scene.render", json!({ "name": "yard", "time": t }))["png"].as_str().unwrap().to_string()).collect()
}

fn keys_of(fx: &Fx, item: &str) -> Vec<Value> {
    let scene = fx.read("scenes/yard.json");
    let track = scene["motion"]["tracks"].as_array().unwrap().iter().find(|t| t["item"] == item).unwrap().clone();
    track["keys"].as_array().unwrap().clone()
}

// ---------------------------------------------------------------------------- R37

#[test]
fn r37_a_pose_family_becomes_one_object_with_a_frame_and_a_state_per_pose() {
    let fx = Fx::new();
    let mut s = fx.session();
    let r = ok(&mut s, "object.states.from-poses", json!({ "name": "hook" }));
    assert_eq!(r["members"], json!(["hook", "hook-drop", "hook-low"]), "the base first, then the poses as first posed: {r}");
    assert_eq!(r["states"], json!(["rest", "drop", "low"]), "named without the base's name");
    assert_eq!(r["start"], "rest");
    let hook = fx.read("objects/hook.json");
    assert_eq!(hook["start"], "rest");
    assert_eq!(
        hook["states"],
        json!({ "rest": { "frames": [0], "ms": 200 }, "drop": { "frames": [1], "ms": 200 }, "low": { "frames": [2], "ms": 200 } })
    );
    // hook-low is 7 wide around an anchor at 3: the union box, with the anchor where it puts it.
    assert_eq!(hook["anchor"], json!([3, 0]));
    let layers = hook["layers"].as_array().unwrap();
    assert_eq!(layers.len(), 1, "one frame loop");
    assert_eq!((layers[0]["loop"].as_str(), layers[0]["prefix"].as_str()), (Some("pf"), Some("p")));
    let frames = layers[0]["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 3);
    assert_eq!(frames[0][0], "...W...", "the base's own drawing is frame 0, padded to the box");
    // hook-drop's two layers are flattened into one drawing.
    assert_eq!(frames[1][3], "...W...");
    assert_eq!(frames[1][4], "..dgd..");
    // hook-low's W is soil-3, which the base's W isn't: it gets a key of its own.
    assert_eq!(hook["keys"]["W"], "wood-3");
    let renamed = hook["keys"].as_object().unwrap().iter().find(|(_, c)| *c == "soil-3").map(|(k, _)| k.clone()).unwrap();
    assert_ne!(renamed, "W");
    assert_eq!(frames[2][0], format!("...{renamed}..."));
    assert_eq!(problems(&mut s), Vec::<String>::new());
}

#[test]
fn r37_the_merge_is_refused_with_the_reason_when_it_would_break_a_rule() {
    let cases: Vec<(&str, Box<dyn Fn(&Fx)>, &str)> = vec![
        (
            "a pose that is itself a frame loop",
            Box::new(|fx: &Fx| {
                let mut o = fx.read("objects/hook-low.json");
                let map = o["layers"][0]["map"].clone();
                o["layers"][0] = json!({ "loop": "ff", "prefix": "f", "frames": [map.clone(), map] });
                fx.write("objects/hook-low.json", &o);
            }),
            "frame loop",
        ),
        (
            "more than 12 colors",
            Box::new(|fx: &Fx| {
                let mut o = fx.read("objects/hook-low.json");
                let colors =
                    ["ink", "cream", "grass-1", "grass-2", "grass-3", "grass-4", "soil-2", "water-2", "gold-2", "roof-1", "roof-3"];
                let keys: serde_json::Map<String, Value> =
                    colors.iter().enumerate().map(|(i, c)| (((b'A' + i as u8) as char).to_string(), json!(c))).collect();
                let row: String = (0..colors.len()).map(|i| (b'A' + i as u8) as char).collect();
                for (k, v) in keys {
                    o["keys"][k] = v;
                }
                o["layers"][0]["map"][7] = json!(format!("{}", &row[..7]));
                o["layers"][0]["map"][5] = json!(format!("{}", &row[4..]));
                fx.write("objects/hook-low.json", &o);
            }),
            "colors",
        ),
        (
            "wider than 64",
            Box::new(|fx: &Fx| {
                let mut o = fx.read("objects/hook-low.json");
                o["anchor"] = json!([62, 0]);
                o["layers"][0]["map"] = json!((0..8).map(|_| format!("{}d", ".".repeat(62))).collect::<Vec<_>>());
                fx.write("objects/hook-low.json", &o);
            }),
            "past the 64×64",
        ),
        (
            "a layer with an ambient loop",
            Box::new(|fx: &Fx| {
                let mut o = fx.read("objects/hook-drop.json");
                o["layers"][1]["ambient"] =
                    json!({ "duration": 1000, "keys": [{ "t": 0, "move": [0, 0] }, { "t": 500, "move": [0, -1] }] });
                fx.write("objects/hook-drop.json", &o);
            }),
            "ambient loop",
        ),
        (
            "a base that already has states",
            Box::new(|fx: &Fx| {
                let mut o = fx.read("objects/hook.json");
                o["start"] = json!("idle");
                o["states"] = json!({ "idle": { "frames": [0], "ms": 200 } });
                fx.write("objects/hook.json", &o);
            }),
            "already has states",
        ),
    ];
    for (what, change, says) in cases {
        let fx = Fx::new();
        change(&fx);
        let before = fx.read("objects/hook.json");
        let mut s = fx.session();
        let (code, message) = refused(call(&mut s, "object.states.from-poses", json!({ "name": "hook" })));
        assert!(message.contains(says), "{what}: {code}: {message}");
        assert_eq!(fx.read("objects/hook.json"), before, "{what}: nothing changed");
        assert!(keys_of(&fx, "hook").iter().any(|k| k.get("pose").is_some()), "{what}: the keys still pose");
    }
    // A base that extends another can't have states of its own, and an object nothing poses has no family.
    let fx = Fx::new();
    let mut s = fx.session();
    let (_, message) = refused(call(&mut s, "object.states.from-poses", json!({ "name": "signal-left" })));
    assert!(message.contains("extends another"), "{message}");
    let (_, message) = refused(call(&mut s, "object.states.from-poses", json!({ "name": "cart" })));
    assert!(message.contains("no pose family"), "{message}");
}

// ---------------------------------------------------------------------------- R38

#[test]
fn r38_the_scene_keys_follow_and_every_render_stays_the_same() {
    let fx = Fx::new();
    let mut s = fx.session();
    let before = renders(&mut s);
    let r = ok(&mut s, "object.states.from-poses", json!({ "name": "hook" }));
    assert_eq!(r["scenes"], json!([{ "scene": "yard", "keys": 4, "folded": 0 }]), "{r}");
    let keys = keys_of(&fx, "hook");
    assert_eq!(
        keys,
        vec![
            json!({ "t": 0 }),
            json!({ "t": 500, "state": "drop" }),
            json!({ "t": 900, "move": [16, 8] }),
            json!({ "t": 1300, "state": "low" }),
            json!({ "t": 1700, "state": "drop" }),
            json!({ "t": 2100, "state": "rest" }),
            json!({ "t": 2500, "move": [0, 0] }),
        ],
        "each pose is its state, and posing back as the base is the start state; the moves are as they were"
    );
    // The signal family is folded next, the cart rolls past both, and every frame matches.
    ok(&mut s, "object.states.from-poses", json!({ "name": "signal", "fold": true }));
    assert_eq!(renders(&mut s), before, "pixel-identical at every 100 ms");
    assert_eq!(problems(&mut s), Vec::<String>::new());
}

#[test]
fn r38_the_check_finds_a_frame_that_would_differ() {
    let fx = Fx::new();
    // The plan as the command makes it, then one pixel of the dropped hook's frame changed: the check names when it shows.
    let load = |fx: &Fx| {
        let read = |dir: &str| -> std::collections::BTreeMap<String, Value> {
            fs::read_dir(fx.root.join(dir))
                .unwrap()
                .map(|e| e.unwrap().path())
                .map(|p| {
                    (p.file_stem().unwrap().to_string_lossy().into_owned(), serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap())
                })
                .collect()
        };
        engine::engine::load_sources(fx.read("palette.json"), read("objects"), read("scenes"))
    };
    let before = load(&fx);
    let plan = poses::plan(&before, "hook", &poses::Options::default()).unwrap();
    let after = poses::applied(&before, &plan);
    assert_eq!(poses::differences(&before, &after, &plan, "yard", 50).unwrap(), Vec::<f64>::new());
    let mut tampered = plan.clone();
    tampered.base["layers"][0]["frames"][1][0] = json!("...d...");
    let after = poses::applied(&before, &tampered);
    let times = poses::differences(&before, &after, &tampered, "yard", 50).unwrap();
    assert!(!times.is_empty() && times.iter().all(|t| (500.0..1300.0).contains(t) || (1700.0..2100.0).contains(t)), "{times:?}");
}

// ---------------------------------------------------------------------------- R39

#[test]
fn r39_an_alternating_run_of_poses_becomes_one_key_into_a_looping_state() {
    let fx = Fx::new();
    let mut s = fx.session();
    let r = ok(&mut s, "object.states.from-poses", json!({ "name": "signal", "fold": true, "as": "flash" }));
    assert_eq!(r["states"], json!(["rest", "left", "right", "flash"]), "{r}");
    assert_eq!(r["folded"], json!([{ "state": "flash", "frames": [1, 2], "ms": 200 }]));
    assert_eq!(r["scenes"], json!([{ "scene": "yard", "keys": 7, "folded": 1 }]));
    // The signal's 8 keys are 3: the rest frame, flash from the first lit lamp, and rest when it goes dark.
    assert_eq!(keys_of(&fx, "sig"), vec![json!({ "t": 0 }), json!({ "t": 1000, "state": "flash" }), json!({ "t": 2100, "state": "rest" })]);
    assert_eq!(fx.read("objects/signal.json")["states"]["flash"], json!({ "frames": [1, 2], "ms": 200 }));
}

#[test]
fn r39_the_folded_state_is_named_after_the_runs_first_state_unless_named() {
    let fx = Fx::new();
    let mut s = fx.session();
    ok(&mut s, "object.states.from-poses", json!({ "name": "signal", "fold": true }));
    assert_eq!(keys_of(&fx, "sig")[1], json!({ "t": 1000, "state": "left-loop" }));
    // A name already taken is refused, and so is a name with nothing to fold.
    let fx = Fx::new();
    let mut s = fx.session();
    let (_, message) = refused(call(&mut s, "object.states.from-poses", json!({ "name": "signal", "fold": true, "as": "left" })));
    assert!(message.contains("already has a state left"), "{message}");
    let (_, message) = refused(call(&mut s, "object.states.from-poses", json!({ "name": "hook", "fold": true, "as": "swing" })));
    assert!(message.contains("no run of keys"), "{message}");
}

// ---------------------------------------------------------------------------- R40

#[test]
fn r40_the_pose_objects_stay_until_nothing_uses_them_and_it_is_all_one_version() {
    let fx = Fx::new();
    let mut s = fx.session();
    let before = renders(&mut s);
    let r = ok(&mut s, "object.states.from-poses", json!({ "name": "hook" }));
    assert_eq!(r["unused"], json!(["hook-drop", "hook-low"]));
    assert_eq!(r["deleted"], json!([]));
    assert!(fx.exists("objects/hook-drop.json") && fx.exists("objects/hook-low.json"), "they stay");
    // signal-right is placed on its own too, so only signal-left goes; both extended signal, so what's left stands alone.
    let versions = ok(&mut s, "history.list", json!({}))["total"].as_u64().unwrap();
    let r = ok(&mut s, "object.states.from-poses", json!({ "name": "signal", "fold": true, "deleteUnused": true }));
    assert_eq!(r["unused"], json!(["signal-left"]), "{r}");
    assert_eq!(r["deleted"], json!(["signal-left"]));
    assert_eq!(r["detached"], json!(["signal-left", "signal-right"]));
    assert!(!fx.exists("objects/signal-left.json"));
    let right = fx.read("objects/signal-right.json");
    assert!(right.get("extends").is_none() && right.get("states").is_none(), "{right}");
    let list = ok(&mut s, "history.list", json!({}));
    assert_eq!(list["total"].as_u64().unwrap(), versions + 1, "one version");
    let files: Vec<&str> = list["versions"][0]["files"].as_array().unwrap().iter().map(|f| f.as_str().unwrap()).collect();
    for f in ["objects/signal.json", "objects/signal-left.json", "objects/signal-right.json", "scenes/yard.json"] {
        assert!(files.contains(&f), "{f} in {files:?}");
    }
    assert_eq!(renders(&mut s), before);
    // Taking the version back puts the poses back.
    let number = list["versions"][0]["number"].clone();
    ok(&mut s, "history.undo", json!({ "version": number }));
    assert!(fx.exists("objects/signal-left.json"));
    assert!(fx.read("objects/signal.json").get("states").is_none());
    assert!(keys_of(&fx, "sig").iter().any(|k| k["pose"] == "signal-left"));
}

#[test]
fn d8_a_dry_run_says_what_it_would_do_and_writes_nothing() {
    let fx = Fx::new();
    let mut s = fx.session();
    let r = ok(&mut s, "object.states.from-poses", json!({ "name": "hook", "dryRun": true }));
    assert_eq!(r["dryRun"], true);
    assert_eq!(r["states"], json!(["rest", "drop", "low"]));
    assert!(fx.read("objects/hook.json").get("states").is_none());
    assert!(s.doc(DocKind::Object, "hook").unwrap().get("states").is_none());
}

#[test]
fn d8_the_command_is_in_all_three_doors() {
    let reg = registry();
    let c = reg.commands.iter().find(|c| c.name == "object.states.from-poses").expect("registered");
    assert!(c.mutates && c.writes_direct);
    assert_eq!(pixellab::server_mcp::tool_name(c.name), "object_states_from_poses");
    assert_eq!(pixellab::argv::command_words(c.name), "object states from-poses");
}
