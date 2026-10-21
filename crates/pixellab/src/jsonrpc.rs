//! Newline-delimited JSON-RPC 2.0 over a pair of streams, for both servers (ports cli/lib/jsonrpc.mjs).
//! One JSON message per line in, one per line out. Nothing else is ever
//! written to the output stream; logs go to stderr.
//!
//! The Node `serveLines` is split here into building blocks the servers put
//! together on one event loop: [`handle_line`] (one line in, the message to
//! send out), [`spawn_lines`] (a reader thread), [`Output`] (the shared
//! writer) and [`serve_lines`] (the simple synchronous loop).
//!
//! `consoleToStderr` has no Rust equivalent: the program never prints to
//! stdout from a server, so there is nothing to redirect.

#![allow(clippy::result_large_err)]

use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use serde_json::{Map, Value};

use crate::errors::{CommandError, Error};

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;
/// A command ran and failed (data.code says how).
pub const COMMAND_ERROR: i64 = -32000;

/// A JSON-RPC error with its own code.
#[derive(Clone, Debug, PartialEq)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        RpcError { code, message: message.into(), data: None }
    }

    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }
}

/// What a handler can fail with.
#[derive(Clone, Debug, PartialEq)]
pub enum Failure {
    Rpc(RpcError),
    Error(Error),
}

impl From<RpcError> for Failure {
    fn from(e: RpcError) -> Self {
        Failure::Rpc(e)
    }
}

impl From<Error> for Failure {
    fn from(e: Error) -> Self {
        Failure::Error(e)
    }
}

impl From<CommandError> for Failure {
    fn from(e: CommandError) -> Self {
        Failure::Error(Error::Command(e))
    }
}

/// An error as a JSON-RPC error object: usage errors are invalid params,
/// other command errors -32000, both with data: { code, details? }.
pub fn to_rpc_error(f: &Failure) -> Value {
    let mut out = Map::new();
    match f {
        Failure::Rpc(e) => {
            out.insert("code".into(), e.code.into());
            out.insert("message".into(), e.message.clone().into());
            if let Some(d) = &e.data {
                out.insert("data".into(), d.clone());
            }
        }
        Failure::Error(Error::Command(e)) => {
            let code = if e.is_usage() { INVALID_PARAMS } else { COMMAND_ERROR };
            out.insert("code".into(), code.into());
            out.insert("message".into(), e.message.clone().into());
            let mut data = Map::new();
            data.insert("code".into(), e.code.clone().into());
            if let Some(d) = &e.details {
                data.insert("details".into(), d.clone());
            }
            out.insert("data".into(), Value::Object(data));
        }
        Failure::Error(Error::Internal(m)) => {
            out.insert("code".into(), INTERNAL_ERROR.into());
            out.insert("message".into(), m.clone().into());
            let mut data = Map::new();
            data.insert("code".into(), "internal".into());
            out.insert("data".into(), Value::Object(data));
        }
    }
    Value::Object(out)
}

struct Inner {
    writer: Box<dyn Write + Send>,
    closed: bool,
}

/// The shared writer of one JSON message per line. Clones write to the same stream.
#[derive(Clone)]
pub struct Output {
    inner: Arc<Mutex<Inner>>,
}

impl Output {
    pub fn new(writer: Box<dyn Write + Send>) -> Self {
        Output { inner: Arc::new(Mutex::new(Inner { writer, closed: false })) }
    }

    /// Writes `message` and a newline, and flushes; nothing after `stop`.
    pub fn send(&self, message: &Value) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.closed {
            return;
        }
        let line = format!("{}\n", engine::format::stringify(message));
        let _ = inner.writer.write_all(line.as_bytes());
        let _ = inner.writer.flush();
    }

    /// Sends a notification.
    pub fn notify(&self, method: &str, params: Value) {
        let mut m = Map::new();
        m.insert("jsonrpc".into(), "2.0".into());
        m.insert("method".into(), method.into());
        m.insert("params".into(), params);
        self.send(&Value::Object(m));
    }

    /// Stops writing (after shutdown).
    pub fn stop(&self) {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).closed = true;
    }
}

