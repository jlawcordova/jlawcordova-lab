//! The program parts of cli/test/depth-motion.test.mjs (docs/intents/2026-10-depth-order/spec.md,
//! R10, R15, D6): the preview `scene.render` draws at a time is what the published SVG shows at
//! that time (the SVG's tree with its CSS evaluated, as a browser would), and `scene.render`'s
//! items come back in the order drawn at that time with their paths into the file. The engine
//! parts of that Node suite (the order while moving, windows, copies, the SVG's text) are not
//! here.
#![allow(clippy::result_large_err)]

mod common;

use std::collections::{BTreeMap, HashMap};

use engine::engine::{composite, motion_copies, render_scene, scene_at, Child, CompositeOptions, Group, Order, Rendered, Sources};
use engine::iso::Pixel;
use engine::motion::{loop_frame, move_runs, pose_items};
use pixellab::context::Ctx;
use pixellab::document::{open_document, OpenOptions};
use pixellab::errors::Error;
use pixellab::registry::registry;
use pixellab::render::{render_scene_image, SceneImageOptions};
use pixellab::session::{Options, Session};
use serde_json::{json, Value};

fn sprite(map: &[&str], anchor: Option<[i64; 2]>) -> Value {
    let mut s = json!({ "kind": "sprite", "keys": { "g": "grass-2", "i": "ink" }, "layers": [{ "map": map }] });
    if let Some(a) = anchor {
        s["anchor"] = json!(a);
    }
    s
}

fn block() -> Value {
    json!({ "kind": "block", "size": [1, 1, 1], "faces": { "top": "grass-2", "left": "grass-4", "right": "wood-3" } })
}

/// A 1x1x1 block (the house) and a small sprite (the walker) standing at its bottom middle.
fn objects() -> BTreeMap<String, Value> {
    BTreeMap::from([("house".to_string(), block()), ("walker".to_string(), sprite(&[".gg.", "gggg", "gggg", ".ii."], Some([2, 4])))])
}

fn tall() -> Value {
    sprite(&[".ii.", "gggg", "gggg", "gggg", "gggg", "gggg", ".ii."], Some([2, 7]))
}

/// A scene whose motion is `tracks`, a loop of 10000 ms.
fn scene(items: Vec<Value>, tracks: Vec<Value>) -> Value {
    let mut s = json!({ "viewBox": [-80, -20, 160, 140], "items": items });
    if !tracks.is_empty() {
        s["motion"] = json!({ "duration": 10000, "tracks": tracks });
    }
    s
}

fn track(item: &str, keys: Vec<Value>) -> Value {
    json!({ "item": item, "keys": keys })
}

struct Nice {
    objects: BTreeMap<String, Value>,
    scene: Value,
}

/// A scene whose keys fall on whole percentages and whose steps on exact ones, with items
/// crossing each other and still ones: a walker down a column past two houses, one up the same
/// column the other way, a diagonal one, a group with its own house and walker, and one that
/// hides, changes pose and shows again.
fn nice() -> Nice {
    let mut objs = objects();
    objs.insert("walker-b".into(), sprite(&[".gg.", "gggg", "gggg", "ii.."], Some([2, 4])));
    objs.insert("tall".into(), tall());
    let items = vec![
        json!({ "object": "house", "at": { "tile": [2, 2, 1] } }),
        json!({ "object": "house", "at": { "tile": [4, 1, 1] } }),
        json!({ "object": "tall", "at": { "tile": [-1, 3, 0] } }),
        json!({ "object": "walker", "id": "down", "at": { "px": [0, -4] } }),
        json!({ "object": "walker", "id": "up", "at": { "px": [0, 60] } }),
        json!({ "object": "tall", "id": "diag", "at": { "px": [-40, 12] } }),
        json!({ "object": "walker", "id": "blink", "class": "cargo", "at": { "px": [30, 18] } }),
        json!({
            "group": { "class": "enter" },
            "items": [
                { "object": "house", "at": { "tile": [-1, 2, 1] } },
                { "object": "walker", "id": "inner", "at": { "px": [-60, 4] } },
                { "object": "walker", "at": { "px": [-60, 40] } },
            ],
        }),
    ];
    let tracks = vec![
        track(
            "down",
            vec![
                json!({ "t": 0 }),
                json!({ "t": 2500, "move": [0, 20] }),
                json!({ "t": 5000, "move": [0, 40] }),
                json!({ "t": 7500, "move": [0, 40] }),
            ],
        ),
        track("up", vec![json!({ "t": 0 }), json!({ "t": 5000, "move": [0, -40] }), json!({ "t": 7500, "move": [0, -40] })]),
        track("diag", vec![json!({ "t": 0 }), json!({ "t": 5000, "move": [40, 20] })]),
        track(
            "blink",
            vec![
                json!({ "t": 0 }),
                json!({ "t": 2500, "move": [0, 10], "pose": "walker-b" }),
                json!({ "t": 5000, "move": [0, 20], "show": false }),
                json!({ "t": 7500, "show": true }),
            ],
        ),
        track("inner", vec![json!({ "t": 0 }), json!({ "t": 5000, "move": [40, 20] })]),
    ];
    Nice { objects: objs, scene: scene(items, tracks) }
}

