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
