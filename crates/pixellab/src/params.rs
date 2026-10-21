//! The param checker every door shares: a subset of JSON Schema, with coercion from the command line (ports cli/lib/params.mjs).
//!
//! A command's params are a JSON Schema object; this checks a small,
//! well-defined subset of it: type (string, integer, number, boolean, array,
//! object), enum, pattern, minimum, maximum, minItems, maxItems, items,
//! required, default and description. Unknown params are refused. With
//! `coerce`, strings (from the command line) become the declared type: "3" →
//! 3, "true"/"false" → booleans, "14,6,1" → [14, 6, 1], and a JSON string → an
//! object.

#![allow(clippy::result_large_err)]

use engine::format::quote;
use engine::jsnum::{self, num};
use engine::serialize::{entries, keys};
use serde_json::{Map, Value};

use crate::errors::CommandError;

const TYPES: [&str; 6] = ["string", "integer", "number", "boolean", "array", "object"];

/// Checks (and with `coerce`, converts) params against a command's schema.
/// Returns a new object with defaults filled in. Fails with one usage error
/// that lists every problem (its `details` is the list).
pub fn check_params(schema: &Value, input: &Value, coerce: bool, command: Option<&str>) -> Result<Map<String, Value>, CommandError> {
    let mut problems: Vec<String> = Vec::new();
    let empty = Map::new();
    let props = schema.get("properties").and_then(Value::as_object).unwrap_or(&empty);
    let none = Map::new();
    let given = match input {
        Value::Null => &none,
        Value::Object(map) => map,
        _ => return Err(CommandError::usage(format!("{}: params must be an object", command.unwrap_or("params")))),
    };
    let required: Vec<&str> =
        schema.get("required").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let mut out = Map::new();
    for key in keys(given) {
        if !props.contains_key(key.as_str()) {
            let names: Vec<&String> = keys(props);
            problems.push(format!("unknown param {key}{}", suggest(key, &names)));
        }
    }
    for (key, prop) in entries(props) {
        let Some(mut value) = given.get(key.as_str()).cloned() else {
            if required.contains(&key.as_str()) {
                problems.push(format!("{key} is required"));
            } else if let Some(default) = prop.get("default") {
                out.insert(key.clone(), default.clone());
            }
            continue;
        };
        if coerce {
            value = coerce_value(prop, &value);
        }
        let before = problems.len();
        check_value(prop, &value, key, &mut problems)?;
        if problems.len() == before {
            out.insert(key.clone(), value);
        }
    }
    if !problems.is_empty() {
        let prefix = command.map(|c| format!("{c}: ")).unwrap_or_default();
        let details = Value::Array(problems.iter().map(|p| Value::String(p.clone())).collect());
        return Err(CommandError::usage(format!("{prefix}{}", problems.join("; "))).with_details(details));
    }
    Ok(out)
}

/// What a definition's mistake throws (a plain `Error` in the Node CLI).
fn internal(message: String) -> CommandError {
    CommandError::new(message, "internal")
}

fn num_string(v: &Value) -> String {
    match v {
        Value::Number(n) => jsnum::number_string(n),
        _ => String::new(),
    }
}

/// `String(value)` for a JSON value, as JavaScript prints it.
fn js_string(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => jsnum::number_string(n),
        Value::String(s) => s.clone(),
        Value::Array(a) => join_values(a, ","),
        Value::Object(_) => "[object Object]".into(),
    }
}

/// `array.join(sep)`: null prints as nothing.
fn join_values(list: &[Value], sep: &str) -> String {
    list.iter().map(|v| if v.is_null() { String::new() } else { js_string(v) }).collect::<Vec<_>>().join(sep)
}

/// `show(v)`: strings quoted, lists bracketed, anything else as `String(v)`.
fn show(v: &Value) -> String {
    match v {
        Value::String(s) => quote(s),
        Value::Array(a) => format!("[{}]", a.iter().map(show).collect::<Vec<_>>().join(", ")),
        other => js_string(other),
    }
}

