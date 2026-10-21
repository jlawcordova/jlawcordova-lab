//! What a command runs against (ports cli/lib/context.mjs). Commands never
//! read the disk themselves: they ask the ctx for documents and engine
//! sources, and the session's working copies stand in for what's on disk.
//!
//! A ctx is made per call over a session (`Ctx::new`), or without one for a
//! command that runs without a document (`document: false`). The sources
//! are kept by the session until an edit changes its working copies; the
//! disk is read once per call (`Session::begin_call`).

use std::collections::BTreeMap;
use std::rc::Rc;

use engine::engine::Sources;
use engine::format::FileStat;
use serde_json::Value;

use crate::document::Document;
use crate::errors::{CommandError, Result};
use crate::session::{Edit, Session};

/// The engine's sources with the session's working copies in place of
/// what's on disk, and the files that didn't load.
#[derive(Debug)]
pub struct Loaded {
    pub sources: Sources,
    pub load_errors: Vec<String>,
}

/// A doc's kind: the palette, an object or a scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DocKind {
    Palette,
    Object,
    Scene,
}

impl DocKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DocKind::Palette => "palette",
            DocKind::Object => "object",
            DocKind::Scene => "scene",
        }
    }

    /// "object" or "scene".
    pub fn parse(text: &str) -> Option<DocKind> {
        match text {
            "object" => Some(DocKind::Object),
            "scene" => Some(DocKind::Scene),
            "palette" => Some(DocKind::Palette),
            _ => None,
        }
    }
}

/// What a command found by name (`ctx.find`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub kind: DocKind,
    pub name: String,
    pub file: String,
}

/// `docFile`: the file of a doc.
pub fn doc_file(kind: DocKind, name: &str) -> String {
    match kind {
        DocKind::Palette => engine::format::PALETTE_FILE.to_string(),
        DocKind::Object => engine::format::object_file(name),
        DocKind::Scene => engine::format::scene_file(name),
    }
}

/// A command's context: the session it runs in (None for a command that
/// runs without a document).
pub struct Ctx<'a> {
    pub session: Option<&'a mut Session>,
}

impl<'a> Ctx<'a> {
    /// A ctx for a command that runs without a document (`document: false`).
    pub fn none() -> Ctx<'static> {
        Ctx { session: None }
    }

    /// A ctx for one call over a session.
    pub fn new(session: &'a mut Session) -> Ctx<'a> {
        session.begin_call();
        Ctx { session: Some(session) }
    }

    /// The session (`sessionOf`): a command that needs one and has none is
    /// an error.
    pub fn session(&mut self) -> Result<&mut Session> {
        match self.session.as_deref_mut() {
            Some(s) => Ok(s),
            None => Err(CommandError::new("this command needs a session (pixellab serve --stdio or --mcp)", "no_session").into()),
        }
    }

    /// The open document (`ctx.document`, `ctx.site`).
    pub fn document(&self) -> Result<&Document> {
        match self.session.as_deref() {
            Some(s) => Ok(&s.document),
            None => Err(CommandError::new("this command needs a document", "no_session").into()),
        }
    }

    /// Forgets the merged sources (`invalidate`).
    pub fn invalidate(&mut self) {
        if let Some(s) = self.session.as_deref_mut() {
            s.invalidate();
        }
    }

    /// The engine's sources, with the working copies (`ctx.sources()`).
    pub fn sources(&mut self) -> Result<Rc<Loaded>> {
        self.session()?.loaded()
    }

    /// One document's data (`ctx.doc`).
    pub fn doc(&mut self, kind: DocKind, name: &str) -> Result<Value> {
        self.session()?.doc(kind, name)
    }

    /// The names of every object or scene (`ctx.names`).
    pub fn names(&mut self, kind: DocKind) -> Result<Vec<String>> {
        let loaded = self.sources()?;
        let names = match kind {
            DocKind::Object => loaded.sources.objects.keys().cloned().collect(),
            _ => loaded.sources.scenes.keys().cloned().collect(),
        };
        Ok(names)
    }

    /// The load error of a doc's file, if it exists but didn't load.
    pub fn load_error(&mut self, kind: DocKind, name: &str) -> Result<Option<String>> {
        self.session()?.load_error(kind, name)
    }

    /// Files on disk, with their stats (`ctx.files()`).
    pub fn files(&mut self) -> Result<BTreeMap<String, FileStat>> {
        Ok(self.session()?.disk()?.files.clone())
    }

    /// Finds an object or scene by name, or by objects/<name> or
    /// scenes/<name> (a trailing .json is fine) (`ctx.find`).
    pub fn find(&mut self, query: &str, kind: Option<DocKind>) -> Result<Found> {
        self.session()?.find(query, kind)
    }

    /// Edits a doc as one undo step, opening it first if needed (`ctx.edit`).
    /// `key` coalesces rapid repeats with the same key.
    pub fn edit(
        &mut self,
        kind: DocKind,
        name: &str,
        key: Option<&str>,
        f: impl FnOnce(&mut Value, &Loaded) -> Result<Edit>,
    ) -> Result<Value> {
        self.session()?.edit(kind, name, key, f)
    }
}
