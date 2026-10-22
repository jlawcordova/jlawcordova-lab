import AppKit
import Observation
import SwiftUI

/// Object mode's tools (B, E, G, I). Space held pans, as in scene mode.
enum PaintTool: String, CaseIterable, Identifiable, Sendable {
    case pencil, eraser, fill, picker
    var id: String { rawValue }
    var title: String {
        switch self {
        case .pencil: "Pencil"
        case .eraser: "Eraser"
        case .fill: "Fill"
        case .picker: "Picker"
        }
    }
    var symbol: String {
        switch self {
        case .pencil: "pencil"
        case .eraser: "eraser"
        case .fill: "drop.fill"
        case .picker: "eyedropper"
        }
    }
    var key: Character {
        switch self {
        case .pencil: "b"
        case .eraser: "e"
        case .fill: "g"
        case .picker: "i"
        }
    }
}

/// Pixels painted on the stage before the server's render shows them: drawn
/// over the art at once, dropped when a render that includes them lands.
struct OverlayStamp: Sendable {
    /// The paint job that sends them; a render started after it was answered includes them.
    var job: Int
    var pixels: [PixelPoint]
    /// nil: erased.
    var hex: String?
    var layer: Int
    var frame: Int
}

/// Object mode's editing state and actions: the paint tools with strokes
/// batched into few `object.paint` calls, the color, layers, frames, onion
/// skin, frame playback and the object's facts, all through the session
/// server (`object.*`). Behaves as the web lab's object mode
/// (object-mode.ts, palette-panel.ts, layers-panel.ts, frames-panel.ts).
@MainActor @Observable
final class ObjectEditor {
    @ObservationIgnored weak var app: AppModel?

    enum SidebarTab: String, Hashable { case palette, library }

    var tool: PaintTool = .pencil
    var color = "ink"
    /// The layer painting goes to, and the frame every loop shows.
    var layer = 0
    var frame = 0
    /// Layers hidden in the editor only (never saved).
    var hidden: Set<Int> = []
    /// The previous frame of the layer at 30% under the art.
    var onion = false
    /// The pixel grid, from 8×.
    var pixelGrid = UserDefaults.standard.object(forKey: "objectPixelGrid") as? Bool ?? true {
        didSet { UserDefaults.standard.set(pixelGrid, forKey: "objectPixelGrid") }
    }
    var sidebarTab: SidebarTab = .palette
    /// `object.show` for the open object.
    var info: ObjectShowResult?
    /// The pixel under the pointer, and the keyboard cursor.
    var hover: PixelPoint?
    var cursor = PixelPoint(0, 0)
    /// Painted but not rendered yet.
    var overlay: [OverlayStamp] = []
    /// The previous frame of the layer (onion skin).
    var onionImage: StageImage?
    /// Renders by frame and visible layers, for playback and the frame strip;
    /// emptied on every change to the doc.
    private(set) var frameImages: [String: StageImage] = [:]
    var playing = false
    /// The open render's pixels, for the color under the pointer.
    var renderColors: PixelColors?
    /// The Object panel's size fields.
    var widthDraft = ""
    var heightDraft = ""

    /// Edits sent and not answered yet, and loads in flight (the script hook waits on them).
    @ObservationIgnored var pendingEdits = 0
    @ObservationIgnored private var loadingInfo = false
    @ObservationIgnored private var needsInfo = false
    @ObservationIgnored private var loadingExtras = false
    @ObservationIgnored private var needsExtras = false
    /// Bumped when the doc changes, so a render started before it isn't cached.
    @ObservationIgnored private(set) var cacheGeneration = 0

