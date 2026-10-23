//! Autosave and authors (docs/intents/2026-10-autosave/spec.md): who made
//! each undo step, and the Edit menu's labels.

mod common;

use pixellab::author::Author;
use pixellab::context::{Ctx, DocKind};
use pixellab::document::{open_document, OpenOptions};
use pixellab::session::{Edit, Options, Session};
use serde_json::{json, Map, Value};

fn session(doc: &common::TempDocument) -> Session {
    let env = doc.env();
    let document = open_document(&OpenOptions {
        doc: Some(doc.root.to_string_lossy().into_owned()),
        site: None,
        env: env.clone(),
        cwd: doc.root.clone(),
    })
    .unwrap();
    Session::new(document, Options { env, ..Options::default() })
}

fn place(s: &mut Session, col: i64, key: Option<&str>) -> Value {
    let mut ctx = Ctx::new(s);
    ctx.edit(DocKind::Scene, "sample", key, |doc, _| {
        doc["items"].as_array_mut().unwrap().push(json!({ "object": "pebble", "at": { "tile": [col, 2, 0] } }));
        Ok(Edit::new(Map::new(), format!("Placed pebble at {col}")))
    })
    .unwrap()
}

#[test]
fn r20_the_edit_menu_names_an_agents_step() {
    let doc = common::make_document();
    let mut s = session(&doc);
    place(&mut s, 1, None);
    let live = s.live_status("scene:sample");
    assert_eq!(live["undoLabel"], "Placed pebble at 1");
    s.author = Author::agent(Some("claude-code 2.1.0".into()), None);
    place(&mut s, 2, None);
    let live = s.live_status("scene:sample");
    assert_eq!(live["undoLabel"], "Placed pebble at 2 (Claude)");
    assert_eq!(live["author"], json!({ "kind": "agent", "name": "claude-code 2.1.0" }));
    s.author = Author::owner();
    s.step("scene:sample", true).unwrap();
    let live = s.live_status("scene:sample");
    assert_eq!(live["undoLabel"], "Placed pebble at 1");
    assert_eq!(live["redoLabel"], "Placed pebble at 2 (Claude)");
}

#[test]
fn r20_edits_by_two_authors_never_coalesce() {
    let doc = common::make_document();
    let mut s = session(&doc);
    place(&mut s, 1, Some("drag"));
    s.author = Author::agent(Some("claude".into()), None);
    place(&mut s, 2, Some("drag"));
    assert_eq!(s.entries.get("scene:sample").unwrap().undo.len(), 2);
}
