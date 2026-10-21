//! Format 1 → 2 (docs/intents/2026-10-rust-engine/spec.md, R8, D3): reads the
//! subset of JavaScript that `serialize.mjs` wrote and the owner's palette
//! uses — `//` comments, `export default` and one object literal, identifier
//! or quoted keys, single- or double-quoted strings with `\\`, `\'` and `\"`
//! escapes, numbers, `true`, `false`, `null`, arrays and nested objects, and
//! trailing commas — and carries comments into notes. Anything else is
//! refused with its file, line and column.
//!
//! Comments go to notes:
//! - the palette's header lines → `notes`;
//! - a comment after a palette entry, on its line → `colorNotes[name]`;
//! - the generated header line ("Pixel-art object. How to edit it: …") → dropped;
//! - any other comment → the file's `notes`, prefixed with the path it sat
//!   at (`items[3]: …`); a header comment of an object or scene has none.

use serde_json::{Map, Value};

use crate::jsnum;

/// Which source a file is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Palette,
    Object,
    Scene,
}

impl SourceKind {
    /// The kind of a source by its file: `palette.mjs`, `objects/…`, `scenes/…`.
    pub fn of(file: &str) -> SourceKind {
        if file.ends_with("palette.mjs") || file.ends_with("palette.json") {
            SourceKind::Palette
        } else if file.contains("objects/") {
            SourceKind::Object
        } else {
            SourceKind::Scene
        }
    }
}

/// A comment and where it sat.
#[derive(Clone, Debug, PartialEq)]
pub struct Comment {
    /// The text after `//` (and one space).
    pub text: String,
    /// The path of the value it belongs to: the value it trails, or the next
    /// value in its list or record (the list's or record's own path when
    /// none follows). Empty before `export default`.
    pub path: Vec<Step>,
    /// It follows a value on that value's line.
    pub trailing: bool,
    /// It comes before `export default`.
    pub header: bool,
}

/// A step of a path: a record's key or a list's index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Key(String),
    Index(usize),
}

/// A path as a note prefixes it: `items[3].at`, `world.soil-3`.
pub fn path_text(path: &[Step]) -> String {
    let mut out = String::new();
    for step in path {
        match step {
            Step::Key(k) => {
                if !out.is_empty() {
                    out.push('.');
                }
                out.push_str(k);
            }
            Step::Index(i) => out.push_str(&format!("[{i}]")),
        }
    }
    out
}

struct Reader<'a> {
    file: &'a str,
    src: &'a [u8],
    text: &'a str,
    at: usize,
    comments: Vec<Comment>,
    /// The line of the last value that ended, and its path, for trailing comments.
    last_value: Option<(usize, Vec<Step>)>,
}

impl<'a> Reader<'a> {
    fn position(&self, at: usize) -> (usize, usize) {
        let before = &self.text[..at.min(self.text.len())];
        let line = before.matches('\n').count() + 1;
        let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
        (line, col)
    }

    fn error(&self, at: usize, what: &str) -> String {
        let (line, col) = self.position(at);
        format!("{}:{line}:{col}: {what}", self.file)
    }

    fn line_of(&self, at: usize) -> usize {
        self.position(at).0
    }

    fn what_is_here(&self) -> String {
        let rest = &self.text[self.at..];
        if rest.is_empty() {
            return "the end of the file".into();
        }
        let word: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$').collect();
        if !word.is_empty() {
            return format!("'{word}'");
        }
        format!("'{}'", rest.chars().next().expect("not empty"))
    }

