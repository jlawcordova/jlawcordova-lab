//! The state dir: proposals, the activity log and proposal numbers (ports cli/lib/state.mjs).
//!
//! The state dir is where MCP saves become proposals and every MCP tool call
//! is logged, for the app's Agent window to show. `PIXELLAB_STATE` overrides
//! the default, `~/Library/Application Support/Pixel Lab/`.
//!
//! ```text
//! proposals/<id>.json   { id, number, format, doc, baseHash, source, summary, createdAt, agent, chat? }
//! retired-proposals/    proposals from before format 2, moved here when the state dir is opened (R25, D11)
//! proposal-numbers.log  one proposal id per line, appended when a proposal is first written
//! activity.jsonl        one { time, tool, args, ok, summary, agent?, chat? } per line
//! ```
//!
//! A proposal is a JSON object (`serde_json::Value`), as in the Node CLI.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use engine::format::{stringify, stringify_pretty};
use serde_json::{Map, Value};

use crate::errors::{CommandError, Error, Result};

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

/// The proposal format this program writes (`"format": 2`). A proposal file without it is retired.
pub const PROPOSAL_FORMAT: u64 = 2;

const ID_LEN: usize = 22;

/// `ID`: `/^[0-9]{8}-[0-9]{6}-[0-9a-f]{6}$/`.
fn is_proposal_id(id: &str) -> bool {
    let b = id.as_bytes();
    b.len() == ID_LEN
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 15 => *c == b'-',
            0..=14 => c.is_ascii_digit(),
            _ => c.is_ascii_digit() || (b'a'..=b'f').contains(c),
        })
}

