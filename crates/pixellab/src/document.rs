//! The document (ports cli/lib/document.mjs;
//! docs/intents/2026-10-pixellab-document/spec.md, D2): a `.pixellab` folder
//! holding manifest.json, palette.json, objects/*.json and scenes/*.json in
//! format 2. Where it comes from (--document, PIXELLAB_DOC, the config,
//! --site, PIXELLAB_SITE, the default site), whether it looks like one, what
//! its manifest says, and its sources read fresh. A format-1 document gives
//! R9's problem. Filled in by U13.

/// The format this program reads and writes.
pub const FORMAT: i64 = 2;
pub const EXTENSION: &str = ".pixellab";
/// Where a site keeps its document, and the legacy sources it had before documents.
pub const SITE_ART_DIR: &str = "src/assets/pixel-art";
pub const LEGACY_SOURCE_DIR: &str = "src/assets/pixel-art/source";
/// The night theme's page color when a manifest names none.
pub const DEFAULT_DARK_PAGE: &str = "#1C1712";
