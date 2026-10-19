import AppKit
import Observation

/// The timeline under the stage: the open scene's motion (its loop length and
/// tracks), the playhead, play and pause. The stage renders at `renderTime`;
/// frames are cached per time step until the doc changes, so a second loop
/// (and scrubbing back) never waits on the server.
@MainActor @Observable
final class TimelineModel {
    @ObservationIgnored weak var app: AppModel?

    /// The loop length in ms (nil: the scene has no motion).
    var duration: Double?
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
    var reduceMotion: Bool { NSWorkspace.shared.accessibilityDisplayShouldReduceMotion }

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
        tracks = []
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
        tracks = (m?.tracks ?? []).map { MotionTrack(item: $0.item, keys: ($0.keys ?? []).compactMap(\.t)) }
        if !hasMotion { pause(); time = 0 }
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
