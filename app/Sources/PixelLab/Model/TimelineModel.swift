import AppKit
import Observation

/// The timeline under the stage: the open scene's motion (its loop length and
/// tracks) and ambient loops, the playhead, play and pause. The stage renders
/// at `renderTime` (the server places motion and ambient loops at `time`);
/// frames are cached per time step until the doc changes, so a second loop
/// (and scrubbing back) never waits on the server. Also an action's Play
/// (docs/intents/2026-10-rust-engine/spec.md, R23, D10).
@MainActor @Observable
final class TimelineModel {
    @ObservationIgnored weak var app: AppModel?

    /// The timeline's length in ms: the longest of the scene's motion and its
    /// ambient loops, at most 60 s (nil: the scene has neither).
    var duration: Double?
    /// The scene's motion length, and its longest ambient loop (uncapped).
    var motionDuration: Double?
    var ambientDuration: Double?
    /// The ambient loops and actions of the scene's items, by item path, and the
    /// object each item places; and the layers of the objects the scene uses.
    var itemAnimation: [String: ElementAnimation] = [:]
    var itemObject: [String: String] = [:]
    @ObservationIgnored var objectAnimation: [String: [Int: ElementAnimation]] = [:]

    /// An action is on the stage (playing, or held by the script hook): the
    /// stage keeps its frame until it ends.
    var actionActive = false
    var playingAction: String?
    /// The frame of the action now on the stage; the stage draws it in place of the rest frame.
    var actionImage: StageImage?
    /// What the last Play did ("hop: 12 frames", "hop: shown at rest").
    var lastPlay = ""
    var lastAnnouncement = ""
    /// Tests and the script hook set this; nil follows the system.
    @ObservationIgnored var reduceMotionOverride: Bool?
    @ObservationIgnored private var actionToken = 0
    /// The playhead, in ms.
    var time: Double = 0
    var playing = false
    var tracks: [MotionTrack] = []
    var showTracks = false

    /// Render times are whole steps, so the frame cache hits.
    static let step = 100.0

    @ObservationIgnored private var playTask: Task<Void, Never>?
    @ObservationIgnored private var cache: [Int: CachedFrame] = [:]
    @ObservationIgnored private(set) var generation = 0

    struct CachedFrame {
        var png: Data
        var scale: Int
        var viewBox: CGRect
        var items: [RenderItem]
    }

    var hasMotion: Bool { (duration ?? 0) > 0 }
    var reduceMotion: Bool { reduceMotionOverride ?? NSWorkspace.shared.accessibilityDisplayShouldReduceMotion }
    /// The scene has ambient loops (on items, or on layers of the objects it uses).
    var hasAmbient: Bool { (ambientDuration ?? 0) > 0 }

    /// The time the stage renders now: the playhead, snapped to a step.
    var renderTime: Int {
        guard hasMotion else { return 0 }
        return Int((time / Self.step).rounded(.down) * Self.step)
    }

    /// A new doc: stop, back to 0, forget its frames.
    func reset() {
        pause()
        time = 0
        duration = nil
        motionDuration = nil
        ambientDuration = nil
        itemAnimation = [:]
        itemObject = [:]
        objectAnimation = [:]
        tracks = []
        endAction(render: false)
        invalidate()
    }

    /// The doc changed: cached frames are stale.
    func invalidate() {
        cache = [:]
        generation += 1
    }

    /// Reads the scene's source: its motion, and the ambient loops and actions
    /// of its items and of the layers of the objects it places (`objects`, by name).
    func load(_ r: SceneShowMotion, objects: [String: [Int: ElementAnimation]] = [:]) {
        let m = r.doc?.motion
        motionDuration = m?.duration
        tracks = (m?.tracks ?? []).map { MotionTrack(item: $0.item, keys: ($0.keys ?? []).compactMap(\.t)) }
        objectAnimation.merge(objects) { _, new in new }
        var items: [String: ElementAnimation] = [:]
        var placed: [String: String] = [:]
        var ambient: [Double] = []
        Animation.walk(r.doc?.items) { path, item in
            let object = item["object"]?.stringValue
            if let object { placed[path] = object }
            let own = Animation.parse(item, source: "item")
            if !own.isEmpty { items[path] = own }
            if let a = own.ambient { ambient.append(a) }
            if let object { ambient += (objectAnimation[object] ?? [:]).values.compactMap(\.ambient) }
        }
        itemAnimation = items
        itemObject = placed
        ambientDuration = ambient.max()
        duration = Animation.timelineLength(motion: motionDuration, ambient: ambient)
        if !hasMotion { pause(); time = 0 } else if let duration, time >= duration { time = 0 }
    }

    /// Objects changed on disk: their layers' loops and actions are read again.
    func forgetObjects(_ names: [String]?) {
        if let names { for n in names { objectAnimation[n] = nil } } else { objectAnimation = [:] }
    }

    // MARK: Actions

