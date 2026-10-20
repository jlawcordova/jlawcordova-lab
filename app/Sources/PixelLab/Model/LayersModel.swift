import AppKit
import Observation

/// An action of the right-click menu (docs/intents/2026-10-scene-layers/spec.md, D9).
enum LayerMenuAction: String, CaseIterable, Sendable {
    case group, ungroup, inside, hide, lock
}

/// One line of the right-click menu.
struct LayerMenuItem: Identifiable, Equatable, Sendable {
    var action: LayerMenuAction
    var title: String
    var shortcut: String
    var enabled: Bool
    var separatorBefore = false
    var id: String { action.rawValue }
}

/// The open right-click menu: its top-left in the window overlay's points,
/// and the item the keyboard is on.
struct LayerMenuState: Equatable, Sendable {
    var origin: CGPoint
    var highlighted: Int?
}

/// Dragging layers, or several things, on the stage: by whole tiles (D7).
struct SelectionDrag: Equatable, Sendable {
    var targets: [LayerTarget]
    var paths: [String]
    var startView: CGPoint
    var startScene: CGPoint
    var moved = false
    var dc = 0, dr = 0

    /// How far the moved items' art is drawn from where it stands, in scene pixels.
    var sceneOffset: CGPoint { CGPoint(x: CGFloat(dc - dr) * StageGeometry.tileW / 2, y: CGFloat(dc + dr) * StageGeometry.tileH / 2) }
    var isZero: Bool { dc == 0 && dr == 0 }
}

/// A scene's hidden and locked layers, as `layer-view.json` keeps them.
struct LayerViewState: Codable, Equatable, Sendable {
    var hidden: [String] = []
    var locked: [String] = []
}

/// Scene layers in the app (docs/intents/2026-10-scene-layers/spec.md, D5–D13):
/// the layer you're inside, the tree's expanded layers, hidden and locked
/// layers (view state, kept per document and scene in the state dir, never
/// in the document), picking and dragging on the stage, the right-click
/// menu, and the layer edits, all through the session server
/// (`scene.layer.*`). The selection itself is `DocumentModel.selection`.
@MainActor @Observable
final class LayersModel {
    @ObservationIgnored weak var app: AppModel?
    /// The window-wide view the menu is drawn in: window points convert into it.
    @ObservationIgnored weak var menuAnchor: NSView?

    /// The layer you're working inside (nil: the whole scene).
    var inside: String?
    var expanded: Set<String> = []
    var hidden: Set<String> = []
    var locked: Set<String> = []
    /// The target under the pointer on the stage (Select tool).
    var hover: LayerTarget?
    var menu: LayerMenuState?
    var drag: SelectionDrag?
    /// The inspector's Layer name field.
    var nameDraft = ""
    /// Where the selected layer stood when it was selected (Moved by).
    var baseline: MovedBaseline?
    /// A row the sidebar should scroll into view (the script hook, before a real right-click on it).
    var scrollRequest: LayerTarget?
    /// Each tree row's frame in window points (top-left origin), for the script hook's real right-clicks.
    @ObservationIgnored var rowFrames: [String: CGRect] = [:]

    private var doc: DocumentModel? {
        guard let d = app?.document, d.ref.kind == .scene else { return nil }
        return d
    }

    var tree: LayerTree { doc?.layerTree ?? .empty }
    var selection: [LayerTarget] { doc?.selection ?? [] }

    // MARK: A new doc, new items

    /// A doc opened: back to the whole scene, and its saved hidden and locked layers.
    func reset() {
        inside = nil
        expanded = []
        hover = nil
        menu = nil
        drag = nil
        baseline = nil
        nameDraft = ""
        draftSource = nil
        loadViewState()
    }

    /// The `hide` and `focus` the stage's render last asked for.
    @ObservationIgnored var renderedParams: [String: JSONValue] = [:]

    /// `scene.items` came back: forget what's gone, keep the inspector in step,
    /// and render again if hidden layers or the layer you're in only now exist.
    func itemsChanged() {
        if let i = inside, tree.byID[i] == nil { inside = nil }
        if renderParams != renderedParams {
            // The timeline's cached frames were drawn without these layers hidden.
            app?.timeline.invalidate()
            app?.requestRender()
        }
        if let h = hover, !tree.exists(h) { hover = nil }
        if doc?.selectedLayer == nil { baseline = nil } else if baseline == nil { selectionChanged() }
        syncDraft()
    }

    // MARK: Selection