    // Strokes
    private struct Stroke {
        var color: String?
        var last: PixelPoint
        var layer: Int
        var frame: Int
        var count = 0
    }
    private struct PaintJob {
        var id: Int
        var color: String?
        var layer: Int
        var frame: Int
        var batch = StrokeBatch()
    }
    @ObservationIgnored private var stroke: Stroke?
    @ObservationIgnored private var jobs: [PaintJob] = []
    @ObservationIgnored private var sending = false
    @ObservationIgnored private var nextJob = 0
    /// The last paint job the server answered.
    @ObservationIgnored private(set) var ackedJob = 0
    /// Where the last stroke ended (Shift-click draws a line from it).
    @ObservationIgnored var lastPoint: PixelPoint?
    @ObservationIgnored private var playTask: Task<Void, Never>?
    @ObservationIgnored private var lastSend = Date.distantPast
    @ObservationIgnored private var pumpScheduled = false
    /// Mid-stroke, the least time between two paint calls.
    static let minInterval: TimeInterval = 0.06
    /// How many calls a stroke took (the script hook reports it).
    @ObservationIgnored private(set) var lastStrokeCalls = 0
    @ObservationIgnored private var strokeCalls = 0

    var busy: Bool { pendingEdits > 0 || sending || !jobs.isEmpty || pumpScheduled || loadingInfo || loadingExtras }
    var stroking: Bool { stroke != nil }

    private var doc: DocumentModel? {
        guard let d = app?.document, d.ref.kind == .object else { return nil }
        return d
    }

    /// A new object: start on its first layer and frame.
    func reset() {
        stopPlaying()
        layer = 0
        frame = 0
        hidden = []
        hover = nil
        cursor = PixelPoint(0, 0)
        overlay = []
        onionImage = nil
        frameImages = [:]
        renderColors = nil
        info = nil
        stroke = nil
        jobs = []
        lastPoint = nil
        cacheGeneration += 1
    }

    /// The doc changed (an edit, undo, a file on disk): renders are stale.
    func docChanged() {
        cacheGeneration += 1
        frameImages = [:]
        requestInfo()
    }

    // MARK: Facts

    var layers: [ObjectLayerInfo] { info?.layers ?? [] }
    var currentLayer: ObjectLayerInfo? { layers.indices.contains(layer) ? layers[layer] : nil }
    /// Frames of the selected layer (1 for a still map).
    var frameCount: Int { currentLayer?.frames ?? 1 }
    var isBlock: Bool { info?.isBlock ?? false }
    var editable: Bool { info?.layersEditable ?? false }
    var visibleLayers: [Int] { layers.map(\.index).filter { !hidden.contains($0) } }

    /// "Layer 2 · cbob" or "Layer 3 · loop wf, 5 frames" (1-based, as the lab says).
    func label(_ index: Int) -> String {
        guard layers.indices.contains(index) else { return "Layer \(index + 1)" }
        let l = layers[index]
        if l.surface == true { return l.loop.map { "Surface · loop \($0), \(l.frames) frame\(l.frames == 1 ? "" : "s")" } ?? "Surface" }
        if let loop = l.loop { return "Layer \(index + 1) · loop \(loop), \(l.frames) frame\(l.frames == 1 ? "" : "s")" }
        return "Layer \(index + 1)\(l.class.map { " · \($0)" } ?? "")"
    }

    /// The ACTIONS rows of the selected layer (a map layer's own actions).
    var actionRows: [ActionRow] {
        Animation.rows(own: Animation.layers(of: info?.doc)[layer], layers: [])
    }

    /// The frame's name in its loop: f0, w3…
    func frameName(_ i: Int) -> String { "\(currentLayer?.prefix ?? "f")\(i)" }

    /// Loads `object.show` for the open object; coalesces.
    func requestInfo() {
        guard let app, let doc, let connection = app.connection else { return }
        if loadingInfo {
            needsInfo = true
            return
        }
        loadingInfo = true
        Task {
            repeat {
                needsInfo = false
                do {
                    let r: ObjectShowResult = try await connection.call("object.show", ["name": .string(doc.ref.name), "doc": true])
                    guard app.document === doc else { break }
                    let firstLoad = info == nil
                    info = r
                    clamp()
                    syncSizeDrafts()
                    // The first render went out before the layers were known.
                    if firstLoad, !r.isBlock { app.requestRender() }
                } catch {
                    if app.document === doc { app.notice = "Couldn't read \(doc.ref.name): \(error.localizedDescription)" }
                }
            } while needsInfo && app.document === doc
            loadingInfo = false
            requestExtras()
        }
    }