    /// The ACTIONS rows of a scene item: its own actions and those of the map
    /// layers of its object.
    func actionRows(forItem path: String) -> [ActionRow] {
        let layers = itemObject[path].flatMap { objectAnimation[$0] } ?? [:]
        return Animation.rows(own: itemAnimation[path], layers: layers.keys.sorted().compactMap { layers[$0] })
    }

    /// Shows an action once on the stage: its frames are rendered with `play`
    /// (every 40 ms of it, at the playhead's time), shown in turn, and the
    /// stage goes back to the rest frame. With `hold`, it shows that one frame
    /// (ms into the action) and stays until `endAction()`. Under Reduce Motion
    /// it shows the rest frame and says so.
    func playAction(_ row: ActionRow, hold: Int? = nil) async {
        guard let app, let doc = app.document else { return }
        pause()
        actionToken += 1
        let token = actionToken
        if reduceMotion {
            endAction(render: false)
            lastPlay = "\(row.name): shown at rest"
            lastAnnouncement = lastPlay
            app.editor.announce(lastPlay)
            app.requestRender()
            return
        }
        guard let connection = app.connection else { return }
        actionActive = true
        playingAction = row.name
        let times = hold.map { [$0] } ?? Animation.sampleTimes(duration: row.duration)
        var frames: [StageImage] = []
        do {
            for t in times {
                let image = try await renderFrame(row.name, at: t, doc: doc, connection: connection)
                guard token == actionToken, app.document === doc else { return }
                frames.append(image)
            }
        } catch {
            guard token == actionToken else { return }
            app.notice = "Couldn't play \(row.name): \(error.localizedDescription)"
            endAction(render: true)
            return
        }
        guard token == actionToken, app.document === doc else { return }
        if hold != nil, let frame = frames.first {
            actionImage = frame
            lastPlay = "\(row.name): held at \(times[0]) ms"
            return
        }
        for (i, frame) in frames.enumerated() {
            guard token == actionToken, app.document === doc else { return }
            actionImage = frame
            let next = i + 1 < times.count ? Double(times[i + 1]) : row.duration
            try? await Task.sleep(for: .milliseconds(max(next - Double(times[i]), 1)))
        }
        guard token == actionToken else { return }
        lastPlay = "\(row.name): \(frames.count) frame\(frames.count == 1 ? "" : "s")"
        endAction(render: true)
    }

    /// The action is over (or Escape-d, or the doc changed): the stage draws
    /// the playhead's frame again.
    func endAction(render: Bool) {
        actionToken += 1
        let was = actionActive
        actionActive = false
        playingAction = nil
        actionImage = nil
        if render, was { app?.requestRender() }
    }

    private func renderFrame(_ name: String, at ms: Int, doc: DocumentModel, connection: ServerConnection) async throws -> StageImage {
        guard let app else { throw CancellationError() }
        let isScene = doc.ref.kind == .scene
        var params: [String: JSONValue] = ["name": .string(doc.ref.name), "scale": 1, "play": .string("\(name)@\(ms)")]
        if isScene {
            params["time"] = JSONValue(renderTime)
            params["theme"] = app.stageTheme
            params.merge(app.layers.renderParams) { _, b in b }
        } else {
            // No `layers`: the program places loops and actions on the whole sprite.
            params["frame"] = JSONValue(app.objectEditor.frame)
            params["time"] = 0
        }
        let r: RenderResult = try await connection.call(isScene ? "scene.render" : "object.render", params)
        guard let image = PixelImage(png: r.png, renderScale: r.scale) else { throw CancellationError() }
        return StageImage(
            image: image,
            viewBox: CGRect(x: r.viewBox[0], y: r.viewBox[1], width: r.viewBox[2], height: r.viewBox[3]),
            scale: r.scale,
            items: r.items,
            anchor: r.anchor.map { CGPoint(x: $0[0], y: $0[1]) },
            time: r.time
        )
    }

    // MARK: Frames

    func cached(_ t: Int) -> CachedFrame? { cache[t] }

    func store(_ t: Int, _ frame: CachedFrame, generation g: Int) {
        guard g == generation, hasMotion else { return }
        if cache.count > 600 { cache = [:] }
        cache[t] = frame
    }

    // MARK: Playback

    func toggle() { playing ? pause() : play() }

    func play() {
        guard hasMotion, !playing, !reduceMotion, let duration else { return }
        playing = true
        playTask?.cancel()
        let started = Date()
        let from = time
        playTask = Task { [weak self] in
            while !Task.isCancelled {
                guard let self, self.playing else { return }
                let elapsed = Date().timeIntervalSince(started) * 1000
                self.time = (from + elapsed).truncatingRemainder(dividingBy: duration)
                self.app?.requestRender()
                try? await Task.sleep(for: .milliseconds(33))
            }
        }
    }

    func pause() {
        playing = false
        playTask?.cancel()
        playTask = nil
    }

    /// Moves the playhead (scrubbing): the stage renders the latest time asked
    /// for and drops the ones in between.
    func scrub(to t: Double) {
        guard let duration else { return }
        time = min(max(t, 0), duration - 1)
        app?.requestRender()
    }

    static func label(_ ms: Double) -> String {
        String(format: "%.1f s", ms / 1000)
    }
}