fn error_response(id: Value, code: i64, message: String) -> Value {
    let mut err = Map::new();
    err.insert("code".into(), code.into());
    err.insert("message".into(), message.into());
    let mut m = Map::new();
    m.insert("jsonrpc".into(), "2.0".into());
    m.insert("id".into(), id);
    m.insert("error".into(), Value::Object(err));
    Value::Object(m)
}

/// What one input line comes to: the message to send back, if any.
/// `handle(method, params, message)` answers requests (its value is the
/// result; a failure is the error) and notifications (ignored).
pub fn handle_line(
    line: &str,
    handle: &mut dyn FnMut(&str, Value, &Value) -> Result<Value, Failure>,
    log: &mut dyn FnMut(&str),
) -> Option<Value> {
    if line.trim().is_empty() {
        return None;
    }
    let message: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return Some(error_response(Value::Null, PARSE_ERROR, format!("parse error: {e}"))),
    };
    let obj = message.as_object();
    let method =
        obj.filter(|o| o.get("jsonrpc").and_then(Value::as_str) == Some("2.0")).and_then(|o| o.get("method")).and_then(Value::as_str);
    let Some(method) = method else {
        // A response to something we sent (we send none that expect one) or junk.
        if let Some(o) = obj {
            if o.contains_key("result") || o.contains_key("error") {
                return None;
            }
        }
        let id = obj.and_then(|o| o.get("id")).filter(|v| !v.is_null()).cloned().unwrap_or(Value::Null);
        return Some(error_response(
            id,
            INVALID_REQUEST,
            "invalid request: expected a JSON-RPC 2.0 request object (batches are not supported)".into(),
        ));
    };
    let id = obj.and_then(|o| o.get("id")).filter(|v| !v.is_null()).cloned();
    let params = obj.and_then(|o| o.get("params")).filter(|v| !v.is_null()).cloned().unwrap_or_else(|| Value::Object(Map::new()));
    match handle(method, params, &message) {
        Ok(result) => id.map(|id| {
            let mut m = Map::new();
            m.insert("jsonrpc".into(), "2.0".into());
            m.insert("id".into(), id);
            m.insert("result".into(), result);
            Value::Object(m)
        }),
        Err(f) => {
            if let Failure::Error(Error::Internal(m)) = &f {
                log(&format!("internal error in {method}: {m}"));
            }
            id.map(|id| {
                let mut m = Map::new();
                m.insert("jsonrpc".into(), "2.0".into());
                m.insert("id".into(), id);
                m.insert("error".into(), to_rpc_error(&f));
                Value::Object(m)
            })
        }
    }
}

/// A reader thread: calls `each(Some(line))` per line (a trailing `\r` is
/// stripped) and `each(None)` when the input ends.
pub fn spawn_lines(input: impl BufRead + Send + 'static, mut each: impl FnMut(Option<String>) + Send + 'static) -> JoinHandle<()> {
    std::thread::spawn(move || {
        for line in input.split(b'\n') {
            let Ok(bytes) = line else { break };
            let mut s = String::from_utf8_lossy(&bytes).into_owned();
            if s.ends_with('\r') {
                s.pop();
            }
            each(Some(s));
        }
        each(None);
    })
}

/// The simple synchronous loop: reads each line, answers it, until the input ends.
pub fn serve_lines(
    input: impl BufRead,
    output: &Output,
    handle: &mut dyn FnMut(&str, Value, &Value) -> Result<Value, Failure>,
    log: &mut dyn FnMut(&str),
) {
    for line in input.split(b'\n') {
        let Ok(bytes) = line else { break };
        let mut s = String::from_utf8_lossy(&bytes).into_owned();
        if s.ends_with('\r') {
            s.pop();
        }
        if let Some(reply) = handle_line(&s, handle, log) {
            output.send(&reply);
        }
    }
}
