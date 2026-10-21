//! A three-way line merge (ports cli/lib/merge.mjs;
//! docs/intents/2026-10-mcp-gaps/spec.md, D4), for a working copy whose file
//! changed on disk. Sources are canonical (one map row or item per line), so
//! lines are the grain a person reads in a diff: edits to different items or
//! rows merge, the same row edited on both sides clashes. With `union`, two
//! insertions at the same point both go in (ours first).
//!
//! Format 2's JSON puts no comma after the last element of a list, so adding
//! an item at the end of a list would also change the line before it, and a
//! line merge would clash where the `.mjs` sources merged. The session merges
//! sources in their trailing-comma form instead ([`trailing`], every element
//! line ending in a comma, as the `.mjs` sources had them) and turns the
//! result back with [`untrailing`].

use crate::diff::diff_lines;

/// Base lines [from, to) become `lines`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    pub from: usize,
    pub to: usize,
    pub lines: Vec<String>,
}

/// Where both sides changed differently: base lines (1-based, inclusive;
/// an empty range is [n + 1, n]) and each side's lines there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    pub base: [usize; 2],
    pub ours: Vec<String>,
    pub theirs: Vec<String>,
}

/// A merge's text, and its conflicts (empty when it's clean).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Merged {
    pub text: String,
    pub conflicts: Vec<Conflict>,
}

/// The changes from `base` to `other`, as hunks over the base's lines
/// (`hunks`).
pub fn hunks(base: &[&str], other: &[&str]) -> Vec<Hunk> {
    let mut out = Vec::new();
    let mut at = 0;
    let mut open: Option<Hunk> = None;
    for (op, line) in diff_lines(base, other) {
        if op == ' ' {
            if let Some(h) = open.take() {
                out.push(h);
            }
            at += 1;
            continue;
        }
        let h = open.get_or_insert_with(|| Hunk { from: at, to: at, lines: vec![] });
        if op == '-' {
            h.to += 1;
            at += 1;
        } else {
            h.lines.push(line.to_string());
        }
    }
    if let Some(h) = open {
        out.push(h);
    }
    out
}

/// Whether two hunks touch the same base lines (two insertions at one point count).
fn overlaps(a: &Hunk, b: &Hunk) -> bool {
    if a.from == a.to && b.from == b.to {
        return a.from == b.from;
    }
    if a.from == a.to {
        return a.from > b.from && a.from < b.to;
    }
    if b.from == b.to {
        return b.from > a.from && b.from < a.to;
    }
    a.from < b.to && b.from < a.to
}

fn same(a: &Hunk, b: &Hunk) -> bool {
    a.from == b.from && a.to == b.to && a.lines == b.lines
}

/// Merges two texts that both started from `base` (`merge3`).
pub fn merge3(base: &str, ours: &str, theirs: &str, union: bool) -> Merged {
    let clean = |text: &str| Merged { text: text.to_string(), conflicts: vec![] };
    if ours == theirs {
        return clean(ours);
    }
    if ours == base {
        return clean(theirs);
    }
    if theirs == base {
        return clean(ours);
    }
    let b: Vec<&str> = base.split('\n').collect();
    let mine = hunks(&b, &ours.split('\n').collect::<Vec<_>>());
    let yours = hunks(&b, &theirs.split('\n').collect::<Vec<_>>());
    let mut conflicts = Vec::new();
    let mut apply: Vec<Hunk> = Vec::new();
    // Group hunks that overlap, transitively, across the two sides.
    let mut all: Vec<(&Hunk, usize)> = mine.iter().map(|h| (h, 0)).chain(yours.iter().map(|h| (h, 1))).collect();
    all.sort_by(|x, y| x.0.from.cmp(&y.0.from).then(x.0.to.cmp(&y.0.to)).then(x.1.cmp(&y.1)));
    let mut i = 0;
    while i < all.len() {
        let mut group = vec![all[i]];
        let mut j = i + 1;
        while j < all.len() && group.iter().any(|g| g.1 != all[j].1 && overlaps(g.0, all[j].0)) {
            group.push(all[j]);
            j += 1;
        }
        let one_side = group.iter().all(|g| g.1 == group[0].1);
        if one_side {
            apply.extend(group.iter().map(|g| g.0.clone()));
        } else {
            let a: Vec<&Hunk> = group.iter().filter(|g| g.1 == 0).map(|g| g.0).collect();
            let c: Vec<&Hunk> = group.iter().filter(|g| g.1 == 1).map(|g| g.0).collect();
            let inserts = |list: &[&Hunk]| list.iter().all(|h| h.from == h.to);
            let one_point = group.iter().all(|g| g.0.from == group[0].0.from);
            if a.len() == 1 && c.len() == 1 && same(a[0], c[0]) {
                apply.push(a[0].clone());
            } else if union && inserts(&a) && inserts(&c) && one_point {
                let lines = a.iter().chain(&c).flat_map(|h| h.lines.iter().cloned()).collect();
                apply.push(Hunk { from: a[0].from, to: a[0].from, lines });
            } else {
                let from = group.iter().map(|g| g.0.from).min().unwrap();
                let to = group.iter().map(|g| g.0.to).max().unwrap();
                conflicts.push(Conflict { base: [from + 1, to], ours: side(&b, &a, from, to), theirs: side(&b, &c, from, to) });
            }
        }
        i = j;
    }
    if !conflicts.is_empty() {
        return Merged { text: String::new(), conflicts };
    }
    apply.sort_by(|x, y| x.from.cmp(&y.from).then(x.to.cmp(&y.to)));
    let mut out: Vec<String> = Vec::new();
    let mut at = 0;
    for h in &apply {
        out.extend(b[at..h.from].iter().map(|s| s.to_string()));
        out.extend(h.lines.iter().cloned());
        at = h.to;
    }
    out.extend(b[at..].iter().map(|s| s.to_string()));
    Merged { text: out.join("\n"), conflicts }
}

