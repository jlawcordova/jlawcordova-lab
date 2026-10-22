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
