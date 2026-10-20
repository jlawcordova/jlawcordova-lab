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
    /// Inside a layer: the render of only its items, drawn over the faded
    /// rest (docs/intents/2026-10-scene-layers/spec.md, D6), and which layer it's for.
    var focusImage: PixelImage?
    var focusLayer: String?

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
    /// `check`'s warnings: they don't fail it (off the grid, out of depth order, a moving item's copies).
    var warnings: [CheckWarning] = []
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
    /// The scene's layers and which items are in them (docs/intents/2026-10-scene-layers/spec.md, D5).
    var layerTree = LayerTree.empty
    var itemsByPath: [String: SceneItemInfo] = [:]
    var itemsLoaded = false
    @ObservationIgnored var loadingItems = false
    @ObservationIgnored var needsItems = false
    /// What's selected: layers and items, in the order they were picked.
    var selection: [LayerTarget] = []

    /// The selected item's path, when the selection is one item.
    var selectedPath: String? {
        get {
            if selection.count == 1, case .item(let p) = selection[0] { return p }
            return nil
        }
        set { selection = newValue.map { [.item($0)] } ?? [] }
    }

    var selectedItem: SceneItemInfo? { selectedPath.flatMap { itemsByPath[$0] } }

    /// The selected layer, when the selection is one layer.
    var selectedLayer: String? {
        if selection.count == 1, case .layer(let id) = selection[0] { return id }
        return nil
    }

    func setItems(_ r: SceneItemsResult) {
        items = r.items
        groups = r.groups
        itemsByPath = Dictionary(r.items.map { ($0.path, $0) }, uniquingKeysWith: { a, _ in a })
        layerTree = LayerTree(items: r.items, layers: r.layers ?? [])
        itemsLoaded = true
        let kept = selection.filter { layerTree.exists($0) }
        if kept != selection { selection = kept }
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

    /// The warning that this scene's file isn't in depth order, when it isn't (Sort fixes it).
    var outOfOrder: CheckWarning? { warnings.first { $0.kind == .outOfOrder && ($0.scene ?? ref.name) == ref.name } }

    /// A scene edit put the items in depth order and moved some: the selection
    /// follows its items to their new paths.
    func follow(_ reordered: [String: String]) {
        guard !reordered.isEmpty else { return }
        let next = selection.map { $0.following(reordered) }
        if next != selection { selection = next }
    }
}
