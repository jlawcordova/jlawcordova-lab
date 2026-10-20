import AppKit
import Observation

/// A key of the pane picked by the owner: the track's item and the key's time.
struct SelectedKey: Hashable, Sendable {
    var item: String
    var t: Int
}

/// A key being dragged along its row (docs/intents/2026-10-animation-pane/spec.md, R5).
struct KeyDrag: Equatable, Sendable {
    var item: String
    var from: Int
    /// Where it would land, snapped.
    var to: Int
}

/// A press that started on a track's row: which key it hit, if any.
struct RowPress: Equatable, Sendable {
    var item: String
    var key: Int?
}

/// The timeline under the stage: the open scene's motion (its loop length and
/// tracks), the playhead, play and pause, and the animation pane's state: the
/// selected key, a key drag, the pane's focus, and the key, track and loop
/// edits (docs/intents/2026-10-animation-pane/spec.md, D2). The stage renders
/// at `renderTime`; frames are cached per time step until the doc changes, so
/// a second loop (and scrubbing back) never waits on the server.
@MainActor @Observable
final class TimelineModel {
    @ObservationIgnored weak var app: AppModel?

    /// The loop length in ms (nil: the scene has no motion).
    var duration: Double?
    /// The playhead, in ms.
    var time: Double = 0
    var playing = false
    var tracks: [MotionTrack] = []
    /// The animation pane is open (remembered between launches).
    var showTracks: Bool = UserDefaults.standard.object(forKey: SettingsKey.showTracks) as? Bool ?? false {
        didSet { UserDefaults.standard.set(showTracks, forKey: SettingsKey.showTracks) }
    }
    /// The key picked in the pane (R2).
    var selectedKey: SelectedKey?
    var keyDrag: KeyDrag?
    var rowPress: RowPress?
    /// The pane has keyboard focus (R10).
    var paneFocused = false
    /// The off-grid warnings of the last key edit, for the KEY section (R7).
    var lastWarnings: [String] = []
    /// Bumped when the loop field should go back to the loop's length (a refused length, R9).
    var loopFieldGeneration = 0

    /// Render times are whole steps, so the frame cache hits.
    nonisolated static let step = 100.0
    /// The loop a scene gets when its first item is animated from the app (R8).
    static let defaultLoop = 4000

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
    var reduceMotion: Bool { NSWorkspace.shared.accessibilityDisplayShouldReduceMotion }

    /// The time the stage renders now: the playhead, snapped to a step.
    var renderTime: Int {
        guard hasMotion else { return 0 }
        return Int((time / Self.step).rounded(.down) * Self.step)
    }

    /// A new doc: stop, back to 0, forget its frames and the pane's selection.
    func reset() {
        pause()
        time = 0
        duration = nil
        tracks = []
        selectedKey = nil
        keyDrag = nil
        rowPress = nil
        lastWarnings = []
        invalidate()
    }

    /// The doc changed: cached frames are stale.
    func invalidate() {
        cache = [:]
        generation += 1
    }

    func load(_ r: SceneShowMotion) {
        let m = r.doc?.motion
        duration = m?.duration
        tracks = (m?.tracks ?? []).map { track in
            MotionTrack(item: track.item, keys: (track.keys ?? []).compactMap { k in
                k.t.map { MotionKey(t: $0, move: k.move, show: k.show, pose: k.pose) }
            })
        }
        if !hasMotion { pause(); time = 0 }
        // A selected key that went (deleted, moved, undone, reloaded) is dropped.
        if let sk = selectedKey, track(sk.item)?.key(at: sk.t) == nil { selectedKey = nil }
    }

    // MARK: Tracks and keys

    func track(_ item: String) -> MotionTrack? { tracks.first { $0.item == item } }

    /// The selected item's id, when it has one.
    var selectedItemID: String? { app?.document?.selectedItem?.id }

    /// The selected item's track.
    var selectedTrack: MotionTrack? { selectedItemID.flatMap(track) }

    /// The selected key, as the source holds it.
    var selectedKeyData: MotionKey? { selectedKey.flatMap { sk in track(sk.item)?.key(at: sk.t) } }

    /// The selected key with its inherited values.
    var selectedFilled: FilledKey? { selectedKey.flatMap { sk in track(sk.item)?.filled(at: sk.t) } }

