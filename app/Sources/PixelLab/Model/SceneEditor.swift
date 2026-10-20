import AppKit
import Observation
import SwiftUI

/// The stage's tools (V, A, H). Space held pans with any tool.
enum StageTool: String, CaseIterable, Identifiable, Sendable {
    case select, place, pan
    var id: String { rawValue }
    var title: String {
        switch self {
        case .select: "Select"
        case .place: "Place"
        case .pan: "Pan"
        }
    }
    var symbol: String {
        switch self {
        case .select: "cursorarrow"
        case .place: "plus.square.on.square"
        case .pan: "hand.raised"
        }
    }
    var key: Character {
        switch self {
        case .select: "v"
        case .place: "a"
        case .pan: "h"
        }
    }
}

struct Tile: Hashable, Sendable, CustomStringConvertible {
    var col: Int
    var row: Int
    var level: Int
    var description: String { "\(col),\(row),\(level)" }
    var json: JSONValue { [JSONValue(col), JSONValue(row), JSONValue(level)] }
}

/// A key press on the stage, independent of NSEvent (the script hook sends
/// these too).
struct KeyInput: Equatable, Sendable {
    enum Key: Equatable, Sendable {
        case left, right, up, down, pageUp, pageDown
        case delete, returnKey, escape, space
        case char(Character)
    }
    var key: Key
    var option = false
    var shift = false
    var command = false
    var control = false

    /// "right", "opt+up", "shift+down", "delete", "return", "esc", "v", "2", "]".
    init?(spec: String) {
        var parts = spec.lowercased().split(separator: "+").map(String.init)
        guard let last = parts.popLast() else { return nil }
        for m in parts {
            switch m {
            case "opt", "option", "alt": option = true
            case "shift": shift = true
            case "cmd", "command": command = true
            case "ctrl", "control": control = true
            default: return nil
            }
        }
        switch last {
        case "left": key = .left
        case "right": key = .right
        case "up": key = .up
        case "down": key = .down
        case "pageup": key = .pageUp
        case "pagedown": key = .pageDown
        case "delete", "backspace": key = .delete
        case "return", "enter": key = .returnKey
        case "esc", "escape": key = .escape
        case "space": key = .space
        default:
            guard last.count == 1, let c = last.first else { return nil }
            key = .char(c)
        }
    }

    init(key: Key, option: Bool = false, shift: Bool = false, command: Bool = false, control: Bool = false) {
        self.key = key
        self.option = option
        self.shift = shift
        self.command = command
        self.control = control
    }
}

/// Dragging the selected item on the stage: by tiles, or by pixels with ⌥.
struct ItemDrag: Equatable {
    var path: String
    var startView: CGPoint
    var startScene: CGPoint
    var byPixel: Bool
    var moved = false
    /// Tiles (col, row) or pixels (x, y) moved so far.
    var dc = 0, dr = 0, dx = 0, dy = 0

    /// The ghost's offset from the item, in scene pixels.
    var sceneOffset: CGPoint {
        byPixel
            ? CGPoint(x: dx, y: dy)
            : CGPoint(x: CGFloat(dc - dr) * StageGeometry.tileW / 2, y: CGFloat(dc + dr) * StageGeometry.tileH / 2)
    }
    var isZero: Bool { byPixel ? (dx == 0 && dy == 0) : (dc == 0 && dr == 0) }
}

/// Scene mode's editing state and actions: tools, selection, place, drag,
/// keys and the item edits, all through the session server (`scene.*`).
@MainActor @Observable
final class SceneEditor {
    @ObservationIgnored weak var app: AppModel?
    /// The stage's input view, to give it focus.
    @ObservationIgnored weak var stageView: NSView?

