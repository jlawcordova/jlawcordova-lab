import SwiftUI

/// The right inspector: SELECTED (the item, its object and thumbnail),
/// POSITION (coordinates, class, id and the item actions), PLACE (the Place
/// tool's object and level) and CHECKS, the engine's validation problems for
/// the open doc.
struct InspectorView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        // A VStack rather than one ScrollView: in the inspector column a scroll
        // view at the top edge insets its content by the toolbar height twice.
        // Only the checks list scrolls.
        VStack(alignment: .leading, spacing: 18) {
            if let doc = model.document {
                documentCard(doc)
                if doc.ref.kind == .scene {
                    SelectionInspector(doc: doc)
                    if model.editor.tool == .place || model.editor.placeObject != nil {
                        PlaceInspector()
                    }
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
            if doc.ref.kind == .scene, doc.itemsLoaded {
                Text("\(doc.items.count) items\(model.timeline.hasMotion ? " · motion \(TimelineModel.label(model.timeline.duration ?? 0))" : "")")
                    .font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(1)
            } else if let summary = doc.summary {
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
    @Environment(AppModel.self) private var model

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
                        let target = doc.ref.kind == .scene ? ProblemLocator.locate(p.text, items: doc.items) : nil
                        Button {
                            if let target {
                                model.editor.select(target)
                                model.editor.focusStage()
                            }
                        } label: {
                            HStack(alignment: .firstTextBaseline, spacing: 7) {
                                Image(systemName: "exclamationmark.triangle.fill")
                                    .foregroundStyle(Theme.gold)
                                    .imageScale(.small)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(p.message).font(.caption).foregroundStyle(Theme.ink)
                                        .fixedSize(horizontal: false, vertical: true)
                                    Text(p.file).font(Theme.monoFont(10)).foregroundStyle(Theme.inkMuted)
                                }
                                Spacer(minLength: 0)
                                if target != nil {
                                    Image(systemName: "scope").foregroundStyle(Theme.accent).imageScale(.small)
                                }
                            }
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .help(target.map { "Select item \($0)" } ?? p.text)
                        .accessibilityElement(children: .combine)
                        .accessibilityHint(target == nil ? "" : "Selects the item it names")
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

