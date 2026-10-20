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
    /// The open document (`document.mjs`): its folder, name, kind and publish folder.
    var document: DocumentInfo?
    /// The document's folder (the old name; `document.root` is the same path).
    var site: String
    var sourceDir: String
    var stateDir: String
    var saves: String
    var docs: [DocStatus]
}

/// `session.info.document`: the `.pixellab` folder (or a legacy `source/` folder,
/// kind `legacy-site`), its manifest name, and where `art.build` publishes.
struct DocumentInfo: Codable, Sendable, Equatable {
    var root: String
    var name: String
    var kind: String
    var publishDir: String?
    var siteRoot: String?
}

/// `document.new`: `{ root, name }`. `document.copy`: `{ from, to }`.
struct DocumentNewResult: Codable, Sendable {
    var root: String
    var name: String
}

struct DocumentCopyResult: Codable, Sendable {
    var from: String
    var to: String
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
    /// `scene.render --focus <layer>`: the same render with only that layer's
    /// items (docs/intents/2026-10-scene-layers/spec.md, D6's fade).
    var focusPng: Data?
}

struct CheckProblem: Codable, Sendable, Hashable {
    var file: String
    var message: String
    var text: String
}

/// A warning of `check`: it doesn't fail the check. `code` says which
/// (docs/intents/2026-10-depth-order/spec.md, R11, R12).
struct CheckWarning: Codable, Sendable, Hashable {
    /// `off_grid`: motion that steps off the grid. `out_of_order`: a scene whose
    /// file isn't in depth order (Sort fixes it). `copies`: a moving item drawn
    /// once for each place it takes in the depth order.
    enum Code: String, Sendable { case offGrid = "off_grid", outOfOrder = "out_of_order", copies }

    var file: String
    var message: String
    var text: String
    var code: String
    var scene: String?
    /// `out_of_order`: how many items a sort would move.
    var moves: Int?
    /// `copies`: the moving item's id and how many times it's drawn.
    var item: String?
    var copies: Int?

    var kind: Code? { Code(rawValue: code) }
}

struct CheckResult: Codable, Sendable {
    var ok: Bool
    var checked: Int?
    var problems: [CheckProblem]
    var warnings: [CheckWarning]?
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
    /// An open doc with unsaved edits whose file changed on disk (docs/intents/2026-10-mcp-gaps/spec.md, D5).
    case docStale(doc: String, file: String)
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

// MARK: Scene editing (slice 4)

/// Where an item is placed: `{ tile: [col, row, level] }` or `{ px: [x, y] }`.
struct ItemAt: Codable, Sendable, Hashable {
    var tile: [Int]?
    var px: [Int]?
    /// A pixel-placed item's level, how high it stands (absent at 0;
    /// docs/intents/2026-10-depth-order/spec.md, R7).
    var pxLevel: Int?

    private enum CodingKeys: String, CodingKey {
        case tile, px
        case pxLevel = "level"
    }

    var isTile: Bool { tile?.count == 3 }
    var col: Int? { tile.flatMap { $0.count == 3 ? $0[0] : nil } }
    var row: Int? { tile.flatMap { $0.count == 3 ? $0[1] : nil } }
    var level: Int? { tile.flatMap { $0.count == 3 ? $0[2] : nil } }
    var x: Int? { px.flatMap { $0.count == 2 ? $0[0] : nil } }
    var y: Int? { px.flatMap { $0.count == 2 ? $0[1] : nil } }

    var isPixel: Bool { px?.count == 2 }
    /// The level the item stands on: a tile's third number, or a pixel item's
    /// level (0 unless it says). nil when it has no placement.
    var standingLevel: Int? { level ?? (isPixel ? (pxLevel ?? 0) : nil) }

    /// "column 3, row 4, level 0" or "x 31, y 6" (", level 2" when a pixel item stands above 0).
    var spoken: String {
        if let t = tile, t.count == 3 { return "column \(t[0]), row \(t[1]), level \(t[2])" }
        if let p = px, p.count == 2 { return "x \(p[0]), y \(p[1])" + (pxLevel.flatMap { $0 != 0 ? ", level \($0)" : nil } ?? "") }
        return "nowhere"
    }

