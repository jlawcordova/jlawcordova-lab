import SwiftUI

/// The right inspector: SELECTED and POSITION (placeholders until slice 4) and
/// CHECKS, the engine's validation problems for the open doc.
struct InspectorView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        // A VStack rather than one ScrollView: in the inspector column a scroll
        // view at the top edge insets its content by the toolbar height twice.
        // Only the checks list scrolls.
        VStack(alignment: .leading, spacing: 18) {
            if let doc = model.document {
                documentCard(doc)
                section("Selected") {
                    Text("Nothing selected")
                        .font(.callout)
                        .foregroundStyle(Theme.inkMuted)
                }
                section("Position") {
                    HStack(spacing: 6) {
                        ForEach(["Col", "Row", "Level"], id: \.self) { field in
                            VStack(alignment: .leading, spacing: 3) {
                                Text(field).font(.caption).foregroundStyle(Theme.inkMuted)
                                Text("—")
                                    .font(Theme.monoFont(12))
                                    .foregroundStyle(Theme.inkMuted)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .padding(.horizontal, 7)
                                    .frame(height: 24)
                                    .background(Theme.card, in: RoundedRectangle(cornerRadius: 5, style: .continuous))
                            }
                        }
                    }
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel("Position: nothing selected")
                }
                section("Checks") {
                    ScrollView {
                        ChecksList(doc: doc).frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .scrollBounceBehavior(.basedOnSize)
                }
            } else {
                Text("No document open").font(.callout).foregroundStyle(Theme.inkMuted)
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Theme.sidebar)
    }

    private func documentCard(_ doc: DocumentModel) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 6) {
                Image(systemName: doc.ref.kind == .scene ? "square.stack.3d.up" : "cube")
                    .foregroundStyle(Theme.accent)
                Text(doc.ref.name)
                    .font(Theme.nameFont(15))
                    .foregroundStyle(Theme.ink)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            if let summary = doc.summary {
                Text(summary).font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(2)
            }
            if let render = doc.render {
                Text("\(Int(render.viewBox.width))×\(Int(render.viewBox.height)) px · \(model.effectiveZoom)×")
                    .font(Theme.monoFont(10.5))
                    .foregroundStyle(Theme.inkMuted)
            }
        }
    }

    private func section(_ title: String, @ViewBuilder content: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            SectionLabel(title: title)
            content()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

struct ChecksList: View {
    var doc: DocumentModel

    var body: some View {
        if let problems = doc.problems {
            if problems.isEmpty {
                HStack(alignment: .firstTextBaseline, spacing: 7) {
                    Image(systemName: "checkmark.circle.fill").foregroundStyle(Theme.accent)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("No problems").font(.callout.weight(.medium)).foregroundStyle(Theme.ink)
                        if let n = doc.checkedCount {
                            Text("\(n) source\(n == 1 ? "" : "s") checked with the engine")
                                .font(.caption).foregroundStyle(Theme.inkMuted)
                        }
                    }
                }
                .accessibilityElement(children: .combine)
            } else {
                VStack(alignment: .leading, spacing: 8) {
                    Text("\(problems.count) problem\(problems.count == 1 ? "" : "s")")
                        .font(.callout.weight(.medium)).foregroundStyle(Theme.ink)
                    ForEach(Array(problems.prefix(50).enumerated()), id: \.offset) { _, p in
                        HStack(alignment: .firstTextBaseline, spacing: 7) {
                            Image(systemName: "exclamationmark.triangle.fill")
                                .foregroundStyle(Theme.gold)
                                .imageScale(.small)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(p.message).font(.caption).foregroundStyle(Theme.ink)
                                    .fixedSize(horizontal: false, vertical: true)
                                Text(p.file).font(Theme.monoFont(10)).foregroundStyle(Theme.inkMuted)
                            }
                        }
                        .accessibilityElement(children: .combine)
                    }
                }
            }
        } else {
            HStack(spacing: 6) {
                ProgressView().controlSize(.mini)
                Text("Checking…").font(.caption).foregroundStyle(Theme.inkMuted)
            }
        }
    }
}

/// The strip under the stage reserved for the timeline (slice 4).
struct TimelineStrip: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        HStack(spacing: 12) {
            SectionLabel(title: "Timeline")
            Button {} label: { Image(systemName: "play.fill") }
                .buttonStyle(.borderless)
                .disabled(true)
                .accessibilityLabel("Play (coming soon)")
            GeometryReader { proxy in
                ZStack(alignment: .leading) {
                    Capsule().fill(Theme.border).frame(height: 3)
                    ForEach(0..<9) { i in
                        Rectangle().fill(Theme.border)
                            .frame(width: 1, height: i % 4 == 0 ? 9 : 5)
                            .offset(x: proxy.size.width * CGFloat(i) / 8 - 0.5)
                    }
                    RoundedRectangle(cornerRadius: 1).fill(Theme.gold).frame(width: 2, height: 14)
                }
                .frame(maxHeight: .infinity)
            }
            .frame(height: 20)
            .accessibilityHidden(true)
            Text(timeLabel).font(Theme.monoFont(11)).foregroundStyle(Theme.inkMuted)
        }
        .padding(.horizontal, 14)
        .frame(height: 40)
        .background(Theme.sidebar)
        .overlay(alignment: .top) { Rectangle().fill(Theme.border).frame(height: 1) }
    }

    private var timeLabel: String {
        if let doc = model.document, let t = doc.render?.time { return "\(Int(t)) ms" }
        return "0 ms"
    }
}

