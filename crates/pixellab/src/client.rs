//! A client of the app's server (docs/intents/2026-10-autosave/spec.md, D10): the CLI and an MCP
//! server look for the owner of their document before each command (owner.rs), and when there is
//! one they send it the command over its socket instead of writing the files themselves. They say
//! who they are first (`client.hello`), so the owner records the right author.
//!
//! A request whose answer never comes, because the owner went away mid-call, fails with
//! `owner-gone` and is never sent again (R24). A client of another protocol may only ask the owner
//! to save (`doc.flush`) and read from disk; its edits refuse with `version` (R28).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use engine::format::stringify;
use serde_json::{json, Value};

use crate::errors::{CommandError, Error};
use crate::owner::{self, PROTOCOL};
use crate::server_stdio::version;

/// How long a client waits for the owner's answer: long enough for a big render.
const ANSWER_WAIT: Duration = Duration::from_secs(120);

/// A connection to the owner of a document.
pub struct Owner {
    stream: BufReader<UnixStream>,
    next: u64,
    /// The owner's lock contents ({ pid, appPid, socket, version, … }).
    pub holder: Value,
    /// Whether the owner speaks this client's protocol: false, only reads and saves go through.
    pub accepted: bool,
    /// The owner's program version, for the `version` refusal.
    pub owner_version: String,
}

/// Why a call to the owner didn't answer.
#[derive(Debug)]
pub enum Failed {
    /// The connection ended before the answer: the owner went away mid-call.
    Gone,
    /// The owner answered with an error (the command failed there).
    Rpc(Value),
}

impl Owner {
    /// Connects to the owner of the document at `root`, if one holds it, and says hello as `hello`
    /// ({ kind, name?, chat? }). None: no one owns it. An owner that holds the lock but can't be
    /// reached is an error: writing the files under it would race it.
    pub fn connect(state_dir: &Path, root: &Path, hello: Value) -> Option<Result<Owner, Error>> {
        let holder = owner::holder(state_dir, root)?;
        let socket = holder.get("socket").and_then(Value::as_str).map(PathBuf::from).unwrap_or_else(|| owner::socket_path(state_dir, root));
        let mut last = None;
        for _ in 0..20 {
            match UnixStream::connect(&socket) {
                Ok(stream) => {
                    let _ = stream.set_read_timeout(Some(ANSWER_WAIT));
                    let mut o = Owner { stream: BufReader::new(stream), next: 0, holder, accepted: false, owner_version: String::new() };
                    let mut params = hello;
                    params["protocol"] = json!(PROTOCOL);
                    params["version"] = json!(version());
                    return Some(match o.call("client.hello", params) {
                        Ok(r) => {
                            o.accepted = r["accepted"] == json!(true);
                            o.owner_version = r.get("version").and_then(Value::as_str).unwrap_or("?").to_string();
                            Ok(o)
                        }
                        Err(f) => Err(failure(f, "client.hello")),
                    });
                }
                Err(e) => last = Some(e),
            }
            // The owner may have only just taken the lock.
            std::thread::sleep(Duration::from_millis(25));
        }
        // Gone between the lock check and now: no owner after all.
        owner::holder(state_dir, root)?;
        let why = last.map(|e| e.to_string()).unwrap_or_default();
        Some(Err(CommandError::new(
            format!("Pixel Lab has this document open but can't be reached ({why}); try again, or quit Pixel Lab"),
            "owner-unreachable",
        )
        .into()))
    }

    /// Sends one request and waits for its answer.
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, Failed> {
        self.next += 1;
        let id = self.next;
        let line = stringify(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        if writeln!(self.stream.get_mut(), "{line}").and_then(|_| self.stream.get_mut().flush()).is_err() {
            return Err(Failed::Gone);
        }
        loop {
            let mut reply = String::new();
            match self.stream.read_line(&mut reply) {
                Ok(0) | Err(_) => return Err(Failed::Gone),
                Ok(_) => {}
            }
            let Ok(message) = serde_json::from_str::<Value>(&reply) else { continue };
            if message.get("id") != Some(&json!(id)) {
                continue;
            }
            if let Some(err) = message.get("error") {
                return Err(Failed::Rpc(err.clone()));
            }
            return Ok(message.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    /// The `version` refusal for an edit through an owner of another protocol (R28).
    pub fn version_error(&self) -> Error {
        CommandError::new(
            format!(
                "Pixel Lab ({}) has this document open, and this pixellab is {}. Use the app's bundled pixellab, or close the document in Pixel Lab.",
                self.owner_version,
                version()
            ),
            "version",
        )
        .into()
    }
}

/// A failed call as the error a local run would give: the owner's command error as it was, or
/// `owner-gone` naming what was running (R24).
pub fn failure(f: Failed, what: &str) -> Error {
    match f {
        Failed::Gone => CommandError::new(
            format!("Pixel Lab closed the document while {what} ran; check what's there before trying again"),
            "owner-gone",
        )
        .into(),
        Failed::Rpc(err) => {
            let data = err.get("data").cloned().unwrap_or(Value::Null);
            let code = data.get("code").and_then(Value::as_str).unwrap_or("internal").to_string();
            let message = err.get("message").and_then(Value::as_str).unwrap_or("the command failed in Pixel Lab").to_string();
            let mut e = if code == "usage" { CommandError::usage(message) } else { CommandError::new(message, &code) };
            if let Some(d) = data.get("details").filter(|d| !d.is_null()) {
                e = e.with_details(d.clone());
            }
            e.into()
        }
    }
}
