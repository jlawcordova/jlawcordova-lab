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
                if let status = doc.status, status.blocked { NotSavedRow(status: status) }
                documentCard(doc)
                if doc.ref.kind == .scene {
                    // A layer, several things, or an item (docs/intents/2026-10-scene-layers/spec.md, D12).
                    // Ahead of the Checks list, so its notes get their lines.
                    Group {
                        if let layer = doc.selectedLayer {
                            LayerInspector(doc: doc, layer: layer)
                        } else if doc.selection.count > 1 {
                            MultiInspector(doc: doc)
                        } else {
                            SelectionInspector(doc: doc)
                        }
                    }
                    .layoutPriority(1)
                    if !doc.selection.isEmpty { AskClaudeButton(count: doc.selection.count) }
                    if model.editor.placeObject != nil {
                        PlaceInspector()
                    }
                    PublishedSizeSection(doc: doc)
                    section("Checks") {
                        ScrollView {
                            ChecksList(doc: doc).frame(maxWidth: .infinity, alignment: .leading)
                        }
                        .scrollBounceBehavior(.basedOnSize)
                    }
                } else {
                    // Below the document card, so the scroll view isn't at the
                    // column's top edge (see above).
                    ScrollView {
                        VStack(alignment: .leading, spacing: 18) {
                            ObjectInspector(doc: doc)
                            section("Checks") {
                                ChecksList(doc: doc).frame(maxWidth: .infinity, alignment: .leading)
                            }
                        }
                        .padding(.bottom, 8)
                    }
                    .scrollBounceBehavior(.basedOnSize)
                    .scrollIndicators(.automatic)
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
                let layerCount = doc.layerTree.layers.count
                Text("\(doc.items.count) items\(layerCount > 0 ? " · \(layerCount) layer\(layerCount == 1 ? "" : "s")" : "")\(model.timeline.hasMotion ? " · \((model.timeline.motionDuration ?? 0) > 0 ? "motion" : "loops") \(TimelineModel.label(model.timeline.duration ?? 0))" : "")")
                    .font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(1)
            } else if doc.ref.kind == .object, let summary = model.objectEditor.info?.description ?? doc.summary {
                Text(summary).font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(2)
            } else if let summary = doc.summary {
                Text(summary).font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(2)
            }
            if doc.ref.kind == .scene {
                Text("\(Int(doc.publishSize.width))×\(Int(doc.publishSize.height)) px · \(doc.isFixedFrame ? "fixed" : "fits") · \(model.zoomText)×")
                    .font(Theme.monoFont(10.5))
                    .foregroundStyle(Theme.inkMuted)
            } else if let render = doc.render {
                Text("\(Int(render.viewBox.width))×\(Int(render.viewBox.height)) px · \(model.zoomText)×")
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
        VStack(alignment: .leading, spacing: 10) {
            problemsList
            if doc.problems != nil, !doc.warnings.isEmpty {
                warningsList
            }
        }
    }

    /// Warnings don't fail the check. A scene whose file isn't in depth order
    /// gets a Sort button (`scene.sort`; docs/intents/2026-10-depth-order/spec.md, R11, D7).
    private var warningsList: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("\(doc.warnings.count) warning\(doc.warnings.count == 1 ? "" : "s")")
                .font(.callout.weight(.medium)).foregroundStyle(Theme.ink)
            ForEach(Array(doc.warnings.prefix(50).enumerated()), id: \.offset) { _, w in
                HStack(alignment: .firstTextBaseline, spacing: 7) {
                    Image(systemName: w.kind == .outOfOrder ? "square.3.layers.3d.down.right" : "exclamationmark.circle.fill")
                        .foregroundStyle(Theme.gold)
                        .imageScale(.small)
                        .accessibilityHidden(true)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(w.message).font(.caption).foregroundStyle(Theme.ink)
                            .fixedSize(horizontal: false, vertical: true)
                        Text(w.file).font(Theme.monoFont(10)).foregroundStyle(Theme.inkMuted)
                    }
                    Spacer(minLength: 0)
                    if w.kind == .outOfOrder, doc.ref.kind == .scene {
                        Button("Sort") { Task { await model.editor.sort() } }
                            .controlSize(.regular)
                            .help("Put the items in depth order, in one undo step (⌘Z undoes it)")
                            .accessibilityLabel("Sort \(doc.ref.name) by depth")
                    }
                }
                .accessibilityElement(children: .contain)
            }
        }
    }

    @ViewBuilder
    private var problemsList: some View {
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


/// Ask Claude about this (docs/intents/2026-10-agent-chat/spec.md, R6): the Agent tab, with the
/// selection as the chat's context and the focus in its input.
struct AskClaudeButton: View {
    @Environment(AppModel.self) private var model
    var count: Int

    var body: some View {
        Button { model.showAgentTab(focusInput: true) } label: {
            HStack(spacing: 6) {
                Image(systemName: "sparkle").font(.system(size: 12))
                Text(count > 1 ? "Ask Claude about these \(count)" : "Ask Claude about this").font(.system(size: 13, weight: .semibold))
            }
            .foregroundStyle(Theme.accent)
            .frame(maxWidth: .infinity, minHeight: 32)
            .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).strokeBorder(Theme.accent, lineWidth: 1))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help("Chat with Claude about the selection (⌥⌘A)")
    }
}

/// At the top of the Inspector while autosave can't write the open doc (docs/intents/2026-10-autosave/spec.md, R2,
/// D2): why, in gold, and Check, which lists the problems below. Nothing pops up while you edit.
struct NotSavedRow: View {
    @Environment(AppModel.self) private var model
    var status: DocStatus

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(Theme.gold).accessibilityHidden(true)
            Text(model.saveStateText).font(.system(size: 12.5, weight: .semibold)).foregroundStyle(Theme.ink)
            Spacer(minLength: 6)
            if status.saveState == "problems" {
                Button("Check") { model.requestCheck() }
                    .help("List the problems in Checks below; the doc saves itself once they're fixed")
            }
        }
        .padding(.horizontal, 10).padding(.vertical, 7)
        .background(Theme.gold.opacity(0.16), in: RoundedRectangle(cornerRadius: 7, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).strokeBorder(Theme.gold.opacity(0.6), lineWidth: 1))
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(model.saveStateText). It saves itself once the problems are fixed.")
    }
}
