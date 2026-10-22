import CoreGraphics

/// The scene pixels a scene's render covers while it is edited: the published
/// box (and where the cursor has been) with room around it, so there is always
/// space to scroll to and place in (docs/intents/2026-10-scene-canvas/spec.md,
/// R9). It grows toward an edge and never shrinks, so nothing moves under the
/// pointer during a drag.
enum StageWindow {
    /// The most room added on a side, in scene pixels (a bigger window costs a
    /// bigger render: 3500×3300 took 0.15 s on the town example, 0.06 s for its box).
    static let maxPad: CGFloat = 1024
    static let minPad: CGFloat = 128
    /// Windows are whole multiples of this, so they don't change on every small move.
    static let grain: CGFloat = 32

    /// The room to leave on a side: a viewport's worth at this zoom.
    static func pad(viewport: CGSize, zoom: Int) -> CGFloat {
        let z = CGFloat(max(zoom, 1))
        let want = max(viewport.width, viewport.height) / z
        return min(max((want / grain).rounded(.up) * grain, minPad), maxPad)
    }

    /// The window to render: `current` while `needed` is well inside it, else
    /// `needed` with `pad` on every side, joined with `current` (it only grows).
    static func grow(current: CGRect?, needed: CGRect, pad: CGFloat) -> CGRect {
        if let current, current.contains(needed.insetBy(dx: -pad / 2, dy: -pad / 2)) { return current }
        let wanted = snap(needed.insetBy(dx: -pad, dy: -pad))
        guard let current else { return wanted }
        return current.union(wanted)
    }

    private static func snap(_ r: CGRect) -> CGRect {
        let x0 = (r.minX / grain).rounded(.down) * grain, y0 = (r.minY / grain).rounded(.down) * grain
        let x1 = (r.maxX / grain).rounded(.up) * grain, y1 = (r.maxY / grain).rounded(.up) * grain
        return CGRect(x: x0, y: y0, width: x1 - x0, height: y1 - y0)
    }
}