// ------------------------------------------------------- a small CSS evaluator

struct Stop {
    at: f64,
    decl: Vec<(String, String)>,
}

#[derive(Default)]
struct Css {
    keyframes: Vec<(String, Vec<Stop>)>,
    rules: Vec<(String, Vec<(String, String)>)>,
}

impl Css {
    fn keyframes(&self, name: &str) -> &[Stop] {
        &self.keyframes.iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("no keyframes {name}")).1
    }

    fn rule(&self, class: &str) -> Option<&Vec<(String, String)>> {
        self.rules.iter().find(|(c, _)| c == class).map(|(_, d)| d)
    }
}

fn decls(text: &str) -> Vec<(String, String)> {
    text.split(';')
        .filter(|d| !d.is_empty())
        .map(|d| {
            let colon = d.find(':').unwrap();
            (d[..colon].to_string(), d[colon + 1..].to_string())
        })
        .collect()
}

fn get<'a>(decl: &'a [(String, String)], key: &str) -> Option<&'a str> {
    decl.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// Reads the CSS a scene's motion writes: its @keyframes (stops with their declarations) and its
/// class rules. Only the shapes motion.rs writes.
fn parse_css(css: &str) -> Css {
    let mut out = Css::default();
    let mut at = 0;
    while at < css.len() {
        let open = css[at..].find('{').unwrap() + at;
        let mut depth = 1;
        let mut end = open + 1;
        let bytes = css.as_bytes();
        while depth > 0 {
            match bytes[end] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            end += 1;
        }
        let (head, body) = (&css[at..open], &css[open + 1..end - 1]);
        at = end;
        if let Some(name) = head.strip_prefix("@keyframes ") {
            let mut stops = Vec::new();
            let mut rest = body;
            while let Some(pct) = rest.find("%{") {
                let start = rest[..pct].rfind(|c: char| !(c.is_ascii_digit() || c == '.')).map_or(0, |i| i + 1);
                let close = rest[pct..].find('}').unwrap() + pct;
                stops.push(Stop { at: rest[start..pct].parse().unwrap(), decl: decls(&rest[pct + 2..close]) });
                rest = &rest[close + 1..];
            }
            out.keyframes.push((name.to_string(), stops));
        } else if head.starts_with("@media") {
            // reduced motion: not needed
        } else {
            out.rules.push((head[1..].to_string(), decls(body)));
        }
    }
    out
}

/// The numbers in a `translate(x,y)` value, or None.
fn translation(value: &str) -> Option<[f64; 2]> {
    let inner = value.strip_prefix("translate(")?.strip_suffix(')')?;
    let (x, y) = inner.split_once(',')?;
    let num = |s: &str| s.strip_suffix("px").unwrap_or(s).parse::<f64>().ok();
    Some([num(x)?, num(y)?])
}

fn steps_of(timing: &str) -> Option<f64> {
    timing.strip_prefix("steps(")?.strip_suffix(",end)")?.parse().ok()
}

/// A property's value in a keyframe animation at `p` percent of the loop, with step-end and
/// steps(n,end) timing.
fn sample_keyframes(stops: &[Stop], timing: &str, p: f64) -> Vec<(String, String)> {
    let mut i: Option<usize> = None;
    for (k, stop) in stops.iter().enumerate() {
        if stop.at <= p {
            i = Some(k);
        }
    }
    let Some(i) = i else { return vec![] };
    let (from, to) = (&stops[i], stops.get(i + 1));
    let f = get(&from.decl, "animation-timing-function").unwrap_or(timing);
    let mut out = Vec::new();
    for (prop, value) in &from.decl {
        if prop == "animation-timing-function" {
            continue;
        }
        match (steps_of(f), to) {
            (Some(n), Some(to)) => {
                let k = (n * (p - from.at) / (to.at - from.at)).floor();
                let (a, b) = (translation(value).unwrap(), translation(get(&to.decl, prop).unwrap()).unwrap());
                out.push((
                    prop.clone(),
                    format!(
                        "translate({},{})",
                        engine::jsnum::to_string(a[0] + (b[0] - a[0]) * k / n),
                        engine::jsnum::to_string(a[1] + (b[1] - a[1]) * k / n)
                    ),
                ));
            }
            _ => out.push((prop.clone(), value.clone())),
        }
    }
    out
}

