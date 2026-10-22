import AppKit
import SwiftUI

/// The zoom readout over the stage's bottom-right corner ("2×") for about a second after the
/// zoom changes (docs/intents/2026-10-floating-library/spec.md, R3, D2). It fades, unless Reduce
/// Motion is on, when it just comes and goes.
struct ZoomReadout: View {
    @Environment(AppModel.self) private var model
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        ZStack {
            if let readout = model.zoomReadout {
                Text(readout.text)
                    .font(Theme.monoFont(12))
                    .foregroundStyle(Theme.ink)
                    .padding(.horizontal, 10)
                    .frame(height: 26)
                    .background(Theme.halo, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).strokeBorder(Theme.border, lineWidth: 1))
                    .transition(reduceMotion ? .identity : .opacity)
                    .accessibilityLabel("Zoom \(readout.text)")
            }
        }
        .animation(reduceMotion ? nil : .easeOut(duration: 0.25), value: model.zoomReadout)
        .allowsHitTesting(false)
    }
}

/// Quiet scroll bars (R5, D3): the scroll view this sits inside gets small, autohiding scrollers and
/// otherwise keeps the system's style, so AppKit follows "Show scroll bars": overlay scrollers
/// that fade and take no room, or legacy ones whose room is reserved from the start. It never
/// sets `scrollerStyle`.
struct QuietScrollers: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView { Probe() }
    func updateNSView(_ view: NSView, context: Context) { Self.configure(view.enclosingScrollView) }

    static func configure(_ scroll: NSScrollView?) {
        guard let scroll else { return }
        scroll.autohidesScrollers = true
        scroll.horizontalScroller?.controlSize = .small
        scroll.verticalScroller?.controlSize = .small
    }

    /// Finds its scroll view once it is in the window (the hosting view adds it a moment later).
    private final class Probe: NSView {
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            DispatchQueue.main.async { [weak self] in QuietScrollers.configure(self?.enclosingScrollView) }
        }
    }
}