    var tool: StageTool = .select
    /// Space held: pan with any tool.
    var spaceHeld = false
    var panning = false
    /// The library object the Place tool places.
    var placeObject: String?
    /// The level the Place tool places at (follows the selected item's).
    var placeLevel = 0
    /// The tile under the pointer (Place tool).
    var hoverTile: Tile?
    /// The scene pixel under the pointer.
    var hoverPixel: CGPoint?
    /// ⌥ while placing: place by pixel instead of on a tile.
    var placeByPixel = false
    /// The stage cursor: where Return places; arrows move it when nothing is selected.
    var cursorTile: Tile?
    var drag: ItemDrag?
    /// An object dragged from the library over the stage.
    var dropObject: String?
    var stageFocused = false
    /// The sidebar's lower section: the open scene's Layers (first, and the
    /// default) or the Library (docs/intents/2026-10-scene-layers/spec.md, D11).
    var sidebarTab: SidebarTab = .layers
    /// Drafts of the inspector's Class and ID fields.
    var classDraft = ""
    var idDraft = ""
    /// Edits sent and not answered yet (the script hook waits on them).
    @ObservationIgnored var pendingEdits = 0

    enum SidebarTab: String, Hashable { case layers, library }

    var effectiveTool: StageTool { spaceHeld || panning ? .pan : tool }

    private var doc: DocumentModel? {
        guard let d = app?.document, d.ref.kind == .scene else { return nil }
        return d
    }

    var selected: SceneItemInfo? { doc?.selectedItem }

    /// A new doc: drop what pointed into the old one.
    func reset() {
        drag = nil
        hoverTile = nil
        hoverPixel = nil
        cursorTile = nil
        dropObject = nil
        syncDrafts()
    }

    // MARK: Selection

    func select(_ path: String?, reveal: Bool = true) {
        guard let doc else { return }
        doc.selectedPath = path
        if let item = doc.selectedItem {
            if let l = item.at.standingLevel { placeLevel = l }
            if reveal { self.reveal(path!) }
            announce("\(item.object) selected, \(item.at.spoken)")
        }
        app?.layers.selectionChanged()
        syncDrafts()
    }

    /// Selects the item with this id (motion tracks, problems).
    func select(id: String) {
        guard let path = doc?.items.first(where: { $0.id == id })?.path else { return }
        select(path)
    }

    /// Expands the layers that hold the item, so its row shows in Layers.
    func reveal(_ path: String) {
        app?.layers.reveal(.item(path))
    }

    func syncDrafts() {
        classDraft = selected?.class ?? ""
        idDraft = selected?.id ?? ""
    }

    func focusStage() {
        guard let v = stageView, let w = v.window else { return }
        w.makeFirstResponder(v)
    }

    func setTool(_ t: StageTool) {
        tool = t
        if t != .place { hoverTile = nil }
        announce("\(t.title) tool")
    }

    func choosePlaceObject(_ name: String) {
        placeObject = name
        setTool(.place)
    }

    // MARK: Hit-testing

    /// The topmost item painted at a scene pixel (render bounds, refined by
    /// each object's own pixels); `LayersModel.pick` turns it into what a click picks.
    /// `allowed` leaves items out of the test (layers: outside the layer you're
    /// in, or locked; docs/intents/2026-10-scene-layers/spec.md, D6).
    func hitTest(scenePixel x: Int, _ y: Int, render: StageImage, allowed: ((String) -> Bool)? = nil) -> String? {
        let arts = app?.thumbnails.arts ?? [:]
        let candidates: [HitTester.Candidate] = render.items.compactMap { item in
            guard let path = item.path, allowed?(path) ?? true, let rect = render.sceneRect(of: item) else { return nil }
            return HitTester.Candidate(path: path, rect: rect, object: item.pose ?? item.object)
        }
        return HitTester.pick(atPixel: x, y, in: candidates) { c, dx, dy in
            guard let art = arts[c.object] else {
                app?.thumbnails.request(c.object)
                return nil
            }
            let box = art.box ?? art.viewBox
            return art.isOpaque(objectX: Int(box.minX) + dx, objectY: Int(box.minY) + dy)
        }
    }

    // MARK: Pointer

    func pointerMoved(_ p: CGPoint, geometry: StageGeometry, option: Bool) {
        hoverPixel = geometry.scenePoint(fromView: p)
        placeByPixel = option
        if effectiveTool == .select, dropObject == nil { app?.layers.stageHover(p, geometry: geometry) }
        guard effectiveTool == .place || dropObject != nil else { return }
        if let t = geometry.tile(atView: p, level: placeLevel) {
            let tile = Tile(col: t.col, row: t.row, level: t.level)
            if hoverTile != tile { hoverTile = tile }
            cursorTile = tile
        }
    }

    func pointerExited() {
        hoverTile = nil
        hoverPixel = nil
        app?.layers.hover = nil
    }