/// Splits on commas outside parentheses.
fn split_animations(text: &str) -> Vec<&str> {
    let (mut parts, mut depth, mut start) = (Vec::new(), 0, 0);
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

struct Style {
    opacity: f64,
    translate: [f64; 2],
}

/// A class's opacity and translation at `p` percent of the loop; with `animate: false`, with
/// animations off (reduced motion).
fn style_of(css: &Css, class: &str, p: f64, animate: bool) -> Style {
    let Some(rule) = css.rule(class) else { return Style { opacity: 1.0, translate: [0.0, 0.0] } };
    let mut style = Style { opacity: get(rule, "opacity").map_or(1.0, |o| o.parse().unwrap()), translate: [0.0, 0.0] };
    let Some(animation) = get(rule, "animation").filter(|_| animate) else { return style };
    for part in split_animations(animation) {
        let fields: Vec<&str> = part.split(' ').collect();
        let got = sample_keyframes(css.keyframes(fields[0]), fields.get(2).copied().unwrap_or(""), p);
        if let Some(o) = get(&got, "opacity") {
            style.opacity = o.parse().unwrap();
        }
        if let Some(t) = get(&got, "transform") {
            style.translate = translation(t).unwrap();
        }
    }
    style
}

/// The pixels the published tree shows at `p` percent of a loop of `duration` ms, reading only
/// its CSS: mv- groups take the opacity and translation the rules and keyframes give, frame
/// loops show the frame of that time.
fn paint_css(scene: &Rendered, p: f64, duration: f64, animate: bool) -> HashMap<Pixel, String> {
    let css = parse_css(scene.style.as_deref().unwrap_or(""));
    let time = if animate { p / 100.0 * duration } else { 0.0 };
    let mut out = HashMap::new();
    #[allow(clippy::too_many_arguments)]
    fn walk(group: &Group, dx: f64, dy: f64, css: &Css, p: f64, time: f64, animate: bool, out: &mut HashMap<Pixel, String>) {
        for child in &group.children {
            match child {
                Child::Layer(l) => {
                    for (px, color) in l.pixels.iter() {
                        out.insert((px.0 + dx as i32, px.1 + dy as i32), color.to_string());
                    }
                }
                Child::Group(g) => {
                    let class = g.attrs.iter().find(|(k, _)| k == "class").map_or("", |(_, v)| v.as_str());
                    if let Some(frame) = g.frame {
                        if frame != loop_frame(class.split(' ').next().unwrap(), g.frames.unwrap_or(1), time) {
                            continue;
                        }
                    }
                    let (mut ox, mut oy) = (dx, dy);
                    if class.starts_with("mv-") && !class.contains(' ') {
                        let style = style_of(css, class, p, animate);
                        if style.opacity == 0.0 {
                            continue;
                        }
                        (ox, oy) = (dx + style.translate[0], dy + style.translate[1]);
                    }
                    walk(g, ox, oy, css, p, time, animate, out);
                }
            }
        }
    }
    walk(&scene.root, 0.0, 0.0, &css, p, time, animate, &mut out);
    out
}

// ------------------------------------------------------- the document

struct Open {
    session: Session,
    sources: Sources,
    _doc: common::TempDocument,
}

/// The scene as a document in a temp folder, with its sources as the engine loads them.
fn open(nice: &Nice) -> Open {
    let doc = common::make_document();
    for (name, obj) in &nice.objects {
        doc.write(&format!("objects/{name}.json"), &engine::format::serialize_object(obj));
    }
    doc.write("scenes/nice.json", &engine::format::serialize_scene(&nice.scene));
    let env = doc.env();
    let document = open_document(&OpenOptions {
        doc: Some(doc.root.to_string_lossy().into_owned()),
        site: None,
        env: env.clone(),
        cwd: doc.root.clone(),
    })
    .unwrap();
    let mut session = Session::new(document, Options { env, ..Options::default() });
    let sources = Ctx::new(&mut session).sources().unwrap().sources.clone();
    Open { session, sources, _doc: doc }
}

fn run(s: &mut Session, name: &str, params: Value) -> Result<Value, Error> {
    registry().call(&mut Ctx::new(s), name, params, false)
}

fn unbase64(text: &str) -> Vec<u8> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        _ => 63,
    };
    let mut out = Vec::new();
    for chunk in text.as_bytes().chunks(4) {
        let n: Vec<u32> = chunk.iter().filter(|&&c| c != b'=').map(|&c| value(c) as u32).collect();
        let bits = n.iter().fold(0u32, |a, &v| a << 6 | v) << (6 * (4 - n.len()));
        for i in 0..n.len() - 1 {
            out.push((bits >> (16 - 8 * i)) as u8);
        }
    }
    out
}