    /// Skips whitespace and comments; a comment is kept with the path of
    /// what comes next (`next`), or as trailing the last value on its line.
    fn skip(&mut self, next: &[Step], header: bool) -> Result<(), String> {
        loop {
            while self.at < self.src.len() && (self.src[self.at] as char).is_ascii_whitespace() {
                self.at += 1;
            }
            if self.src[self.at..].starts_with(b"//") {
                let start = self.at;
                let end = self.text[start..].find('\n').map_or(self.src.len(), |i| start + i);
                let raw = &self.text[start + 2..end];
                let text = raw.strip_prefix(' ').unwrap_or(raw).trim_end_matches('\r').to_string();
                let line = self.line_of(start);
                let trailing = self.last_value.as_ref().filter(|(l, _)| *l == line);
                let comment = match trailing {
                    Some((_, path)) => Comment { text, path: path.clone(), trailing: true, header },
                    None => Comment { text, path: next.to_vec(), trailing: false, header },
                };
                self.comments.push(comment);
                self.at = end;
                continue;
            }
            if self.src[self.at..].starts_with(b"/*") {
                return Err(self.error(self.at, "block comments (/* */) aren't supported; use // comments"));
            }
            return Ok(());
        }
    }

    fn eat(&mut self, c: u8) -> bool {
        if self.src.get(self.at) == Some(&c) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn word(&mut self) -> Option<String> {
        let start = self.at;
        if !self.src.get(self.at).is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_' || *b == b'$') {
            return None;
        }
        while self.src.get(self.at).is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'$') {
            self.at += 1;
        }
        Some(self.text[start..self.at].to_string())
    }

    fn string(&mut self) -> Result<String, String> {
        let quote = self.src[self.at] as char;
        let start = self.at;
        self.at += 1;
        let mut out = String::new();
        loop {
            let Some(c) = self.text[self.at..].chars().next() else { return Err(self.error(start, "a string that never ends")) };
            match c {
                '\n' | '\r' => return Err(self.error(start, "a string that never ends on its line")),
                '\\' => match self.text[self.at + 1..].chars().next() {
                    Some(e @ ('\\' | '\'' | '"')) => {
                        out.push(e);
                        self.at += 2;
                    }
                    _ => return Err(self.error(self.at, "an escape other than \\\\, \\' or \\\" isn't supported")),
                },
                c if c == quote => {
                    self.at += 1;
                    return Ok(out);
                }
                c => {
                    out.push(c);
                    self.at += c.len_utf8();
                }
            }
        }
    }

    fn number(&mut self) -> Result<Value, String> {
        let start = self.at;
        self.eat(b'-');
        let digits = |r: &mut Self| {
            let s = r.at;
            while r.src.get(r.at).is_some_and(u8::is_ascii_digit) {
                r.at += 1;
            }
            r.at - s
        };
        let mut n = digits(self);
        if self.eat(b'.') {
            n += digits(self);
        }
        if n == 0 {
            return Err(self.error(start, "expected a number"));
        }
        if self.src.get(self.at).is_some_and(|b| *b == b'e' || *b == b'E') {
            self.at += 1;
            if !self.eat(b'+') {
                self.eat(b'-');
            }
            if digits(self) == 0 {
                return Err(self.error(start, "expected a number"));
            }
        }
        let x = jsnum::parse(&self.text[start..self.at]);
        if !x.is_finite() {
            return Err(self.error(start, "expected a finite number"));
        }
        Ok(jsnum::num(x))
    }

    fn value(&mut self, path: &mut Vec<Step>) -> Result<Value, String> {
        self.skip(path, false)?;
        let start = self.at;
        let value = match self.src.get(self.at) {
            Some(b'{') => self.record(path)?,
            Some(b'[') => self.list(path)?,
            Some(b'\'' | b'"') => Value::String(self.string()?),
            Some(b'-' | b'.' | b'0'..=b'9') => self.number()?,
            _ => match self.word().as_deref() {
                Some("true") => Value::Bool(true),
                Some("false") => Value::Bool(false),
                Some("null") => Value::Null,
                Some(w) => {
                    return Err(self.error(
                        start,
                        &format!("'{w}' isn't a value this format reads (only strings, numbers, true, false, lists and records)"),
                    ))
                }
                None => return Err(self.error(start, &format!("expected a value, found {}", self.what_is_here()))),
            },
        };
        self.last_value = Some((self.line_of(self.at.saturating_sub(1)), path.clone()));
        Ok(value)
    }

    fn list(&mut self, path: &mut Vec<Step>) -> Result<Value, String> {
        self.at += 1;
        let mut out = Vec::new();
        loop {
            path.push(Step::Index(out.len()));
            self.skip(path, false)?;
            path.pop();
            if self.eat(b']') {
                return Ok(Value::Array(out));
            }
            path.push(Step::Index(out.len()));
            let v = self.value(path)?;
            path.pop();
            out.push(v);
            path.push(Step::Index(out.len()));
            self.skip(path, false)?;
            path.pop();
            if self.eat(b',') {
                continue;
            }
            if self.eat(b']') {
                return Ok(Value::Array(out));
            }
            return Err(self.error(self.at, &format!("expected , or ] in a list, found {}", self.what_is_here())));
        }
    }

    fn record(&mut self, path: &mut Vec<Step>) -> Result<Value, String> {
        self.at += 1;
        let mut out = Map::new();
        loop {
            self.skip(path, false)?;
            if self.eat(b'}') {
                return Ok(Value::Object(out));
            }
            let start = self.at;
            let key = match self.src.get(self.at) {
                Some(b'\'' | b'"') => self.string()?,
                Some(b'0'..=b'9') => {
                    let s = self.at;
                    while self.src.get(self.at).is_some_and(u8::is_ascii_digit) {
                        self.at += 1;
                    }
                    self.text[s..self.at].to_string()
                }
                _ => match self.word() {
                    Some(w) => w,
                    None => return Err(self.error(start, &format!("expected a key, found {}", self.what_is_here()))),
                },
            };
            path.push(Step::Key(key.clone()));
            self.skip(path, false)?;
            if !self.eat(b':') {
                let found = self.what_is_here();
                path.pop();
                return Err(self.error(self.at, &format!("expected : after the key {key}, found {found}")));
            }
            let v = self.value(path)?;
            path.pop();
            out.insert(key, v);
            self.skip(path, false)?;
            if self.eat(b',') {
                continue;
            }
            if self.eat(b'}') {
                return Ok(Value::Object(out));
            }
            return Err(self.error(self.at, &format!("expected , or }} in a record, found {}", self.what_is_here())));
        }
    }
}