    func mouseDown(_ p: CGPoint, geometry: StageGeometry, option: Bool, shift: Bool = false, clickCount: Int) {
        guard doc != nil else { return }
        switch effectiveTool {
        case .pan:
            panning = true
        case .place:
            pointerMoved(p, geometry: geometry, option: option)
            Task { await placeHere(geometry: geometry, at: p, byPixel: option) }
        case .select:
            // Picks layers, toggles with ⇧, goes inside on a double-click and starts the drag (D6, D7).
            app?.layers.stageMouseDown(p, geometry: geometry, option: option, shift: shift, clickCount: clickCount)
        }
    }

    func mouseDragged(_ p: CGPoint, geometry: StageGeometry, option: Bool) {
        if app?.layers.stageMouseDragged(p, geometry: geometry) == true { return }
        guard var d = drag, let item = doc?.itemsByPath[d.path] else { return }
        if !d.moved, hypot(p.x - d.startView.x, p.y - d.startView.y) < 3 { return }
        d.moved = true
        d.byPixel = option
        let s = geometry.scenePoint(fromView: p)
        if d.byPixel {
            d.dx = Int(floor(s.x)) - Int(floor(d.startScene.x))
            d.dy = Int(floor(s.y)) - Int(floor(d.startScene.y))
        } else if let origin = geometry.tileOrigin {
            let level = item.at.standingLevel ?? 0
            let a = StageGeometry.sceneToTile(x: Int(floor(d.startScene.x)), y: Int(floor(d.startScene.y)), level: level, origin: origin)
            let b = StageGeometry.sceneToTile(x: Int(floor(s.x)), y: Int(floor(s.y)), level: level, origin: origin)
            d.dc = b.col - a.col
            d.dr = b.row - a.row
        }
        if d != drag { drag = d }
    }

    func mouseUp() {
        panning = false
        if app?.layers.stageMouseUp() == true { return }
        guard let d = drag else { return }
        drag = nil
        guard d.moved, !d.isZero else { return }
        Task {
            if d.byPixel { await nudge(dx: d.dx, dy: d.dy) } else { await move(dc: d.dc, dr: d.dr, dl: 0) }
        }
    }

    // MARK: Place

    /// The tile at the middle of the view at the place level (where the stage
    /// cursor starts, as in the lab).
    func defaultCursor() -> Tile? {
        guard let doc, let render = doc.render, let origin = doc.origin else { return nil }
        let t = StageGeometry.sceneToTile(x: Int(render.viewBox.midX), y: Int(render.viewBox.midY), level: placeLevel, origin: origin)
        return Tile(col: t.col, row: t.row, level: t.level)
    }

    private func placeHere(geometry: StageGeometry, at p: CGPoint, byPixel: Bool) async {
        guard let object = placeObject else {
            app?.notice = "Choose an object in the Library to place."
            return
        }
        if byPixel {
            let px = geometry.scenePixel(fromView: p)
            await place(object, px: (px.x, px.y))
        } else if let t = geometry.tile(atView: p, level: placeLevel) {
            await place(object, at: Tile(col: t.col, row: t.row, level: t.level))
        }
    }

    /// Return: place the library object at the stage cursor.
    func placeAtCursor() async {
        guard let object = placeObject else {
            app?.notice = "Choose an object in the Library to place."
            return
        }
        guard let t = cursorTile ?? defaultCursor() else { return }
        await place(object, at: t)
    }

    /// Places into the selected layer, the selected item's, or the one you're inside (D13).
    @discardableResult
    func place(_ object: String, at tile: Tile) async -> EditResult? {
        cursorTile = tile
        let into = app?.layers.placeParams ?? [:]
        return await edit("scene.place", ["object": .string(object), "at": tile.json].merging(into) { a, _ in a })
    }

    @discardableResult
    func place(_ object: String, px: (Int, Int)) async -> EditResult? {
        let into = app?.layers.placeParams ?? [:]
        return await edit("scene.place", ["object": .string(object), "px": [JSONValue(px.0), JSONValue(px.1)]].merging(into) { a, _ in a })
    }

