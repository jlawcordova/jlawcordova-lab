//! The app's open docs, in the state dir (ports cli/lib/open-docs.mjs;
//! docs/intents/2026-10-mcp-gaps/spec.md, D6), so an agent can see what the
//! owner has open with unsaved edits before it proposes against it. The
//! app's server (serve --stdio) writes `open/<pid>.json` = `{ pid,
//! document, updatedAt, docs: [{ doc, dirty }] }` whenever its open docs
//! change, and removes it when it stops. Readers skip files whose process is
//! gone and other documents'.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::state::{iso_time, now_ms, write_atomic};

/// The folder of open-docs files in a state dir.
pub fn open_dir(state_dir: &Path) -> PathBuf {
    state_dir.join("open")
}

/// Whether a process is running (`alive`): `ps -p <pid>` finds it.
pub fn alive(pid: i64) -> bool {
    if pid <= 0 {
        return false;
    }
    if pid == std::process::id() as i64 {
        return true;
    }
    std::process::Command::new("ps")
        .args(["-p", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// `path.resolve`: an absolute path, `.` and `..` folded, without touching the disk.
pub fn resolve(path: &Path) -> PathBuf {
    let base = if path.is_absolute() { PathBuf::new() } else { std::env::current_dir().unwrap_or_default() };
    let mut out = PathBuf::new();
    for c in base.join(path).components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

/// Writes this process's open-docs file (`openDocsWriter`).
pub struct OpenDocsWriter {
    pub file: PathBuf,
    pid: u32,
    document: PathBuf,
    last: String,
}

impl OpenDocsWriter {
    pub fn new(state_dir: &Path, document: &Path) -> OpenDocsWriter {
        let pid = std::process::id();
        OpenDocsWriter { file: open_dir(state_dir).join(format!("{pid}.json")), pid, document: resolve(document), last: String::new() }
    }

    /// Writes the file when the list changed since the last write; an error
    /// is logged, not thrown.
    pub fn update(&mut self, docs: &[(String, bool)], log: &mut dyn FnMut(&str)) {
        let mut list: Vec<(String, bool)> = docs.to_vec();
        list.sort_by(|a, b| a.0.cmp(&b.0));
        let list: Vec<Value> = list.into_iter().map(|(doc, dirty)| json!({ "doc": doc, "dirty": dirty })).collect();
        let key = engine::format::stringify(&Value::Array(list.clone()));
        if key == self.last {
            return;
        }
        self.last = key;
        let data = json!({ "pid": self.pid, "document": self.document.to_string_lossy(), "updatedAt": iso_time(now_ms()), "docs": list });
        let text = format!("{}\n", engine::format::stringify_pretty(&data));
        if let Err(e) = write_atomic(&self.file, &text) {
            log(&format!("can't write {}: {e}", self.file.display()));
        }
    }

    /// Deletes the file.
    pub fn remove(&self) {
        let _ = std::fs::remove_file(&self.file);
    }
}

/// The docs open in a running app on this document, with whether each has
/// unsaved edits (`appOpenDocs`).
pub fn app_open_docs(state_dir: &Path, document: &Path) -> Vec<(String, bool, i64)> {
    let Ok(entries) = std::fs::read_dir(open_dir(state_dir)) else { return vec![] };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|f| f.strip_suffix(".json").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())))
        .collect();
    names.sort();
    let root = resolve(document);
    let mut out = Vec::new();
    for f in names {
        let Ok(text) = std::fs::read_to_string(open_dir(state_dir).join(&f)) else { continue };
        let Ok(data) = serde_json::from_str::<Value>(&text) else { continue };
        let pid = data.get("pid").and_then(Value::as_i64).unwrap_or(0);
        let same = data.get("document").map(|d| resolve(Path::new(&engine::engine::js_string(Some(d))))) == Some(root.clone());
        if pid == std::process::id() as i64 || !alive(pid) || !same {
            continue;
        }
        let Some(docs) = data.get("docs").and_then(Value::as_array) else { continue };
        for d in docs {
            if let Some(doc) = d.get("doc").and_then(Value::as_str) {
                out.push((doc.to_string(), d.get("dirty") == Some(&Value::Bool(true)), pid));
            }
        }
    }
    out
}

/// The refs the app has open with unsaved edits, sorted (`unsavedInApp`).
pub fn unsaved_in_app(state_dir: &Path, document: &Path) -> Vec<String> {
    let mut out: Vec<String> = app_open_docs(state_dir, document).into_iter().filter(|d| d.1).map(|d| d.0).collect();
    out.sort();
    out.dedup();
    out
}
