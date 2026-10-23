//! Categories, object libraries (`.pixellib`) and palette files (`.pixelpal`)
//! (docs/intents/2026-10-object-libraries/spec.md, D1–D6).
//!
//! This file holds what the commands share: the category helpers, the two
//! file kinds (read, validated, written canonically) and the import and
//! palette plans, as pure functions over data. The commands in
//! `commands/library.rs` and `commands/palette.rs` do the I/O and the edits.

use serde_json::Value;

use engine::engine::category_path_problem;

// ------------------------------------------------------------ categories

/// The separators a typed category path may use (spec R4): `›` or `/`.
const SEPARATORS: [char; 2] = ['›', '/'];

/// Splits typed text into category names: "Nature / Trees" and
/// "Nature › Trees" are `["Nature", "Trees"]`. Spaces around a separator
/// don't count (a name has none at its ends, spec R2).
pub fn split_category(text: &str) -> Vec<String> {
    text.split(SEPARATORS).map(|n| n.trim().to_string()).collect()
}

/// A category from a param: a list of names or of typed paths, joined in
/// order. `["Nature", "Trees"]` and `["Nature/Trees"]` are the same path.
/// Refuses what spec R2 refuses, saying why.
pub fn category_from_param(value: &Value) -> Result<Vec<String>, String> {
    let parts: Vec<&str> = match value {
        Value::String(s) => vec![s.as_str()],
        Value::Array(list) => {
            let mut parts = Vec::new();
            for v in list {
                parts.push(v.as_str().ok_or_else(|| format!("a category name must be text, got {v}"))?);
            }
            parts
        }
        other => return Err(format!("a category is a list of names, got {other}")),
    };
    let names: Vec<String> = parts.into_iter().flat_map(split_category).collect();
    match category_path_problem(&names) {
        Some(why) => Err(why),
        None => Ok(names),
    }
}

/// "Nature › Trees".
pub fn category_text(names: &[String]) -> String {
    names.join(" › ")
}

/// An object source's category, if it has a well-formed one.
pub fn category_of(doc: &Value) -> Option<Vec<String>> {
    let list = doc.get("category")?.as_array()?;
    let names: Vec<String> = list.iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
    (!names.is_empty() && names.len() == list.len()).then_some(names)
}

/// Whether `category` is `prefix` or below it, without regard to case (spec R9).
pub fn category_within(category: &[String], prefix: &[String]) -> bool {
    category.len() >= prefix.len() && category.iter().zip(prefix).all(|(a, b)| a.to_lowercase() == b.to_lowercase())
}