/// `Array.prototype.includes` (SameValueZero) for JSON values.
fn includes(list: &[Value], value: &Value) -> bool {
    list.iter().any(|item| match (item, value) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Value::Array(_), _) | (Value::Object(_), _) => false,
        (a, b) => a == b,
    })
}

fn is_plain_object(v: &Value) -> bool {
    v.is_object()
}

fn check_value(schema: &Value, value: &Value, whence: &str, problems: &mut Vec<String>) -> Result<(), CommandError> {
    let ty = schema.get("type").and_then(Value::as_str);
    if let Some(t) = ty {
        if !TYPES.contains(&t) {
            return Err(internal(format!("schema for {whence}: unsupported type {t}")));
        }
    }
    let is_whole = matches!(value, Value::Number(n) if n.as_f64().is_some_and(jsnum::is_integer));
    match ty {
        Some("integer") if !is_whole => {
            problems.push(format!("{whence} must be a whole number, got {}", show(value)));
            return Ok(());
        }
        Some("number") if !value.is_number() => {
            problems.push(format!("{whence} must be a number, got {}", show(value)));
            return Ok(());
        }
        Some("string") if !value.is_string() => {
            problems.push(format!("{whence} must be a string, got {}", show(value)));
            return Ok(());
        }
        Some("boolean") if !value.is_boolean() => {
            problems.push(format!("{whence} must be true or false, got {}", show(value)));
            return Ok(());
        }
        Some("array") if !value.is_array() => {
            problems.push(format!("{whence} must be a list, got {}", show(value)));
            return Ok(());
        }
        Some("object") if !is_plain_object(value) => {
            problems.push(format!("{whence} must be an object, got {}", show(value)));
            return Ok(());
        }
        _ => {}
    }
    if let Some(options) = schema.get("enum").and_then(Value::as_array) {
        if !includes(options, value) {
            problems.push(format!("{whence} must be one of {}, got {}", join_values(options, ", "), show(value)));
            return Ok(());
        }
    }
    if let Value::Number(n) = value {
        let x = n.as_f64().unwrap_or(f64::NAN);
        if let Some(min) = schema.get("minimum").filter(|m| m.is_number()) {
            if x < min.as_f64().unwrap_or(f64::NAN) {
                problems.push(format!("{whence} must be at least {}, got {}", num_string(min), num_string(value)));
            }
        }
        if let Some(max) = schema.get("maximum").filter(|m| m.is_number()) {
            if x > max.as_f64().unwrap_or(f64::NAN) {
                problems.push(format!("{whence} must be at most {}, got {}", num_string(max), num_string(value)));
            }
        }
    }
    if let (Value::String(text), Some(pattern)) = (value, schema.get("pattern").and_then(Value::as_str)) {
        if !pattern.is_empty() {
            let matched = rx::test(pattern, text).map_err(|e| internal(format!("Invalid regular expression: /{pattern}/: {e}")))?;
            if !matched {
                problems.push(format!("{whence} {} doesn't match {pattern}", show(value)));
            }
        }
    }
    if let Value::Array(list) = value {
        let len = list.len() as f64;
        if let Some(min) = schema.get("minItems").filter(|m| m.is_number()) {
            if len < min.as_f64().unwrap_or(f64::NAN) {
                problems.push(format!("{whence} needs at least {} items, got {}", num_string(min), list.len()));
            }
        }
        if let Some(max) = schema.get("maxItems").filter(|m| m.is_number()) {
            if len > max.as_f64().unwrap_or(f64::NAN) {
                problems.push(format!("{whence} takes at most {} items, got {}", num_string(max), list.len()));
            }
        }
        if let Some(items) = schema.get("items").filter(|i| i.is_object()) {
            for (i, v) in list.iter().enumerate() {
                check_value(items, v, &format!("{whence}[{i}]"), problems)?;
            }
        }
    }
    if ty == Some("object") {
        if let (Some(properties), Value::Object(obj)) = (schema.get("properties").and_then(Value::as_object), value) {
            for req in schema.get("required").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
                if !obj.contains_key(req) {
                    problems.push(format!("{whence}.{req} is required"));
                }
            }
            for (key, v) in entries(obj) {
                if let Some(prop) = properties.get(key.as_str()) {
                    check_value(prop, v, &format!("{whence}.{key}"), problems)?;
                } else if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
                    problems.push(format!("{whence}: unknown property {key}"));
                }
            }
        }
    }
    Ok(())
}