/// A sortable, file-safe proposal id: `20261019-142301-a1b2c3` (UTC).
pub fn new_proposal_id(now_ms: f64) -> String {
    let stamp: String = iso_time(now_ms).replace(['-', ':'], "").replacen('T', "-", 1).chars().take(15).collect();
    format!("{stamp}-{}", random_hex(3))
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

/// Proposals and activity in one state dir (`createState`).
#[derive(Clone, Debug)]
pub struct State {
    pub dir: PathBuf,
    pub proposals_dir: PathBuf,
    pub activity_file: PathBuf,
    pub numbers_file: PathBuf,
}

/// `createState(dir)`.
pub fn create_state(dir: impl Into<PathBuf>) -> State {
    State::new(dir)
}

fn js_string(v: Option<&Value>) -> String {
    match v {
        None => "undefined".into(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => stringify(other),
    }
}

impl State {
    /// Opens a state dir, first retiring any proposal from before format 2 (R25, D11).
    pub fn new(dir: impl Into<PathBuf>) -> State {
        let dir = dir.into();
        let state = State {
            proposals_dir: dir.join("proposals"),
            activity_file: dir.join("activity.jsonl"),
            numbers_file: dir.join("proposal-numbers.log"),
            dir,
        };
        state.retire_old_proposals();
        state
    }

    /// Moves every proposal file without `"format": 2` to `retired-proposals/<id>.json` and logs one
    /// activity line. Nothing is logged when there is nothing to retire. The numbers log, the
    /// activity log and everything else in the dir are left alone.
    fn retire_old_proposals(&self) {
        let Ok(entries) = fs::read_dir(&self.proposals_dir) else {
            return;
        };
        let old: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|f| f.ends_with(".json") && is_proposal_id(&f[..f.len() - 5]))
            .filter(|f| {
                let current = fs::read(self.proposals_dir.join(f))
                    .ok()
                    .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                    .is_some_and(|v| v.get("format").and_then(Value::as_u64) == Some(PROPOSAL_FORMAT));
                !current
            })
            .collect();
        let retired_dir = self.dir.join("retired-proposals");
        let mut moved = 0;
        for f in old {
            if fs::create_dir_all(&retired_dir).is_ok() && fs::rename(self.proposals_dir.join(&f), retired_dir.join(&f)).is_ok() {
                moved += 1;
            }
        }
        if moved > 0 {
            let summary = format!("retired {moved} proposal{} from before the format change", if moved == 1 { "" } else { "s" });
            let mut entry = Map::new();
            entry.insert("time".into(), Value::String(iso_time(now_ms())));
            entry.insert("tool".into(), Value::String("proposal_retire".into()));
            entry.insert("args".into(), Value::Object(Map::new()));
            entry.insert("ok".into(), Value::Bool(true));
            entry.insert("summary".into(), Value::String(summary));
            let _ = self.append_activity(&Value::Object(entry));
        }
    }

    /// The ids in proposal-numbers.log, in order (line n is proposal #n).
    fn numbered(&self) -> Vec<String> {
        match fs::read(&self.numbers_file) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).split('\n').filter(|l| !l.trim().is_empty()).map(String::from).collect(),
            Err(_) => Vec::new(),
        }
    }

    /// A proposal's number (#n), or `None` if it was never numbered.
    pub fn number_of(&self, id: &str) -> Option<u64> {
        self.numbered().iter().position(|l| l == id).map(|i| i as u64 + 1)
    }

    fn proposal_file(&self, id: &str) -> Result<PathBuf> {
        if !is_proposal_id(id) {
            return Err(CommandError::not_found(format!(
                "no proposal {id}; ids look like 20261019-142301-a1b2c3 (pixellab proposal list)"
            ))
            .into());
        }
        Ok(self.proposals_dir.join(format!("{id}.json")))
    }

    /// Writes a proposal. The first time an id is written it gets the next
    /// number: its id is appended to proposal-numbers.log (an append is
    /// atomic, so two servers proposing at once each get their own line) and
    /// the number is its line. A rewritten proposal keeps its number.
    pub fn write_proposal(&self, proposal: &Value) -> Result<Value> {
        let id = js_string(proposal.get("id"));
        let file = self.proposal_file(&id)?;
        let given = proposal.get("number").filter(|n| !n.is_null()).and_then(engine::jsnum::as_f64);
        let mut number = match (proposal.get("number"), given) {
            (Some(n), None) if !n.is_null() => Some(f64::NAN),
            (_, Some(n)) => Some(n),
            _ => self.number_of(&id).map(|n| n as f64),
        };
        if number.is_none() {
            fs::create_dir_all(&self.proposals_dir)?;
            append(&self.numbers_file, &format!("{id}\n"))?;
            number = self.number_of(&id).map(|n| n as f64);
        }
        let mut out = Map::new();
        out.insert("id".into(), Value::String(id));
        if let Some(n) = number.filter(|n| *n != 0.0 && !n.is_nan()) {
            out.insert("number".into(), engine::jsnum::num(n));
        }
        out.insert("format".into(), Value::from(PROPOSAL_FORMAT));
        if let Some(map) = proposal.as_object() {
            for (k, v) in map {
                if k == "format" {
                    continue;
                }
                out.insert(k.clone(), v.clone());
            }
        }
        let out = Value::Object(out);
        write_atomic(&file, &format!("{}\n", stringify_pretty(&out)))?;
        Ok(out)
    }

    /// Reads one proposal.
    pub fn read_proposal(&self, id: &str) -> Result<Value> {
        let file = self.proposal_file(id)?;
        let text = fs::read(&file).map(|b| String::from_utf8_lossy(&b).into_owned());
        let Ok(text) = text else {
            return Err(CommandError::not_found(format!("no proposal {id} (pixellab proposal list)")).into());
        };
        serde_json::from_str::<Value>(&text)
            .map_err(|e| Error::Command(CommandError::new(format!("proposal {id} is not valid JSON: {e}"), "bad_proposal")))
    }

    pub fn has_proposal(&self, id: &str) -> bool {
        match self.proposal_file(id) {
            Ok(file) => fs::metadata(file).is_ok(),
            Err(_) => false,
        }
    }

    /// Every proposal, oldest first. Unreadable files are skipped.
    pub fn list_proposals(&self) -> Vec<Value> {
        let Ok(entries) = fs::read_dir(&self.proposals_dir) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|f| f.ends_with(".json") && is_proposal_id(&f[..f.len() - 5]))
            .collect();
        names.sort();
        let mut out: Vec<Value> = Vec::new();
        for f in names {
            // a half-written or hand-edited file: not listed
            if let Ok(bytes) = fs::read(self.proposals_dir.join(&f)) {
                if let Ok(v) = serde_json::from_str::<Value>(&String::from_utf8_lossy(&bytes)) {
                    out.push(v);
                }
            }
        }
        // Ids sort by second only (two saves in one second get random suffixes): order by createdAt (ms), then id.
        let at = |p: &Value| p.get("createdAt").and_then(Value::as_str).and_then(parse_iso_time).unwrap_or(0.0);
        let id = |p: &Value| p.get("id").and_then(Value::as_str).map(String::from);
        out.sort_by(|a, b| at(a).partial_cmp(&at(b)).unwrap_or(std::cmp::Ordering::Equal).then_with(|| id(a).cmp(&id(b))));
        out
    }

    pub fn remove_proposal(&self, id: &str) -> Result<()> {
        let file = self.proposal_file(id)?;
        if fs::metadata(&file).is_err() {
            return Err(CommandError::not_found(format!("no proposal {id} (pixellab proposal list)")).into());
        }
        let _ = fs::remove_file(&file);
        Ok(())
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