    /// "14,6,1" or "px 31,6" ("px 31,6 level 2" above level 0).
    var short: String {
        if let t = tile, t.count == 3 { return "\(t[0]),\(t[1]),\(t[2])" }
        if let p = px, p.count == 2 { return "px \(p[0]),\(p[1])" + (pxLevel.flatMap { $0 != 0 ? " level \($0)" : nil } ?? "") }
        return "?"
    }
}

/// One placed item of `scene.items`, in paint order.
struct SceneItemInfo: Codable, Sendable, Hashable, Identifiable {
    var path: String
    var object: String
    var at: ItemAt
    var `class`: String?
    var id: String?
    /// The scene layer it belongs to (docs/intents/2026-10-scene-layers/spec.md, D1).
    var layer: String?
    /// The enclosing group's path.
    var group: String?
    /// The placement point in scene pixels.
    var point: [Double]?
    /// `front` or `back` when the item carries the depth override; nil draws it by position.
    var depth: String?

    /// The Depth control's value.
    var depthMode: DepthMode { DepthMode(wire: depth) }
}

/// An item's or a group's depth override (docs/intents/2026-10-depth-order/spec.md, R5):
/// by where it stands (`auto`), over everything else in its list (`front`) or under it (`back`).
enum DepthMode: String, CaseIterable, Identifiable, Sendable {
    case auto, front, back
    var id: String { rawValue }

    /// A `depth` field from the wire: absent or unknown is `auto`.
    init(wire: String?) { self = wire.flatMap(DepthMode.init(rawValue:)) ?? .auto }

    var title: String {
        switch self {
        case .auto: "Auto"
        case .front: "Front"
        case .back: "Back"
        }
    }

    /// What it does, for the help text and VoiceOver.
    var detail: String {
        switch self {
        case .auto: "Drawn by where it stands"
        case .front: "Drawn over everything else in its list"
        case .back: "Drawn under everything else in its list"
        }
    }
}

/// A layer of `scene.items`: a named, nestable part of the scene, in the
/// order the Layers panel shows it. `items` counts what's in it and in the
/// layers inside it.
struct SceneLayerInfo: Codable, Sendable, Hashable, Identifiable {
    var id: String
    var name: String
    var parent: String?
    var items: Int
}

/// What a `scene.layer.*` edit returns: the layer it made or renamed, what
/// an ungroup released, how many items a move moved.
struct LayerEditResult: Codable, Sendable {
    struct Layer: Codable, Sendable, Hashable {
        var id: String
        var name: String
        var parent: String?
    }
    var doc: String
    var changed: Bool?
    var summary: String?
    var layer: Layer?
    /// Ungroup: the layer it was in (nil: the top level).
    var parent: String?
    /// New and assign: the layers that joined; ungroup: the layers it held.
    var layers: [String]?
    /// New and assign: how many items joined; ungroup: the paths it held.
    var items: JSONValue?
    var moved: Int?
    /// Move: the moved items' paths, after the sort.
    var paths: [String]?
    /// The paths the depth sort moved (see `EditResult.reordered`).
    var reordered: [String: String]?

    var itemPaths: [String] { items?.arrayValue?.compactMap(\.stringValue) ?? [] }
    var edit: EditResult { EditResult(doc: doc, changed: changed, summary: summary, reordered: reordered) }
}

/// A group of `scene.items` (its attributes are `class` and `data-*`).
struct SceneGroupInfo: Codable, Sendable, Hashable {
    var path: String
    var attrs: [String: String]
    var items: Int
    var group: String?
    /// `front` or `back` when the group carries the depth override.
    var depth: String?

