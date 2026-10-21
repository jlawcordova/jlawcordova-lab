//! Watching the document's sources and the state dir, by polling (spec D2) (ports cli/lib/watch.mjs).
//!
//! The stdio server calls the pollers every `POLL_MS` from its event loop; nothing here starts a
//! thread. A poll only stats files (and reads the new bytes of the activity log), never a source's
//! contents.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;

use crate::state::{parse_activity, State};

/// How long after the latest change a report waits, so a burst of edits is one report.
pub const DEBOUNCE_MS: f64 = 150.0;
/// How often the server polls.
pub const POLL_MS: u64 = 250;

type Stamp = (u64, Option<SystemTime>);

/// `name.json` files directly in `dir` (no nested folders), as `(prefix + name, stamp)`.
fn scan_json(dir: &Path, prefix: &str, out: &mut HashMap<String, Stamp>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.ends_with(".json") || name.len() == ".json".len() {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        out.insert(format!("{prefix}{name}"), (meta.len(), meta.modified().ok()));
    }
}

/// What changed between two snapshots: files that came, changed or went.
fn changed(old: &HashMap<String, Stamp>, new: &HashMap<String, Stamp>) -> Vec<String> {
    let mut files = Vec::new();
    for (name, stamp) in new {
        if old.get(name) != Some(stamp) {
            files.push(name.clone());
        }
    }
    for name in old.keys() {
        if !new.contains_key(name) {
            files.push(name.clone());
        }
    }
    files
}

/// Watches the art sources: `palette.json`, `objects/<name>.json`, `scenes/<name>.json`.
pub struct SourcesWatcher {
    dir: PathBuf,
    snapshot: HashMap<String, Stamp>,
    pending: BTreeSet<String>,
    last_change: f64,
}

impl SourcesWatcher {
    /// Takes the first snapshot of `dir`.
    pub fn new(dir: &Path) -> SourcesWatcher {
        let mut w = SourcesWatcher { dir: dir.to_path_buf(), snapshot: HashMap::new(), pending: BTreeSet::new(), last_change: 0.0 };
        w.snapshot = w.scan();
        w
    }

    fn scan(&self) -> HashMap<String, Stamp> {
        let mut out = HashMap::new();
        if let Ok(meta) = fs::metadata(self.dir.join("palette.json")) {
            if meta.is_file() {
                out.insert("palette.json".to_string(), (meta.len(), meta.modified().ok()));
            }
        }
        scan_json(&self.dir.join("objects"), "objects/", &mut out);
        scan_json(&self.dir.join("scenes"), "scenes/", &mut out);
        out
    }

    /// The pending source files (sorted) once `DEBOUNCE_MS` has passed since the latest change, else empty.
    pub fn poll(&mut self, now_ms: f64) -> Vec<String> {
        let now = self.scan();
        let files = changed(&self.snapshot, &now);
        self.snapshot = now;
        if !files.is_empty() {
            self.pending.extend(files);
            self.last_change = now_ms;
        }
        if !self.pending.is_empty() && now_ms - self.last_change >= DEBOUNCE_MS {
            return std::mem::take(&mut self.pending).into_iter().collect();
        }
        Vec::new()
    }
}

/// What a `StateWatcher` poll found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StateChanges {
    /// A proposal file came, changed or went (debounced).
    pub proposals: bool,
    /// The entries appended to activity.jsonl since the last poll.
    pub activity: Vec<Value>,
}

/// Watches the state dir: the proposals and the activity log.
pub struct StateWatcher {
    proposals_dir: PathBuf,
    activity_file: PathBuf,
    snapshot: HashMap<String, Stamp>,
    proposals_pending: bool,
    last_change: f64,
    offset: u64,
}

impl StateWatcher {
    /// Creates the proposals dir and starts at the activity log's current end.
    pub fn new(state: &State) -> StateWatcher {
        let _ = fs::create_dir_all(&state.proposals_dir);
        let offset = fs::metadata(&state.activity_file).map(|m| m.len()).unwrap_or(0);
        let mut w = StateWatcher {
            proposals_dir: state.proposals_dir.clone(),
            activity_file: state.activity_file.clone(),
            snapshot: HashMap::new(),
            proposals_pending: false,
            last_change: 0.0,
            offset,
        };
        w.snapshot = w.scan();
        w
    }

    fn scan(&self) -> HashMap<String, Stamp> {
        let mut out = HashMap::new();
        scan_json(&self.proposals_dir, "", &mut out);
        out
    }

    fn read_activity(&mut self) -> Vec<Value> {
        let Ok(meta) = fs::metadata(&self.activity_file) else { return Vec::new() };
        let size = meta.len();
        if size < self.offset {
            self.offset = 0; // truncated or replaced
        }
        if size == self.offset {
            return Vec::new();
        }
        let Ok(mut file) = fs::File::open(&self.activity_file) else { return Vec::new() };
        if file.seek(SeekFrom::Start(self.offset)).is_err() {
            return Vec::new();
        }
        let mut buf = Vec::new();
        if file.take(size - self.offset).read_to_end(&mut buf).is_err() {
            return Vec::new();
        }
        // Only whole lines; a partial last line waits for the next poll.
        let Some(end) = buf.iter().rposition(|&b| b == b'\n').map(|i| i + 1) else { return Vec::new() };
        self.offset += end as u64;
        parse_activity(&String::from_utf8_lossy(&buf[..end]))
    }

    /// Looks for changes; `proposals` is true once `DEBOUNCE_MS` has passed since the latest one.
    pub fn poll(&mut self, now_ms: f64) -> StateChanges {
        let now = self.scan();
        if !changed(&self.snapshot, &now).is_empty() {
            self.proposals_pending = true;
            self.last_change = now_ms;
        }
        self.snapshot = now;
        let mut proposals = false;
        if self.proposals_pending && now_ms - self.last_change >= DEBOUNCE_MS {
            self.proposals_pending = false;
            proposals = true;
        }
        StateChanges { proposals, activity: self.read_activity() }
    }
}