    /// The selected key belongs to the selected item (the KEY section shows then).
    var selectedKeyIsOfSelection: Bool { selectedKey != nil && selectedKey?.item == selectedItemID }

    var canAddKey: Bool { hasMotion && selectedTrack != nil }
    var canDeleteKey: Bool { (selectedKey?.t ?? 0) != 0 && selectedKeyData != nil }
    var canAnimate: Bool { selectedItemID != nil && selectedTrack == nil && app?.document?.ref.kind == .scene }
    var canRemoveTrack: Bool { selectedTrack != nil }

    // MARK: Selection

    /// A row's name clicked: its item becomes the selection.
    func selectTrack(_ item: String) {
        if selectedKey?.item != item { selectedKey = nil }
        app?.editor.select(id: item)
    }

    /// A key clicked (R2): it's selected, its item is, and the playhead goes to it.
    func selectKey(_ item: String, _ t: Int, scrub: Bool = true) {
        guard let key = track(item)?.key(at: t) else { return }
        if selectedItemID != item { app?.editor.select(id: item) }
        if selectedKey?.item != item { lastWarnings = [] }
        selectedKey = SelectedKey(item: item, t: t)
        showTracks = true
        if scrub {
            pause()
            self.scrub(to: Double(t))
        }
        app?.editor.announce("Key at \(Self.label(Double(t))) selected: \(key.spoken)")
    }

    func deselectKey() {
        selectedKey = nil
    }

    /// The stage keyed a move past 0 (R6): that key is the selected one.
    func keyedByStage(item: String, t: Int) async {
        await app?.loadMotion()
        selectKey(item, t, scrub: false)
    }

    /// ⇧← / ⇧→ (R10): the selected track's previous or next key from the playhead (or the selected key).
    func stepKey(_ direction: Int) {
        guard let track = selectedTrack ?? tracks.first else { return }
        let from = selectedKey?.item == track.item ? selectedKey!.t : renderTime
        let keys = track.keys.map(\.t)
        let target = direction > 0 ? keys.first { $0 > from } : keys.last { $0 < from }
        guard let target else {
            app?.editor.announce(direction > 0 ? "No key after \(Self.label(Double(from)))" : "No key before \(Self.label(Double(from)))")
            return
        }
        selectKey(track.item, target)
    }

    /// ← / → (R10): the playhead by one step.
    func stepTime(_ delta: Double) {
        pause()
        scrub(to: time + delta)
    }

    // MARK: Edits

    /// One key or track command on an item's track, by its path, through the
    /// scene editor (one undo step); keeps the warnings it returns.
    @discardableResult
    private func trackEdit(_ method: String, item: String, _ params: [String: JSONValue]) async -> EditResult? {
        guard let app, let path = app.document?.items.first(where: { $0.id == item })?.path else {
            app?.notice = "No item with the id \(item) in the open scene."
            return nil
        }
        var p = params
        p["item"] = .string(path)
        let r = await app.editor.edit(method, p)
        if let r {
            lastWarnings = r.warnings ?? []
            await app.loadMotion()
        }
        return r
    }

    /// Add Key (R3): a key at the playhead, pinned where the item is; a key already there is selected.
    func addKey() async {
        guard hasMotion, let item = selectedItemID, let track = track(item) else {
            app?.notice = selectedItemID == nil ? "Select an animated item to add a key." : "Animate the item first (Animation in the inspector)."
            return
        }
        let t = renderTime
        if track.key(at: t) != nil {
            selectKey(item, t, scrub: false)
            return
        }
        let r = await trackEdit("scene.key.set", item: item, ["time": JSONValue(t)])
        if r != nil { selectKey(item, t, scrub: false) }
    }

    /// Delete Key (R4): the selected key, not the rest frame.
    func deleteKey() async {
        guard let sk = selectedKey, sk.t != 0 else {
            if selectedKey?.t == 0 { app?.notice = "The key at 0 is the rest frame; remove the track to drop it." }
            return
        }
        let r = await trackEdit("scene.key.delete", item: sk.item, ["time": JSONValue(sk.t)])
        if r != nil { selectedKey = nil }
    }

    /// The selected key moved in time (R5, R7).
    func moveKey(to: Int) async {
        guard let sk = selectedKey, sk.t != 0, to != sk.t else { return }
        let r = await trackEdit("scene.key.move", item: sk.item, ["time": JSONValue(sk.t), "to": JSONValue(to)])
        if r?.changed == true {
            selectedKey = SelectedKey(item: sk.item, t: to)
            scrub(to: Double(to))
        }
    }