    var depthMode: DepthMode { DepthMode(wire: depth) }
}

struct SceneItemsResult: Codable, Sendable {
    var scene: String
    var items: [SceneItemInfo]
    var groups: [SceneGroupInfo]
    var layers: [SceneLayerInfo]?
}

/// One motion track: the item id it moves and its key times (ms).
struct MotionTrack: Sendable, Hashable, Identifiable {
    var item: String
    var keys: [Double]
    var id: String { item }
}

/// `scene.show --doc`, only the motion part of the doc.
struct SceneShowMotion: Codable, Sendable {
    struct Doc: Codable, Sendable {
        struct Motion: Codable, Sendable {
            struct Track: Codable, Sendable {
                struct Key: Codable, Sendable { var t: Double? }
                var item: String
                var keys: [Key]?
            }
            var duration: Double?
            var tracks: [Track]?
        }
        var motion: Motion?
    }
    var name: String
    var doc: Doc?
}

/// An edit's result: `{ doc, changed, summary, path?, item? }`; object edits
/// add `pixels` (paint), `layer` (layer edits) or `frame` (frame edits).
struct EditResult: Codable, Sendable {
    var doc: String
    var changed: Bool?
    var summary: String?
    var path: String?
    var pixels: Int?
    var layer: Int?
    var frame: Int?
    /// A scene edit puts the items in depth order. When that moved any, this
    /// maps each old path to its new one, relative to the doc as the edit left
    /// it before the sort (docs/intents/2026-10-depth-order/plan.md, "Sorting in the session").
    var reordered: [String: String]?
}

/// `scene.sort` on one scene: it names no doc (the command can sort every scene),
/// so it isn't an `EditResult`. `reordered` is as there.
struct SceneSortResult: Codable, Sendable {
    var changed: Bool
    var summary: String?
    /// How many published pixels the sort changes (the file's list order against depth order).
    var changedPixels: Int?
    var reordered: [String: String]?
}

// MARK: Object editing (slice 5)

/// One color of `palette.get`.
struct PaletteColor: Codable, Sendable, Hashable, Identifiable {
    var name: String
    var hex: String
    /// "world" or "outfit".
    var tier: String
    var id: String { name }
}

/// `palette.get`: `colors` keeps the palette's order (world, then outfit).
struct PaletteResult: Codable, Sendable {
    var colors: [PaletteColor]
}

/// A layer of `object.show`.
struct ObjectLayerInfo: Codable, Sendable, Hashable {
    var index: Int
    var frames: Int
    var surface: Bool?
    var `class`: String?
    /// ff, wf or hf for a frame loop (nil: a still map).
    var loop: String?
    var prefix: String?

    var isLoop: Bool { loop != nil }
}

/// `object.show`: what the object panels need.
struct ObjectShowResult: Codable, Sendable {
    var name: String
    var kind: String
    var `extends`: String?
    var size: [Int]
    var blockSize: [Int]?
    var layers: [ObjectLayerInfo]
    var frames: Int
    var colors: Int
    var description: String?
    var file: String?
    var anchor: [Int]
    /// The tiles a sprite covers, wide and deep, for depth order (docs/intents/2026-10-depth-order/spec.md, R8);
    /// nil for a block, whose size is its footprint.
    var footprint: [Int]?
    var character: Bool?
    /// The hex values the object paints (the colors meter counts them).
    var usedColors: [String]

    var isBlock: Bool { kind == "block" }
    /// Layers and frames can change: a sprite with layers of its own.
    var layersEditable: Bool { !isBlock && `extends` == nil }
}

/// `object.pick`: `{ color, hex }`, both nil for a transparent pixel.
struct PickResult: Codable, Sendable {
    var color: String?
    var hex: String?
}

/// `art.build`: `{ ok, publishDir, outputs, previewOnly, problems, ms, unsaved }`.
/// `ok` is false when any source has a problem (nothing written) or an output
/// is over its budget (written anyway, as `npm run art` did).
struct ArtBuildResult: Codable, Sendable, Equatable {
    var ok: Bool
    var publishDir: String
    var outputs: [ArtOutput]
    var previewOnly: [ArtOutput]
    var problems: [String]
    var ms: Double
    var unsaved: [String]

    /// The report as the CLI prints it: one line per output, one per preview-only scene, then the problems.
    var report: [String] {
        outputs.map { "\($0.file ?? $0.scene): \($0.report)" }
            + previewOnly.map { "\($0.scene) (preview only, not written): \($0.report)" }
            + problems
    }
    /// The lines the sheet marks: problems, and outputs over their budget.
    var marked: Set<String> {
        Set(problems + outputs.filter(\.over).map { "\($0.file ?? $0.scene): \($0.report)" })
    }
    var overBudget: [ArtOutput] { outputs.filter(\.over) }
}

/// One compiled scene: its output file (nil for a preview-only scene), sizes and report line.
struct ArtOutput: Codable, Sendable, Equatable {
    var file: String?
    var scene: String
    var raw: Int
    var gzip: Int
    var over: Bool
    var report: String
}