/// Reads a format-1 source: its data and its comments.
pub fn read(file: &str, text: &str) -> Result<(Value, Vec<Comment>), String> {
    let mut r = Reader { file, src: text.as_bytes(), text, at: 0, comments: Vec::new(), last_value: None };
    r.skip(&[], true)?;
    let start = r.at;
    if r.word().as_deref() != Some("export") {
        r.at = start;
        let what = r.what_is_here();
        return Err(r.error(start, &format!("expected export default, found {what} (imports, functions and other code aren't supported)")));
    }
    r.skip(&[], true)?;
    let at = r.at;
    if r.word().as_deref() != Some("default") {
        r.at = at;
        return Err(r.error(at, "expected export default"));
    }
    r.skip(&[], false)?;
    if r.src.get(r.at) != Some(&b'{') {
        return Err(r.error(r.at, &format!("expected an object literal after export default, found {}", r.what_is_here())));
    }
    let value = r.value(&mut Vec::new())?;
    r.skip(&[], false)?;
    r.eat(b';');
    r.skip(&[], false)?;
    if r.at < r.src.len() {
        return Err(r.error(r.at, &format!("expected the end of the file after the object, found {}", r.what_is_here())));
    }
    Ok((value, r.comments))
}

/// Whether a comment is the generated header line.
fn generated(text: &str) -> bool {
    text.starts_with("Pixel-art object. How to edit it:") || text.starts_with("Pixel-art scene. How to edit it:")
}