    /// Selects targets, reveals their rows, and tells VoiceOver.
    func select(_ targets: [LayerTarget], reveal: Bool = true, announce say: Bool = true) {
        guard let doc else { return }
        doc.selection = targets
        if reveal { for t in targets { self.reveal(t) } }
        selectionChanged()
        if say { announce(selectionLabel.map { "\($0) selected" } ?? "Nothing selected") }
    }

    /// ⇧-click: adds a target, or takes it away if it's selected.
    func toggle(_ t: LayerTarget) {
        var s = selection
        if let i = s.firstIndex(of: t) { s.remove(at: i) } else { s.append(t) }
        select(s)
    }

    /// A row of the tree clicked: select it (⇧ adds it).
    func selectRow(_ t: LayerTarget, shift: Bool) {
        if shift { toggle(t) } else { select([t], reveal: false) }
    }

    /// The selection changed (here or in `SceneEditor.select`): Moved by's
    /// baseline, the name field, the item fields and the place level follow.
    func selectionChanged() {
        if let id = doc?.selectedLayer, let doc {
            if baseline?.layer != id { baseline = MovedBaseline(layer: id, tree: tree, items: doc.itemsByPath) }
        } else {
            baseline = nil
        }
        syncDraft()
        app?.editor.syncDrafts()
        if let l = doc?.selectedItem?.at.standingLevel { app?.editor.placeLevel = l }
    }

    /// A scene edit put the items in depth order and moved some (`reordered`,
    /// old path → new): the selection, the Moved by baseline and the hover
    /// follow their items.
    func followSort(_ reordered: [String: String]?) {
        guard let reordered, !reordered.isEmpty else { return }
        doc?.follow(reordered)
        baseline = baseline?.following(reordered)
        hover = hover?.following(reordered)
    }

    /// The layer and name the name field last took, so a reload doesn't overwrite typing.
    @ObservationIgnored private var draftSource: String?

    /// The name field follows the selected layer's name, when that layer or its name changed.
    func syncDraft(force: Bool = false) {
        let source = doc?.selectedLayer.map { "\($0)\u{0}\(tree.name(of: $0))" }
        guard force || source != draftSource else { return }
        draftSource = source
        nameDraft = doc?.selectedLayer.map { tree.name(of: $0) } ?? ""
    }

    /// Expands the layers that hold a target, so its row shows.
    func reveal(_ t: LayerTarget) {
        for l in tree.reveal(t) where !expanded.contains(l) { expanded.insert(l) }
    }

    func toggleExpanded(_ id: String) {
        if expanded.contains(id) { expanded.remove(id) } else { expanded.insert(id) }
    }

    /// "Railway", "pine", "3 things".
    var selectionLabel: String? {
        let s = selection
        guard !s.isEmpty else { return nil }
        if s.count > 1 { return "\(s.count) things" }
        return label(s[0])
    }

    func label(_ t: LayerTarget) -> String {
        switch t {
        case .layer(let id): return tree.name(of: id)
        case .item(let p):
            let item = doc?.itemsByPath[p]
            return item.map { $0.id ?? $0.object } ?? p
        }
    }

    // MARK: Inside a layer (D6)

    /// Works inside a layer: everything else fades and clicks pick its parts.
    func goInside(_ id: String) {
        guard tree.byID[id] != nil else { return }
        inside = id
        for l in tree.chain(ofLayer: id) { expanded.insert(l) }
        hover = nil
        select([], announce: false)
        app?.requestRender()
        announce("Inside \(tree.name(of: id)). Clicks pick what's in it; Done or Escape leaves.")
    }

    /// Done: up one level, with the layer you were in selected.
    func done() {
        guard let i = inside else { return }
        inside = tree.byID[i]?.parent
        hover = nil
        select([.layer(i)], announce: false)
        app?.requestRender()
        announce(inside.map { "Inside \(tree.name(of: $0)), \(tree.name(of: i)) selected" } ?? "The whole scene, \(tree.name(of: i)) selected")
    }

    /// A crumb: that level (nil: the whole scene).
    func goTo(level: String?) {
        guard level != inside else { return }
        if let level { goInside(level) } else {
            inside = nil
            hover = nil
            select([], announce: false)
            app?.requestRender()
            announce("The whole scene")
        }
    }

    /// The breadcrumb: the scene, then each layer down to the one you're in.
    var crumbs: [(label: String, level: String?)] {
        var out: [(String, String?)] = [(doc?.ref.name ?? "", nil)]
        if let inside { for l in tree.chain(ofLayer: inside) { out.append((tree.name(of: l), l)) } }
        return out
    }