    func syncSizeDrafts() {
        guard let info else { return }
        widthDraft = String(info.size.first ?? 0)
        heightDraft = String(info.size.count > 1 ? info.size[1] : 0)
    }

    private func clamp() {
        layer = min(max(layer, 0), max(layers.count - 1, 0))
        frame = frameCount > 1 ? min(frame, frameCount - 1) : 0
        if let size = info?.size, size.count == 2 {
            cursor = PixelPoint(min(cursor.x, max(size[0] - 1, 0)), min(cursor.y, max(size[1] - 1, 0)))
        }
        hidden = hidden.filter { $0 < layers.count }
    }

    // MARK: Palette

    func paletteColor(_ name: String) -> PaletteColor? { app?.paletteByName[name] }

    /// The palette name of a hex the art shows.
    func colorName(hex: String) -> String? { app?.paletteByHex[hex.uppercased()] }

    var usedHexes: Set<String> { Set((info?.usedColors ?? []).map { $0.uppercased() }) }
    static let colorCap = 12

    /// Whether the object may paint a color: its tier, then the 12-color cap
    /// (palette-panel.ts allowed).
    func allowed(_ name: String) -> Bool {
        guard let c = paletteColor(name) else { return false }
        if c.tier == "outfit" && info?.character != true { return false }
        let used = usedHexes
        return used.contains(c.hex.uppercased()) || used.count < Self.colorCap
    }

    func whyNot(_ name: String) -> String {
        if paletteColor(name)?.tier == "outfit" { return "\(name) is an outfit color; only objects that extend character may use it" }
        if paletteColor(name) == nil { return "There's no palette color called \(name)." }
        return "This object already uses \(Self.colorCap) colors. Pick one it uses, or free one up first."
    }

    func setColor(_ name: String) {
        guard allowed(name) else {
            say(whyNot(name))
            return
        }
        color = name
        if tool == .eraser || tool == .picker { tool = .pencil }
        say("\(name) chosen")
    }

    /// The bar's Hand button (H): a held pan, like Space held, until another tool is picked.
    var handHeld = false

    func setTool(_ t: PaintTool) {
        tool = t
        handHeld = false
        if let v = app?.editor.stageView { v.window?.invalidateCursorRects(for: v) }
        say("\(t.title) tool")
    }

    func setHand(_ on: Bool) {
        handHeld = on
        if let v = app?.editor.stageView { v.window?.invalidateCursorRects(for: v) }
        say(on ? "Hand tool" : "\(tool.title) tool")
    }

    /// Why painting is off on the selected layer, or nil (ObjectMode.paintBlocked).
    var paintBlocked: String? {
        guard let info else { return "Loading the object…" }
        if info.isBlock { return nil }
        if let ext = info.extends, currentLayer?.isLoop == true {
            return "This layer is a frame loop of \(ext), which an outfit can't override."
        }
        return nil
    }

    // MARK: Pointer

    func pointerMoved(_ p: CGPoint, geometry: StageGeometry) {
        let px = StrokeMath.pixel(atView: p, geometry: geometry)
        if px != hover { hover = px }
    }

    func pointerExited() { hover = nil }

    func mouseDown(_ p: CGPoint, geometry: StageGeometry, option: Bool, shift: Bool) {
        guard let px = StrokeMath.pixel(atView: p, geometry: geometry) else { return }
        cursor = px
        hover = px
        press(px, option: option, shift: shift)
    }

    /// A click at a pixel with the current tool (the script hook calls this too).
    func press(_ px: PixelPoint, option: Bool = false, shift: Bool = false) {
        if tool == .picker || (tool == .pencil && option) {
            Task { await pick(px) }
            return
        }
        if tool == .fill {
            Task { await fill(px) }
            return
        }
        if shift, let from = lastPoint {
            beginStroke(at: from, paintFirst: false)
            extendStroke(to: px)
            endStroke()
            return
        }
        beginStroke(at: px)
    }

