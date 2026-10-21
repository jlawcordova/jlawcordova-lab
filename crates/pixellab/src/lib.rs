//! The pixellab program (docs/intents/2026-10-rust-engine/spec.md, D1, R1):
//! the CLI, the Mac app's stdio server and the MCP server, over one command
//! registry. Each module ports one file of the Node CLI in `cli/lib/`, and
//! names it. The executable is `main.rs`; the library is here so the tests
//! can call the doors directly.

pub mod activity_summary;
pub mod argv;
pub mod chat_context;
pub mod cli;
pub mod commands;
pub mod context;
pub mod diff;
pub mod document;
pub mod errors;
pub mod help;
pub mod jsonrpc;
pub mod layer_ops;
pub mod merge;
pub mod motion_ops;
pub mod open_docs;
pub mod params;
pub mod registry;
pub mod render;
pub mod scene_ops;
pub mod serve;
pub mod server_mcp;
pub mod server_stdio;
pub mod session;
pub mod state;
pub mod watch;
