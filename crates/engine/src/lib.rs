//! The Pixel Lab engine (docs/intents/2026-10-rust-engine/spec.md, D1): it
//! takes a document's palette, objects and scenes as data, validates them,
//! renders scenes and objects, puts items in depth order, plays motion,
//! compiles scenes to the SVGs a site inlines, and makes PNG previews. It
//! does no I/O beyond what its callers pass in, apart from reading a
//! document's folder (`format::read_docs`), writing a build's outputs and
//! asking git for the sources at HEAD (`compile`).
//!
//! Each module ports one file of the Node engine in `cli/lib/engine/`, and
//! names it. Documents are `serde_json::Value`s, as the Node engine held
//! plain JavaScript data; `jsnum` and `serialize` keep JavaScript's number
//! printing and key order, so outputs stay byte for byte the Node CLI's.

#![allow(clippy::type_complexity)]

pub mod ambient;
pub mod compile;
pub mod deflate;
pub mod depth;
pub mod edit;
#[allow(clippy::module_inception)]
pub mod engine;
pub mod format;
pub mod iso;
pub mod jsnum;
pub mod migrate;
pub mod motion;
pub mod omap;
pub mod png;
pub mod preview;
pub mod serialize;
pub mod sha256;
pub mod starter;
pub mod svg;

use std::fmt;

/// An error the engine throws: what the Node engine threw as a plain
/// `Error` (a missing object, an extends cycle, markup the optimizer can't
/// read). Callers that report it use its message as it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineError(pub String);

impl EngineError {
    pub fn new(message: impl Into<String>) -> Self {
        EngineError(message.into())
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for EngineError {}

pub type Result<T> = std::result::Result<T, EngineError>;
