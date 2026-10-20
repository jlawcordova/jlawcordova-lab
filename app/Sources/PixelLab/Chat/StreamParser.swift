import Foundation

/// What a line of `claude -p --output-format stream-json` means to the chat
/// (docs/intents/2026-10-agent-chat/spec.md, D1).
enum StreamEvent: Equatable, Sendable {
    /// `system`/`init`: the Claude session, and the pixellab MCP server's status (`connected`, `failed`, …).
    case started(session: String, mcpStatus: String?)
    /// Claude's words.
    case text(String)
    /// A tool call: the tool's name as Claude Code gives it (`mcp__pixellab__scene_move`) and its input.
    case toolUse(id: String, name: String, input: JSONValue)
    /// A tool call's result: its text (JSON for pixellab's tools), or the error's.
    case toolResult(id: String, ok: Bool, text: String)
    /// The CLI isn't signed in (or its login expired): `"error": "authentication_failed"`.
    case authFailed(String)
    /// `result`: the turn is over.
    case finished(session: String?, isError: Bool, result: String?)
}

/// stream-json lines → events. Unknown line types (`stream_event`,
/// `rate_limit_event`, …) and unknown content blocks are ignored; a line
/// that isn't JSON is nil, for the turn's diagnostics.
enum StreamParser {
    static func parse(line: String) -> [StreamEvent]? {
        let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return [] }
        guard let data = trimmed.data(using: .utf8), let value = try? JSONDecoder().decode(JSONValue.self, from: data), case .object = value else { return nil }
        let type = value["type"]?.stringValue
        let session = value["session_id"]?.stringValue
        switch type {
        case "system":
            guard value["subtype"]?.stringValue == "init", let session else { return [] }
            let servers = value["mcp_servers"]?.arrayValue ?? []
            let status = servers.first { $0["name"]?.stringValue == "pixellab" }?["status"]?.stringValue
            return [.started(session: session, mcpStatus: status)]
        case "assistant":
            let blocks = value["message"]?["content"]?.arrayValue ?? []
            if value["error"]?.stringValue == "authentication_failed" {
                return [.authFailed(blocks.compactMap { $0["text"]?.stringValue }.joined(separator: "\n"))]
            }
            var out: [StreamEvent] = []
            for block in blocks {
                switch block["type"]?.stringValue {
                case "text":
                    if let t = block["text"]?.stringValue, !t.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { out.append(.text(t)) }
                case "tool_use":
                    if let id = block["id"]?.stringValue, let name = block["name"]?.stringValue {
                        out.append(.toolUse(id: id, name: name, input: block["input"] ?? [:]))
                    }
                default:
                    break
                }
            }
            return out
        case "user":
            let blocks = value["message"]?["content"]?.arrayValue ?? []
            return blocks.compactMap { block in
                guard block["type"]?.stringValue == "tool_result", let id = block["tool_use_id"]?.stringValue else { return nil }
                let isError: Bool = if case .bool(true) = block["is_error"] { true } else { false }
                return .toolResult(id: id, ok: !isError, text: text(of: block["content"]))
            }
        case "result":
            let isError: Bool = if case .bool(true) = value["is_error"] { true } else { false }
            return [.finished(session: session, isError: isError || value["subtype"]?.stringValue?.hasPrefix("error") == true, result: value["result"]?.stringValue)]
        default:
            return []
        }
    }

    /// A tool result's content: a string, or text blocks joined (images left out).
    static func text(of content: JSONValue?) -> String {
        switch content {
        case .string(let s): return s
        case .array(let blocks): return blocks.compactMap { $0["type"]?.stringValue == "text" ? $0["text"]?.stringValue : nil }.joined(separator: "\n")
        default: return ""
        }
    }

    /// A pixellab tool's name without Claude Code's prefix: `mcp__pixellab__scene_move` → `scene_move`.
    static func toolName(_ name: String) -> String {
        name.hasPrefix("mcp__pixellab__") ? String(name.dropFirst("mcp__pixellab__".count)) : name
    }
}