    func mouseDragged(_ p: CGPoint, geometry: StageGeometry) {
        hover = StrokeMath.pixel(atView: p, geometry: geometry)
        guard stroke != nil else { return }
        extendStroke(to: StrokeMath.unclampedPixel(atView: p, geometry: geometry))
    }

    func mouseUp() { endStroke() }

    // MARK: Strokes

    /// Starts a pencil or eraser stroke at a pixel.
    func beginStroke(at px: PixelPoint, paintFirst: Bool = true) {
        if let blocked = paintBlocked {
            say(blocked)
            return
        }
        let c: String? = tool == .eraser ? nil : color
        if let c, !allowed(c) {
            say(whyNot(c))
            return
        }
        stopPlaying()
        stroke = Stroke(color: c, last: px, layer: layer, frame: frame)
        strokeCalls = 0
        if paintFirst { enqueue([px]) }
    }

    /// Continues the stroke to a pixel, filling the gap from the last one.
    func extendStroke(to px: PixelPoint) {
        guard let s = stroke, px != s.last else { return }
        enqueue(StrokeMath.line(s.last, px))
        stroke?.last = px
    }

    func endStroke() {
        guard let s = stroke else { return }
        stroke = nil
        lastPoint = s.last
        pump(now: true)
        if !sending, jobs.isEmpty { lastStrokeCalls = strokeCalls }
    }

    private func enqueue(_ points: [PixelPoint]) {
        guard let s = stroke, let size = info.map({ CGSize(width: CGFloat($0.size[0]), height: CGFloat($0.size[1])) }) ?? app?.document?.render?.viewBox.size else { return }
        let pts = StrokeMath.clip(points, size: size)
        guard !pts.isEmpty else { return }
        if let i = jobs.indices.last, jobs[i].color == s.color, jobs[i].layer == s.layer, jobs[i].frame == s.frame {
            jobs[i].batch.add(pts)
            overlay.append(OverlayStamp(job: jobs[i].id, pixels: pts, hex: s.color.flatMap { paletteColor($0)?.hex }, layer: s.layer, frame: s.frame))
        } else {
            nextJob += 1
            var job = PaintJob(id: nextJob, color: s.color, layer: s.layer, frame: s.frame)
            job.batch.add(pts)
            jobs.append(job)
            overlay.append(OverlayStamp(job: job.id, pixels: pts, hex: s.color.flatMap { paletteColor($0)?.hex }, layer: s.layer, frame: s.frame))
        }
        pump()
    }

    /// Sends the oldest waiting paint job, one call at a time; what gathers
    /// meanwhile goes in the next call. Rapid calls with the same color,
    /// layer and frame merge into one undo step on the server.
    private func pump(now: Bool = false) {
        guard !sending, let app, let doc, let connection = app.connection else { return }
        guard !jobs.isEmpty else { return }
        // Mid-stroke, at most one call every `minInterval`: the samples in
        // between gather into the next call. The stroke's end goes at once.
        let wait = Self.minInterval - Date().timeIntervalSince(lastSend)
        if !now, stroke != nil, wait > 0 {
            if !pumpScheduled {
                pumpScheduled = true
                Task {
                    try? await Task.sleep(for: .milliseconds(Int(wait * 1000) + 1))
                    pumpScheduled = false
                    pump(now: true)
                }
            }
            return
        }
        lastSend = Date()
        var job = jobs.removeFirst()
        let pixels = job.batch.take()
        guard !pixels.isEmpty else { return pump() }
        sending = true
        strokeCalls += 1
        let erase = job.color == nil
        var params: [String: JSONValue] = [
            "name": .string(doc.ref.name), "pixels": .array(pixels.map(\.json)),
            "layer": JSONValue(job.layer), "frame": JSONValue(job.frame),
        ]
        if let c = job.color { params["color"] = .string(c) }
        Task {
            do {
                let r: EditResult = try await connection.call(erase ? "object.erase" : "object.paint", params)
                if let s = r.summary { app.lastEdit = s }
                app.notice = nil
            } catch {
                app.notice = error.localizedDescription
                overlay.removeAll { $0.job == job.id }
                say(error.localizedDescription)
            }
            ackedJob = max(ackedJob, job.id)
            sending = false
            if stroke == nil, jobs.isEmpty { lastStrokeCalls = strokeCalls }
            // A render after this answer shows the pixels; then the overlay goes.
            app.requestRender()
            pump()
        }
    }

