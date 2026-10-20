import Foundation

/// Something that can be selected in a scene: a layer, or a placed item by
/// its path. The script hook writes them `layer:<id>` and `<path>`.
enum LayerTarget: Hashable, Sendable, CustomStringConvertible {
    case layer(String)
    case item(String)

    var description: String {
        switch self {
        case .layer(let id): "layer:\(id)"
        case .item(let path): path
        }
    }

    var layerID: String? { if case .layer(let id) = self { return id }; return nil }
    var itemPath: String? { if case .item(let p) = self { return p }; return nil }

    /// `layer:<id>` or an item path (`12`, `5.2`).
    init?(spec: String) {
        if spec.hasPrefix("layer:") {
            let id = String(spec.dropFirst("layer:".count))
            guard !id.isEmpty else { return nil }
            self = .layer(id)
        } else if !spec.isEmpty, spec.split(separator: ".", omittingEmptySubsequences: false).allSatisfy({ !$0.isEmpty && $0.allSatisfy(\.isNumber) }) {
            self = .item(spec)
        } else {
            return nil
        }
    }
}

/// A scene's layers and which items are in them (docs/intents/2026-10-scene-layers/spec.md,
/// D5), built from `scene.items`. Pure, so the picking rules (D6), the tree's
/// rows (D11), the placement target (D13) and Moved by (D12) are unit-tested.
struct LayerTree: Sendable, Equatable {
    struct Layer: Sendable, Equatable, Hashable {
        var id: String
        var name: String
        var parent: String?
        var items: Int
    }

    /// One row of the Layers panel: a layer or an item, at a depth.
    struct Row: Identifiable, Hashable, Sendable {
        var target: LayerTarget
        var depth: Int
        var id: String { target.description }
    }

    /// In the scene's order (the tree's order among siblings).
    private(set) var layers: [Layer] = []
    private(set) var byID: [String: Layer] = [:]
    /// Child layers of each layer ("" for the top level), in order.
    private(set) var childLayers: [String: [String]] = [:]
    /// The items directly in each layer ("" for the top level), in paint order.
    private(set) var directItems: [String: [String]] = [:]
    /// Each item's layer (only layers the scene has).
    private(set) var layerOfItem: [String: String] = [:]
    /// Every item path, in paint order.
    private(set) var order: [String] = []

    static let empty = LayerTree()

    init() {}

    init(items: [SceneItemInfo], layers given: [SceneLayerInfo]) {
        // A layer whose parent is missing (or that would be inside itself) goes to the top level.
        let known = Set(given.map(\.id))
        var parentOf: [String: String] = [:]
        for l in given { if let p = l.parent, known.contains(p) { parentOf[l.id] = p } }
        for l in given {
            var seen: Set<String> = [l.id]
            var next = parentOf[l.id]
            while let n = next, !seen.contains(n) { seen.insert(n); next = parentOf[n] }
            if next != nil { parentOf[l.id] = nil }
        }
        var seenIDs: Set<String> = []
        for l in given where !seenIDs.contains(l.id) {
            seenIDs.insert(l.id)
            let layer = Layer(id: l.id, name: l.name, parent: parentOf[l.id], items: l.items)
            layers.append(layer)
            byID[l.id] = layer
            childLayers[layer.parent ?? "", default: []].append(l.id)
        }
        for item in items {
            order.append(item.path)
            if let l = item.layer, byID[l] != nil {
                layerOfItem[item.path] = l
                directItems[l, default: []].append(item.path)
            } else {
                directItems["", default: []].append(item.path)
            }
        }
    }

    var isEmpty: Bool { layers.isEmpty }

    func name(of id: String) -> String { byID[id]?.name ?? id }

    // MARK: Chains

    /// The layers that hold a layer, outermost first, ending with it.
    func chain(ofLayer id: String) -> [String] {
        var out: [String] = []
        var next: String? = id
        while let n = next, byID[n] != nil, !out.contains(n) {
            out.insert(n, at: 0)
            next = byID[n]?.parent
        }
        return out
    }

    /// The layers that hold an item, outermost first.
    func chain(ofItem path: String) -> [String] {
        layerOfItem[path].map { chain(ofLayer: $0) } ?? []
    }

    /// The layers that hold a target, outermost first (a layer's ends with itself).
    func chain(of target: LayerTarget) -> [String] {
        switch target {
        case .layer(let id): chain(ofLayer: id)
        case .item(let p): chain(ofItem: p)
        }
    }

    /// The layer a target is in (nil: the top level).
    func parent(of target: LayerTarget) -> String? {
        switch target {
        case .layer(let id): byID[id]?.parent
        case .item(let p): layerOfItem[p]
        }
    }

    func exists(_ target: LayerTarget) -> Bool {
        switch target {
        case .layer(let id): byID[id] != nil
        case .item(let p): order.contains(p)
        }
    }

    /// Whether a target is `ancestor` or inside it (nil: the top level holds everything).
    func isWithin(_ target: LayerTarget, _ ancestor: String?) -> Bool {
        guard let ancestor else { return true }
        return chain(of: target).contains(ancestor)
    }

    /// Every item in a layer and the layers inside it, in paint order.
    func items(inside id: String) -> [String] {
        order.filter { chain(ofItem: $0).contains(id) }
    }

