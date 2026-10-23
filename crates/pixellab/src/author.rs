//! Who made an edit or a version (docs/intents/2026-10-autosave/spec.md,
//! D8): the owner in the app, an agent over MCP, the CLI, a change from
//! outside Pixel Lab, a restore, several authors folded together by
//! thinning, or the document as it was first opened.

use serde_json::{json, Map, Value};

/// What kind of author.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthorKind {
    Owner,
    Agent,
    Cli,
    Outside,
    Restore,
    Several,
    Opened,
}

impl AuthorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AuthorKind::Owner => "owner",
            AuthorKind::Agent => "agent",
            AuthorKind::Cli => "cli",
            AuthorKind::Outside => "outside",
            AuthorKind::Restore => "restore",
            AuthorKind::Several => "several",
            AuthorKind::Opened => "opened",
        }
    }

    pub fn parse(text: &str) -> Option<AuthorKind> {
        Some(match text {
            "owner" => AuthorKind::Owner,
            "agent" => AuthorKind::Agent,
            "cli" => AuthorKind::Cli,
            "outside" => AuthorKind::Outside,
            "restore" => AuthorKind::Restore,
            "several" => AuthorKind::Several,
            "opened" => AuthorKind::Opened,
            _ => return None,
        })
    }
}

/// An author: its kind, an agent's name, and the Agent chat it worked in.
#[derive(Clone, Debug, PartialEq)]
pub struct Author {
    pub kind: AuthorKind,
    pub name: Option<String>,
    pub chat: Option<Value>,
}

impl Author {
    pub fn owner() -> Author {
        Author { kind: AuthorKind::Owner, name: None, chat: None }
    }

    pub fn cli() -> Author {
        Author { kind: AuthorKind::Cli, name: None, chat: None }
    }

    pub fn of(kind: AuthorKind) -> Author {
        Author { kind, name: None, chat: None }
    }

    /// An agent, by its MCP client's name, and the chat it works for.
    pub fn agent(name: Option<String>, chat: Option<Value>) -> Author {
        Author { kind: AuthorKind::Agent, name, chat }
    }

    /// `{ kind, name?, chat? }`.
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("kind".into(), json!(self.kind.as_str()));
        if let Some(n) = &self.name {
            m.insert("name".into(), json!(n));
        }
        if let Some(c) = &self.chat {
            m.insert("chat".into(), c.clone());
        }
        Value::Object(m)
    }

    /// The author a JSON object names; the owner when it names none.
    pub fn from_json(v: Option<&Value>) -> Author {
        let Some(v) = v else { return Author::owner() };
        let kind = v.get("kind").and_then(Value::as_str).and_then(AuthorKind::parse).unwrap_or(AuthorKind::Owner);
        let name = v.get("name").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string);
        let chat = v.get("chat").filter(|c| c.is_object()).cloned();
        Author { kind, name, chat }
    }

    /// Whether two requests come from the same author: the same kind, name and chat.
    pub fn same(&self, other: &Author) -> bool {
        self.kind == other.kind && self.name == other.name && chat_id(&self.chat) == chat_id(&other.chat)
    }

    /// How a menu or a version names it: "you", "Claude", "the CLI", ...
    pub fn display(&self) -> String {
        match self.kind {
            AuthorKind::Owner => "you".into(),
            AuthorKind::Agent => short_name(self.name.as_deref()),
            AuthorKind::Cli => "the CLI".into(),
            AuthorKind::Outside => "outside Pixel Lab".into(),
            AuthorKind::Restore => "a restore".into(),
            AuthorKind::Several => "several authors".into(),
            AuthorKind::Opened => "the opened document".into(),
        }
    }

    /// " (Claude)" after an undo label, for anyone but the owner (R20).
    pub fn label_suffix(&self) -> String {
        match self.kind {
            AuthorKind::Owner => String::new(),
            AuthorKind::Agent => format!(" ({})", short_name(self.name.as_deref())),
            AuthorKind::Cli => " (CLI)".into(),
            _ => format!(" ({})", self.display()),
        }
    }
}

fn chat_id(chat: &Option<Value>) -> Option<String> {
    chat.as_ref().and_then(|c| c.get("id")).and_then(Value::as_str).map(str::to_string)
}

/// An agent's name without its version: "claude-code 2.1.0" → "Claude".
fn short_name(name: Option<&str>) -> String {
    let Some(name) = name.filter(|n| !n.trim().is_empty()) else { return "an agent".into() };
    let first = name.split_whitespace().next().unwrap_or(name);
    if first.to_ascii_lowercase().starts_with("claude") {
        return "Claude".into();
    }
    first.to_string()
}
