//! The one-line summary of an MCP tool call that the activity log keeps
//! (ports cli/lib/activity-summary.mjs): what the agent looked at or changed,
//! in a few words ("listed 15 scenes", "rendered hero-island at 2×"). Edits
//! already carry a summary; read commands get one here (first), else the
//! result's summary, else the first line of the command's human format.

use serde_json::{Map, Value};

use crate::registry::Command;

/// JavaScript's `\s` (and what `trim` strips).
fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'
    )
}

/// `r.key`, or `None` (undefined) when `r` is not an object or lacks it.
fn get<'a>(r: &'a Value, key: &str) -> Option<&'a Value> {
    r.as_object().and_then(|m| m.get(key))
}

/// `a ?? b` over possibly-missing values: null counts as missing.
fn nn(v: Option<&Value>) -> Option<&Value> {
    v.filter(|v| !v.is_null())
}

fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => engine::jsnum::as_f64(&Value::Number(n.clone())).is_some_and(|x| x != 0.0 && !x.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// `${v}` in a template string.
fn text(v: Option<&Value>) -> String {
    match v {
        None => "undefined".into(),
        Some(Value::Null) => "null".into(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => engine::jsnum::number_string(n),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(a)) => a.iter().map(|e| if e.is_null() { String::new() } else { text(Some(e)) }).collect::<Vec<_>>().join(","),
        Some(Value::Object(_)) => "[object Object]".into(),
    }
}

/// `plural(n, word, many)` where `n` is whatever the result held.
fn plural(n: Option<&Value>, word: &str, many: Option<&str>) -> String {
    let one = matches!(n, Some(Value::Number(x)) if engine::jsnum::as_f64(&Value::Number(x.clone())) == Some(1.0));
    let many = many.map(String::from).unwrap_or_else(|| format!("{word}s"));
    format!("{} {}", text(n), if one { word } else { many.as_str() })
}

fn plural_n(n: usize, word: &str) -> String {
    plural(Some(&Value::from(n)), word, None)
}

/// `count(v)`: the length of an array, else 0.
fn count(v: Option<&Value>) -> usize {
    v.and_then(Value::as_array).map_or(0, Vec::len)
}

/// `(r.frame ? ", frame …" : "")` style optional parts.
fn when(cond: bool, s: String) -> String {
    if cond {
        s
    } else {
        String::new()
    }
}

/// The read commands' summaries; `None` when the command has none.
fn read(name: &str, r: &Value) -> Option<String> {
    let g = |k: &str| get(r, k);
    Some(match name {
        "site.info" => {
            format!("site {}: {}, {}", text(g("root")), plural(g("scenes"), "scene", None), plural(g("objects"), "object", None))
        }
        "palette.get" => format!("listed {}", plural_n(count(g("colors")), "palette color")),
        "scene.list" => format!("listed {}", plural_n(count(g("scenes")), "scene")),
        "scene.show" => format!(
            "showed scene {} ({}{})",
            text(g("name")),
            plural(g("items"), "item", None),
            when(truthy(g("motion")), ", motion".into())
        ),
        "scene.items" => format!("listed {} of {}", plural_n(count(g("items")), "item"), text(g("scene"))),
        "scene.render" => format!(
            "rendered {} at {}×{}",
            text(g("scene")),
            text(g("scale")),
            when(truthy(g("time")), format!(", {} ms", text(g("time"))))
        ),
        "object.list" => format!("listed {}", plural_n(count(g("objects")), "object")),
        "object.show" => {
            format!("showed object {}{}", text(g("name")), when(truthy(g("description")), format!(" ({})", text(g("description")))))
        }
        "object.render" => format!(
            "rendered {} at {}×{}{}",
            text(g("object")),
            text(g("scale")),
            when(truthy(g("frame")), format!(", frame {}", text(g("frame")))),
            when(truthy(g("time")), format!(", {} ms", text(g("time"))))
        ),
        "object.pick" => format!(
            "picked {} at {},{}: {}",
            text(g("object")),
            text(g("x")),
            text(g("y")),
            nn(g("color")).map_or("empty".to_string(), |c| text(Some(c)))
        ),
        "check" => {
            let target = nn(g("target")).and_then(|t| nn(get(t, "name")));
            let problems = count(g("problems"));
            format!(
                "checked {} ({}): {}",
                target.map_or("every source".to_string(), |t| text(Some(t))),
                plural(Some(nn(g("checked")).unwrap_or(&Value::from(0))), "file", None),
                if problems > 0 { plural_n(problems, "problem") } else { "no problems".into() }
            )
        }
        "proposal.list" => format!("listed {}", plural_n(count(g("proposals")), "proposal")),
        "proposal.show" => format!("showed proposal {} ({}, {})", text(g("id")), text(g("doc")), plural_n(count(g("summary")), "edit")),
        "proposal.render" => format!("rendered proposal {} ({}) at {}×", text(g("id")), text(g("doc")), text(g("scale"))),
        "activity.list" => {
            format!("listed {}", plural(Some(&Value::from(count(g("entries")))), "activity entry", Some("activity entries")))
        }
        "doc.status" => {
            if truthy(g("docs")) {
                format!("{} open", plural_n(count(g("docs")), "doc"))
            } else {
                format!("{}: {}", text(g("doc")), if truthy(g("dirty")) { "unsaved changes" } else { "saved" })
            }
        }
        "doc.open" => format!("opened {}", text(g("doc"))),
        "chat.ask" => format!("asked: {}{}", text(g("question")), if truthy(g("asked")) { "" } else { " (no chat attached)" }),
        "chat.context" => {
            let target = if nn(g("scene")).is_some() { g("scene") } else { g("object") };
            let unsaved = match g("unsaved") {
                Some(Value::Array(a)) => a.len(),
                Some(Value::String(s)) => s.encode_utf16().count(),
                _ => 0,
            };
            format!(
                "built the selection block for {}{}",
                text(target),
                when(unsaved > 0, format!(" ({} not saved)", plural_n(unsaved, "target")))
            )
        }
        _ => return None,
    })
}

/// The activity line for a command's result.
pub fn activity_summary(cmd: &Command, result: &Value, params: &Map<String, Value>) -> String {
    if let Some(line) = read(cmd.name, result) {
        if !line.is_empty() {
            return line;
        }
    }
    if let Some(Value::String(s)) = get(result, "summary") {
        return s.clone();
    }
    let Some(format) = cmd.format else {
        return String::new();
    };
    let out = format(result, params);
    let first = out.split('\n').next().unwrap_or("");
    // replace(/\s+/g, ' ').trim().slice(0, 200), the slice in UTF-16 units
    let mut collapsed = String::new();
    let mut in_space = false;
    for c in first.chars() {
        if is_js_space(c) {
            in_space = true;
        } else {
            if in_space {
                collapsed.push(' ');
            }
            in_space = false;
            collapsed.push(c);
        }
    }
    if in_space {
        collapsed.push(' ');
    }
    let trimmed = collapsed.trim_matches(is_js_space);
    let mut units = 0;
    let mut end = trimmed.len();
    for (i, c) in trimmed.char_indices() {
        units += c.len_utf16();
        if units > 200 {
            end = i;
            break;
        }
    }
    trimmed[..end].to_string()
}
