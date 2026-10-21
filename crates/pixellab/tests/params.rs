//! The shared param checker and the command-line parser (ports cli/test/params.test.mjs).

use pixellab::argv::{parse_command_line, Globals, Parsed};
use pixellab::context::Ctx;
use pixellab::errors::{Error, Result};
use pixellab::params::check_params;
use pixellab::registry::{registry, Command, Params, Registry};
use serde_json::{json, Map, Value};

fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "scale": { "type": "integer", "minimum": 1, "maximum": 16, "default": 1 },
            "ratio": { "type": "number" },
            "flag": { "type": "boolean", "default": false },
            "at": { "type": "array", "items": { "type": "integer" }, "minItems": 3, "maxItems": 3 },
            "kind": { "type": "string", "enum": ["sprite", "block"] },
            "data": { "type": "object" },
            "path": { "type": "string", "pattern": "^\\d+(\\.\\d+)*$" },
        },
        "required": ["name"],
    })
}

fn check(input: Value) -> std::result::Result<Map<String, Value>, pixellab::errors::CommandError> {
    check_params(&schema(), &input, false, None)
}

fn check_coerce(input: Value) -> std::result::Result<Map<String, Value>, pixellab::errors::CommandError> {
    check_params(&schema(), &input, true, None)
}

fn obj(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

#[test]
fn fills_defaults_and_keeps_valid_values() {
    assert_eq!(check(json!({"name": "a"})).unwrap(), obj(json!({"name": "a", "scale": 1, "flag": false})));
    assert_eq!(
        check(json!({"name": "a", "scale": 3, "at": [1, 2, 0], "kind": "block"})).unwrap(),
        obj(json!({"name": "a", "scale": 3, "flag": false, "at": [1, 2, 0], "kind": "block"}))
    );
}

#[test]
fn coerces_command_line_strings_to_the_schema_types() {
    let out = check_coerce(
        json!({"name": "a", "scale": "4", "ratio": "0.5", "flag": "true", "at": "14,6,1", "data": "{\"x\":1}", "path": "5.2"}),
    )
    .unwrap();
    assert_eq!(out, obj(json!({"name": "a", "scale": 4, "ratio": 0.5, "flag": true, "at": [14, 6, 1], "data": {"x": 1}, "path": "5.2"})));
    assert_eq!(check_coerce(json!({"name": "a", "at": ["14", "6", "-1"]})).unwrap()["at"], json!([14, 6, -1]));
    assert_eq!(check_coerce(json!({"name": "a", "flag": "no"})).unwrap()["flag"], json!(false));
}

#[test]
fn without_coerce_strings_are_not_numbers() {
    let err = check(json!({"name": "a", "scale": "4"})).unwrap_err();
    assert!(err.message.contains("scale must be a whole number, got \"4\""), "{}", err.message);
}

#[test]
fn reports_every_problem_at_once_as_a_usage_error() {
    let err = check_params(&schema(), &json!({"scale": 0, "kind": "blob", "at": [1, 2], "sacle": 2, "path": "x"}), false, Some("demo"))
        .unwrap_err();
    assert!(err.is_usage());
    assert_eq!(err.exit_code, 2);
    assert!(err.message.starts_with("demo: "), "{}", err.message);
    for part in [
        "unknown param sacle (did you mean scale?)",
        "name is required",
        "scale must be at least 1",
        "kind must be one of sprite, block",
        "at needs at least 3 items",
        "path \"x\" doesn't match",
    ] {
        assert!(err.message.contains(part), "{} should include {part}", err.message);
    }
    let details = err.details.unwrap();
    assert_eq!(details.as_array().unwrap().len(), 6);
}

#[test]
fn rejects_non_integers_non_finite_numbers_and_bad_booleans() {
    let msg = |input: Value| check_coerce(input).unwrap_err().message;
    assert!(msg(json!({"name": "a", "scale": "2.5"})).contains("scale must be a whole number"));
    assert!(msg(json!({"name": "a", "ratio": "abc"})).contains("ratio must be a number"));
    assert!(msg(json!({"name": "a", "flag": "maybe"})).contains("flag must be true or false"));
    assert!(msg(json!({"name": "a", "data": "{bad"})).contains("data must be an object"));
}

// ---------------------------------------------------------------- parseCommandLine

fn echo(_: &mut Ctx, params: Params) -> Result<Value> {
    Ok(Value::Object(params))
}

/// A few commands shaped like the real scene.*, check ones, for tests that
/// don't need the ported commands.
fn mini() -> Registry {
    let scene_name = json!({"type": "string"});
    let mut list = vec![
        Command::new("scene.list", "list", json!({"type": "object", "properties": {}}), false, echo),
        Command::new(
            "scene.show",
            "show",
            json!({"type": "object", "properties": {"name": scene_name, "doc": {"type": "boolean"}}}),
            false,
            echo,
        ),
        Command::new(
            "scene.render",
            "render",
            json!({"type": "object", "properties": {"name": {"type": "string"}, "scale": {"type": "integer"}, "time": {"type": "integer"}}}),
            false,
            echo,
        ),
        Command::new("check", "check", json!({"type": "object", "properties": {"name": {"type": "string"}}}), false, echo),
    ];
    list[1].args = vec!["name"];
    list[2].args = vec!["name"];
    list[3].args = vec!["name"];
    Registry::new(list)
}

fn argv(line: &str) -> Vec<String> {
    line.split(' ').filter(|s| !s.is_empty()).map(String::from).collect()
}

fn command_of<'a>(parsed: &'a Parsed<'a>) -> (&'a str, &'a Map<String, Value>, &'a Globals) {
    match parsed {
        Parsed::Command { command, params, globals } => (command.name, params, globals),
        Parsed::Help { .. } => panic!("expected a command"),
    }
}

