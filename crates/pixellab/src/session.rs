//! A session: open documents held in memory as working copies, each with its
//! own undo and redo stacks, plugged into every command's ctx (ports
//! cli/lib/session.mjs). Mutating commands edit through `ctx.edit`; nothing
//! is written until `save`. Saving validates, serializes with the canonical
//! format (format 2), refuses a conflict, and writes atomically, or in
//! 'propose' mode (MCP) writes a proposal. Filled in by U14.

/// How a session saves: the document's files, or proposals (MCP).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Direct,
    Propose,
}

pub const UNDO_LIMIT: usize = 200;
pub const COALESCE_MS: f64 = 1000.0;

/// The open docs of one door's run.
pub struct Session {
    pub mode: Mode,
}

/// sha256 of a source's text, hex (`hashText`).
pub fn hash_text(text: &str) -> String {
    engine::sha256::hex(text.as_bytes())
}

/// "scene:hero-island" (`docRef`).
pub fn doc_ref(kind: &str, name: &str) -> String {
    format!("{kind}:{name}")
}