/// JavaScript's whitespace (`String.prototype.trim`, regex `\s`).
fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'
    )
}

fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_space)
}

/// `/^[-+]?\d+$/`.
fn is_integer_text(t: &str) -> bool {
    let digits = t.strip_prefix(['-', '+']).unwrap_or(t);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// `/^[-+]?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$/i`.
fn is_number_text(t: &str) -> bool {
    let t = t.strip_prefix(['-', '+']).unwrap_or(t);
    let (mantissa, exponent) = match t.find(['e', 'E']) {
        Some(i) => (&t[..i], Some(&t[i + 1..])),
        None => (t, None),
    };
    if let Some(e) = exponent {
        let digits = e.strip_prefix(['-', '+']).unwrap_or(e);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    let (whole, fraction) = match mantissa.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (mantissa, None),
    };
    let all_digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if !all_digits(whole) || !fraction.is_none_or(all_digits) {
        return false;
    }
    // \d+\.?\d* or \.\d+
    !whole.is_empty() || fraction.is_some_and(|f| !f.is_empty())
}

/// Converts a command-line string (or list of strings) to the schema's type.
/// Anything it can't convert is returned as it is, for `check_value` to
/// report. (A number text that overflows to Infinity is also returned as it
/// is: a JSON value can't hold Infinity.)
pub fn coerce_value(schema: &Value, value: &Value) -> Value {
    match schema.get("type").and_then(Value::as_str) {
        Some(ty @ ("integer" | "number")) => {
            let Value::String(s) = value else { return value.clone() };
            let text = js_trim(s);
            let ok = if ty == "integer" { is_integer_text(text) } else { is_number_text(text) };
            if !ok {
                return value.clone();
            }
            match text.parse::<f64>() {
                Ok(x) if x.is_finite() => num(x),
                _ => value.clone(),
            }
        }
        Some("boolean") => {
            let Value::String(s) = value else { return value.clone() };
            let text = js_trim(s).to_lowercase();
            if ["true", "yes", "1", "on"].contains(&text.as_str()) {
                Value::Bool(true)
            } else if ["false", "no", "0", "off"].contains(&text.as_str()) {
                Value::Bool(false)
            } else {
                value.clone()
            }
        }
        Some("array") => {
            let list: Vec<Value> = match value {
                Value::String(_) => vec![value.clone()],
                Value::Array(a) => a.clone(),
                _ => return value.clone(),
            };
            let items = schema.get("items").cloned().unwrap_or_else(|| Value::Object(Map::new()));
            // A list of lists ([[3, 4], [5, 6]]): "3,4 5,6" or "3,4;5,6", or
            // the flag (or positional) repeated, one inner list each.
            if items.get("type").and_then(Value::as_str) == Some("array") {
                let mut out = Vec::new();
                for v in &list {
                    match v {
                        Value::String(s) => {
                            for group in s.split(|c: char| is_js_space(c) || c == ';').filter(|g| !g.is_empty()) {
                                out.push(coerce_value(&items, &Value::String(group.to_string())));
                            }
                        }
                        other => out.push(coerce_value(&items, other)),
                    }
                }
                return Value::Array(out);
            }
            // "14,6,1" or repeated flags; strings split on commas.
            let mut flat: Vec<Value> = Vec::new();
            for v in &list {
                match v {
                    Value::String(s) if s.is_empty() => {}
                    Value::String(s) => flat.extend(s.split(',').map(|p| Value::String(js_trim(p).to_string()))),
                    other => flat.push(other.clone()),
                }
            }
            Value::Array(flat.iter().map(|v| coerce_value(&items, v)).collect())
        }
        Some("object") => match value {
            Value::String(s) => serde_json::from_str::<Value>(s).unwrap_or_else(|_| value.clone()),
            _ => value.clone(),
        },
        _ => value.clone(),
    }
}

/// " (did you mean x?)" for a close name, else "".
pub fn suggest<S: AsRef<str>>(word: &str, options: &[S]) -> String {
    let mut best = "";
    let mut best_score = usize::MAX;
    for option in options {
        let d = distance(word, option.as_ref());
        if d < best_score {
            best = option.as_ref();
            best_score = d;
        }
    }
    let length = word.encode_utf16().count();
    if !best.is_empty() && best_score <= 2.max(length / 3) {
        format!(" (did you mean {best}?)")
    } else {
        String::new()
    }
}

/// Levenshtein distance (over UTF-16 code units, as JavaScript indexes).
fn distance(a: &str, b: &str) -> usize {
    let a: Vec<u16> = a.encode_utf16().collect();
    let b: Vec<u16> = b.encode_utf16().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut prev = row[0];
        row[0] = i;
        for j in 1..=b.len() {
            let tmp = row[j];
            row[j] = (row[j] + 1).min(row[j - 1] + 1).min(prev + usize::from(a[i - 1] != b[j - 1]));
            prev = tmp;
        }
    }
    row[b.len()]
}

