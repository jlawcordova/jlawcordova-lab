//! A small CSS evaluator for the CSS a scene's motion writes (ports the
//! `parseCss`, `styleOf` and `paintCss` helpers of cli/test/depth-motion.test.mjs):
//! the published tree with its CSS evaluated at a time, as a browser would.

use std::collections::HashMap;

use engine::engine::{Child, Group, Rendered};
use engine::iso::Pixel;
use engine::motion::loop_frame;

pub struct Stop {
    pub at: f64,
    pub decl: Vec<(String, String)>,
}

#[derive(Default)]
pub struct Css {
    pub keyframes: Vec<(String, Vec<Stop>)>,
    pub rules: Vec<(String, Vec<(String, String)>)>,
}

impl Css {
    pub fn keyframes(&self, name: &str) -> &[Stop] {
        &self.keyframes.iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("no keyframes {name}")).1
    }

    pub fn rule(&self, class: &str) -> Option<&Vec<(String, String)>> {
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
pub fn parse_css(css: &str) -> Css {
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

pub struct Style {
    pub opacity: f64,
    pub translate: [f64; 2],
}

/// A class's opacity and translation at `p` percent of the loop; with `animate: false`, with
/// animations off (reduced motion).
pub fn style_of(css: &Css, class: &str, p: f64, animate: bool) -> Style {
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
pub fn paint_css(scene: &Rendered, p: f64, duration: f64, animate: bool) -> HashMap<Pixel, String> {
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
