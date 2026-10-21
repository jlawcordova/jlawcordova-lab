//! Layer tree → rect SVG (ports cli/lib/engine/svg.mjs; pixel-art engine
//! spec D6 step 3): in each layer there's one `<g fill>` per color, in order
//! of first paint, holding one `<rect>` per horizontal run, sorted by y and
//! then x. `compile::optimize_svg` turns it into paths.

use std::fmt::Write;

use crate::engine::{Child, Group, Layer, Rendered};
use crate::jsnum::number_string;
use crate::omap::OMap;
use serde_json::Value;

fn escape(v: &str) -> String {
    v.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;")
}

/// A scene with motion carries its CSS in a `<style>` first (`toRectSvg`).
/// `hex_of` maps a color name to "#RRGGBB".
pub fn to_rect_svg(scene: &Rendered, hex_of: &dyn Fn(&str) -> String) -> String {
    let view_box: Vec<String> = scene
        .view_box
        .iter()
        .map(|v| match v {
            Value::Number(n) => number_string(n),
            other => other.as_str().map(str::to_string).unwrap_or_else(|| other.to_string()),
        })
        .collect();
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" aria-hidden=\"true\" viewBox=\"{}\" shape-rendering=\"crispEdges\">",
        view_box.join(" ")
    );
    if let Some(style) = scene.style.as_deref().filter(|s| !s.is_empty()) {
        let _ = write!(out, "<style>{style}</style>");
    }
    children(&scene.root, hex_of, &mut out);
    out.push_str("</svg>");
    out
}

fn children(group: &Group, hex_of: &dyn Fn(&str) -> String, out: &mut String) {
    for child in &group.children {
        match child {
            Child::Layer(l) => layer(l, hex_of, out),
            Child::Group(g) => {
                out.push_str("<g");
                for (k, v) in &g.attrs {
                    let _ = write!(out, " {k}=\"{}\"", escape(v));
                }
                out.push('>');
                children(g, hex_of, out);
                out.push_str("</g>");
            }
        }
    }
}

fn layer(layer: &Layer, hex_of: &dyn Fn(&str) -> String, out: &mut String) {
    // Colors are grouped by value, so two names for one hex share a group.
    let mut by_hex: OMap<String, Vec<(i32, i32)>> = OMap::new();
    for name in layer.colors.iter() {
        by_hex.get_or_insert_with(hex_of(name), Vec::new);
    }
    for ((x, y), name) in layer.pixels.iter() {
        by_hex.get_or_insert_with(hex_of(name), Vec::new).push((*x, *y));
    }
    for (hex, points) in by_hex.iter() {
        if points.is_empty() {
            continue;
        }
        let mut points = points.clone();
        points.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
        let _ = write!(out, "<g fill=\"{hex}\">");
        let mut i = 0;
        while i < points.len() {
            let mut j = i;
            while j + 1 < points.len() && points[j + 1].1 == points[i].1 && points[j + 1].0 == points[j].0 + 1 {
                j += 1;
            }
            let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"1\"/>", points[i].0, points[i].1, j - i + 1);
            i = j + 1;
        }
        out.push_str("</g>");
    }
}
