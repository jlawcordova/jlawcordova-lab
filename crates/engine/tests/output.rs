//! R20's two new engine tests, over the fixture and both examples (read
//! through format-2 copies, as `depthlib::example` makes them): compiling a
//! document twice gives identical bytes, and a compiled SVG carries no
//! script, link, text or external reference.

mod common;
mod depthlib;

use depthlib::{example, read_sources, Temp};
use engine::compile::{compile_scene, Budgets};
use engine::engine::{validate, Order, Sources};

/// (name, sources) of the fixture and the two examples.
fn documents(base: &Temp) -> Vec<(String, Sources)> {
    let mut out = Vec::new();
    for (name, dir) in [
        ("sample".to_string(), common::fixture()),
        ("harbor".to_string(), example(&base.0, "harbor")),
        ("town".to_string(), example(&base.0, "town")),
    ] {
        let (sources, load_errors) = read_sources(&dir);
        assert_eq!(load_errors, Vec::<String>::new(), "{name}");
        assert_eq!(validate(&sources), Vec::<String>::new(), "{name}");
        out.push((name, sources));
    }
    out
}

fn compiled(sources: &Sources) -> Vec<(String, String)> {
    let names: Vec<String> = sources.scenes.keys().cloned().collect();
    assert!(!names.is_empty());
    names.into_iter().map(|n| (n.clone(), compile_scene(sources, &n, &Budgets::new(), Order::Depth).unwrap().output)).collect()
}

#[test]
fn r20_compiling_twice_gives_identical_bytes() {
    let base = Temp::new();
    for (name, sources) in documents(&base) {
        let (first, second) = (compiled(&sources), compiled(&sources));
        assert_eq!(first, second, "{name}: two compiles differ");
        let (again, _) = read_sources(&match name.as_str() {
            "sample" => common::fixture(),
            other => base.0.join(format!("{other}.pixellab")),
        });
        assert_eq!(first, compiled(&again), "{name}: a compile of freshly read sources differs");
    }
}

#[test]
fn r20_a_compiled_svg_has_no_script_link_text_or_external_reference() {
    let base = Temp::new();
    for (name, sources) in documents(&base) {
        for (scene, svg) in compiled(&sources) {
            let lower = svg.to_lowercase();
            for banned in ["<script", "<a ", "<a>", "<a\n", "<text", "<foreignobject", "href", "<image", "@import", "javascript:"] {
                assert!(!lower.contains(banned), "{name}/{scene}: contains {banned}");
            }
            let mut rest = lower.as_str();
            while let Some(i) = rest.find("url(") {
                let arg = rest[i + 4..].trim_start_matches(['\'', '"', ' ']);
                assert!(arg.starts_with('#'), "{name}/{scene}: url( to {:?}", &arg[..arg.len().min(30)]);
                rest = &rest[i + 4..];
            }
            assert!(
                !lower.contains("http://") || svg.matches("http://").count() == svg.matches("xmlns=\"http://www.w3.org/2000/svg\"").count(),
                "{name}/{scene}: an external address"
            );
        }
    }
}