    /// Drops overlay pixels a render started after `job` was answered includes.
    func reconcile(upTo job: Int) {
        if overlay.contains(where: { $0.job <= job }) { overlay.removeAll { $0.job <= job } }
    }

    /// Overlay pixels for what the stage shows now: color hex (nil: erased) by pixel.
    var visibleOverlay: [(PixelPoint, String?)] {
        var out: [PixelPoint: String?] = [:]
        var order: [PixelPoint] = []
        for stamp in overlay where stamp.frame == frame || frameCount <= 1 {
            for p in stamp.pixels {
                if out[p] == nil { order.append(p) }
                out[p] = .some(stamp.hex)
            }
        }
        return order.map { ($0, out[$0]!) }
    }

    // MARK: Fill and pick

    @discardableResult
    func fill(_ px: PixelPoint) async -> EditResult? {
        if let blocked = paintBlocked {
            say(blocked)
            return nil
        }
        guard allowed(color) else {
            say(whyNot(color))
            return nil
        }
        stopPlaying()
        return await edit("object.fill", ["x": JSONValue(px.x), "y": JSONValue(px.y), "color": .string(color), "layer": JSONValue(layer), "frame": JSONValue(frame)])
    }

    /// The Picker: the color of the pixel on the selected layer.
    @discardableResult
    func pick(_ px: PixelPoint) async -> String? {
        guard let app, let doc, let connection = app.connection else { return nil }
        pendingEdits += 1
        defer { pendingEdits -= 1 }
        do {
            let r: PickResult = try await connection.call("object.pick", [
                "name": .string(doc.ref.name), "x": JSONValue(px.x), "y": JSONValue(px.y), "layer": JSONValue(layer), "frame": JSONValue(frame),
            ])
            guard let c = r.color else {
                say("x \(px.x), y \(px.y) is empty")
                app.lastEdit = "x \(px.x), y \(px.y) is empty on \(label(layer))"
                return nil
            }
            color = c
            app.lastEdit = "Picked \(c)"
            say("Picked \(c)")
            return c
        } catch {
            app.notice = error.localizedDescription
            return nil
        }
    }

    // MARK: Layers

    func selectLayer(_ i: Int, announce: Bool = true) {
        guard !layers.isEmpty else { return }
        let next = min(max(i, 0), layers.count - 1)
        if next != layer {
            stopPlaying()
            layer = next
            frame = frameCount > 1 ? min(frame, frameCount - 1) : 0
            app?.requestRender()
            requestExtras()
        }
        if announce { say("\(label(layer)) selected") }
    }

    func toggleVisible(_ i: Int) {
        if hidden.contains(i) { hidden.remove(i) } else { hidden.insert(i) }
        app?.requestRender()
        requestExtras()
        say("\(label(i)) \(hidden.contains(i) ? "hidden" : "shown") in the editor")
    }

    @discardableResult func addLayer() async -> EditResult? { await layerEdit("object.layer.add", [:]) }
    @discardableResult func duplicateLayer() async -> EditResult? { await layerEdit("object.layer.duplicate", ["layer": JSONValue(layer)]) }
    @discardableResult func deleteLayer() async -> EditResult? {
        guard layers.count > 1 else {
            say("\(label(layer)) is the only layer, so it stays")
            app?.notice = "\(label(layer)) is the only layer, so it stays."
            return nil
        }
        return await layerEdit("object.layer.delete", ["layer": JSONValue(layer)])
    }
    @discardableResult func moveLayer(by: Int) async -> EditResult? {
        let target = layer + by
        guard target >= 0, target < layers.count else {
            say(by > 0 ? "\(label(layer)) is already the top layer" : "\(label(layer)) is already the bottom layer")
            return nil
        }
        return await layerEdit("object.layer.move", ["layer": JSONValue(layer), "by": JSONValue(by)])
    }