/// A small backtracking regex matcher for the patterns the registry's
/// schemas use (and a little more): literals, `.`, escapes (`\d \D \w \W \s
/// \S` and escaped punctuation), classes `[...]`, groups `(...)` and
/// `(?:...)`, alternation, `* + ?` and `{n,m}` (greedy), and the `^` and `$`
/// anchors. Anything else (lazy quantifiers, lookahead, back-references,
/// `\b`) is an error, never a silent pass. Unanchored, like `RegExp.test`.
mod rx {
    enum Item {
        Range(char, char),
        Digit(bool),
        Word(bool),
        Space(bool),
    }

    enum Node {
        Char(char),
        Any,
        Class(Vec<Item>, bool),
        Start,
        End,
        Alt(Vec<Vec<Node>>),
        Repeat(Box<Node>, usize, Option<usize>),
    }

    struct Parser {
        chars: Vec<char>,
        pos: usize,
    }

    impl Parser {
        fn peek(&self) -> Option<char> {
            self.chars.get(self.pos).copied()
        }

        fn alt(&mut self, depth: usize) -> Result<Node, String> {
            let mut branches = vec![self.seq(depth)?];
            while self.peek() == Some('|') {
                self.pos += 1;
                branches.push(self.seq(depth)?);
            }
            Ok(Node::Alt(branches))
        }

        fn seq(&mut self, depth: usize) -> Result<Vec<Node>, String> {
            let mut out = Vec::new();
            while let Some(c) = self.peek() {
                if c == '|' || (c == ')' && depth > 0) {
                    break;
                }
                let atom = self.atom(depth)?;
                out.push(self.quantified(atom)?);
            }
            Ok(out)
        }

        fn quantified(&mut self, atom: Node) -> Result<Node, String> {
            let (min, max) = match self.peek() {
                Some('*') => (0, None),
                Some('+') => (1, None),
                Some('?') => (0, Some(1)),
                Some('{') => return self.braces(atom),
                _ => return Ok(atom),
            };
            self.pos += 1;
            self.no_lazy()?;
            Ok(Node::Repeat(Box::new(atom), min, max))
        }

        fn no_lazy(&self) -> Result<(), String> {
            if matches!(self.peek(), Some('?' | '+' | '*' | '{')) {
                return Err("lazy or nested quantifiers are not supported".into());
            }
            Ok(())
        }