    /// The items a target covers: a layer's, or the item.
    func paths(of target: LayerTarget) -> [String] {
        switch target {
        case .layer(let id): items(inside: id)
        case .item(let p): [p]
        }
    }

    /// The items a selection covers, each once, in paint order.
    func paths(of selection: [LayerTarget]) -> [String] {
        let all = Set(selection.flatMap { paths(of: $0) })
        return order.filter { all.contains($0) }
    }

    // MARK: Picking on the stage (D6)

    /// What a click on an item picks: outside every layer, its outermost
    /// layer (or the item); inside `inside`, the next level down. Nil when the
    /// item is outside the layer you're in.
    func target(ofItem path: String, inside: String?) -> LayerTarget? {
        let chain = chain(ofItem: path)
        var from = 0
        if let inside {
            guard let i = chain.firstIndex(of: inside) else { return nil }
            from = i + 1
        }
        return from < chain.count ? .layer(chain[from]) : .item(path)
    }

    /// Whether a click can pick the item: it's within the layer you're in,
    /// and no layer holding it below that level is locked (so a locked
    /// layer's contents are picked from inside it).
    func pickable(_ path: String, inside: String?, locked: Set<String>) -> Bool {
        let chain = chain(ofItem: path)
        var from = 0
        if let inside {
            guard let i = chain.firstIndex(of: inside) else { return false }
            from = i + 1
        }
        return !chain[from...].contains { locked.contains($0) }
    }

    /// Whether an item draws: no layer holding it is hidden.
    func isHidden(_ path: String, hidden: Set<String>) -> Bool {
        chain(ofItem: path).contains { hidden.contains($0) }
    }

    // MARK: Rows (D11)

    /// The tree's rows: each layer's child layers, then its items; a layer's
    /// contents only when it's expanded.
    func rows(expanded: Set<String>) -> [Row] {
        var out: [Row] = []
        func add(_ parent: String, depth: Int) {
            for id in childLayers[parent] ?? [] {
                out.append(Row(target: .layer(id), depth: depth))
                if expanded.contains(id) { add(id, depth: depth + 1) }
            }
            for p in directItems[parent] ?? [] { out.append(Row(target: .item(p), depth: depth)) }
        }
        add("", depth: 0)
        return out
    }

    /// The search's flat list: layers whose name holds `query`, and items
    /// whose object, id or class does.
    func rows(matching query: String, items: [String: SceneItemInfo]) -> [Row] {
        let q = query.lowercased()
        let ls = layers.filter { $0.name.lowercased().contains(q) }.map { Row(target: .layer($0.id), depth: 0) }
        let its = order.filter { p in
            guard let i = items[p] else { return false }
            return i.object.lowercased().contains(q) || (i.id?.lowercased().contains(q) ?? false) || (i.class?.lowercased().contains(q) ?? false)
        }.map { Row(target: .item($0), depth: 0) }
        return ls + its
    }

    /// The layers to expand so a target's row shows.
    func reveal(_ target: LayerTarget) -> [String] {
        switch target {
        case .layer(let id): Array(chain(ofLayer: id).dropLast())
        case .item(let p): chain(ofItem: p)
        }
    }

    // MARK: Placing (D13)

    /// Where new items go: the selected layer, the selected item's layer, the
    /// layer you're inside, or the top level (nil).
    func placeTarget(selection: [LayerTarget], inside: String?) -> String? {
        if selection.count == 1 {
            switch selection[0] {
            case .layer(let id) where byID[id] != nil: return id
            case .item(let p): if let l = layerOfItem[p] { return l }
            default: break
            }
        }
        if let inside, byID[inside] != nil { return inside }
        return nil
    }
}

/// Moved by (D12): where a layer's first item stood when the layer was
/// selected, to say how far the layer has moved since.
struct MovedBaseline: Equatable, Sendable {
    var layer: String
    var path: String
    var object: String
    var at: ItemAt

    /// The layer's first item on a tile, else its first item.
    init?(layer: String, tree: LayerTree, items: [String: SceneItemInfo]) {
        let paths = tree.items(inside: layer)
        guard let path = paths.first(where: { items[$0]?.at.isTile == true }) ?? paths.first, let item = items[path] else { return nil }
        self.layer = layer
        self.path = path
        object = item.object
        at = item.at
    }

    /// Whole tiles (col, row) from where the item stood; nil if it's gone or
    /// moved off the tile grid.
    func moved(items: [String: SceneItemInfo]) -> (col: Int, row: Int)? {
        guard let now = items[path], now.object == object else { return nil }
        if let a = at.tile, let b = now.at.tile, a.count == 3, b.count == 3 { return (b[0] - a[0], b[1] - a[1]) }
        if let a = at.px, let b = now.at.px, a.count == 2, b.count == 2 {
            return Self.tiles(dx: b[0] - a[0], dy: b[1] - a[1])
        }
        return nil
    }

    /// A pixel offset as whole tiles: a column is (+16, +8), a row (−16, +8).
    static func tiles(dx: Int, dy: Int) -> (col: Int, row: Int)? {
        // dx = (c − r)·16, dy = (c + r)·8  ⇒  c = (dx/16 + dy/8)/2, r = (dy/8 − dx/16)/2
        let a = dx * 1 + dy * 2, b = dy * 2 - dx
        guard a % 32 == 0, b % 32 == 0 else { return nil }
        return (a / 32, b / 32)
    }
}