/// A format-1 source as format-2 data: its value with its comments carried
/// into `notes` (and the palette's `colorNotes`).
pub fn convert(file: &str, text: &str) -> Result<Value, String> {
    let kind = SourceKind::of(file);
    let (value, comments) = read(file, text)?;
    let Value::Object(map) = value else { return Err(format!("{file}: the default export must be an object")) };
    let mut notes: Vec<Value> = Vec::new();
    let mut color_notes = Map::new();
    for c in comments {
        if c.header && generated(&c.text) {
            continue;
        }
        if c.header {
            notes.push(Value::String(c.text));
            continue;
        }
        let is_color = kind == SourceKind::Palette
            && c.trailing
            && c.path.len() == 2
            && matches!(&c.path[0], Step::Key(t) if t == "world" || t == "outfit")
            && matches!(&c.path[1], Step::Key(_));
        if is_color {
            if let Step::Key(name) = &c.path[1] {
                color_notes.insert(name.clone(), Value::String(c.text));
            }
            continue;
        }
        let at = path_text(&c.path);
        notes.push(Value::String(if at.is_empty() { c.text } else { format!("{at}: {}", c.text) }));
    }
    let mut out = Map::new();
    if !notes.is_empty() {
        out.insert("notes".into(), Value::Array(notes));
    }
    for (k, v) in map {
        if k != "notes" && k != "colorNotes" {
            out.insert(k, v);
        }
    }
    if !color_notes.is_empty() {
        out.insert("colorNotes".into(), Value::Object(color_notes));
    }
    Ok(Value::Object(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_what_serialize_mjs_wrote() {
        let text = "// Pixel-art scene. How to edit it: .claude/skills/pixel-art/SKILL.md\nexport default {\n  viewBox: [0, -16, 64, 48],\n  items: [\n    { object: 'pebble', 'id': \"a\", at: { px: [8, -12], level: 1.5 } },\n  ],\n};\n";
        let v = convert("scenes/x.mjs", text).unwrap();
        assert_eq!(
            v,
            json!({ "viewBox": [0, -16, 64, 48], "items": [{ "object": "pebble", "id": "a", "at": { "px": [8, -12], "level": 1.5 } }] })
        );
    }

    #[test]
    fn carries_comments_into_notes_and_color_notes() {
        let text = "// The palette.\n//\n//   indented\nexport default {\n  world: {\n    // the greens\n    'grass-1': '#B5C79C',\n    'soil-3': '#5A3E2B', // --color-earth\n  },\n  outfit: {},\n};\n";
        let v = convert("palette.mjs", text).unwrap();
        assert_eq!(
            v,
            json!({ "notes": ["The palette.", "", "  indented", "world: the greens"], "world": { "grass-1": "#B5C79C", "soil-3": "#5A3E2B" }, "outfit": {}, "colorNotes": { "soil-3": "--color-earth" } })
        );
    }

    #[test]
    fn refuses_anything_else_with_its_place() {
        let cases = [
            (
                "import x from './y.mjs';\nexport default {};\n",
                "objects/a.mjs:1:1: expected export default, found 'import' (imports, functions and other code aren't supported)",
            ),
            ("export default {\n  a: `x`,\n};\n", "objects/a.mjs:2:6: expected a value, found '`'"),
            ("export default {\n  ...base,\n};\n", "objects/a.mjs:2:3: expected a key, found '.'"),
            (
                "export default {\n  a: f(),\n};\n",
                "objects/a.mjs:2:6: 'f' isn't a value this format reads (only strings, numbers, true, false, lists and records)",
            ),
            ("export default {\n  a: 'x\\n',\n};\n", "objects/a.mjs:2:8: an escape other than \\\\, \\' or \\\" isn't supported"),
            ("export default {\n  a: 1 + 2,\n};\n", "objects/a.mjs:2:8: expected , or } in a record, found '+'"),
            ("export default {};\nconsole.log(1);\n", "objects/a.mjs:2:1: expected the end of the file after the object, found 'console'"),
            ("/* x */ export default {};\n", "objects/a.mjs:1:1: block comments (/* */) aren't supported; use // comments"),
        ];
        for (text, want) in cases {
            assert_eq!(convert("objects/a.mjs", text).unwrap_err(), want, "{text}");
        }
    }
}
