//! pixellab serve --stdio | --mcp: starts one of the servers on stdin and stdout (ports cli/lib/serve.mjs).

/// `pixellab serve …`: the stdio or MCP server. Filled in by U28.
pub fn serve(argv: &[String], io: &mut crate::cli::Io) -> i32 {
    let _ = (argv, io);
    todo!("U28")
}