/// One side's text for base lines [from, to): its hunks there over the base.
fn side(base: &[&str], list: &[&Hunk], from: usize, to: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut at = from;
    for h in list {
        out.extend(base[at..h.from].iter().map(|s| s.to_string()));
        out.extend(h.lines.iter().cloned());
        at = h.to;
    }
    out.extend(base[at..to].iter().map(|s| s.to_string()));
    out
}

/// Whether a line closes a list or a record (its first non-space is `]` or `}`).
fn closes(line: &str) -> bool {
    matches!(line.trim_start().as_bytes().first(), Some(b']' | b'}'))
}

/// Canonical text in its trailing-comma form: every line before a closing
/// `]` or `}` that isn't itself an opening ends in a comma, as every
/// element did in the `.mjs` sources.
pub fn trailing(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        let next_closes = lines.get(i + 1).is_some_and(|n| closes(n));
        if next_closes && !line.is_empty() && !line.ends_with(['[', '{', ',']) {
            out.push(format!("{line},"));
        } else {
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

/// The canonical text from its trailing-comma form: a comma before a
/// closing line is dropped.
pub fn untrailing(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        let next_closes = lines.get(i + 1).is_some_and(|n| closes(n));
        match line.strip_suffix(',') {
            Some(kept) if next_closes => out.push(kept.to_string()),
            _ => out.push(line.to_string()),
        }
    }
    out.join("\n")
}

/// A folded scene and what couldn't be folded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folded {
    pub text: String,
    pub clashes: Vec<String>,
}

/// A top-level key's line, `  "<key>": …`, and its key.
fn top_key(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("  \"")?;
    if rest.starts_with(' ') {
        return None;
    }
    let end = rest.find('"')?;
    rest[end..].starts_with("\": ").then(|| &rest[..end])
}

/// A layer entry's id: `"id": "<id>"`.
fn id_of(line: &str) -> Option<&str> {
    let at = line.find("\"id\": \"")? + "\"id\": \"".len();
    let end = line[at..].find('"')?;
    Some(&line[at..at + end])
}

/// After a scene's union merge, in trailing-comma form (`foldScene`): when
/// both sides made a scene's first layers, both `"layers": [ … ]` blocks
/// went in. Folds them into one list, ours first, and returns what can't be
/// folded: a layer id both sides made, and any other top-level key both
/// sides added.
pub fn fold_scene(text: &str) -> Folded {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut blocks: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i] == "  \"layers\": [" {
            let mut end = i + 1;
            while end < lines.len() && lines[end] != "  ]," {
                end += 1;
            }
            blocks.push((i, end));
            i = end;
        }
        i += 1;
    }
    let mut clashes = Vec::new();
    let mut out: Vec<&str> = lines.clone();
    if blocks.len() > 1 {
        let mut seen = std::collections::HashSet::new();
        let mut kept: Vec<&str> = Vec::new();
        for &(start, end) in &blocks {
            for l in &lines[start + 1..end] {
                if let Some(id) = id_of(l) {
                    if !seen.insert(id) {
                        clashes.push(format!("layer {id}"));
                    }
                }
                kept.push(l);
            }
        }
        let drop: std::collections::HashSet<usize> = blocks[1..].iter().flat_map(|&(s, e)| s..=e).collect();
        let (first_start, first_end) = blocks[0];
        out = Vec::new();
        for (i, l) in lines.iter().enumerate() {
            if i == first_start {
                out.push(l);
                out.extend(kept.iter());
            } else if (i > first_start && i < first_end) || drop.contains(&i) {
                continue;
            } else {
                out.push(l);
            }
        }
    }
    let mut counts: engine::omap::OMap<String, usize> = engine::omap::OMap::new();
    for l in &out {
        if let Some(k) = top_key(l) {
            let n = counts.get(k).copied().unwrap_or(0);
            counts.set(k.to_string(), n + 1);
        }
    }
    for (key, n) in counts.iter() {
        if *n > 1 {
            clashes.push(key.clone());
        }
    }
    Folded { text: out.join("\n"), clashes }
}