/// The PNG that scene.render makes at `time`, as a map of pixel to the hex of the pixels it paints.
fn preview_at(o: &mut Open, time: f64) -> HashMap<Pixel, String> {
    // scene.render takes whole milliseconds; a window can end between two, which the same function renders.
    let r = if time.fract() == 0.0 {
        run(&mut o.session, "scene.render", json!({ "name": "nice", "time": time })).unwrap()
    } else {
        render_scene_image(&o.sources, "nice", "#1C1712", &SceneImageOptions { time, ..SceneImageOptions::default() }).unwrap()
    };
    let png = engine::png::decode(&unbase64(r["png"].as_str().unwrap())).unwrap();
    let (vx, vy) = (r["viewBox"][0].as_i64().unwrap() as i32, r["viewBox"][1].as_i64().unwrap() as i32);
    let mut out = HashMap::new();
    for y in 0..png.height {
        for x in 0..png.width {
            let at = ((y * png.width + x) * 4) as usize;
            if png.rgba[at + 3] == 0 {
                continue;
            }
            out.insert((x as i32 + vx, y as i32 + vy), format!("{:02x}{:02x}{:02x}", png.rgba[at], png.rgba[at + 1], png.rgba[at + 2]));
        }
    }
    out
}

/// A painted map with colors as lower-case hex digits, cut to the scene's view box as the PNG is.
fn in_hex(sources: &Sources, nice: &Nice, pixels: &HashMap<Pixel, String>) -> HashMap<Pixel, String> {
    let v: Vec<i32> = nice.scene["viewBox"].as_array().unwrap().iter().map(|n| n.as_i64().unwrap() as i32).collect();
    pixels
        .iter()
        .filter(|((x, y), _)| *x >= v[0] && *y >= v[1] && *x < v[0] + v[2] && *y < v[1] + v[3])
        .map(|(p, color)| (*p, sources.colors.get(color.as_str()).unwrap().hex[1..].to_lowercase()))
        .collect()
}

/// The pixels where two painted maps differ.
fn differences(a: &HashMap<Pixel, String>, b: &HashMap<Pixel, String>) -> Vec<Pixel> {
    let mut keys: Vec<Pixel> = a.keys().chain(b.keys()).copied().collect();
    keys.sort();
    keys.dedup();
    keys.into_iter().filter(|p| a.get(p) != b.get(p)).collect()
}

