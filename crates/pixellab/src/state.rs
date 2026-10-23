//! The state dir (ports cli/lib/state.mjs): Pixel Lab's own files, never the document's.
//! `PIXELLAB_STATE` overrides the default, `~/Library/Application Support/Pixel Lab/`.
//!
//! ```text
//! activity.jsonl        one { time, tool, args, ok, summary, agent?, chat? } per MCP tool call
//! history/<key>/        a document's version history (history.rs)
//! owners/<key>.lock     the app's server that owns a document, and its socket (owner.rs)
//! retired-proposals/    proposals left from before edits applied, set aside with a notice (R21)
//! ```

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use engine::format::stringify;
use serde_json::{Map, Value};

use crate::errors::Result;

// ------------------------------------------------------------ time and ids

/// Milliseconds since the epoch (`Date.now()`).
pub fn now_ms() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_millis() as f64,
        Err(e) => -(e.duration().as_millis() as f64),
    }
}

/// `n` random bytes as lowercase hex (`randomBytes(n).toString('hex')`), from /dev/urandom.
pub fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    let ok = fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut buf)).is_ok();
    if !ok {
        // No /dev/urandom: fall back to the clock, so ids still differ.
        let mut x = (now_ms() as u64) ^ ((std::process::id() as u64) << 32) ^ 0x9e37_79b9_7f4a_7c15;
        for b in buf.iter_mut() {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            *b = (x >> 24) as u8;
        }
    }
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// Days since the epoch → (year, month, day), proleptic Gregorian.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// (year, month, day) → days since the epoch.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `new Date(ms).toISOString()`: `2026-10-19T14:23:01.123Z`, UTC. Years
/// outside 0..9999 print as `+012345` / `-000001`, as JavaScript does.
pub fn iso_time(ms: f64) -> String {
    let ms = ms.trunc() as i64;
    let days = ms.div_euclid(86_400_000);
    let rem = ms.rem_euclid(86_400_000);
    let (y, m, d) = civil_from_days(days);
    let year = if (0..=9999).contains(&y) { format!("{y:04}") } else { format!("{}{:06}", if y < 0 { '-' } else { '+' }, y.abs()) };
    format!("{year}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:03}Z", rem / 3_600_000, rem / 60_000 % 60, rem / 1000 % 60, rem % 1000)
}

