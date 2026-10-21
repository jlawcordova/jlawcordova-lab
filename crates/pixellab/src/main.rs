//! pixellab: the Pixel Lab program (ports cli/bin/pixellab.mjs).
//! `pixellab help` lists the commands.

use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let registry = pixellab::registry::registry();
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    let (mut out, mut err) = (stdout.lock(), stderr.lock());
    let code = {
        let mut io = pixellab::cli::Io {
            stdout: &mut out,
            stderr: &mut err,
            env: pixellab::document::process_env(),
            cwd: std::env::current_dir().unwrap_or_default(),
            registry: &registry,
        };
        pixellab::cli::main(&args, &mut io)
    };
    // A closed stdout (piped into head) stops quietly.
    let _ = out.flush();
    std::process::exit(code);
}
