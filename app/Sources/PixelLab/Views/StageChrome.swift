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

/// Over an object, at the stage's top left: **‹ Back · hero-island › house**. Back, or the scene's name,
/// returns to the scene the owner came from; the object's name is where they are. It has the capsule
/// of the layers breadcrumb, so the two read as one way in and out (docs/intents/2026-10-floating-library/spec.md, R11, D7).
struct BackPill: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        let scene = model.backTarget
        HStack(spacing: 4) {
            Button { Task { await model.backToScene() } } label: {
                HStack(spacing: 3) {
                    Image(systemName: "chevron.left").font(.system(size: 10, weight: .semibold))
                    Text("Back").font(Theme.nameFont(12, weight: .medium))
                }
                .foregroundStyle(Theme.ink)
                .padding(.horizontal, 6)
                .frame(height: 24)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .disabled(scene == nil)
            .help("Back to the scene (⌘[)")
            .accessibilityLabel(scene.map { "Back to \($0)" } ?? "Back")
            if let scene {
                Rectangle().fill(Theme.border).frame(width: 1, height: 16).accessibilityHidden(true)
                Button { Task { await model.backToScene() } } label: {
                    Text(scene)
                        .font(Theme.nameFont(12))
                        .foregroundStyle(Theme.inkMuted)
                        .lineLimit(1)
                        .padding(.horizontal, 6)
                        .frame(height: 24)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help("Go to \(scene)")
                .accessibilityLabel("Scene \(scene)")
                Text("›").foregroundStyle(Theme.inkMuted).accessibilityHidden(true)
            }
            Text(model.document?.ref.name ?? "")
                .font(Theme.nameFont(12, weight: .semibold))
                .foregroundStyle(Theme.ink)
                .lineLimit(1)
                .padding(.trailing, 8)
                .accessibilityLabel("Editing \(model.document?.ref.name ?? "")")
        }
        .padding(.horizontal, 3)
        .frame(height: 30)
        .background(Theme.halo, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).strokeBorder(Theme.border, lineWidth: 1))
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Where you are")
    }
}