    private func layerEdit(_ method: String, _ params: [String: JSONValue]) async -> EditResult? {
        guard editable else { return nil }
        stopPlaying()
        let r = await edit(method, params)
        if let l = r?.layer {
            // Visibility follows the layers it was set on only while nothing moves.
            hidden = []
            layer = l
            frame = 0
        }
        return r
    }

    // MARK: Frames

    func selectFrame(_ i: Int, announce: Bool = true) {
        let n = frameCount
        guard n > 1 else {
            if announce { say("This layer has no frames") }
            return
        }
        frame = ((i % n) + n) % n
        app?.requestRender()
        if announce { say("Frame \(frameName(frame)), \(frame + 1) of \(n)") }
    }

    @discardableResult func addFrame() async -> EditResult? { await frameEdit("object.frame.add", [:]) }
    @discardableResult func duplicateFrame() async -> EditResult? { await frameEdit("object.frame.duplicate", [:]) }
    @discardableResult func deleteFrame() async -> EditResult? {
        guard frameCount > 1 else {
            say("A loop keeps at least one frame")
            app?.notice = "A loop keeps at least one frame."
            return nil
        }
        return await frameEdit("object.frame.delete", [:])
    }
    @discardableResult func moveFrame(by: Int) async -> EditResult? {
        let target = frame + by
        guard target >= 0, target < frameCount else {
            say(by > 0 ? "\(frameName(frame)) is already last" : "\(frameName(frame)) is already first")
            return nil
        }
        return await frameEdit("object.frame.move", ["by": JSONValue(by)])
    }

    private func frameEdit(_ method: String, _ params: [String: JSONValue]) async -> EditResult? {
        guard editable, currentLayer?.isLoop == true else { return nil }
        stopPlaying()
        var p = params
        p["layer"] = JSONValue(layer)
        p["frame"] = JSONValue(frame)
        let r = await edit(method, p)
        if let f = r?.frame { frame = f }
        return r
    }

    /// Animate this layer (a still map becomes a loop), or play a loop as another.
    @discardableResult
    func animate(_ loop: FrameLoop = .ff) async -> EditResult? {
        guard editable else { return nil }
        stopPlaying()
        let wasLoop = currentLayer?.isLoop == true
        let r = await edit("object.animate", ["layer": JSONValue(layer), "loop": .string(loop.rawValue)])
        if r != nil, !wasLoop { frame = 0 }
        return r
    }

    /// Stop animating: the loop becomes a still map keeping the shown frame.
    @discardableResult
    func makeStill() async -> EditResult? {
        guard editable, currentLayer?.isLoop == true else { return nil }
        stopPlaying()
        let r = await edit("object.still", ["layer": JSONValue(layer), "frame": JSONValue(frame)])
        if r != nil { frame = 0 }
        return r
    }

    func toggleOnion() {
        onion.toggle()
        requestExtras()
        say("Onion skin \(onion ? "on" : "off")")
    }

    /// The loop note under the strip (frames-panel.ts).
    var loopNote: String? {
        guard let l = currentLayer, let loop = l.loop else { return nil }
        guard let site = FrameLoop(rawValue: loop) else { return "The site's CSS doesn't play \(loop). Choose ff, wf or hf." }
        if l.frames > site.frames { return "The site plays \(site.frames) frames of \(loop); this loop has \(l.frames), so the last \(l.frames - site.frames) never show." }
        if l.frames < site.frames { return "The site plays \(site.frames) frames of \(loop); this loop has \(l.frames), so it blinks off for the rest." }
        return "Frame 0 is the rest frame, what reduced motion shows."
    }

