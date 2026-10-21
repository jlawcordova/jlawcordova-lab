//! Port of cli/test/compile.test.mjs: the compiler (docs/intents/2026-10-pixellab-document/spec.md,
//! R7), the optimizer and lossless check, ported with their tests from the site's
//! scripts/optimize-pixel-art.test.mjs, plus `compile_scene` and `compile_all` on the sample
//! document.
//!
//! Rewritten: the sample document is the format-2 fixture (`fixtures/sample.pixellab`).
//! The two tests on the owner's site checkout are not ported as they were:
//! - "R7: in list order, every committed SVG in the site checkout is what the ported compiler
//!   produces from its sources" is dropped: its subject is the owner's committed SVGs. (The
//!   parity check that compared the compiler with Node's on the fixture and the examples passed
//!   before the Node CLI was removed.)
//! - "R3: in depth order, every scene of the site's art compiles lossless…" is rewritten on
//!   copies of the harbor and town examples (format 2), with the
//!   engine's default budget since an example has no site budgets; it still lists, rather than
//!   fails on, the scenes whose output changed and any over budget.
//!
//! No test here pins a gzip number: the report is checked by shape, as in Node.

mod common;
mod depthlib;

use depthlib::*;
use engine::compile::{
    budget_warnings, compile_all, compile_scene, optimize_svg, verify_lossless, Budget, Budgets, Build, Output, PreviewOnly, BUDGET,
};
use engine::engine::Order;

fn svg(body: &str) -> String {
    format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 8 8\">{body}</svg>")
}

fn rect(x: i64, y: i64) -> String {
    rect_wh(x, y, 1, 1)
}

fn rect_wh(x: i64, y: i64, w: i64, h: i64) -> String {
    format!("<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\"></rect>")
}

/// Deterministic pseudo-random generator, so the fuzz fixture never changes. The product
/// exceeds 2^53, so it is computed in doubles as JavaScript does.
fn lcg(seed: f64) -> impl FnMut() -> f64 {
    let mut s = seed;
    move || {
        s = (s * 1103515245.0 + 12345.0) % 2147483648.0;
        s / 2147483648.0
    }
}

// ------------------------------------------------------- optimize-pixel-art

#[test]
fn merges_a_horizontal_run_of_pixels_into_one_path() {
    let out = optimize_svg(&svg(&format!("<g fill=\"#111111\">{}{}{}</g>", rect(0, 0), rect(1, 0), rect(2, 0)))).unwrap();
    assert_eq!(out, svg("<path fill=\"#111111\" d=\"M0 0h3v1h-3z\"/>"));
}

#[test]
fn merges_identical_runs_in_consecutive_rows_into_a_taller_rectangle() {
    let out = optimize_svg(&svg(&format!("<g fill=\"#111111\">{}{}{}</g>", rect_wh(0, 0, 2, 1), rect(0, 1), rect(1, 1)))).unwrap();
    assert_eq!(out, svg("<path fill=\"#111111\" d=\"M0 0h2v2h-2z\"/>"));
}

#[test]
fn handles_negative_coordinates_and_overlapping_rects() {
    let out = optimize_svg(&svg(&format!("<g fill=\"#111111\">{}{}</g>", rect_wh(-3, -2, 2, 1), rect_wh(-2, -2, 3, 1)))).unwrap();
    assert_eq!(out, svg("<path fill=\"#111111\" d=\"M-3 -2h4v1h-4z\"/>"));
}

#[test]
fn keeps_class_data_class_and_element_order() {
    let out = optimize_svg(&svg(&format!(
        "<g fill=\"#222222\">{}</g><g class=\"pcloud pc0\"><g fill=\"#333333\">{}</g></g><g data-class=\"2\"><g fill=\"#444444\">{}</g><g fill=\"#222222\">{}</g></g>",
        rect(0, 0),
        rect(1, 1),
        rect(2, 2),
        rect(3, 3)
    )))
    .unwrap();
    assert_eq!(
        out,
        svg("<path fill=\"#222222\" d=\"M0 0h1v1h-1z\"/>\
             <g class=\"pcloud pc0\"><path fill=\"#333333\" d=\"M1 1h1v1h-1z\"/></g>\
             <g data-class=\"2\"><path fill=\"#444444\" d=\"M2 2h1v1h-1z\"/><path fill=\"#222222\" d=\"M3 3h1v1h-1z\"/></g>")
    );
}

#[test]
fn handles_a_class_group_wrapping_several_fill_groups_nested_two_deep() {
    let out =
        optimize_svg(&svg(&format!("<g class=\"wf w0\"><g class=\"cbob\"><g fill=\"#555555\">{}{}</g></g></g>", rect(0, 0), rect(0, 1))))
            .unwrap();
    assert_eq!(out, svg("<g class=\"wf w0\"><g class=\"cbob\"><path fill=\"#555555\" d=\"M0 0h1v2h-1z\"/></g></g>"));
}

