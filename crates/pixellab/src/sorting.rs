//! Depth order in scene edits (ports `sortScene`, `stayed` and `pathChanges`
//! from cli/lib/scene-ops.mjs; docs/intents/2026-10-depth-order/spec.md,
//! R4): every scene edit ends by putting each list of items in depth order,
//! so the file, scene items and the Layers panel read back to front. Kept
//! apart from scene_ops.rs because the session uses them on every edit.

use std::collections::HashMap;

use engine::engine::{depth_order, origin_of, tracks_of, Sources};
use engine::omap::OMap;
use serde_json::{Map, Value};

fn is_group(item: &Value) -> bool {
    item.as_object().is_some_and(|o| o.contains_key("group"))
}

/// "5.2" for [5, 2].
pub fn show(path: &[usize]) -> String {
    path.iter().map(usize::to_string).collect::<Vec<_>>().join(".")
}

/// Two paths in `localeCompare(…, 'en', { numeric: true })` order: number
/// by number, a shorter path (a prefix) first.
pub fn compare_paths(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| s.split('.').map(|n| n.parse::<u64>().unwrap_or(u64::MAX)).collect::<Vec<_>>();
    parse(a).cmp(&parse(b))
}

/// The old indices that keep their relative order in `order` (`stayed`):
/// the longest such run, the earliest on a tie.
fn stayed(order: &[usize]) -> std::collections::HashSet<usize> {
    let n = order.len();
    let mut length = vec![1usize; n];
    let mut from = vec![-1i64; n];
    let mut best: i64 = -1;
    for k in 0..n {
        for j in 0..k {
            if order[j] < order[k] && length[j] + 1 > length[k] {
                length[k] = length[j] + 1;
                from[k] = j as i64;
            }
        }
        if best < 0 || length[k] > length[best as usize] {
            best = k as i64;
        }
    }
    let mut out = std::collections::HashSet::new();
    let mut k = best;
    while k >= 0 {
        out.insert(order[k as usize]);
        k = from[k as usize];
    }
    out
}

/// What a sort did: the items it took out of their place (`{ path, to,
/// object | group }`, by path before), and every path that changed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sorted {
    pub moved: Vec<Value>,
    pub remap: OMap<String, String>,
}

/// A scene's items as ids, with their paths: the same tree, so a reorder of
/// the items can be followed.
#[derive(Clone, Debug)]
struct Node {
    id: usize,
    children: Option<Vec<Node>>,
}

fn id_tree(items: &[Value], next: &mut usize) -> Vec<Node> {
    items
        .iter()
        .map(|item| {
            let id = *next;
            *next += 1;
            let children = if is_group(item) {
                Some(id_tree(item.get("items").and_then(Value::as_array).map_or(&[][..], |v| v.as_slice()), next))
            } else {
                None
            };
            Node { id, children }
        })
        .collect()
}

fn paths_of(nodes: &[Node], prefix: &mut Vec<usize>, out: &mut HashMap<usize, String>) {
    for (i, node) in nodes.iter().enumerate() {
        prefix.push(i);
        out.insert(node.id, show(prefix));
        if let Some(children) = &node.children {
            paths_of(children, prefix, out);
        }
        prefix.pop();
    }
}

fn list_at<'a>(items: &'a mut Vec<Value>, path: &[usize]) -> &'a mut Vec<Value> {
    match path.split_first() {
        None => items,
        Some((first, rest)) => {
            let group = &mut items[*first];
            if !group.get("items").is_some_and(Value::is_array) {
                group["items"] = Value::Array(vec![]);
            }
            list_at(group["items"].as_array_mut().unwrap(), rest)
        }
    }
}

fn nodes_at<'a>(nodes: &'a mut Vec<Node>, path: &[usize]) -> &'a mut Vec<Node> {
    match path.split_first() {
        None => nodes,
        Some((first, rest)) => nodes_at(nodes[*first].children.get_or_insert_with(Vec::new), rest),
    }
}

/// Puts every list of the scene's items in depth order, in place
/// (`sortScene`). Nothing is touched if the order can't be worked out (an
/// item names an object that doesn't resolve).
pub fn sort_scene(sources: &Sources, doc: &mut Value) -> Sorted {
    let origin = origin_of(doc);
    let tracks = tracks_of(doc);
    let items: Vec<Value> = doc.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
    // The lists, inner ones first, each with its order.
    let mut plans: Vec<(Vec<usize>, Vec<usize>)> = Vec::new();
    fn plan(
        sources: &Sources,
        list: &[Value],
        path: &mut Vec<usize>,
        origin: [f64; 2],
        tracks: &OMap<String, Value>,
        plans: &mut Vec<(Vec<usize>, Vec<usize>)>,
    ) -> bool {
        for (i, item) in list.iter().enumerate() {
            if is_group(item) {
                path.push(i);
                let inner = item.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
                let ok = plan(sources, &inner, path, origin, tracks, plans);
                path.pop();
                if !ok {
                    return false;
                }
            }
        }
        match depth_order(sources, list, origin, tracks) {
            Ok(order) => {
                plans.push((path.clone(), order));
                true
            }
            Err(_) => false,
        }
    }
    if !plan(sources, &items, &mut Vec::new(), origin, &tracks, &mut plans) {
        return Sorted::default();
    }
    if plans.iter().all(|(_, order)| order.iter().enumerate().all(|(k, &old)| old == k)) {
        return Sorted::default();
    }
    let mut next = 0;
    let mut tree = id_tree(&items, &mut next);
    let mut before = HashMap::new();
    paths_of(&tree, &mut Vec::new(), &mut before);
    let mut jumped: Vec<(usize, Value)> = Vec::new();
    let mut list_items = items;
    for (path, order) in &plans {
        let keep = stayed(order);
        let list = list_at(&mut list_items, path);
        let nodes = nodes_at(&mut tree, path);
        for &old in order {
            if !keep.contains(&old) {
                jumped.push((nodes[old].id, list[old].clone()));
            }
        }
        let (old_items, old_nodes) = (list.clone(), nodes.clone());
        for (k, &old) in order.iter().enumerate() {
            list[k] = old_items[old].clone();
            nodes[k] = old_nodes[old].clone();
        }
    }
    let mut after = HashMap::new();
    paths_of(&tree, &mut Vec::new(), &mut after);
    let mut remap = OMap::new();
    let mut ids: Vec<usize> = before.keys().copied().collect();
    ids.sort();
    for id in ids {
        if after[&id] != before[&id] {
            remap.set(before[&id].clone(), after[&id].clone());
        }
    }
    let mut moved: Vec<Value> = jumped
        .iter()
        .map(|(id, item)| {
            let mut m = Map::new();
            m.insert("path".into(), Value::String(before[id].clone()));
            m.insert("to".into(), Value::String(after[id].clone()));
            if is_group(item) {
                m.insert("group".into(), Value::Bool(true));
            } else {
                if let Some(object) = item.get("object") {
                    m.insert("object".into(), object.clone());
                }
            }
            Value::Object(m)
        })
        .collect();
    moved.sort_by(|a, b| compare_paths(a["path"].as_str().unwrap(), b["path"].as_str().unwrap()));
    doc["items"] = Value::Array(list_items);
    Sorted { moved, remap }
}