    // MARK: Playback

    func togglePlay() { if playing { stopPlaying(announce: true) } else { play() } }

    /// Plays the selected loop at its site timing; never by itself.
    func play() {
        guard let l = currentLayer, l.frames > 1 else { return }
        let step = FrameLoop.step(loop: l.loop, count: l.frames)
        playing = true
        playTask?.cancel()
        playTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .milliseconds(step))
                guard let self, self.playing, !Task.isCancelled else { return }
                self.selectFrame(self.frame + 1, announce: false)
            }
        }
        say("Playing \(l.loop ?? "the loop"), \(step) ms a frame")
    }

    func stopPlaying(announce: Bool = false) {
        playTask?.cancel()
        playTask = nil
        guard playing else { return }
        playing = false
        if announce { say("Paused on frame \(frameName(frame))") }
    }

    // MARK: Object

    @discardableResult
    func resize(width: Int, height: Int) async -> EditResult? {
        guard editable, (1...64).contains(width), (1...64).contains(height) else {
            say("Use whole numbers from 1 to 64")
            syncSizeDrafts()
            return nil
        }
        let r = await edit("object.resize", ["width": JSONValue(width), "height": JSONValue(height)])
        if r == nil { syncSizeDrafts() }
        return r
    }

    /// A sprite's footprint, tiles wide and deep (nil for a block, whose size is its footprint).
    var footprint: (width: Int, depth: Int)? {
        guard let f = info?.footprint, f.count == 2 else { return nil }
        return (f[0], f[1])
    }

    static let footprintRange = 1...16

    /// The Footprint steppers (`object.footprint`): how many tiles the sprite covers,
    /// so it sorts against its neighbours by the ground under it
    /// (docs/intents/2026-10-depth-order/spec.md, R8).
    @discardableResult
    func setFootprint(width: Int, depth: Int) async -> EditResult? {
        guard let current = footprint else { return nil }
        guard Self.footprintRange.contains(width), Self.footprintRange.contains(depth) else {
            say("Use whole numbers from 1 to 16")
            return nil
        }
        guard current.width != width || current.depth != depth else { return nil }
        return await edit("object.footprint", ["to": [JSONValue(width), JSONValue(depth)]])
    }

    // MARK: Renders

    /// The render params for a frame with the visible layers.
    func renderParams(frame f: Int, layers only: [Int]? = nil) -> [String: JSONValue] {
        var p: [String: JSONValue] = ["frame": JSONValue(f)]
        if let info, !info.isBlock { p["layers"] = .array((only ?? visibleLayers).map { JSONValue($0) }) }
        return p
    }

    func cacheKey(frame f: Int, layers only: [Int]? = nil) -> String {
        guard let info, !info.isBlock else { return "\(f)" }
        return "\(f)|\((only ?? visibleLayers).map(String.init).joined(separator: ","))"
    }

    func cached(frame f: Int) -> StageImage? { frameImages[cacheKey(frame: f)] }

    func store(_ image: StageImage, key: String, generation: Int) {
        guard generation == cacheGeneration else { return }
        frameImages[key] = image
    }

    /// After a render: the onion skin and the frame strip's thumbnails.
    func requestExtras() {
        guard let app, let doc, let connection = app.connection else { return }
        if loadingExtras {
            needsExtras = true
            return
        }
        loadingExtras = true
        Task {
            repeat {
                needsExtras = false
                // Onion skin: the layer's previous frame alone.
                if onion, frame > 0, frameCount > 1 {
                    let only = isBlock ? nil : [layer]
                    onionImage = await render(doc: doc, connection: connection, frame: frame - 1, layers: only)
                } else {
                    onionImage = nil
                }
                // The strip: every frame of a loop, with the visible layers.
                if currentLayer?.isLoop == true {
                    for f in 0..<frameCount where cached(frame: f) == nil {
                        guard app.document === doc, !needsExtras else { break }
                        let generation = cacheGeneration
                        let key = cacheKey(frame: f)
                        if let image = await render(doc: doc, connection: connection, frame: f, layers: nil) {
                            store(image, key: key, generation: generation)
                        }
                    }
                }
            } while needsExtras && app.document === doc
            loadingExtras = false
        }
    }

    private func render(doc: DocumentModel, connection: ServerConnection, frame f: Int, layers only: [Int]?) async -> StageImage? {
        var params = renderParams(frame: f, layers: only)
        params["name"] = .string(doc.ref.name)
        params["scale"] = 1
        guard let r: RenderResult = try? await connection.call("object.render", params),
              let image = PixelImage(png: r.png, renderScale: r.scale) else { return nil }
        return StageImage(image: image, viewBox: CGRect(x: r.viewBox[0], y: r.viewBox[1], width: r.viewBox[2], height: r.viewBox[3]),
                          scale: r.scale, items: r.items, anchor: r.anchor.map { CGPoint(x: $0[0], y: $0[1]) }, time: nil)
    }

    // MARK: Edits

    /// Runs one object edit on the open doc.
    @discardableResult
    func edit(_ method: String, _ params: [String: JSONValue]) async -> EditResult? {
        guard let app, let doc, let connection = app.connection else { return nil }
        var p = params
        p["name"] = .string(doc.ref.name)
        pendingEdits += 1
        defer { pendingEdits -= 1 }
        do {
            let r: EditResult = try await connection.call(method, p)
            if let s = r.summary { say(s) }
            app.lastEdit = r.summary
            app.notice = nil
            return r
        } catch let e as RPCError {
            app.notice = e.message
            say(e.message)
        } catch {
            app.notice = error.localizedDescription
        }
        return nil
    }

    // MARK: Keys

    /// Stage keys in object mode. Returns false for keys it doesn't use.
    func handleKey(_ k: KeyInput) -> Bool {
        guard let app else { return false }
        if k.command || k.control { return false }
        switch k.key {
        case .escape:
            if stroke != nil { endStroke() } else if playing { stopPlaying(announce: true) }
            return true
        case .left, .right, .up, .down:
            let step = k.shift ? 4 : 1
            let (dx, dy): (Int, Int) = switch k.key {
            case .left: (-step, 0)
            case .right: (step, 0)
            case .up: (0, -step)
            default: (0, step)
            }
            moveCursor(dx: dx, dy: dy)
            return true
        case .returnKey:
            press(cursor)
            endStroke()
            return true
        case .delete:
            let t = tool
            tool = .eraser
            beginStroke(at: cursor)
            endStroke()
            tool = t
            return true
        case .tab:
            return false
        case .pageUp, .pageDown:
            selectFrame(frame + (k.key == .pageUp ? -1 : 1))
            return true
        case .space:
            return true
        case .char(let c):
            if k.option { return false }
            if let t = PaintTool.allCases.first(where: { $0.key == c }) {
                setTool(t)
                return true
            }
            if c == StageTool.pan.key { setHand(!handHeld); return true }
            if let n = c.wholeNumberValue {
                if n == 0 { app.zoom = .fit; return true }
                if n <= 4 { app.setZoomLevel(index: n - 1); return true }
                return false
            }
            if c == "]" { selectLayer(layer + 1); return true }
            if c == "[" { selectLayer(layer - 1); return true }
            if c == "," { selectFrame(frame - 1); return true }
            if c == "." { selectFrame(frame + 1); return true }
            return false
        }
    }

    func moveCursor(dx: Int, dy: Int) {
        guard let size = info?.size, size.count == 2 else { return }
        cursor = PixelPoint(min(max(cursor.x + dx, 0), size[0] - 1), min(max(cursor.y + dy, 0), size[1] - 1))
        let under = renderColors?.hex(cursor).flatMap { colorName(hex: $0) } ?? "empty"
        say("x \(cursor.x), y \(cursor.y): \(under)")
    }

    // MARK: VoiceOver

    func say(_ text: String) { app?.editor.announce(text) }
}