    /// A library thumbnail dropped on the stage.
    func drop(_ object: String, at p: CGPoint, geometry: StageGeometry, option: Bool) {
        dropObject = nil
        placeObject = object
        Task { await placeHere(geometry: geometry, at: p, byPixel: option) }
    }

    // MARK: Item edits

    private var timeParam: [String: JSONValue] {
        let t = Int(app?.timeline.time ?? 0)
        return t > 0 ? ["time": JSONValue(t)] : [:]
    }

    @discardableResult
    func move(dc: Int, dr: Int, dl: Int) async -> EditResult? {
        guard let item = selected else {
            // Layers, or several things: they move together by tiles (D7).
            if let layers = app?.layers, !layers.selection.isEmpty { return await layers.move(dc: dc, dr: dr, dl: dl) }
            moveCursor(dc: dc, dr: dr, dl: dl)
            return nil
        }
        return await edit("scene.move", ["item": .string(item.path), "by": [JSONValue(dc), JSONValue(dr), JSONValue(dl)]].merging(timeParam) { a, _ in a })
    }

    @discardableResult
    func nudge(dx: Int, dy: Int) async -> EditResult? {
        guard let item = selected else {
            if app?.layers.selection.isEmpty == false {
                app?.notice = "Pixel moves are for one item; layers move by tiles."
                announce("Pixel moves are for one item; layers move by tiles")
            }
            return nil
        }
        return await edit("scene.move", ["item": .string(item.path), "nudge": [JSONValue(dx), JSONValue(dy)]].merging(timeParam) { a, _ in a })
    }

    func moveCursor(dc: Int, dr: Int, dl: Int) {
        guard var t = cursorTile ?? defaultCursor() else { return }
        t.col += dc
        t.row += dr
        t.level += dl
        cursorTile = t
        placeLevel = t.level
        if tool == .place { hoverTile = t }
        announce("Cursor at column \(t.col), row \(t.row), level \(t.level)")
    }

    /// One field of the selected item: col, row, level, x, y (`scene.set`); a
    /// pixel-placed item has a level too.
    @discardableResult
    func set(_ field: String, _ value: Int) async -> EditResult? {
        guard let item = selected else { return nil }
        return await edit("scene.set", ["item": .string(item.path), field: JSONValue(value)])
    }

    func setClass(_ value: String) async {
        guard let item = selected, value != (item.class ?? "") else { return }
        await edit("scene.set", ["item": .string(item.path), "class": .string(value.trimmingCharacters(in: .whitespaces))])
        syncDrafts()
    }

    func setID(_ value: String) async {
        guard let item = selected, value != (item.id ?? "") else { return }
        await edit("scene.set", ["item": .string(item.path), "id": .string(value.trimmingCharacters(in: .whitespaces))])
        syncDrafts()
    }

    /// The Depth control: draw the selected item by where it stands (`auto`), over
    /// everything else in its list (`front`) or under it (`back`)
    /// (docs/intents/2026-10-depth-order/spec.md, R5, R14). An item alone in its
    /// SVG group sorts as that group, so the control sets the group's depth.
    @discardableResult
    func setDepth(_ mode: DepthMode) async -> EditResult? {
        guard let doc, let item = selected, doc.depthMode(of: item) != mode else { return nil }
        if let group = doc.soleGroup(of: item) { return await setGroupDepth(group.path, mode) }
        return await edit("scene.set", ["item": .string(item.path), "depth": .string(mode.rawValue)])
    }

    /// The wrapper line's menu: a group's depth. `scene.set` takes a group's path for this alone;
    /// the result's path is the group's, not an item to select.
    @discardableResult
    func setGroupDepth(_ group: String, _ mode: DepthMode) async -> EditResult? {
        guard let current = doc?.groups.first(where: { $0.path == group }), current.depthMode != mode else { return nil }
        return await edit("scene.set", ["item": .string(group), "depth": .string(mode.rawValue)], selectResult: false)
    }

    /// Checks' Sort button: put the open scene's items in depth order, in one undo step.
    @discardableResult
    func sort() async -> SceneSortResult? {
        guard let app, let doc, let connection = app.connection else { return nil }
        pendingEdits += 1
        defer { pendingEdits -= 1 }
        do {
            let r: SceneSortResult = try await connection.call("scene.sort", ["name": .string(doc.ref.name)])
            if app.document === doc { app.layers.followSort(r.reordered) }
            if let s = r.summary { announce(s) }
            app.lastEdit = r.summary
            app.notice = nil
            return r
        } catch let e as RPCError {
            app.notice = e.message
        } catch {
            app.notice = error.localizedDescription
        }
        return nil
    }