#[test]
fn leaves_non_rect_content_untouched() {
    let body = format!(
        "<polygon points=\"0,0 1,1 0,1\" fill=\"#666666\"></polygon><g fill=\"#777777\">{}<circle cx=\"1\" cy=\"1\" r=\"1\"></circle></g><g fill=\"#888888\"></g>",
        rect(0, 0)
    );
    assert_eq!(optimize_svg(&svg(&body)).unwrap(), svg(&body));
}

#[test]
fn keeps_extra_attributes_of_a_fill_group_on_the_path() {
    let out = optimize_svg(&svg(&format!("<g fill=\"#999999\" opacity=\"0.5\">{}</g>", rect(0, 0)))).unwrap();
    assert_eq!(out, svg("<path fill=\"#999999\" opacity=\"0.5\" d=\"M0 0h1v1h-1z\"/>"));
}

#[test]
fn is_lossless_on_a_large_random_fixture() {
    let mut rand = lcg(42.0);
    let mut body = String::new();
    for g in 0..6 {
        body.push_str(&format!("<g fill=\"#00000{g}\">"));
        for _ in 0..400 {
            let x = (rand() * 40.0).floor() as i64 - 20;
            let y = (rand() * 30.0).floor() as i64 - 10;
            let w = 1 + (rand() * 3.0).floor() as i64;
            let h = 1 + (rand() * 2.0).floor() as i64;
            body.push_str(&rect_wh(x, y, w, h));
        }
        body.push_str("</g>");
    }
    let source = svg(&format!("<g class=\"hf h0\">{body}</g>"));
    verify_lossless(&source, &optimize_svg(&source).unwrap()).expect("lossless");
}

#[test]
fn fails_when_a_group_gains_or_loses_a_pixel() {
    let source = svg(&format!("<g fill=\"#111111\">{}</g><g fill=\"#222222\">{}</g>", rect_wh(0, 0, 3, 1), rect(0, 1)));
    let output = optimize_svg(&source).unwrap();
    let err = verify_lossless(&source, &output.replace("h3v1h-3z", "h2v1h-2z")).unwrap_err();
    assert!(err.contains("#111111"), "{err}");
    let err = verify_lossless(&source, &output.replace("M0 1h1", "M1 1h1")).unwrap_err();
    assert!(err.contains("#222222"), "{err}");
}

#[test]
fn fails_when_the_structure_or_z_order_changes() {
    let source = svg(&format!("<g class=\"a\"><g fill=\"#111111\">{}</g></g><g fill=\"#222222\">{}</g>", rect(0, 0), rect(1, 1)));
    let output = optimize_svg(&source).unwrap();
    assert!(verify_lossless(&source, &output.replace("class=\"a\"", "class=\"b\"")).is_err());
    let swapped = svg("<g class=\"a\"><path fill=\"#222222\" d=\"M1 1h1v1h-1z\"/></g><path fill=\"#111111\" d=\"M0 0h1v1h-1z\"/>");
    assert!(verify_lossless(&source, &swapped).is_err());
}

#[test]
fn rejects_markup_it_does_not_understand() {
    let err = optimize_svg(&svg(&format!("<!-- note --><g fill=\"#111111\">{}</g>", rect(0, 0)))).unwrap_err();
    assert!(err.to_lowercase().contains("unsupported"), "{err}");
    let err = optimize_svg(&svg("<g fill=\"#111111\"><rect x=\"0.5\" y=\"0\" width=\"1\" height=\"1\"></rect></g>")).unwrap_err();
    assert!(err.to_lowercase().contains("whole-number"), "{err}");
}

// ------------------------------------ compileScene and compileAll (spec R7)

/// `^[\d.]+ KB \/ [\d.]+ KB gzip → [\d.]+ KB \/ [\d.]+ KB gzip, lossless$`
fn is_report(s: &str) -> bool {
    let number = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit() || b == b'.');
    let side = |t: &str| t.split_once(" KB / ").is_some_and(|(raw, rest)| number(raw) && rest.strip_suffix(" KB gzip").is_some_and(number));
    s.strip_suffix(", lossless").and_then(|s| s.split_once(" → ")).is_some_and(|(a, b)| side(a) && side(b))
}

fn sample() -> engine::engine::Sources {
    let (sources, load_errors) = read_sources(&common::fixture());
    assert_eq!(load_errors, Vec::<String>::new());
    sources
}

fn budgets(file: &str, raw: u64, gzip: u64) -> Budgets {
    [(file.to_string(), Budget { raw, gzip })].into_iter().collect()
}

#[test]
fn r7_compiles_a_scene_to_one_path_per_color_lossless_with_the_engine_budget_unless_the_manifest_gives_one() {
    let sources = sample();
    let r = compile_scene(&sources, "sample", &Budgets::new(), Order::Depth).unwrap();
    assert!(
        r.output.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" aria-hidden=\"true\" viewBox=\"-16 -16 64 48\" shape-rendering=\"crispEdges\" data-pixellab=\"sample\"><path fill=\"#"),
        "{}",
        r.output
    );
    assert_eq!(r.output.matches("<path ").count(), 5, "one path per color used");
    assert!(!r.over);
    assert_eq!(r.budget, BUDGET);
    assert!(is_report(&r.report), "{}", r.report);
    let tight = compile_scene(&sources, "sample", &budgets("sample.svg", 10, 10), Order::Depth).unwrap();
    assert!(tight.over);
    assert!(tight.report.ends_with(" — over its 0.0 KB budget and over its 0.0 KB gzip budget (warning)"), "{}", tight.report);
    assert_eq!(tight.warnings, ["over its 0.0 KB budget", "over its 0.0 KB gzip budget"]);
    assert!(r.warnings.is_empty());
    assert!(
        !compile_scene(&sources, "sample-preview", &budgets("sample.svg", 10, 10), Order::Depth).unwrap().over,
        "a preview-only scene keeps the engine budget"
    );
}

