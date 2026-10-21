//! Pieces the commands share (ports cli/lib/commands/common.mjs): doc
//! references, the output of a change, refusing params given together, and
//! small helpers for porting the commands' JavaScript (`String(v)`,
//! `padEnd`, reading params).

use serde_json::{json, Map, Value};

use crate::context::{Ctx, DocKind};
use crate::errors::{CommandError, Result};
use crate::registry::Params;

/// A doc param: scene:<name>, object:<name>, or a name find() resolves (`DOC`).
pub fn doc_param() -> Value {
    json!({ "type": "string", "description": "the doc: scene:<name>, object:<name>, or a name (scenes/<name>, objects/<name>)" })
}

/// A doc found by reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocRef {
    pub kind: DocKind,
    pub name: String,
    pub ref_: String,
}

/// Resolves a doc reference: scene:<name>, object:<name>, or a name (`resolveDoc`).
pub fn resolve_doc(ctx: &mut Ctx, query: &str) -> Result<DocRef> {
    let found = match query.split_once(':') {
        Some((k @ ("scene" | "object"), name)) if !name.is_empty() => ctx.find(name, DocKind::parse(k))?,
        _ => ctx.find(query, None)?,
    };
    let ref_ = format!("{}:{}", found.kind.as_str(), found.name);
    Ok(DocRef { kind: found.kind, name: found.name, ref_ })
}

/// Human output for a change: its summary (`formatChange`).
pub fn format_change(r: &Value, _: &Params) -> String {
    str_of(r.get("summary"))
}

/// Refuses when none of `keys` is given (`needOne`).
pub fn need_one(params: &Params, keys: &[&str], command: &str) -> Result<()> {
    if keys.iter().any(|k| params.get(*k).is_some_and(|v| !v.is_null())) {
        return Ok(());
    }
    let (last, rest) = keys.split_last().expect("keys");
    let list: Vec<String> = rest.iter().map(|k| format!("--{k}")).collect();
    Err(CommandError::usage(format!("{command}: give {} or --{last}", list.join(", "))).into())
}

/// Refuses when more than one of `keys` is given (`atMostOne`).
pub fn at_most_one(params: &Params, keys: &[&str], command: &str) -> Result<()> {
    let given: Vec<String> = keys.iter().filter(|k| params.get(**k).is_some_and(|v| !v.is_null())).map(|k| format!("--{k}")).collect();
    if given.len() > 1 {
        return Err(CommandError::usage(format!("{command}: give only one of {}", given.join(", "))).into());
    }
    Ok(())
}

/// `String(v)` (`undefined` for None).
pub fn str_of(v: Option<&Value>) -> String {
    engine::engine::js_string(v)
}

/// A string param, if given.
pub fn opt_str<'a>(params: &'a Params, key: &str) -> Option<&'a str> {
    params.get(key).and_then(Value::as_str)
}

/// A number param, if given.
pub fn opt_num(params: &Params, key: &str) -> Option<f64> {
    params.get(key).and_then(Value::as_f64)
}

/// A boolean param (false when absent).
pub fn flag(params: &Params, key: &str) -> bool {
    params.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// `s.padEnd(w)`, counting UTF-16 units as JavaScript does.
pub fn pad_end(s: &str, w: usize) -> String {
    let n = engine::engine::js_len(s);
    if n >= w {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(w - n))
    }
}

/// `s.padStart(w)`.
pub fn pad_start(s: &str, w: usize) -> String {
    let n = engine::engine::js_len(s);
    if n >= w {
        s.to_string()
    } else {
        format!("{}{s}", " ".repeat(w - n))
    }
}

/// `list.join(sep)` of JSON values as JavaScript prints them.
pub fn join(list: Option<&Value>, sep: &str) -> String {
    list.and_then(Value::as_array).map(|l| l.iter().map(|v| str_of(Some(v))).collect::<Vec<_>>().join(sep)).unwrap_or_default()
}

/// An object from pairs, in order.
pub fn obj(pairs: Vec<(&str, Value)>) -> Map<String, Value> {
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}
