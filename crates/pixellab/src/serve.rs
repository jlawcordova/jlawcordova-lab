//! `pixellab serve --stdio | --mcp [--direct] [--document <path> | --site
//! <path>]` (ports cli/lib/serve.mjs): starts one of the servers on stdin
//! and stdout. Logs go to stderr.

use crate::cli::Io;
use crate::document::{open_document, OpenOptions};
use crate::errors::{CommandError, Error};

pub const SERVE_HELP: &str = "Usage: pixellab serve --stdio [--document <path.pixellab> | --site <checkout>]
       pixellab serve --mcp [--direct] [--document <path.pixellab> | --site <checkout>]

Starts a server on stdin and stdout (newline-delimited JSON-RPC 2.0; logs on stderr).
  --stdio   the Mac app's server: methods are the command names, plus session.info and
            shutdown; edits stay in memory with undo until doc.save writes them
  --mcp     an MCP server for agents: tools are the commands (scene.place → scene_place);
            doc_save writes a proposal to the state dir for a person to approve
  --direct  with --mcp: doc_save writes the document instead of a proposal

Register with Claude Code:
  claude mcp add pixellab -- <path to pixellab> serve --mcp

The document comes from --document, else $PIXELLAB_DOC, else ~/.config/pixellab/config.json, else
--site / $PIXELLAB_SITE (the document inside a site checkout). The state dir is $PIXELLAB_STATE,
else ~/Library/Application Support/Pixel Lab/. See docs/cli.md.";

#[derive(Default)]
struct Flags {
    stdio: bool,
    mcp: bool,
    direct: bool,
    help: bool,
    site: Option<String>,
    document: Option<String>,
}

fn parse(argv: &[String]) -> Result<Flags, CommandError> {
    let mut flags = Flags::default();
    let mut i = 0;
    while i < argv.len() {
        let token = argv[i].as_str();
        match token {
            "--stdio" => flags.stdio = true,
            "--mcp" => flags.mcp = true,
            "--direct" => flags.direct = true,
            "--help" | "-h" => flags.help = true,
            _ => {
                let path = ["site", "document"].into_iter().find_map(|key| {
                    let flag = format!("--{key}");
                    if token == flag {
                        Some((key, None))
                    } else {
                        token.strip_prefix(&format!("{flag}=")).map(|v| (key, Some(v.to_string())))
                    }
                });
                let Some((key, value)) = path else {
                    return Err(CommandError::usage(format!("serve: unknown argument {token}; run pixellab serve --help")));
                };
                let value = match value {
                    Some(v) => v,
                    None => {
                        if i + 1 >= argv.len() {
                            return Err(CommandError::usage(format!("--{key} needs a value")));
                        }
                        i += 1;
                        argv[i].clone()
                    }
                };
                if key == "site" {
                    flags.site = Some(value);
                } else {
                    flags.document = Some(value);
                }
            }
        }
        i += 1;
    }
    Ok(flags)
}

/// Starts a server (`serve`); returns the exit code when it stops.
pub fn serve(argv: &[String], io: &mut Io) -> i32 {
    match run(argv, io) {
        Ok(code) => code,
        Err(Error::Command(e)) => {
            let _ = writeln!(io.stderr, "pixellab: {}", e.message);
            e.exit_code
        }
        Err(e) => {
            let _ = writeln!(io.stderr, "pixellab: internal error: {}", e.message());
            1
        }
    }
}

fn run(argv: &[String], io: &mut Io) -> Result<i32, Error> {
    let flags = parse(argv)?;
    if flags.help {
        let _ = writeln!(io.stdout, "{SERVE_HELP}");
        return Ok(0);
    }
    if flags.stdio == flags.mcp {
        return Err(CommandError::usage("serve: give --stdio or --mcp; run pixellab serve --help").into());
    }
    if flags.direct && !flags.mcp {
        return Err(CommandError::usage("serve: --direct goes with --mcp").into());
    }
    let document = open_document(&OpenOptions { doc: flags.document, site: flags.site, env: io.env.clone(), cwd: io.cwd.clone() })?;
    if flags.stdio {
        return Ok(crate::server_stdio::start(
            document,
            io.env.clone(),
            io.registry,
            Box::new(std::io::BufReader::new(std::io::stdin())),
            Box::new(std::io::stdout()),
            true,
        ));
    }
    Ok(crate::server_mcp::start(
        document,
        io.env.clone(),
        io.registry,
        flags.direct,
        Box::new(std::io::BufReader::new(std::io::stdin())),
        Box::new(std::io::stdout()),
    ))
}