fn help_of<'a>(parsed: &'a Parsed<'a>) -> (&'a [String], bool) {
    match parsed {
        Parsed::Help { topic, usage, .. } => (topic, *usage),
        Parsed::Command { .. } => panic!("expected help"),
    }
}

fn parse_err(reg: &Registry, line: &str) -> String {
    match parse_command_line(&argv(line), reg) {
        Err(e) => {
            assert!(e.is_usage());
            e.message
        }
        Ok(_) => panic!("{line} should fail"),
    }
}

fn t_leading_words_name_the_command_positionals_fill_its_args(reg: &Registry) {
    let p = parse_command_line(&argv("scene render hero-island --scale 2"), reg).unwrap();
    let (name, params, _) = command_of(&p);
    assert_eq!(name, "scene.render");
    assert_eq!(params, &obj(json!({"name": "hero-island", "scale": "2"})));
}

fn t_a_single_word_command_takes_positionals(reg: &Registry) {
    let p = parse_command_line(&argv("check hero-island"), reg).unwrap();
    let (name, params, _) = command_of(&p);
    assert_eq!(name, "check");
    assert_eq!(params, &obj(json!({"name": "hero-island"})));
}

fn t_flags_value_equals_booleans_and_no_flag(reg: &Registry) {
    let a = parse_command_line(&argv("scene render --name=hero-island --time=400"), reg).unwrap();
    assert_eq!(command_of(&a).1, &obj(json!({"name": "hero-island", "time": "400"})));
    let b = parse_command_line(&argv("scene show hero-island --doc"), reg).unwrap();
    assert_eq!(command_of(&b).1, &obj(json!({"name": "hero-island", "doc": "true"})));
    let c = parse_command_line(&argv("scene show hero-island --no-doc"), reg).unwrap();
    assert_eq!(command_of(&c).1, &obj(json!({"name": "hero-island", "doc": "false"})));
}

fn t_global_flags_go_anywhere_and_are_not_params(reg: &Registry) {
    let r = parse_command_line(&argv("--json scene list --site /x"), reg).unwrap();
    let (name, params, globals) = command_of(&r);
    assert_eq!(name, "scene.list");
    assert_eq!(globals, &Globals { json: true, help: false, site: Some("/x".into()), ..Globals::default() });
    assert_eq!(params, &Map::new());
}

#[test]
fn lists_comma_separated_repeated_flags_or_the_rest_of_the_positionals() {
    let mut cmd = Command::new(
        "demo.place",
        "demo",
        json!({"type": "object", "properties": {
            "scene": {"type": "string"},
            "at": {"type": "array", "items": {"type": "integer"}},
            "tags": {"type": "array", "items": {"type": "string"}},
        }}),
        false,
        echo,
    );
    cmd.args = vec!["scene", "tags"];
    let reg = Registry::new(vec![cmd]);
    let r = parse_command_line(&argv("demo place island a b --at 14,6 --at 1"), &reg).unwrap();
    let (_, params, _) = command_of(&r);
    assert_eq!(params, &obj(json!({"scene": "island", "tags": ["a", "b"], "at": ["14,6", "1"]})));
    let checked = check_params(&reg.get("demo.place").unwrap().params, &Value::Object(params.clone()), true, None).unwrap();
    assert_eq!(checked, obj(json!({"scene": "island", "tags": ["a", "b"], "at": [14, 6, 1]})));
}