fn leaves(items: Option<&Value>, prefix: &mut Vec<usize>, out: &mut Vec<(String, Value)>) {
    for (i, item) in items.and_then(Value::as_array).into_iter().flatten().enumerate() {
        prefix.push(i);
        if is_group(item) {
            leaves(item.get("items"), prefix, out);
        } else {
            out.push((show(prefix), item.clone()));
        }
        prefix.pop();
    }
}

fn first_leaf(group: &Value, path: &mut Vec<usize>) -> Option<String> {
    let first = group.get("items").and_then(Value::as_array).and_then(|l| l.first())?;
    path.push(0);
    let out = if is_group(first) { first_leaf(first, path) } else { Some(show(path)) };
    path.pop();
    out
}

/// Where a scene's items and groups went between two states of the same
/// scene (`pathChanges`): old path → new path, for the paths that changed.
pub fn path_changes(before: &Value, after: &Value) -> OMap<String, String> {
    let (mut was, mut now) = (Vec::new(), Vec::new());
    leaves(before.get("items"), &mut Vec::new(), &mut was);
    leaves(after.get("items"), &mut Vec::new(), &mut now);
    let mut to: OMap<String, String> = OMap::new();
    let mut taken = std::collections::HashSet::new();
    let mut by_content: HashMap<String, std::collections::VecDeque<usize>> = HashMap::new();
    for (i, (_, item)) in now.iter().enumerate() {
        by_content.entry(engine::format::stringify(item)).or_default().push_back(i);
    }
    let mut left = Vec::new();
    for (path, item) in &was {
        match by_content.get_mut(&engine::format::stringify(item)).and_then(|q| q.pop_front()) {
            Some(i) => {
                taken.insert(i);
                to.set(path.clone(), now[i].0.clone());
            }
            None => left.push((path.clone(), item.clone())),
        }
    }
    let object_key = |item: &Value| item.get("object").map(engine::format::stringify).unwrap_or_default();
    let mut by_object: HashMap<String, std::collections::VecDeque<usize>> = HashMap::new();
    for (i, (_, item)) in now.iter().enumerate() {
        if !taken.contains(&i) {
            by_object.entry(object_key(item)).or_default().push_back(i);
        }
    }
    for (path, item) in &left {
        if let Some(i) = by_object.get_mut(&object_key(item)).and_then(|q| q.pop_front()) {
            to.set(path.clone(), now[i].0.clone());
        }
    }
    // A group goes where its first item went (the same depth of nesting).
    let mut groups_now = std::collections::HashSet::new();
    fn collect(items: Option<&Value>, prefix: &mut Vec<usize>, out: &mut std::collections::HashSet<String>) {
        for (i, item) in items.and_then(Value::as_array).into_iter().flatten().enumerate() {
            if !is_group(item) {
                continue;
            }
            prefix.push(i);
            out.insert(show(prefix));
            collect(item.get("items"), prefix, out);
            prefix.pop();
        }
    }
    collect(after.get("items"), &mut Vec::new(), &mut groups_now);
    fn groups(
        items: Option<&Value>,
        prefix: &mut Vec<usize>,
        to: &mut OMap<String, String>,
        groups_now: &std::collections::HashSet<String>,
    ) {
        for (i, item) in items.and_then(Value::as_array).into_iter().flatten().enumerate() {
            if !is_group(item) {
                continue;
            }
            prefix.push(i);
            let moved = first_leaf(item, &mut prefix.clone());
            if let Some(there) = moved.and_then(|m| to.get(&m).cloned()) {
                let parent = there.split('.').take(prefix.len()).collect::<Vec<_>>().join(".");
                if groups_now.contains(&parent) {
                    to.set(show(prefix), parent);
                }
            }
            groups(item.get("items"), prefix, to, groups_now);
            prefix.pop();
        }
    }
    groups(before.get("items"), &mut Vec::new(), &mut to, &groups_now);
    let same: Vec<String> = to.iter().filter(|(k, v)| k == v).map(|(k, _)| k.clone()).collect();
    for k in same {
        to.delete(&k);
    }
    to
}

/// `Object.fromEntries(remap)`.
pub fn remap_json(remap: &OMap<String, String>) -> Value {
    Value::Object(remap.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect())
}