        fn braces(&mut self, atom: Node) -> Result<Node, String> {
            let close = self.chars[self.pos..].iter().position(|c| *c == '}').ok_or("unterminated {")?;
            let body: String = self.chars[self.pos + 1..self.pos + close].iter().collect();
            let bad = || format!("unsupported quantifier {{{body}}}");
            let (min, max) = match body.split_once(',') {
                None => {
                    let n: usize = body.parse().map_err(|_| bad())?;
                    (n, Some(n))
                }
                Some((a, "")) => (a.parse().map_err(|_| bad())?, None),
                Some((a, b)) => (a.parse().map_err(|_| bad())?, Some(b.parse().map_err(|_| bad())?)),
            };
            if max.is_some_and(|m| m < min) {
                return Err("numbers out of order in {} quantifier".into());
            }
            self.pos += close + 1;
            self.no_lazy()?;
            Ok(Node::Repeat(Box::new(atom), min, max))
        }

        fn atom(&mut self, depth: usize) -> Result<Node, String> {
            let c = self.peek().ok_or("unexpected end")?;
            self.pos += 1;
            match c {
                '^' => Ok(Node::Start),
                '$' => Ok(Node::End),
                '.' => Ok(Node::Any),
                '(' => {
                    if self.peek() == Some('?') {
                        if self.chars.get(self.pos + 1) == Some(&':') {
                            self.pos += 2;
                        } else {
                            return Err("lookahead, lookbehind and named groups are not supported".into());
                        }
                    }
                    let inner = self.alt(depth + 1)?;
                    if self.peek() != Some(')') {
                        return Err("unterminated group".into());
                    }
                    self.pos += 1;
                    Ok(inner)
                }
                ')' => Err("unmatched )".into()),
                '[' => self.class(),
                '\\' => {
                    let e = self.peek().ok_or("\\ at end of pattern")?;
                    self.pos += 1;
                    Ok(match e {
                        'd' => Node::Class(vec![Item::Digit(true)], false),
                        'D' => Node::Class(vec![Item::Digit(false)], false),
                        'w' => Node::Class(vec![Item::Word(true)], false),
                        'W' => Node::Class(vec![Item::Word(false)], false),
                        's' => Node::Class(vec![Item::Space(true)], false),
                        'S' => Node::Class(vec![Item::Space(false)], false),
                        'n' => Node::Char('\n'),
                        't' => Node::Char('\t'),
                        'r' => Node::Char('\r'),
                        e if e.is_ascii_alphanumeric() => return Err(format!("unsupported escape \\{e}")),
                        e => Node::Char(e),
                    })
                }
                '*' | '+' | '?' => Err("nothing to repeat".into()),
                '{' | '}' | ']' => Ok(Node::Char(c)),
                c => Ok(Node::Char(c)),
            }
        }

        fn class(&mut self) -> Result<Node, String> {
            let negate = self.peek() == Some('^');
            if negate {
                self.pos += 1;
            }
            let mut items = Vec::new();
            loop {
                let c = self.peek().ok_or("unterminated character class")?;
                self.pos += 1;
                if c == ']' {
                    break;
                }
                let lo = if c == '\\' {
                    let e = self.peek().ok_or("\\ at end of pattern")?;
                    self.pos += 1;
                    match e {
                        'd' => {
                            items.push(Item::Digit(true));
                            continue;
                        }
                        'w' => {
                            items.push(Item::Word(true));
                            continue;
                        }
                        's' => {
                            items.push(Item::Space(true));
                            continue;
                        }
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        e if e.is_ascii_alphanumeric() => return Err(format!("unsupported escape \\{e} in class")),
                        e => e,
                    }
                } else {
                    c
                };
                if self.peek() == Some('-') && self.chars.get(self.pos + 1).is_some_and(|n| *n != ']') {
                    self.pos += 1;
                    let mut hi = self.peek().ok_or("unterminated character class")?;
                    self.pos += 1;
                    if hi == '\\' {
                        hi = self.peek().ok_or("\\ at end of pattern")?;
                        self.pos += 1;
                        if hi.is_ascii_alphanumeric() {
                            return Err("unsupported range end".into());
                        }
                    }
                    if hi < lo {
                        return Err("range out of order in character class".into());
                    }
                    items.push(Item::Range(lo, hi));
                } else {
                    items.push(Item::Range(lo, lo));
                }
            }
            Ok(Node::Class(items, negate))
        }
    }

