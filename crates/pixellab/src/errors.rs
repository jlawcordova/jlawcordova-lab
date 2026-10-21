//! Errors every door understands (ports cli/lib/errors.mjs). A
//! `CommandError` carries a stable `code` and the CLI exit code: 1 for a
//! command that ran and failed, 2 for a usage error. Anything else that goes
//! wrong is an internal error, as a plain `Error` was in the Node CLI.

use std::fmt;

use serde_json::{Map, Value};

/// A command that ran and failed, or was called wrong.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandError {
    pub message: String,
    pub code: String,
    pub exit_code: i32,
    pub details: Option<Value>,
}

impl CommandError {
    /// `new CommandError(message, { code })`, exit code 1.
    pub fn new(message: impl Into<String>, code: &str) -> Self {
        CommandError { message: message.into(), code: code.into(), exit_code: 1, details: None }
    }

    /// With `details`.
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    /// `UsageError`: bad params, an unknown command or flag; exit code 2.
    pub fn usage(message: impl Into<String>) -> Self {
        CommandError { message: message.into(), code: "usage".into(), exit_code: 2, details: None }
    }

    /// `NotFoundError`: a scene, object or file that doesn't exist.
    pub fn not_found(message: impl Into<String>) -> Self {
        CommandError::new(message, "not_found")
    }

    /// `DocumentError`: the document can't be found or isn't one.
    pub fn document(message: impl Into<String>) -> Self {
        CommandError::new(message, "document")
    }

    /// Whether it's a usage error (`instanceof UsageError`).
    pub fn is_usage(&self) -> bool {
        self.code == "usage" && self.exit_code == 2
    }

    /// `toJSON()`: `{ code, message, details? }`.
    pub fn to_json(&self) -> Value {
        let mut out = Map::new();
        out.insert("code".into(), Value::String(self.code.clone()));
        out.insert("message".into(), Value::String(self.message.clone()));
        if let Some(d) = &self.details {
            out.insert("details".into(), d.clone());
        }
        Value::Object(out)
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// What a command, a session or a server can fail with.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// A `CommandError` (or one of its kinds).
    Command(CommandError),
    /// Anything else: what the Node CLI threw as a plain `Error`.
    Internal(String),
}

impl Error {
    pub fn message(&self) -> &str {
        match self {
            Error::Command(e) => &e.message,
            Error::Internal(m) => m,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for Error {}

impl From<CommandError> for Error {
    fn from(e: CommandError) -> Self {
        Error::Command(e)
    }
}

impl From<engine::EngineError> for Error {
    fn from(e: engine::EngineError) -> Self {
        Error::Internal(e.0)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Internal(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