/// `Date.parse(text)` for ISO timestamps (`2026-10-19T11:28:30.100Z`, a date alone, a `+hh:mm`
/// offset); `None` (NaN) for anything else.
pub fn parse_iso_time(text: &str) -> Option<f64> {
    let b = text.as_bytes();
    let num = |from: usize, len: usize| -> Option<i64> {
        let s = b.get(from..from + len)?;
        if !s.iter().all(u8::is_ascii_digit) {
            return None;
        }
        std::str::from_utf8(s).ok()?.parse().ok()
    };
    let (year, mut i) = match b.first() {
        Some(b'+') | Some(b'-') => {
            let v = num(1, 6)?;
            (if b[0] == b'-' { -v } else { v }, 7)
        }
        _ => (num(0, 4)?, 4),
    };
    let (mut month, mut day) = (1, 1);
    if b.get(i) == Some(&b'-') {
        month = num(i + 1, 2)?;
        i += 3;
        if b.get(i) == Some(&b'-') {
            day = num(i + 1, 2)?;
            i += 3;
        }
    }
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let (mut h, mut mi, mut s, mut ms) = (0, 0, 0, 0);
    let mut offset = 0;
    if i < b.len() {
        if b[i] != b'T' && b[i] != b't' && b[i] != b' ' {
            return None;
        }
        h = num(i + 1, 2)?;
        if b.get(i + 3) != Some(&b':') {
            return None;
        }
        mi = num(i + 4, 2)?;
        i += 6;
        if b.get(i) == Some(&b':') {
            s = num(i + 1, 2)?;
            i += 3;
            if b.get(i) == Some(&b'.') {
                let digits = b[i + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
                if digits == 0 {
                    return None;
                }
                let frac = format!("{:0<3}", &text[i + 1..i + 1 + digits.min(3)]);
                ms = frac.parse().ok()?;
                i += 1 + digits;
            }
        }
        match b.get(i) {
            None => {}
            Some(b'Z') | Some(b'z') if i + 1 == b.len() => {}
            Some(&sign @ (b'+' | b'-')) => {
                let oh = num(i + 1, 2)?;
                if b.get(i + 3) != Some(&b':') || i + 6 != b.len() {
                    return None;
                }
                let om = num(i + 4, 2)?;
                offset = (oh * 60 + om) * if sign == b'-' { -1 } else { 1 };
            }
            _ => return None,
        }
        if h > 24 || mi > 59 || s > 59 || (h == 24 && (mi, s, ms) != (0, 0, 0)) {
            return None;
        }
    }
    let days = days_from_civil(year, month, day);
    Some((days * 86_400_000 + h * 3_600_000 + mi * 60_000 + s * 1000 + ms - offset * 60_000) as f64)
}

// ----------------------------------------------------------------- paths

/// `path.resolve(p)`: absolute against the working directory, `.` and `..` folded.
fn resolve(p: &str) -> PathBuf {
    let mut out = PathBuf::new();
    let path = Path::new(p);
    let full =
        if path.is_absolute() { path.to_path_buf() } else { std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")).join(path) };
    for c in full.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// `path.resolve`: an absolute path, `.` and `..` folded, without touching the disk.
pub fn resolve_path(path: &Path) -> PathBuf {
    resolve(&path.to_string_lossy())
}

/// A document's path as every process names it: its real path when it exists (symlinks and
/// `/private` folded), else `path.resolve`. History and owner keys hash it.
pub fn real_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| resolve(&path.to_string_lossy()))
}

/// The state dir for an environment.
pub fn state_dir(env: &HashMap<String, String>) -> PathBuf {
    if let Some(s) = env.get("PIXELLAB_STATE").filter(|s| !s.is_empty()) {
        return resolve(s);
    }
    let home = env.get("HOME").cloned().or_else(|| std::env::var("HOME").ok()).unwrap_or_else(|| "/".into());
    Path::new(&home).join("Library").join("Application Support").join("Pixel Lab")
}

/// Writes a file atomically: a temp file next to it, then a rename.
pub fn write_atomic(path: &Path, text: &str) -> Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".{}-{}.tmp", std::process::id(), random_hex(4)));
    let done = fs::write(&tmp, text).and_then(|_| fs::rename(&tmp, path));
    if let Err(e) = done {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

fn append(path: &Path, text: &str) -> Result<()> {
    let mut f = fs::OpenOptions::new().create(true).append(true).open(path)?;
    f.write_all(text.as_bytes())?;
    Ok(())
}

// ----------------------------------------------------------------- state

/// The activity log in one state dir (`createState`).
#[derive(Clone, Debug)]
pub struct State {
    pub dir: PathBuf,
    pub activity_file: PathBuf,
}

/// `createState(dir)`.
pub fn create_state(dir: impl Into<PathBuf>) -> State {
    State::new(dir)
}

impl State {
    /// Opens a state dir, first setting aside any proposal left from before edits applied (R21).
    pub fn new(dir: impl Into<PathBuf>) -> State {
        let dir = dir.into();
        let state = State { activity_file: dir.join("activity.jsonl"), dir };
        state.retire_proposals();
        state
    }

    /// Moves every file in `proposals/` to `retired-proposals/<date>/`, writes
    /// `retired-proposals/notice.json` ({ count, dir, at }) for the Agent window to show once, and
    /// logs one activity line (docs/intents/2026-10-autosave/spec.md, R21, D6). Nothing is
    /// deleted, and nothing happens when there's nothing to retire.
    fn retire_proposals(&self) {
        let from = self.dir.join("proposals");
        let Ok(entries) = fs::read_dir(&from) else { return };
        let files: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|f| f.ends_with(".json"))
            .collect();
        if files.is_empty() {
            let _ = fs::remove_dir(&from);
            return;
        }
        let now = now_ms();
        let day: String = iso_time(now).chars().take(10).collect();
        let to = self.dir.join("retired-proposals").join(&day);
        let mut moved = 0;
        for f in &files {
            if fs::create_dir_all(&to).is_ok() && fs::rename(from.join(f), to.join(f)).is_ok() {
                moved += 1;
            }
        }
        let _ = fs::remove_dir(&from);
        if moved == 0 {
            return;
        }
        let notice_file = self.dir.join("retired-proposals").join("notice.json");
        let earlier = fs::read(&notice_file)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .and_then(|v| v.get("count").and_then(Value::as_u64))
            .unwrap_or(0);
        let notice = serde_json::json!({ "count": earlier + moved, "dir": to.to_string_lossy(), "at": iso_time(now) });
        let _ = write_atomic(&notice_file, &stringify(&notice));
        let summary = format!("retired {moved} waiting proposal{}: edits apply now", if moved == 1 { "" } else { "s" });
        let mut entry = Map::new();
        entry.insert("time".into(), Value::String(iso_time(now)));
        entry.insert("tool".into(), Value::String("proposal_retire".into()));
        entry.insert("args".into(), Value::Object(Map::new()));
        entry.insert("ok".into(), Value::Bool(true));
        entry.insert("summary".into(), Value::String(summary));
        let _ = self.append_activity(&Value::Object(entry));
    }

    /// Appends one activity entry.
    pub fn append_activity(&self, entry: &Value) -> Result<()> {
        fs::create_dir_all(&self.dir)?;
        append(&self.activity_file, &format!("{}\n", stringify(entry)))
    }

    /// The last `limit` activity entries, oldest first (the Node default was 50;
    /// as `slice(-limit)`, 0 gives them all).
    pub fn tail_activity(&self, limit: usize) -> Vec<Value> {
        let Ok(bytes) = fs::read(&self.activity_file) else {
            return Vec::new();
        };
        let all = parse_activity(&String::from_utf8_lossy(&bytes));
        let from = if limit == 0 { 0 } else { all.len().saturating_sub(limit) };
        all[from..].to_vec()
    }
}

/// Activity lines → entries; a broken line is skipped.
pub fn parse_activity(text: &str) -> Vec<Value> {
    text.split('\n').filter(|l| !l.trim().is_empty()).filter_map(|l| serde_json::from_str::<Value>(l).ok()).collect()
}