fn t_help_topics(reg: &Registry) {
    assert!(matches!(parse_command_line(&argv("help"), reg).unwrap(), Parsed::Help { .. }));
    assert!(help_of(&parse_command_line(&argv(""), reg).unwrap()).1);
    let g = parse_command_line(&argv("scene --help"), reg).unwrap();
    assert_eq!(help_of(&g).0, ["scene"]);
    let c = parse_command_line(&argv("scene place-nothing --help"), reg).unwrap();
    assert_eq!(help_of(&c).0, ["scene"]);
    let d = parse_command_line(&argv("scene render --help"), reg).unwrap();
    assert_eq!(help_of(&d).0, ["scene", "render"]);
    let e = parse_command_line(&argv("scene"), reg).unwrap();
    assert!(help_of(&e).1);
}

fn t_usage_errors(reg: &Registry) {
    assert!(parse_err(reg, "scene lsit").contains("unknown command scene lsit (did you mean scene list?)"));
    assert!(parse_err(reg, "scene frobnicate").contains("unknown command scene frobnicate"));
    assert!(parse_err(reg, "scnee list").contains("unknown command scnee (did you mean scene?)"));
    assert!(parse_err(reg, "scene render x --scael 2").contains("unknown flag --scael (did you mean --scale?)"));
    assert!(parse_err(reg, "scene render x y").contains("unexpected y"));
    assert!(parse_err(reg, "scene render x --name y").contains("name is given twice"));
    assert!(parse_err(reg, "scene render x --scale").contains("--scale needs a value"));
    assert!(parse_err(reg, "scene render x --site").contains("--site needs a value"));
    assert!(parse_err(reg, "scene render x -z").contains("unknown flag -z"));
}

#[test]
fn negative_numbers_are_values_not_flags() {
    let mut cmd = Command::new("demo.nudge", "demo", json!({"type": "object", "properties": {"dx": {"type": "integer"}}}), false, echo);
    cmd.args = vec!["dx"];
    let reg = Registry::new(vec![cmd]);
    let r = parse_command_line(&argv("demo nudge -3"), &reg).unwrap();
    assert_eq!(command_of(&r).1, &obj(json!({"dx": "-3"})));
}

#[test]
#[ignore = "needs the commands (U22–U26)"]
fn leading_words_name_the_command_positionals_fill_its_args() {
    t_leading_words_name_the_command_positionals_fill_its_args(&registry());
}
#[test]
fn leading_words_name_the_command_positionals_fill_its_args_with_a_test_registry() {
    t_leading_words_name_the_command_positionals_fill_its_args(&mini());
}
#[test]
#[ignore = "needs the commands (U22–U26)"]
fn a_single_word_command_takes_positionals_check_name() {
    t_a_single_word_command_takes_positionals(&registry());
}
#[test]
fn a_single_word_command_takes_positionals_check_name_with_a_test_registry() {
    t_a_single_word_command_takes_positionals(&mini());
}
#[test]
#[ignore = "needs the commands (U22–U26)"]
fn flags_flag_value_flag_value_booleans_and_no_flag() {
    t_flags_value_equals_booleans_and_no_flag(&registry());
}
#[test]
fn flags_flag_value_flag_value_booleans_and_no_flag_with_a_test_registry() {
    t_flags_value_equals_booleans_and_no_flag(&mini());
}
#[test]
#[ignore = "needs the commands (U22–U26)"]
fn global_flags_go_anywhere_and_are_not_params() {
    t_global_flags_go_anywhere_and_are_not_params(&registry());
}
#[test]
fn global_flags_go_anywhere_and_are_not_params_with_a_test_registry() {
    t_global_flags_go_anywhere_and_are_not_params(&mini());
}
#[test]
#[ignore = "needs the commands (U22–U26)"]
fn help_topics() {
    t_help_topics(&registry());
}
#[test]
fn help_topics_with_a_test_registry() {
    t_help_topics(&mini());
}
#[test]
#[ignore = "needs the commands (U22–U26)"]
fn usage_errors() {
    t_usage_errors(&registry());
}
#[test]
fn usage_errors_with_a_test_registry() {
    t_usage_errors(&mini());
}

// ---------------------------------------------------------------- registry