#[test]
fn r4_compile_all_writes_an_over_budget_scene_with_a_warning_and_stays_ok() {
    let out = Temp::new();
    let sources = sample();
    let r = compile_all(&sources, &[], &out.0, &budgets("sample.svg", 512, 100 * 1024)).unwrap();
    assert!(r.ok, "over budget never fails");
    assert!(out.0.join("sample.svg").exists());
    assert_eq!(r.outputs[0].warnings, ["over its 0.5 KB budget"]);
    assert!(r.outputs[0].report.ends_with(", lossless — over its 0.5 KB budget (warning)"), "{}", r.outputs[0].report);
    let gz = compile_all(&sources, &[], &out.0, &budgets("sample.svg", 100 * 1024, 300)).unwrap();
    assert_eq!(gz.outputs[0].warnings, ["over its 0.3 KB gzip budget"]);
    assert_eq!(budget_warnings(Budget { raw: 750 * 1024, gzip: 150 * 1024 }, 750 * 1024 + 1, 1), ["over its 750 KB budget"]);
}

#[test]
fn r7_compile_all_writes_each_output_once_reports_preview_only_scenes_and_writes_nothing_on_problems() {
    let out = Temp::new();
    let sources = sample();
    let r = compile_all(&sources, &[], &out.0, &Budgets::new()).unwrap();
    assert!(r.ok);
    assert_eq!(
        r.outputs.iter().map(|o| (o.file.as_str(), o.scene.as_str(), o.over)).collect::<Vec<_>>(),
        [("sample.svg", "sample", false)]
    );
    assert_eq!(r.preview_only.iter().map(|p| p.scene.as_str()).collect::<Vec<_>>(), ["sample-preview"]);
    assert_eq!(
        std::fs::read_to_string(out.0.join("sample.svg")).unwrap(),
        compile_scene(&sources, "sample", &Budgets::new(), Order::Depth).unwrap().output
    );
    let none = out.0.join("none");
    let refused = compile_all(&sources, &["objects/x.json: broken".to_string()], &none, &Budgets::new()).unwrap();
    assert_eq!(
        refused,
        Build {
            ok: false,
            outputs: Vec::<Output>::new(),
            preview_only: Vec::<PreviewOnly>::new(),
            problems: vec!["objects/x.json: broken".to_string()]
        }
    );
    assert!(!none.exists());
}

#[test]
fn r8_compiling_twice_gives_identical_bytes() {
    let (a, b) = (sample(), sample());
    assert_eq!(
        compile_scene(&a, "sample", &Budgets::new(), Order::Depth).unwrap().output,
        compile_scene(&b, "sample", &Budgets::new(), Order::Depth).unwrap().output
    );
}

// ------------------------------------------- the examples' art (spec R3), rewritten

#[test]
fn r3_in_depth_order_every_scene_of_the_examples_compiles_lossless_the_scenes_whose_output_changed_and_any_over_budget_are_listed_not_failed(
) {
    let dir = Temp::new();
    let mut changed = Vec::new();
    let mut over = Vec::new();
    let mut scenes = 0;
    for name in ["harbor", "town"] {
        let path = example(&dir.0, name);
        let (sources, load_errors) = read_sources(&path);
        assert_eq!(load_errors, Vec::<String>::new());
        for (scene_name, scene) in &sources.scenes {
            let before = compile_scene(&sources, scene_name, &Budgets::new(), Order::List).unwrap();
            let after = compile_scene(&sources, scene_name, &Budgets::new(), Order::Depth).unwrap(); // errors unless lossless
            if after.output != before.output {
                changed.push(format!("{name}/{scene_name} {} → {} bytes", before.raw, after.raw));
            }
            if after.over {
                let file =
                    scene.get("output").and_then(|o| o.as_str()).map_or_else(|| format!("{scene_name} (preview only)"), str::to_string);
                over.push(format!("{name}/{file} {}", after.report));
            }
            assert_eq!(
                compile_scene(&sources, scene_name, &Budgets::new(), Order::Depth).unwrap().output,
                after.output,
                "{name}/{scene_name} compiles to the same bytes twice"
            );
            scenes += 1;
        }
    }
    assert_eq!(scenes, 3, "the examples have scenes");
    eprintln!("scenes whose output changes in depth order: {}", if changed.is_empty() { "none".to_string() } else { changed.join("; ") });
    eprintln!("scenes over budget in depth order: {}", if over.is_empty() { "none".to_string() } else { over.join("; ") });
}
