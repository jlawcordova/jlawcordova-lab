//! Format 1 → 2 (docs/intents/2026-10-rust-engine/spec.md, R8, D3): reads the
//! subset of JavaScript that `serialize.mjs` wrote and the owner's palette
//! uses (`//` comments, `export default` and one object literal, identifier
//! or quoted keys, single- or double-quoted strings, numbers, booleans,
//! arrays and nested objects, trailing commas), and carries comments into
//! `notes` and `colorNotes`. Anything else is refused with its file, line
//! and column. Filled in by U29.
