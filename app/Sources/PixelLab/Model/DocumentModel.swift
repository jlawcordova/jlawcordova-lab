import AppKit
import Observation

/// A rendered frame of the open doc, ready for the stage.
struct StageImage {
    /// Sized in points = art pixels (the PNG's pixels / its render scale).
    var image: PixelImage
    /// The scene pixels the PNG covers: PNG pixel (0, 0) at render scale 1 is scene
    /// pixel (viewBox.minX, viewBox.minY).
    var viewBox: CGRect
    /// The render's integer scale (the app renders at 1 and zooms on screen).
    var scale: Int
    /// Items in paint order, with bounds in output pixels (scale included).
    var items: [RenderItem]
    /// An object's anchor, (0, 0) in its own pixels.
    var anchor: CGPoint?
    var time: Double?

    var artSize: CGSize { viewBox.size }

    /// Rendered (visible) items by path.
    func item(at path: String) -> RenderItem? { items.first { $0.path == path } }

    /// An item's bounds in scene pixels.
    func sceneRect(of item: RenderItem) -> CGRect? {
        guard let b = item.bounds, b.count == 4 else { return nil }
        let s = CGFloat(scale)
        return CGRect(x: viewBox.minX + b[0] / s, y: viewBox.minY + b[1] / s, width: b[2] / s, height: b[3] / s)
    }
}

/// The open scene or object: its session status, latest render and checks.
@MainActor @Observable
final class DocumentModel {
    let ref: DocRef
    var status: DocStatus?
    var render: StageImage?
    /// nil until the first `check` comes back.
    var problems: [CheckProblem]?
    var checkSummary: String?
    var checkedCount: Int?
    var error: String?
    /// Where tile [0, 0, 0]'s top-face center sits, in scene pixels (scenes only).
    var origin: CGPoint?
    var summary: String?

    // Scene editing (slice 4)
    /// `scene.items`: every placed item in paint order, and the groups.
    var items: [SceneItemInfo] = []
    var groups: [SceneGroupInfo] = []
    /// Items and groups in tree order (a group before its items).
    var treeRows: [TreeRow] = []
    var itemsByPath: [String: SceneItemInfo] = [:]
    var itemsLoaded = false
    @ObservationIgnored var loadingItems = false
    @ObservationIgnored var needsItems = false
    /// The selected item's path.
    var selectedPath: String?

    var selectedItem: SceneItemInfo? { selectedPath.flatMap { itemsByPath[$0] } }

    func setItems(_ r: SceneItemsResult) {
        items = r.items
        groups = r.groups
        itemsByPath = Dictionary(r.items.map { ($0.path, $0) }, uniquingKeysWith: { a, _ in a })
        treeRows = TreeRow.build(items: r.items, groups: r.groups)
        itemsLoaded = true
        if let p = selectedPath, itemsByPath[p] == nil { selectedPath = nil }
    }

    @ObservationIgnored var rendering = false
    @ObservationIgnored var needsRender = false
    @ObservationIgnored var checking = false
    @ObservationIgnored var needsCheck = false
    /// Bumped on every finished render (the snapshot waits on it).
    var renderCount = 0

    init(ref: DocRef) { self.ref = ref }

    var isDirty: Bool { status?.dirty ?? false }
    var title: String { ref.name }
}

/// A row of the Items tree: a group or a placed item.
struct TreeRow: Identifiable, Hashable, Sendable {
    enum Kind: Hashable, Sendable {
        case group(SceneGroupInfo)
        case item(SceneItemInfo)
    }
    var path: String
    var kind: Kind
    var id: String { path }
    var components: [Int]
    var depth: Int { components.count - 1 }
    /// The paths of the groups that hold it, outermost first.
    var ancestors: [String] { Self.ancestors(of: components) }

    static func ancestors(of path: String) -> [String] { ancestors(of: components(path)) }

    static func ancestors(of components: [Int]) -> [String] {
        guard components.count > 1 else { return [] }
        return (1..<components.count).map { n in components.prefix(n).map(String.init).joined(separator: ".") }
    }

    static func components(_ path: String) -> [Int] { path.split(separator: ".").compactMap { Int($0) } }

    static func build(items: [SceneItemInfo], groups: [SceneGroupInfo]) -> [TreeRow] {
        let rows = groups.map { TreeRow(path: $0.path, kind: .group($0), components: components($0.path)) }
            + items.map { TreeRow(path: $0.path, kind: .item($0), components: components($0.path)) }
        return rows.sorted { $0.components.lexicographicallyPrecedes($1.components) }
    }
}