#[test]
#[ignore = "needs the commands (U22–U26)"]
fn every_command_is_well_formed_and_has_a_group() {
    let reg = registry();
    let names: Vec<&str> = reg.commands.iter().map(|c| c.name).collect();
    let read_only = [
        "activity.list",
        "art.preview",
        "art.sizes",
        "chat.ask",
        "chat.context",
        "check",
        "doc.close",
        "doc.open",
        "doc.status",
        "document.info",
        "object.list",
        "object.pick",
        "object.render",
        "object.show",
        "palette.get",
        "proposal.list",
        "proposal.render",
        "proposal.show",
        "scene.items",
        "scene.list",
        "scene.render",
        "scene.show",
    ];
    let mutating = [
        "doc.merge",
        "doc.redo",
        "doc.reload",
        "doc.revert",
        "doc.save",
        "doc.undo",
        "document.copy",
        "document.new",
        "object.animate",
        "object.erase",
        "object.fill",
        "object.frame.add",
        "object.frame.delete",
        "object.frame.duplicate",
        "object.frame.move",
        "object.footprint",
        "object.layer.add",
        "object.layer.delete",
        "object.layer.duplicate",
        "object.layer.move",
        "object.map.set",
        "object.new",
        "object.paint",
        "object.resize",
        "object.still",
        "proposal.apply",
        "proposal.reject",
        "scene.duplicate",
        "scene.group.set",
        "scene.key.delete",
        "scene.key.move",
        "scene.key.set",
        "scene.layer.assign",
        "scene.layer.move",
        "scene.layer.new",
        "scene.layer.rename",
        "scene.layer.ungroup",
        "scene.motion.path",
        "scene.motion.set",
        "scene.move",
        "scene.new",
        "scene.place",
        "scene.remove",
        "scene.set",
        "scene.snap",
        "scene.sort",
        "scene.track.add",
        "scene.track.remove",
    ];
    let mut expected: Vec<&str> = read_only.iter().chain(mutating.iter()).copied().chain(["art.build"]).collect();
    expected.sort();
    assert_eq!(names, expected);
    for name in read_only {
        assert!(!reg.get(name).unwrap().mutates, "{name}");
    }
    for name in mutating {
        assert!(reg.get(name).unwrap().mutates, "{name}");
    }
    let session: Vec<&str> = names.iter().copied().filter(|n| reg.get(n).unwrap().session).collect();
    assert_eq!(
        session,
        ["doc.close", "doc.merge", "doc.open", "doc.redo", "doc.reload", "doc.revert", "doc.save", "doc.status", "doc.undo"]
    );
    let writes: Vec<&str> = names.iter().copied().filter(|n| reg.get(n).unwrap().writes_direct).collect();
    assert_eq!(writes, ["art.build", "document.copy", "document.new", "proposal.apply", "proposal.reject"]);
    let groups: Vec<String> = reg.groups().into_iter().map(|(g, _)| g).collect();
    assert_eq!(groups, ["activity", "art", "chat", "check", "doc", "document", "object", "palette", "proposal", "scene"]);
}

fn base(name: &'static str) -> Command {
    Command::new(name, "s", json!({"type": "object", "properties": {}}), false, echo)
}

fn panic_message(list: Vec<Command>) -> String {
    let err = std::panic::catch_unwind(|| Registry::new(list)).err().expect("should panic");
    err.downcast_ref::<String>().cloned().or_else(|| err.downcast_ref::<&str>().map(|s| s.to_string())).unwrap()
}

#[test]
fn malformed_definitions_are_refused() {
    assert!(panic_message(vec![base("A.b")]).contains("name must be dotted"));
    let mut with_args = base("a.b");
    with_args.args = vec!["x"];
    assert!(panic_message(vec![with_args]).contains("args names x"));
    assert!(panic_message(vec![base("a.b"), base("a.b")]).contains("defined twice"));
    // `mutates` is a bool in Rust, so JS's "mutates must be true or false" can't happen; the others:
    let mut no_summary = base("a.b");
    no_summary.summary = "";
    assert!(panic_message(vec![no_summary]).contains("command a.b: summary is required"));
    let mut bad_params = base("a.b");
    bad_params.params = json!({"type": "object"});
    assert!(panic_message(vec![bad_params]).contains("params must be a JSON Schema object with properties"));
    let mut bad_required = base("a.b");
    bad_required.params = json!({"type": "object", "properties": {}, "required": ["z"]});
    assert!(panic_message(vec![bad_required]).contains("required names z"));
}

#[test]
fn call_checks_params_before_running() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static RAN: AtomicBool = AtomicBool::new(false);
    fn run(_: &mut Ctx, p: Params) -> Result<Value> {
        RAN.store(true, Ordering::SeqCst);
        Ok(Value::Object(p))
    }
    let cmd =
        Command::new("demo.x", "s", json!({"type": "object", "properties": {"n": {"type": "integer"}}, "required": ["n"]}), false, run);
    let reg = Registry::new(vec![cmd]);
    let mut ctx = Ctx { session: None, loaded: None };
    let err = reg.call(&mut ctx, "demo.x", json!({}), false).unwrap_err();
    assert!(matches!(&err, Error::Command(e) if e.is_usage()));
    assert!(!RAN.load(Ordering::SeqCst));
    assert_eq!(reg.call(&mut ctx, "demo.x", json!({"n": "2"}), true).unwrap(), json!({"n": 2}));
    let err = reg.call(&mut ctx, "demo.y", Value::Null, false).unwrap_err();
    assert!(matches!(&err, Error::Command(e) if e.code == "not_found" && e.message.contains("did you mean demo.x")), "{err}");
}
