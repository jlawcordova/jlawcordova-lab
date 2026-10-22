import SwiftUI

/// The box the open scene would publish at, drawn over the stage as a
/// one-point outline in the accent (never part of the art), so the owner sees
/// what the page gets while the stage itself has no edge
/// (docs/intents/2026-10-scene-canvas/spec.md, R10).
struct PublishedFrameOutline: View {
    var geometry: StageGeometry
    var doc: DocumentModel
    @Environment(\.displayScale) private var displayScale

    var body: some View {
        let rect = geometry.viewRect(fromScene: doc.publishBox)
        let w = Int(doc.publishSize.width), h = Int(doc.publishSize.height)
        Rectangle()
            .strokeBorder(Theme.accent.opacity(0.9), lineWidth: 1 / max(displayScale, 1))
            .frame(width: rect.width, height: rect.height)
            .offset(x: rect.minX, y: rect.minY)
            .frame(width: geometry.contentSize.width, height: geometry.contentSize.height, alignment: .topLeading)
            .allowsHitTesting(false)
            .accessibilityElement()
            .accessibilityLabel("Published frame, \(w) by \(h) pixels, \(doc.isFixedFrame ? "fixed" : "fits the content")")
    }
}

/// The scene's published size in the inspector: "Publishes at 272×200 px ·
/// Fits the content" or "Fixed", with Fit to content and a field for the box
/// (R8, R10).
struct PublishedSizeSection: View {
    var doc: DocumentModel
    @Environment(AppModel.self) private var model

    var body: some View {
        let fixed = doc.isFixedFrame
        VStack(alignment: .leading, spacing: 8) {
            SectionLabel(title: "Published size")
            Text("Publishes at \(Int(doc.publishSize.width))×\(Int(doc.publishSize.height)) px · \(fixed ? "Fixed" : "Fits the content")")
                .font(.callout).foregroundStyle(Theme.ink)
                .accessibilityLabel("Publishes at \(Int(doc.publishSize.width)) by \(Int(doc.publishSize.height)) pixels, \(fixed ? "fixed" : "fits the content")")
            HStack(spacing: 8) {
                TextField("x, y, w, h", text: Binding(get: { doc.frameDraft }, set: { doc.frameDraft = $0 }))
                    .textFieldStyle(.roundedBorder)
                    .font(Theme.monoFont(11))
                    .onSubmit { Task { await model.applyFrameDraft() } }
                    .accessibilityLabel("Published box: x, y, width, height")
                    .help("Fix the box the scene publishes at: x, y, width, height in scene pixels (Return)")
                Button("Fix") { Task { await model.applyFrameDraft() } }
                    .controlSize(.regular)
                    .help("Publish at exactly this box")
                    .accessibilityLabel("Fix the published size to this box")
            }
            if fixed {
                Button("Fit to content") { Task { await model.fitFrame() } }
                    .controlSize(.regular)
                    .help("Remove the fixed size: the scene publishes at what it paints")
                    .accessibilityLabel("Fit the published size to the content")
            }
            if doc.frameInfo?.margin ?? 0 > 0 {
                Text("Margin \(doc.frameInfo?.margin ?? 0) px").font(.caption).foregroundStyle(Theme.inkMuted)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .contain)
    }
}