    // MARK: Hide and lock (D8)

    func toggleHidden(_ id: String) {
        if hidden.contains(id) {
            hidden.remove(id)
            announce("\(tree.name(of: id)) shown")
        } else {
            hidden.insert(id)
            if selection.contains(.layer(id)) { select(selection.filter { $0 != .layer(id) }, announce: false) }
            announce("\(tree.name(of: id)) hidden")
        }
        saveViewState()
        app?.timeline.invalidate()
        app?.requestRender()
    }

    func toggleLocked(_ id: String) {
        if locked.contains(id) { locked.remove(id) } else { locked.insert(id) }
        if let h = hover, !(h.itemPath.map { tree.pickable($0, inside: inside, locked: locked) } ?? true) { hover = nil }
        saveViewState()
        announce("\(tree.name(of: id)) \(locked.contains(id) ? "locked" : "unlocked")")
    }

    private var viewStateURL: URL? {
        guard let dir = app?.sessionInfo?.stateDir, !dir.isEmpty else { return nil }
        return URL(fileURLWithPath: dir).appendingPathComponent("layer-view.json")
    }

    private var documentKey: String? { app?.sessionInfo?.document?.root ?? app?.sessionInfo?.site }

    private func readViewStates() -> [String: [String: LayerViewState]] {
        guard let url = viewStateURL, let data = try? Data(contentsOf: url) else { return [:] }
        return (try? JSONDecoder().decode([String: [String: LayerViewState]].self, from: data)) ?? [:]
    }

    /// The scene's hidden and locked layers, from `<state dir>/layer-view.json`.
    func loadViewState() {
        guard let root = documentKey, let scene = doc?.ref.name, let s = readViewStates()[root]?[scene] else {
            hidden = []
            locked = []
            return
        }
        hidden = Set(s.hidden)
        locked = Set(s.locked)
    }

    /// Writes them back. Only the app reads this file; it's never in the document.
    func saveViewState() {
        guard let url = viewStateURL, let root = documentKey, let scene = doc?.ref.name else { return }
        var all = readViewStates()
        var forDoc = all[root] ?? [:]
        let s = LayerViewState(hidden: hidden.sorted(), locked: locked.sorted())
        if s.hidden.isEmpty, s.locked.isEmpty { forDoc[scene] = nil } else { forDoc[scene] = s }
        all[root] = forDoc.isEmpty ? nil : forDoc
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        guard let data = try? encoder.encode(all) else { return }
        try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? data.write(to: url, options: .atomic)
    }

    /// The render's `hide` and `focus` params.
    var renderParams: [String: JSONValue] {
        var p: [String: JSONValue] = [:]
        let shown = hidden.filter { tree.byID[$0] != nil }
        if !shown.isEmpty { p["hide"] = .array(shown.sorted().map { .string($0) }) }
        if let inside, tree.byID[inside] != nil { p["focus"] = .string(inside) }
        return p
    }

    // MARK: Picking on the stage (D6)

    /// The item under a scene pixel that a click can pick, and what it picks.
    func pick(scenePixel x: Int, _ y: Int, render: StageImage) -> (path: String, target: LayerTarget)? {
        guard let editor = app?.editor else { return nil }
        let tree = self.tree, inside = self.inside, locked = self.locked, hidden = self.hidden
        let path = editor.hitTest(scenePixel: x, y, render: render) { p in
            !tree.isHidden(p, hidden: hidden) && tree.pickable(p, inside: inside, locked: locked) && tree.target(ofItem: p, inside: inside) != nil
        }
        guard let path, let t = tree.target(ofItem: path, inside: inside) else { return nil }
        return (path, t)
    }

    /// The pointer moved over the stage with the Select tool: what a click would pick.
    func stageHover(_ p: CGPoint, geometry: StageGeometry) {
        guard drag == nil, app?.editor.drag == nil, menu == nil, let render = doc?.render else { return }
        let px = geometry.scenePixel(fromView: p)
        let t = pick(scenePixel: px.x, px.y, render: render)?.target
        if t != hover { hover = t }
    }

    /// What a click did: the selection to drag, or nothing to drag.
    enum ClickOutcome: Equatable {
        case none
        case selected([LayerTarget])
    }