    /// The selected key's values (R7): any of move, show, pose set, or cleared.
    func setKey(move: [Int]? = nil, show: Bool? = nil, pose: String? = nil, clear: [String] = []) async {
        guard let sk = selectedKey else { return }
        var p: [String: JSONValue] = ["time": JSONValue(sk.t)]
        if let move { p["move"] = .array(move.map { JSONValue($0) }) }
        if let show { p["show"] = .bool(show) }
        if let pose { p["pose"] = .string(pose) }
        if !clear.isEmpty { p["clear"] = .array(clear.map { .string($0) }) }
        await trackEdit("scene.key.set", item: sk.item, p)
    }

    /// Animate (R8): a track for the selected item (a 4 s loop if the scene has none), and the pane opens.
    func addTrack() async {
        guard let item = selectedItemID else {
            app?.notice = "Give the item an ID to animate it."
            return
        }
        guard track(item) == nil else { return }
        var p: [String: JSONValue] = [:]
        if !hasMotion { p["duration"] = JSONValue(Self.defaultLoop) }
        let r = await trackEdit("scene.track.add", item: item, p)
        if r != nil {
            showTracks = true
            selectKey(item, 0, scrub: false)
        }
    }

    /// Remove Track (R8): the selected item's track and its keys.
    func removeTrack() async {
        guard let item = selectedItemID, track(item) != nil else { return }
        if selectedKey?.item == item { selectedKey = nil }
        await trackEdit("scene.track.remove", item: item, [:])
    }

    /// The loop's length (R9); a refused length puts the field back.
    func setDuration(_ ms: Int) async {
        guard let app, ms != Int(duration ?? 0) else { return }
        let r = await app.editor.edit("scene.motion.set", ["duration": JSONValue(ms)])
        if r == nil { loopFieldGeneration += 1 } else { await app.loadMotion() }
    }

    // MARK: Keys into the pane (R10)

    /// Returns false for keys the pane doesn't use.
    func handleKey(_ k: KeyInput) -> Bool {
        if k.command || k.control { return false }
        switch k.key {
        case .left, .right:
            let dir = k.key == .right ? 1 : -1
            if k.shift { stepKey(dir) } else { stepTime(Double(dir) * Self.step) }
            return true
        case .delete:
            guard canDeleteKey else { return false }
            Task { await deleteKey() }
            return true
        case .escape:
            guard selectedKey != nil else { return false }
            deselectKey()
            return true
        case .space:
            toggle()
            return true
        case .char("k"):
            guard !k.option, !k.shift else { return false }
            Task { await addKey() }
            return true
        default:
            return false
        }
    }

    // MARK: Geometry (pure, for the pane and its tests)

    /// The x of a time on a row `width` wide.
    nonisolated static func x(of ms: Double, width: CGFloat, duration: Double) -> CGFloat {
        width * CGFloat(ms / max(duration, 1))
    }

    /// The time at an x on a row.
    nonisolated static func time(atX x: CGFloat, width: CGFloat, duration: Double) -> Double {
        Double(min(max(x / max(width, 1), 0), 1)) * duration
    }

    /// The index of the key under `x` (the nearest within `hit` points), or nil.
    nonisolated static func keyIndex(atX x: CGFloat, width: CGFloat, keys: [Int], duration: Double, hit: CGFloat = 6) -> Int? {
        var best: (index: Int, distance: CGFloat)?
        for (i, t) in keys.enumerated() {
            let d = abs(Self.x(of: Double(t), width: width, duration: duration) - x)
            if d <= hit, best == nil || d < best!.distance { best = (i, d) }
        }
        return best?.index
    }

    /// A dragged key's time, snapped to the 100 ms steps and kept inside the loop (never 0, the rest frame's).
    nonisolated static func snap(_ ms: Double, duration: Double) -> Int {
        let snapped = Int((ms / step).rounded() * step)
        let last = Int(((duration - 1) / step).rounded(.down) * step)
        return min(max(snapped, Int(step)), max(last, Int(step)))
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

    nonisolated static func label(_ ms: Double) -> String {
        String(format: "%.1f s", ms / 1000)
    }
}
