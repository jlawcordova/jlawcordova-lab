import Foundation

/// One Agent chat (docs/intents/2026-10-agent-chat/spec.md, D3), saved as
/// `<state dir>/chats/<id>.json`.
struct Chat: Codable, Identifiable, Hashable, Sendable {
    var version = 1
    var id: String
    var title: String
    /// The document it was made in (its root): the list shows the open document's chats.
    var document: String
    var createdAt: Date
    var updatedAt: Date
    /// The Claude Code session the next turn resumes.
    var session: String?
    var pins: [ChatPin] = []
    var messages: [ChatMessage] = []

    static let untitled = "New chat"

    /// "New chat" until the first message, then that message (30 characters, "…" past that).
    static func title(from text: String) -> String {
        let one = text.split(whereSeparator: \.isNewline).first.map(String.init)?.trimmingCharacters(in: .whitespaces) ?? ""
        guard !one.isEmpty else { return untitled }
        return one.count > 30 ? String(one.prefix(29)).trimmingCharacters(in: .whitespaces) + "…" : one
    }

    /// The proposals made in it that wait for the owner.
    func waiting(in pending: Set<String>) -> Int {
        messages.filter { $0.kind == .proposal && $0.outcome == nil && pending.contains($0.proposal ?? "") }.count
    }
}

/// A pinned target: it stays in the chat's context while the selection changes.
struct ChatPin: Codable, Hashable, Sendable {
    /// `scene:<name>`.
    var doc: String
    /// `layer:<id>` or an item path.
    var target: String
    /// An item's object, to notice when its path comes to hold another one.
    var object: String?
}

struct ChatMessage: Codable, Identifiable, Hashable, Sendable {
    enum Kind: String, Codable, Sendable { case user, claude, step, question, proposal, note }

    var id: String
    var time: Date
    var kind: Kind
    /// A user's or Claude's words, a question, a note.
    var text: String?
    /// User: the chips it was sent with ("pine 14.0", "Boats · 12 items", "rock 51 (not sent)").
    var chips: [String]?

    // A step: a tool call.
    var toolID: String?
    var tool: String?
    var input: JSONValue?
    var ok: Bool?
    var done: Bool?
    var human: String?
    var raw: String?
    var result: String?

    // A question: its answers and the one chosen.
    var options: [String]?
    var chosen: String?

    // A proposal card.
    var proposal: String?
    var number: Int?
    var doc: String?
    var edits: [String]?
    /// `applied`, `rejected`, or `elsewhere` once it's no longer waiting.
    var outcome: String?

    init(kind: Kind, text: String? = nil, time: Date = Date()) {
        id = UUID().uuidString
        self.time = time
        self.kind = kind
        self.text = text
    }
}

/// Reads and writes chats in a folder, atomically.
struct ChatStore: Sendable {
    var dir: URL

    init(stateDir: String) {
        dir = URL(fileURLWithPath: stateDir, isDirectory: true).appendingPathComponent("chats", isDirectory: true)
    }

    static let encoder: JSONEncoder = {
        let e = JSONEncoder()
        e.outputFormatting = [.prettyPrinted, .sortedKeys]
        e.dateEncodingStrategy = .iso8601
        return e
    }()

    static let decoder: JSONDecoder = {
        let d = JSONDecoder()
        d.dateDecodingStrategy = .iso8601
        return d
    }()

    /// Every readable chat, newest first. A broken file is skipped.
    func load() -> [Chat] {
        let files = (try? FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil)) ?? []
        return files.filter { $0.pathExtension == "json" }
            .compactMap { try? Self.decoder.decode(Chat.self, from: Data(contentsOf: $0)) }
            .sorted { $0.updatedAt > $1.updatedAt }
    }

    func save(_ chat: Chat) throws {
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let data = try Self.encoder.encode(chat)
        try data.write(to: dir.appendingPathComponent("\(chat.id).json"), options: .atomic)
    }

    /// A sortable, file-safe id, like a proposal's: 20261021-094112-a1b2c3.
    static func newID(_ now: Date = Date()) -> String {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.timeZone = TimeZone(identifier: "UTC")
        f.dateFormat = "yyyyMMdd-HHmmss"
        let hex = String((0..<6).map { _ in "0123456789abcdef".randomElement()! })
        return "\(f.string(from: now))-\(hex)"
    }
}