    /// A click (or double-click) on a scene pixel with the Select tool: pick,
    /// toggle with ⇧, go inside on a double-click (D6).
    @discardableResult
    func click(scenePixel x: Int, _ y: Int, shift: Bool, clickCount: Int) -> ClickOutcome {
        guard let doc, let render = doc.render else { return .none }
        let hit = pick(scenePixel: x, y, render: render)
        if clickCount >= 2, let hit, case .layer(let id) = hit.target {
            goInside(id)
            if let next = tree.target(ofItem: hit.path, inside: id) { select([next]) }
            return .none
        }
        guard let hit else {
            if !shift, !doc.selection.isEmpty { select([]) }
            return .none
        }
        if shift {
            toggle(hit.target)
            return .none
        }
        let current = doc.selection
        let sel = current.contains(hit.target) ? current : [hit.target]
        if sel != current { select(sel) }
        return .selected(sel)
    }

    /// A mouse down with the Select tool: the click, then dragging what's selected (D7).
    func stageMouseDown(_ p: CGPoint, geometry: StageGeometry, option: Bool, shift: Bool, clickCount: Int) {
        guard let editor = app?.editor else { return }
        let px = geometry.scenePixel(fromView: p)
        guard case .selected(let sel) = click(scenePixel: px.x, px.y, shift: shift, clickCount: clickCount) else { return }
        if sel.count == 1, case .item(let path) = sel[0] {
            editor.drag = ItemDrag(path: path, startView: p, startScene: geometry.scenePoint(fromView: p), byPixel: option)
        } else {
            drag = SelectionDrag(targets: sel, paths: tree.paths(of: sel), startView: p, startScene: geometry.scenePoint(fromView: p))
        }
    }

    /// Returns whether a selection drag took the event.
    func stageMouseDragged(_ p: CGPoint, geometry: StageGeometry) -> Bool {
        guard var d = drag else { return false }
        if !d.moved, hypot(p.x - d.startView.x, p.y - d.startView.y) < 3 { return true }
        d.moved = true
        if let origin = geometry.tileOrigin {
            let s = geometry.scenePoint(fromView: p)
            let a = StageGeometry.sceneToTile(x: Int(floor(d.startScene.x)), y: Int(floor(d.startScene.y)), level: 0, origin: origin)
            let b = StageGeometry.sceneToTile(x: Int(floor(s.x)), y: Int(floor(s.y)), level: 0, origin: origin)
            d.dc = b.col - a.col
            d.dr = b.row - a.row
        }
        if d != drag { drag = d }
        return true
    }

    /// Returns whether a selection drag took the event; a real move is one edit.
    func stageMouseUp() -> Bool {
        guard let d = drag else { return false }
        drag = nil
        if d.moved, !d.isZero { Task { await move(d.targets, dc: d.dc, dr: d.dr, dl: 0) } }
        return true
    }

    /// A right-click on the stage: what's under the pointer joins the selection
    /// rule (D9), then the menu opens there.
    func stageRightClick(_ p: CGPoint, geometry: StageGeometry, windowPoint: CGPoint) {
        guard let doc, let render = doc.render else { return }
        let px = geometry.scenePixel(fromView: p)
        openMenu(atWindow: windowPoint, target: pick(scenePixel: px.x, px.y, render: render)?.target)
    }

    // MARK: The right-click menu (D9)

    var menuItems: [LayerMenuItem] {
        let layer = doc?.selectedLayer
        return [
            LayerMenuItem(action: .group, title: "Group", shortcut: "⌘G", enabled: !selection.isEmpty),
            LayerMenuItem(action: .ungroup, title: "Ungroup", shortcut: "⇧⌘G", enabled: layer != nil),
            LayerMenuItem(action: .inside, title: "Go Inside", shortcut: "", enabled: layer != nil, separatorBefore: true),
            LayerMenuItem(action: .hide, title: layer.map { hidden.contains($0) } == true ? "Show" : "Hide", shortcut: "", enabled: layer != nil),
            LayerMenuItem(action: .lock, title: layer.map { locked.contains($0) } == true ? "Unlock" : "Lock", shortcut: "", enabled: layer != nil),
        ]
    }