    @discardableResult func duplicate() async -> EditResult? { await itemCommand("scene.duplicate") }
    @discardableResult func snap() async -> EditResult? { await itemCommand("scene.snap") }

    @discardableResult
    func remove() async -> EditResult? {
        guard selected != nil else { return nil }
        let r = await itemCommand("scene.remove")
        if r != nil { doc?.selectedPath = nil; syncDrafts() }
        return r
    }

    @discardableResult
    private func itemCommand(_ method: String) async -> EditResult? {
        guard let item = selected else { return nil }
        return await edit(method, ["item": .string(item.path)])
    }

    /// Runs one scene edit on the open doc and selects the item it reports.
    /// The edit leaves the items in depth order: what was selected follows its
    /// item to its new path (`reordered`), and `selectResult: false` keeps the
    /// result's `path` (a group's, say) from being selected.
    @discardableResult
    func edit(_ method: String, _ params: [String: JSONValue], selectResult: Bool = true) async -> EditResult? {
        guard let app, let doc, let connection = app.connection else { return nil }
        var p = params
        p["name"] = .string(doc.ref.name)
        pendingEdits += 1
        defer { pendingEdits -= 1 }
        do {
            let r: EditResult = try await connection.call(method, p)
            if app.document === doc { app.layers.followSort(r.reordered) }
            if selectResult, method != "scene.remove", let path = r.path, app.document === doc {
                doc.selectedPath = path
                reveal(path)
                app.layers.selectionChanged()
            }
            if let s = r.summary { announce(s) }
            app.lastEdit = r.summary
            app.notice = nil
            return r
        } catch let e as RPCError {
            app.notice = e.message
        } catch {
            app.notice = error.localizedDescription
        }
        return nil
    }

    // MARK: Keys

    /// Stage keys. Returns false for keys the stage doesn't use.
    func handleKey(_ k: KeyInput) -> Bool {
        guard let app else { return false }
        if k.command || k.control { return false }
        switch k.key {
        case .escape:
            // A drag, the menu, the selection, then the layer you're inside, then the tool.
            let layers = app.layers
            if drag != nil { drag = nil } else if layers.drag != nil { layers.drag = nil } else if layers.menu != nil { layers.closeMenu() }
            else if !(doc?.selection.isEmpty ?? true) { layers.select([]) } else if layers.inside != nil { layers.done() }
            else if tool != .select { setTool(.select) }
            return true
        case .space:
            return true
        case .left, .right, .up, .down:
            let (x, y): (Int, Int) = switch k.key {
            case .left: (-1, 0)
            case .right: (1, 0)
            case .up: (0, -1)
            default: (0, 1)
            }
            if k.option {
                if selected != nil { Task { await nudge(dx: x, dy: y) } }
            } else if k.shift {
                // ⇧↑ / ⇧↓: a level up or down.
                if y != 0 { Task { await move(dc: 0, dr: 0, dl: -y) } }
            } else {
                Task { await move(dc: x, dr: y, dl: 0) }
            }
            return true
        case .pageUp, .pageDown:
            Task { await move(dc: 0, dr: 0, dl: k.key == .pageUp ? 1 : -1) }
            return true
        case .delete:
            Task { await remove() }
            return true
        case .returnKey:
            Task { await placeAtCursor() }
            return true
        case .char(let c):
            if k.option || k.shift { return false }
            if let t = StageTool.allCases.first(where: { $0.key == c }) {
                setTool(t)
                return true
            }
            if let n = c.wholeNumberValue {
                if n == 0 { app.zoom = .fit; return true }
                if n <= 4 { app.setZoomLevel(index: n - 1); return true }
                return false
            }
            return false
        }
    }

    // MARK: VoiceOver

    func announce(_ text: String) {
        guard let element = stageView ?? NSApp.mainWindow else { return }
        NSAccessibility.post(element: element, notification: .announcementRequested, userInfo: [
            .announcement: text, .priority: NSAccessibilityPriorityLevel.medium.rawValue,
        ])
    }
}