/// Every time to check, in ms: each copy window's boundary, each step of every track, and the
/// middle of every stretch between them.
fn check_times(scn: &Value, out: &Rendered) -> (Vec<f64>, usize) {
    let duration = scn["motion"]["duration"].as_f64().unwrap();
    let css = parse_css(out.style.as_deref().unwrap());
    let mut cuts: Vec<f64> = vec![0.0];
    let is_copy =
        |name: &str| name.rfind("-z").is_some_and(|i| !name[i + 2..].is_empty() && name[i + 2..].bytes().all(|b| b.is_ascii_digit()));
    for (name, stops) in &css.keyframes {
        if is_copy(name) {
            cuts.extend(stops.iter().map(|s| s.at));
        }
    }
    for t in scn["motion"]["tracks"].as_array().unwrap() {
        cuts.extend(move_runs(t, "walker", duration).iter().map(|r| r.at));
    }
    cuts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    cuts.dedup();
    let mut times: Vec<f64> = Vec::new();
    for (i, at) in cuts.iter().enumerate() {
        times.push(at / 100.0 * duration);
        let next = cuts.get(i + 1).copied().unwrap_or(100.0);
        times.push(engine::jsnum::round((at + next) / 2.0 / 100.0 * duration));
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    times.dedup();
    (times, cuts.len())
}

#[test]
fn r10_r15_at_every_window_boundary_and_the_middle_of_every_window_the_svgs_tree_with_its_css_evaluated_at_that_time_paints_the_pixels_scene_render_paints(
) {
    let n = nice();
    let mut o = open(&n);
    let out = render_scene(&o.sources, "nice", Order::Depth).unwrap();
    let copies = motion_copies(&o.sources, "nice").unwrap();
    assert!(copies.len() >= 3, "items cross others and take several places: {copies:?}");
    assert!(copies.iter().any(|c| c.0 == "inner"), "a group member reorders among its group");
    let (times, boundaries) = check_times(&n.scene, &out);
    assert!(boundaries > 40 && times.len() > 80, "{boundaries} cuts, {} times", times.len());
    let duration = n.scene["motion"]["duration"].as_f64().unwrap();
    let mut saw_difference = false;
    for time in times {
        let p = time / (duration / 100.0);
        let got = preview_at(&mut o, time);
        let published = in_hex(&o.sources, &n, &paint_css(&out, p, duration, true));
        let bad = differences(&got, &published);
        assert!(
            bad.is_empty(),
            "at {time} ms ({p}%): {} pixels differ, e.g. {:?}",
            bad.len(),
            bad.iter().take(3).map(|q| format!("{q:?}: preview {:?}, svg {:?}", got.get(q), published.get(q))).collect::<Vec<_>>()
        );
        // What the engine drew before the depth order (the list's order, nothing reordered) must
        // differ somewhere, or this proves nothing.
        let scn = o.sources.scenes["nice"].clone();
        let point_of = |item: &Value| -> [f64; 2] {
            match item["at"].get("tile") {
                Some(t) => {
                    let t: Vec<f64> = t.as_array().unwrap().iter().map(|n| n.as_f64().unwrap()).collect();
                    [(t[0] - t[1]) * 16.0, (t[0] + t[1]) * 8.0 - t[2] * 16.0]
                }
                None => {
                    let px = item["at"]["px"].as_array().unwrap();
                    [px[0].as_f64().unwrap(), px[1].as_f64().unwrap()]
                }
            }
        };
        let posed = pose_items(scn["items"].as_array().unwrap(), scn.get("motion"), time, &point_of);
        let mut flat = scn.clone();
        flat.as_object_mut().unwrap().shift_remove("motion");
        flat["items"] = Value::Array(posed);
        let mut sources = o.sources.clone();
        sources.scenes.insert("nice".into(), flat);
        let listed = render_scene(&sources, "nice", Order::List).unwrap();
        let old = in_hex(
            &o.sources,
            &n,
            &composite(&listed.root, CompositeOptions { time: Some(time), ..CompositeOptions::default() })
                .iter()
                .map(|(p, c)| (*p, c.to_string()))
                .collect(),
        );
        if !differences(&old, &published).is_empty() {
            saw_difference = true;
        }
    }
    assert!(saw_difference, "drawing in list order is wrong somewhere, so the copies matter");
}

#[test]
fn r10_r15_reduced_motion_animations_off_shows_the_frame_of_time_0() {
    let n = nice();
    let mut o = open(&n);
    let out = render_scene(&o.sources, "nice", Order::Depth).unwrap();
    let duration = n.scene["motion"]["duration"].as_f64().unwrap();
    let rest = preview_at(&mut o, 0.0);
    assert!(differences(&rest, &in_hex(&o.sources, &n, &paint_css(&out, 0.0, duration, false))).is_empty());
    assert!(
        differences(&rest, &in_hex(&o.sources, &n, &paint_css(&out, 0.0, duration, true))).is_empty(),
        "and so does the animation at 0%"
    );
    // Every copy but the one holding time 0 is hidden by its base rule.
    let css = parse_css(out.style.as_deref().unwrap());
    let is_copy =
        |name: &str| name.rfind("-z").is_some_and(|i| !name[i + 2..].is_empty() && name[i + 2..].bytes().all(|b| b.is_ascii_digit()));
    let hidden: Vec<&str> =
        css.rules.iter().filter(|(c, d)| is_copy(c) && get(d, "opacity") == Some("0")).map(|(c, _)| c.as_str()).collect();
    assert!(!hidden.is_empty() && hidden.iter().all(|c| !c.ends_with("-z0")), "{hidden:?}");
}

fn path_strings(items: &Value) -> Vec<String> {
    items
        .as_array()
        .unwrap()
        .iter()
        .map(|i| match &i["path"] {
            Value::Array(a) => a.iter().map(|n| n.to_string()).collect::<Vec<_>>().join("."),
            other => other.as_str().unwrap().to_string(),
        })
        .collect()
}

#[test]
fn r10_scene_at_lists_the_items_in_the_order_drawn_at_that_time_and_an_item_hides_and_shows_while_keeping_its_place() {
    let n = nice();
    let o = open(&n);
    let at = |time: f64| scene_at(&o.sources, "nice", time).unwrap();
    let rest = at(0.0);
    assert_eq!(rest["items"].as_array().unwrap().len(), 8, "every item and the group");
    let mut sorted = path_strings(&rest["items"]);
    sorted.sort();
    assert_eq!(sorted, ["0", "1", "2", "3", "4", "5", "6", "7"]);
    assert_ne!(path_strings(&at(3500.0)["items"]), path_strings(&rest["items"]), "the order changed");
    let blink = |time: f64| at(time)["items"].as_array().unwrap().iter().find(|i| i["id"] == "blink").unwrap().clone();
    assert!(blink(3000.0).get("hidden").is_none());
    assert_eq!(blink(6000.0)["hidden"], true, "hidden while it does not show, still in the list");
    assert_eq!(blink(3000.0)["object"], "walker-b", "drawing its pose");
    assert!(rest.get("motion").is_none());
}

fn layered() -> Nice {
    let mut s = scene(
        vec![
            json!({ "object": "house", "at": { "tile": [2, 2, 1] } }),
            json!({ "group": { "data-class": "a" }, "items": [{ "object": "house", "at": { "tile": [6, 0, 0] } }] }),
            json!({ "group": { "data-class": "b" }, "items": [{ "object": "house", "at": { "tile": [7, 0, 0] } }] }),
            json!({ "object": "walker", "id": "down", "layer": "cast", "at": { "px": [0, -4] } }),
        ],
        vec![track("down", vec![json!({ "t": 0 }), json!({ "t": 5000, "move": [0, 40] }), json!({ "t": 7500, "move": [0, 40] })])],
    );
    s["layers"] = json!([{ "id": "cast", "name": "Cast" }]);
    Nice { objects: objects(), scene: s }
}

#[test]
fn r10_through_scene_render_with_variants_and_layers_the_items_come_back_in_the_order_drawn_at_that_time_with_paths_into_the_file_and_show_and_hide_leave_the_rest_where_it_is(
) {
    let n = layered();
    let mut o = open(&n);
    // The walker is drawn before the house (behind it) for part of the loop.
    let before = (0..40)
        .map(|k| k as f64 * 250.0)
        .find(|time| scene_at(&o.sources, "nice", *time).unwrap()["items"][0]["id"] == "down")
        .expect("the walker is behind the house at some time");
    let mut at = |extra: Value| {
        let mut p = json!({ "name": "nice", "time": before });
        for (k, v) in extra.as_object().unwrap() {
            p[k] = v.clone();
        }
        run(&mut o.session, "scene.render", p).unwrap()
    };
    let plain = at(json!({}));
    assert_eq!(
        plain["items"].as_array().unwrap().iter().map(|i| i["path"].as_str().unwrap()).collect::<Vec<_>>(),
        ["3", "0", "1.0"],
        "drawn order: the walker, the house, the first variant"
    );
    assert_eq!(
        plain["items"].as_array().unwrap().iter().map(|i| i.get("id").unwrap_or(&i["object"]).as_str().unwrap()).collect::<Vec<_>>(),
        ["down", "house", "house"]
    );
    // At rest it is drawn after the house; the list never changed.
    let rest = at(json!({ "time": 0 }));
    assert_eq!(rest["items"].as_array().unwrap().iter().map(|i| i["path"].as_str().unwrap()).collect::<Vec<_>>(), ["0", "1.0", "3"]);
    // The second variant is chosen by its path in the file, though the walker's move changed what
    // sits at each place in the drawn list.
    let second = at(json!({ "show": "2" }));
    assert_eq!(second["items"].as_array().unwrap().iter().map(|i| i["path"].as_str().unwrap()).collect::<Vec<_>>(), ["3", "0", "2.0"]);
    // Hiding the walker's layer leaves the others in the order they have.
    let hidden = at(json!({ "hide": ["cast"] }));
    assert_eq!(hidden["items"].as_array().unwrap().iter().map(|i| i["path"].as_str().unwrap()).collect::<Vec<_>>(), ["0", "1.0"]);
    assert_ne!(plain["png"], hidden["png"]);
}
