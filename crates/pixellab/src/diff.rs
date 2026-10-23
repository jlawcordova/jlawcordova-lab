//! Line diffs (ports cli/lib/diff.mjs): Myers' O(ND) algorithm, for dry runs
//! and versions. Sources are canonical (one map row or item per line), so a
//! line diff is the diff a person reads in a pull request.

/// The edit script from `a` to `b` (`diffLines`): (op, line) with op ' ',
/// '-' or '+'.
pub fn diff_lines<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<(char, &'a str)> {
    // Common prefix and suffix first: most edits touch a few lines.
    let mut start = 0;
    while start < a.len() && start < b.len() && a[start] == b[start] {
        start += 1;
    }
    let (mut end_a, mut end_b) = (a.len(), b.len());
    while end_a > start && end_b > start && a[end_a - 1] == b[end_b - 1] {
        end_a -= 1;
        end_b -= 1;
    }
    let mut out: Vec<(char, &str)> = a[..start].iter().map(|l| (' ', *l)).collect();
    out.extend(myers(&a[start..end_a], &b[start..end_b]));
    out.extend(a[end_a..].iter().map(|l| (' ', *l)));
    out
}

fn myers<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<(char, &'a str)> {
    let (n, m) = (a.len() as i64, b.len() as i64);
    if n == 0 {
        return b.iter().map(|l| ('+', *l)).collect();
    }
    if m == 0 {
        return a.iter().map(|l| ('-', *l)).collect();
    }
    let max = n + m;
    let offset = max;
    let mut v = vec![0i64; (2 * max + 2) as usize];
    let mut trace: Vec<Vec<i64>> = Vec::new();
    let mut found = false;
    let at = |v: &[i64], k: i64| v[(offset + k) as usize];
    let mut d = 0;
    while d <= max && !found {
        trace.push(v.clone());
        let mut k = -d;
        while k <= d {
            let mut x = if k == -d || (k != d && at(&v, k - 1) < at(&v, k + 1)) { at(&v, k + 1) } else { at(&v, k - 1) + 1 };
            let mut y = x - k;
            while x < n && y < m && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            v[(offset + k) as usize] = x;
            if x >= n && y >= m {
                found = true;
                break;
            }
            k += 2;
        }
        d += 1;
    }
    // Walk back through the trace.
    let mut out = Vec::new();
    let (mut x, mut y) = (n, m);
    for d in (0..trace.len() as i64).rev() {
        let vd = &trace[d as usize];
        let k = x - y;
        let prev_k = if k == -d || (k != d && at(vd, k - 1) < at(vd, k + 1)) { k + 1 } else { k - 1 };
        let prev_x = at(vd, prev_k);
        let prev_y = prev_x - prev_k;
        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
            out.push((' ', a[x as usize]));
        }
        if d > 0 {
            if x == prev_x {
                y -= 1;
                out.push(('+', b[y as usize]));
            } else {
                x -= 1;
                out.push(('-', a[x as usize]));
            }
        }
    }
    out.reverse();
    out
}

/// A unified diff of two texts, "" when they're equal (`unifiedDiff`).
pub fn unified_diff(before: &str, after: &str, from: &str, to: &str, context: usize) -> String {
    if before == after {
        return String::new();
    }
    let split = |t: &'_ str| -> Vec<String> {
        if t.is_empty() {
            vec![]
        } else {
            t.strip_suffix('\n').unwrap_or(t).split('\n').map(str::to_string).collect()
        }
    };
    let (sa, sb) = (split(before), split(after));
    let a: Vec<&str> = sa.iter().map(String::as_str).collect();
    let b: Vec<&str> = sb.iter().map(String::as_str).collect();
    let ops = diff_lines(&a, &b);
    let mut lines = vec![format!("--- {from}"), format!("+++ {to}")];
    let (mut line_a, mut line_b) = (1usize, 1usize);
    let at: Vec<(usize, usize)> = ops
        .iter()
        .map(|(op, _)| {
            let here = (line_a, line_b);
            if *op != '+' {
                line_a += 1;
            }
            if *op != '-' {
                line_b += 1;
            }
            here
        })
        .collect();
    let mut i = 0;
    while i < ops.len() {
        while i < ops.len() && ops[i].0 == ' ' {
            i += 1;
        }
        if i >= ops.len() {
            break;
        }
        let start = i.saturating_sub(context);
        let mut end = i;
        // Extend while the next change is within 2 × context of this one.
        loop {
            while end < ops.len() && ops[end].0 != ' ' {
                end += 1;
            }
            let mut next = end;
            while next < ops.len() && ops[next].0 == ' ' {
                next += 1;
            }
            if next < ops.len() && next - end <= 2 * context {
                end = next;
            } else {
                end = ops.len().min(end + context);
                break;
            }
        }
        let hunk = &ops[start..end];
        let count_a = hunk.iter().filter(|(op, _)| *op != '+').count();
        let count_b = hunk.iter().filter(|(op, _)| *op != '-').count();
        let (a0, b0) = at[start];
        lines.push(format!(
            "@@ -{},{count_a} +{},{count_b} @@",
            if count_a > 0 { a0 } else { a0 - 1 },
            if count_b > 0 { b0 } else { b0 - 1 }
        ));
        for (op, line) in hunk {
            lines.push(format!("{op}{line}"));
        }
        i = end;
    }
    format!("{}\n", lines.join("\n"))
}