    /// Opens the menu at a window point. A target outside the selection
    /// becomes the selection first; one in it keeps the selection.
    func openMenu(atWindow point: CGPoint, target: LayerTarget?) {
        if let target, !selection.contains(target) { select([target]) }
        drag = nil
        app?.editor.drag = nil
        hover = nil
        var origin = menuAnchor.map { $0.convert(point, from: nil) } ?? point
        if let bounds = menuAnchor?.bounds, bounds.width > 0 {
            origin.x = max(4, min(origin.x, bounds.width - Self.menuSize.width - 4))
            origin.y = max(4, min(origin.y, bounds.height - Self.menuSize.height - 4))
        }
        menu = LayerMenuState(origin: origin, highlighted: nil)
        announce("Menu: \(menuItems.filter(\.enabled).map(\.title).joined(separator: ", "))")
    }

    static let menuSize = CGSize(width: 210, height: 160)

    func closeMenu() { menu = nil }

    /// ↑ ↓ in the menu: the next enabled item.
    func moveMenuHighlight(_ d: Int) {
        guard var m = menu else { return }
        let items = menuItems
        let enabled = items.indices.filter { items[$0].enabled }
        guard !enabled.isEmpty else { return }
        if let h = m.highlighted, let at = enabled.firstIndex(of: h) {
            m.highlighted = enabled[(at + d + enabled.count) % enabled.count]
        } else {
            m.highlighted = d > 0 ? enabled.first : enabled.last
        }
        menu = m
    }

    func perform(_ action: LayerMenuAction) async {
        menu = nil
        let layer = doc?.selectedLayer
        switch action {
        case .group: await group()
        case .ungroup: await ungroup()
        case .inside: if let layer { goInside(layer) }
        case .hide: if let layer { toggleHidden(layer) }
        case .lock: if let layer { toggleLocked(layer) }
        }
    }

    // MARK: Edits (D4, D10)

    /// Runs one `scene.layer.*` edit on the open scene.
    @discardableResult
    func call(_ method: String, _ params: [String: JSONValue]) async -> LayerEditResult? {
        guard let app, let doc, let connection = app.connection else { return nil }
        var p = params
        p["name"] = .string(doc.ref.name)
        app.editor.pendingEdits += 1
        defer { app.editor.pendingEdits -= 1 }
        do {
            let r: LayerEditResult = try await connection.call(method, p)
            // The edit may have put the items in depth order (a move gives them new depths): follow the paths it moved.
            if app.document === doc { followSort(r.reordered) }
            if let s = r.summary { app.lastEdit = s }
            app.notice = nil
            return r
        } catch let e as RPCError {
            app.notice = e.message
        } catch {
            app.notice = error.localizedDescription
        }
        return nil
    }

    private func members(_ targets: [LayerTarget]) -> [String: JSONValue] {
        var p: [String: JSONValue] = [:]
        let layers = targets.compactMap(\.layerID), items = targets.compactMap(\.itemPath)
        if !layers.isEmpty { p["layers"] = .array(layers.map { .string($0) }) }
        if !items.isEmpty { p["items"] = .array(items.map { .string($0) }) }
        return p
    }

    /// Group (⌘G): a new layer under the selection's common parent, named
    /// by the server ("Group n"), then selected.
    @discardableResult
    func group() async -> LayerEditResult? {
        let sel = selection
        guard !sel.isEmpty, let r = await call("scene.layer.new", members(sel)), let made = r.layer else { return nil }
        await app?.reloadItems()
        app?.editor.sidebarTab = .layers
        select([.layer(made.id)], announce: false)
        let n = tree.byID[made.id]?.items ?? 0
        let said = "Grouped \(n) item\(n == 1 ? "" : "s") as \(made.name). Rename it in the inspector."
        app?.lastEdit = said
        announce(said)
        return r
    }

    /// Ungroup (⇧⌘G): the layer's contents move up a level and are selected.
    @discardableResult
    func ungroup() async -> LayerEditResult? {
        guard let id = doc?.selectedLayer, let r = await call("scene.layer.ungroup", ["layer": .string(id)]) else { return nil }
        if inside == id { inside = r.parent }
        hidden.remove(id)
        locked.remove(id)
        expanded.remove(id)
        saveViewState()
        await app?.reloadItems()
        select((r.layers ?? []).map { .layer($0) } + r.itemPaths.map { .item($0) }, announce: false)
        app?.requestRender()
        if let s = r.summary { announce(s) }
        return r
    }

    /// The inspector's name field.
    @discardableResult
    func rename(_ to: String) async -> LayerEditResult? {
        guard let id = doc?.selectedLayer else { return nil }
        let name = to.trimmingCharacters(in: .whitespaces)
        guard !name.isEmpty, name != tree.name(of: id) else {
            syncDraft(force: true)
            return nil
        }
        let r = await call("scene.layer.rename", ["layer": .string(id), "to": .string(name)])
        await app?.reloadItems()
        syncDraft(force: true)
        if let s = r?.summary { announce(s) }
        return r
    }