    fn is_word(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '_'
    }

    fn is_space(c: char) -> bool {
        super::is_js_space(c) || c == '\u{2028}' || c == '\u{2029}'
    }

    fn in_item(item: &Item, c: char) -> bool {
        match item {
            Item::Range(lo, hi) => (*lo..=*hi).contains(&c),
            Item::Digit(yes) => c.is_ascii_digit() == *yes,
            Item::Word(yes) => is_word(c) == *yes,
            Item::Space(yes) => is_space(c) == *yes,
        }
    }

    type Cont<'a> = &'a mut dyn FnMut(usize) -> bool;

    fn step(node: &Node, text: &[char], pos: usize, k: Cont) -> bool {
        match node {
            Node::Char(c) => text.get(pos) == Some(c) && k(pos + 1),
            Node::Any => text.get(pos).is_some_and(|c| !matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')) && k(pos + 1),
            Node::Class(items, negate) => text.get(pos).is_some_and(|c| items.iter().any(|i| in_item(i, *c)) != *negate) && k(pos + 1),
            Node::Start => pos == 0 && k(pos),
            Node::End => pos == text.len() && k(pos),
            Node::Alt(branches) => branches.iter().any(|b| seq(b, text, pos, k)),
            Node::Repeat(inner, min, max) => repeat(inner, *min, *max, 0, text, pos, k),
        }
    }

    fn seq(nodes: &[Node], text: &[char], pos: usize, k: Cont) -> bool {
        match nodes.split_first() {
            None => k(pos),
            Some((first, rest)) => step(first, text, pos, &mut |p| seq(rest, text, p, k)),
        }
    }

    fn repeat(node: &Node, min: usize, max: Option<usize>, count: usize, text: &[char], pos: usize, k: Cont) -> bool {
        if max.is_none_or(|m| count < m) {
            let more = step(node, text, pos, &mut |p| {
                if p == pos && count >= min {
                    return false;
                }
                repeat(node, min, max, count + 1, text, p, k)
            });
            if more {
                return true;
            }
        }
        count >= min && k(pos)
    }

    /// `new RegExp(pattern).test(text)`.
    pub fn test(pattern: &str, text: &str) -> Result<bool, String> {
        let mut parser = Parser { chars: pattern.chars().collect(), pos: 0 };
        let root = parser.alt(0)?;
        if parser.pos < parser.chars.len() {
            return Err("unmatched )".into());
        }
        let chars: Vec<char> = text.chars().collect();
        Ok((0..=chars.len()).any(|start| step(&root, &chars, start, &mut |_| true)))
    }
}

#[cfg(test)]
mod tests {
    use super::rx::test;

    #[test]
    fn the_registry_pattern() {
        let p = "^\\d+(\\.\\d+)*$";
        for ok in ["5", "5.2", "12.0.3"] {
            assert!(test(p, ok).unwrap(), "{ok}");
        }
        for bad in ["", "x", "5.", ".5", "5..2", "5.2x", "5\n"] {
            assert!(!test(p, bad).unwrap(), "{bad:?}");
        }
    }

    #[test]
    fn other_constructs_and_refusals() {
        assert!(test("a|b+", "xbb").unwrap());
        assert!(test("^[a-c]{2,3}$", "abc").unwrap());
        assert!(!test("^[^a-c]$", "b").unwrap());
        assert!(test("^(?:ab)?c$", "c").unwrap());
        assert!(test("(?=a)", "a").is_err());
        assert!(test("a+?", "a").is_err());
        assert!(test("\\bx", "x").is_err());
        assert!(test("(a", "a").is_err());
    }
}
