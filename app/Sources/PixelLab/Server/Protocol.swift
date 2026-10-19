import Foundation

// The shapes of `pixellab serve --stdio` results and notifications the app
// uses (docs/cli.md, "The stdio protocol"). Unknown fields are ignored.

/// A doc reference: `scene:<name>` or `object:<name>` on the wire.
struct DocRef: Hashable, Sendable, Codable, CustomStringConvertible {
    enum Kind: String, Sendable, Codable { case scene, object }
    var kind: Kind
    var name: String

    var id: String { "\(kind.rawValue):\(name)" }
    var description: String { id }

    init(_ kind: Kind, _ name: String) {
        self.kind = kind
        self.name = name
    }

    /// Parses `scene:<name>` / `object:<name>`.
    init?(id: String) {
        guard let colon = id.firstIndex(of: ":"), let kind = Kind(rawValue: String(id[..<colon])) else { return nil }
        self.init(kind, String(id[id.index(after: colon)...]))
    }

    /// The source file a `files.changed` notification names for this doc.
    var file: String { kind == .scene ? "scenes/\(name).mjs" : "objects/\(name).mjs" }

    init(from decoder: Decoder) throws {
        let s = try decoder.singleValueContainer().decode(String.self)
        guard let ref = DocRef(id: s) else {
            throw DecodingError.dataCorrupted(.init(codingPath: decoder.codingPath, debugDescription: "bad doc ref \(s)"))
        }
        self = ref
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        try c.encode(id)
    }
}

struct DocStatus: Codable, Sendable, Equatable {
    var doc: String
    var dirty: Bool
    var canUndo: Bool
    var canRedo: Bool
    var file: String?
    var exists: Bool?
    var stale: Bool?
}

struct SessionInfo: Codable, Sendable {
    var name: String
    var version: String
    var `protocol`: Int
    var pid: Int
    var site: String
    var sourceDir: String
    var stateDir: String
    var saves: String
    var docs: [DocStatus]
}

struct SiteInfo: Codable, Sendable {
    var root: String
    var from: String?
    var branch: String?
    var node: String?
    var scenes: Int
    var objects: Int
    var problems: Int
}

struct SceneSummary: Codable, Sendable, Identifiable, Hashable {
    var name: String
    var file: String
    var output: String?
    var viewBox: [Double]
    var origin: [Double]
    var items: Int
    var motion: Bool
    var id: String { name }
}

struct SceneList: Codable, Sendable { var scenes: [SceneSummary] }

struct ObjectSummary: Codable, Sendable, Identifiable, Hashable {
    var name: String
    var kind: String
    var size: [Int]
    var layers: Int
    var frames: Int
    var colors: Int
    var description: String?
    var id: String { name }
}

struct ObjectList: Codable, Sendable { var objects: [ObjectSummary] }

/// One item of a render, with its bounds in output pixels (scale included).
struct RenderItem: Codable, Sendable, Hashable {
    var path: String?
    var object: String
    var at: JSONValue?
    var id: String?
    var pose: String?
    var bounds: [Double]?
}

/// `scene.render` / `object.render`. `png` arrives base64 and decodes to bytes.
struct RenderResult: Codable, Sendable {
    var png: Data
    var scale: Int
    /// [x, y, w, h] in scene pixels (art pixels); the PNG covers exactly this box.
    var viewBox: [Double]
    var width: Int
    var height: Int
    var time: Double?
    var frame: Int?
    var anchor: [Double]?
    var items: [RenderItem]
}

struct CheckProblem: Codable, Sendable, Hashable {
    var file: String
    var message: String
    var text: String
}

struct CheckResult: Codable, Sendable {
    var ok: Bool
    var checked: Int?
    var problems: [CheckProblem]
    var summary: String?
}

struct SaveResult: Codable, Sendable {
    var doc: String
    var file: String?
    var wrote: Bool?
    var dirty: Bool
    var canUndo: Bool
    var canRedo: Bool
}

struct UndoResult: Codable, Sendable {
    var doc: String
    var changed: Bool?
    var summary: String?
    var dirty: Bool
    var canUndo: Bool
    var canRedo: Bool
}

/// A notification from the server.
enum ServerNotification: Sendable {
    case docChanged(DocStatus)
    case filesChanged([String])
    case proposalsChanged(JSONValue)
    case activityAppended(JSONValue)
    case other(method: String, params: JSONValue)
}

/// What the connection tells the app, in order.
enum ServerEvent: Sendable {
    case notification(ServerNotification)
    /// The server crashed and was started again: open docs are gone, reopen them.
    case restarted(SessionInfo)
    /// The server stopped and won't be restarted (too many crashes, or it couldn't start).
    case stopped(reason: String)
}
