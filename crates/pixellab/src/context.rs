//! What a command runs against (ports cli/lib/context.mjs). Commands never
//! read the disk themselves: they ask the ctx for documents and engine
//! sources, and a session's working copies stand in for what's on disk.
//!
//! A one-shot CLI run, the app's stdio server and the MCP server each make a
//! ctx per call over their session. The exact methods are settled by U14
//! (session.rs), before the command units start.

use std::rc::Rc;

use engine::engine::Sources;

use crate::session::Session;

/// The engine's sources with the session's working copies in place of
/// what's on disk, and the files that didn't load.
pub struct Loaded {
    pub sources: Sources,
    pub load_errors: Vec<String>,
}

/// A doc's kind: the palette, an object or a scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocKind {
    Palette,
    Object,
    Scene,
}

/// What a command found by name (`ctx.find`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub kind: DocKind,
    pub name: String,
    pub file: String,
}

/// A command's context: the session it runs in (None for a command that
/// runs without a document, `document: false`), and the sources it loaded,
/// kept until an edit invalidates them.
pub struct Ctx<'a> {
    pub session: Option<&'a mut Session>,
    pub loaded: Option<Rc<Loaded>>,
}

/// `docFile`: the file of a doc.
pub fn doc_file(kind: DocKind, name: &str) -> String {
    match kind {
        DocKind::Palette => engine::format::PALETTE_FILE.to_string(),
        DocKind::Object => engine::format::object_file(name),
        DocKind::Scene => engine::format::scene_file(name),
    }
}