    /// Moves the selection (or `targets`) by tiles: one item alone with
    /// `scene.move`, as before; layers or several things together with
    /// `scene.layer.move` (D7).
    @discardableResult
    func move(_ targets: [LayerTarget]? = nil, dc: Int, dr: Int, dl: Int) async -> EditResult? {
        let sel = targets ?? selection
        guard !sel.isEmpty else { return nil }
        if sel.count == 1, case .item = sel[0], targets == nil, doc?.selectedItem != nil { return await app?.editor.move(dc: dc, dr: dr, dl: dl) }
        var p = members(sel)
        p["by"] = [JSONValue(dc), JSONValue(dr), JSONValue(dl)]
        let r = await call("scene.layer.move", p)
        if let s = r?.summary { announce(s) }
        return r?.edit
    }

    /// Moved by: how far the selected layer has moved since it was selected.
    var movedBy: (col: Int, row: Int)? {
        guard let doc, let b = baseline, b.layer == doc.selectedLayer else { return nil }
        return b.moved(items: doc.itemsByPath)
    }

    /// Typing in Moved by: move the layer by the difference.
    @discardableResult
    func setMovedBy(col: Int? = nil, row: Int? = nil) async -> EditResult? {
        let now = movedBy ?? (0, 0)
        let dc = (col ?? now.col) - now.col, dr = (row ?? now.row) - now.row
        guard dc != 0 || dr != 0 else { return nil }
        return await move(dc: dc, dr: dr, dl: 0)
    }

    // MARK: Placing (D13)

    /// Where new items go: a layer id, or nil for the top level.
    var placeTarget: String? { tree.placeTarget(selection: selection, inside: inside) }

    var placeTargetName: String { placeTarget.map { tree.name(of: $0) } ?? "Top level" }

    /// `scene.place`'s layer.
    var placeParams: [String: JSONValue] { placeTarget.map { ["layer": .string($0)] } ?? [:] }

    /// Place into it: the Library and the Place tool, with this layer as the target.
    func placeInto() {
        guard let app else { return }
        app.editor.sidebarTab = .library
        app.editor.setTool(.place)
        announce("Placing into \(placeTargetName): choose an object in the Library, then click a tile")
    }

    // MARK: The tree's keys (D11)

    /// The rows the tree shows now.
    func visibleRows(query: String) -> [LayerTree.Row] {
        guard let doc else { return [] }
        return query.isEmpty ? tree.rows(expanded: expanded) : tree.rows(matching: query, items: doc.itemsByPath)
    }

    /// A key in the focused Layers tree. Returns false for keys it doesn't use.
    func treeKey(_ k: KeyInput, query: String) -> Bool {
        if k.command || k.control || k.option { return false }
        let rows = visibleRows(query: query)
        let current = selection.last.flatMap { t in rows.firstIndex { $0.target == t } }
        switch k.key {
        case .up, .down:
            guard !rows.isEmpty else { return true }
            let d = k.key == .up ? -1 : 1
            let next = current.map { min(max($0 + d, 0), rows.count - 1) } ?? (d > 0 ? 0 : rows.count - 1)
            let t = rows[next].target
            if k.shift {
                if !selection.contains(t) { select(selection + [t], reveal: false) }
            } else {
                select([t], reveal: false)
            }
            return true
        case .right:
            guard let i = current, case .layer(let id) = rows[i].target else { return true }
            if !expanded.contains(id) { expanded.insert(id) } else if i + 1 < rows.count, rows[i + 1].depth > rows[i].depth {
                select([rows[i + 1].target], reveal: false)
            }
            return true
        case .left:
            guard let i = current else { return true }
            if case .layer(let id) = rows[i].target, expanded.contains(id) {
                expanded.remove(id)
            } else if let parent = tree.parent(of: rows[i].target) {
                select([.layer(parent)], reveal: false)
            }
            return true
        case .returnKey:
            if let id = doc?.selectedLayer { goInside(id) }
            return true
        case .escape:
            // As the stage (R9): the menu, then the selection, then the layer you're inside.
            if menu != nil { closeMenu() } else if !selection.isEmpty { select([]) } else if inside != nil { done() }
            return true
        default:
            return false
        }
    }

    // MARK: VoiceOver

    func announce(_ text: String) { app?.editor.announce(text) }
}
