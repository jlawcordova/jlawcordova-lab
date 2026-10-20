import SwiftUI

/// The inspector for a selected layer (docs/intents/2026-10-scene-layers/spec.md, D12):
/// SELECTED (its name, what it holds and where, Go inside, Hide, Lock),
/// MOVED BY (Col and Row since it was selected, and the four diagonal
/// nudges) and LAYER (Place into it).
struct LayerInspector: View {
    var doc: DocumentModel
    var layer: String
    @Environment(AppModel.self) private var model

    var body: some View {
        let layers = model.layers
        let tree = doc.layerTree
        let count = tree.items(inside: layer).count
        let parent = tree.byID[layer]?.parent
        let hidden = layers.hidden.contains(layer)
        let locked = layers.locked.contains(layer)
        VStack(alignment: .leading, spacing: 18) {
            InspectorSection("Selected") {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Layer name").font(.caption).foregroundStyle(Theme.inkMuted)
                    TextField("Layer name", text: Binding(get: { layers.nameDraft }, set: { layers.nameDraft = $0 }))
                        .textFieldStyle(.plain)
                        .font(Theme.nameFont(13))
                        .foregroundStyle(Theme.ink)
                        .padding(.horizontal, 8)
                        .frame(height: 28)
                        .background(Theme.card, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 6, style: .continuous).strokeBorder(Theme.border, lineWidth: 1))
                        .onSubmit { Task { await layers.rename(layers.nameDraft) } }
                        .accessibilityLabel("Layer name")
                }
                Text("\(count) item\(count == 1 ? "" : "s") · \(parent.map { "in \(tree.name(of: $0))" } ?? "top level")")
                    .font(.caption)
                    .foregroundStyle(Theme.inkMuted)
                HStack(spacing: 6) {
                    LayerButton(title: "Go inside", symbol: "arrow.right.to.line", help: "Work inside the layer: the rest fades (double-click it, or Return in Layers)") {
                        layers.goInside(layer)
                    }
                    LayerButton(title: hidden ? "Show" : "Hide", symbol: hidden ? "eye" : "eye.slash", help: hidden ? "Draw it again" : "Don't draw it while you work (not saved in the document)") {
                        layers.toggleHidden(layer)
                    }
                    .accessibilityAddTraits(hidden ? .isSelected : [])
                    LayerButton(title: locked ? "Unlock" : "Lock", symbol: locked ? "lock.open" : "lock", help: locked ? "Let clicks on the stage pick it again" : "Clicks on the stage pass it by, except from inside it") {
                        layers.toggleLocked(layer)
                    }
                    .accessibilityAddTraits(locked ? .isSelected : [])
                }
            }
            MoveSection(title: "Moved by", moved: layers.movedBy, note: "Drag on the stage, or nudge a tile at a time. Every item in the layer moves together.")
            InspectorSection("Layer") {
                Button { layers.placeInto() } label: {
                    Label("Place into it", systemImage: "plus.square.on.square").font(.system(size: 11.5))
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .help("Choose an object in the Library and click a tile: it joins \(tree.name(of: layer))")
                Text("Right-click › Ungroup (⇧⌘G) to undo it. Paint order doesn't change: each item still draws back to front by where it stands.")
                    .font(.caption)
                    .foregroundStyle(Theme.inkMuted)
                    .lineLimit(4)
                    .layoutPriority(1)
            }
        }
    }
}

/// Several things selected: how many, and the way to make them one layer.
struct MultiInspector: View {
    var doc: DocumentModel
    @Environment(AppModel.self) private var model

    var body: some View {
        let n = doc.selection.count
        let items = doc.layerTree.paths(of: doc.selection).count
        VStack(alignment: .leading, spacing: 18) {
            InspectorSection("Selected") {
                Text("\(n) selected").font(.callout).foregroundStyle(Theme.ink)
                Text("\(items) item\(items == 1 ? "" : "s") in all. Group them to name, hide and move them as one.")
                    .font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(4)
                    .layoutPriority(1)
                Text("Right-click › Group (⌘G) to make them one layer.")
                    .font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(4)
                    .layoutPriority(1)
            }
            MoveSection(title: "Move together", moved: nil, note: "Nudges move everything selected.")
        }
    }
}

/// MOVED BY / MOVE TOGETHER: Col and Row (Return moves by the difference)
/// and the four diagonal nudges, in the arrow keys' directions.
struct MoveSection: View {
    var title: String
    /// Moved by: tiles since the layer was selected (nil: no fields).
    var moved: (col: Int, row: Int)?
    var note: String
    @Environment(AppModel.self) private var model

    var body: some View {
        let layers = model.layers
        InspectorSection(title) {
            if model.document?.selectedLayer != nil {
                HStack(spacing: 6) {
                    SignedField(label: "Col", value: moved?.col ?? 0) { v in Task { await layers.setMovedBy(col: v) } }
                    SignedField(label: "Row", value: moved?.row ?? 0) { v in Task { await layers.setMovedBy(row: v) } }
                }
            }
            HStack(alignment: .center, spacing: 12) {
                Grid(horizontalSpacing: 4, verticalSpacing: 4) {
                    GridRow {
                        NudgeButton(symbol: "arrow.up.left", label: "Move up-left: column minus 1") { Task { await layers.move(dc: -1, dr: 0, dl: 0) } }
                        NudgeButton(symbol: "arrow.up.right", label: "Move up-right: row minus 1") { Task { await layers.move(dc: 0, dr: -1, dl: 0) } }
                    }
                    GridRow {
                        NudgeButton(symbol: "arrow.down.left", label: "Move down-left: row plus 1") { Task { await layers.move(dc: 0, dr: 1, dl: 0) } }
                        NudgeButton(symbol: "arrow.down.right", label: "Move down-right: column plus 1") { Task { await layers.move(dc: 1, dr: 0, dl: 0) } }
                    }
                }
                Text(note).font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(4)
                    .layoutPriority(1)
            }
        }
    }
}

/// A number shown signed ("+1", "−2", "0"); typing one and Return sets it.
struct SignedField: View {
    var label: String
    var value: Int
    var set: (Int) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(label).font(.caption).foregroundStyle(Theme.inkMuted)
            TextField(label, value: Binding(get: { value }, set: { if $0 != value { set($0) } }), format: SignedFormat())
                .textFieldStyle(.plain)
                .font(Theme.monoFont(12))
                .foregroundStyle(Theme.ink)
                .padding(.horizontal, 7)
                .frame(height: 24)
                .background(Theme.card, in: RoundedRectangle(cornerRadius: 5, style: .continuous))
                .id(value)
                .accessibilityLabel("Moved by, \(label.lowercased())")
                .accessibilityValue("\(value)")
        }
    }
}

/// "+1", "−2", "0" both ways ("-2" and "2" read too).
struct SignedFormat: ParseableFormatStyle {
    var parseStrategy: Strategy { Strategy() }

    func format(_ value: Int) -> String { StageOverlay.signed(value) }

    struct Strategy: ParseStrategy {
        func parse(_ value: String) throws -> Int {
            let t = value.trimmingCharacters(in: .whitespaces).replacingOccurrences(of: "−", with: "-").replacingOccurrences(of: "+", with: "")
            guard let n = Int(t) else { throw CocoaError(.formatting) }
            return n
        }
    }
}

struct NudgeButton: View {
    var symbol: String
    var label: String
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(Theme.ink)
                .frame(width: 36, height: 36)
                .background(Theme.card, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).strokeBorder(Theme.border, lineWidth: 1))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(label)
        .accessibilityLabel(label)
    }
}

struct LayerButton: View {
    var title: String
    var symbol: String
    var help: String
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            Label(title, systemImage: symbol).labelStyle(.titleAndIcon).font(.system(size: 11.5))
        }
        .buttonStyle(.bordered)
        .controlSize(.small)
        .help(help)
    }
}

/// Under the selected item: the layer it's in, with Select <layer>, and its
/// SVG group (the site's wrapper) with its attributes, read-only (D3, D12).
struct ItemLayerLines: View {
    var doc: DocumentModel
    var item: SceneItemInfo
    @Environment(AppModel.self) private var model

    var body: some View {
        let tree = doc.layerTree
        HStack(spacing: 6) {
            if let l = tree.layerOfItem[item.path] {
                Text("in \(tree.name(of: l))").font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(1)
                Button("Select \(tree.name(of: l))") { model.layers.select([.layer(l)]) }
                    .controlSize(.small)
                    .help("Select the layer, to move it with the rest")
            } else {
                Text("Not in a layer").font(.caption).foregroundStyle(Theme.inkMuted)
            }
        }
        if let g = item.group, let group = doc.groups.first(where: { $0.path == g }) {
            Text("SVG group \(g)\(Self.attributes(group).map { " · \($0)" } ?? "")")
                .font(Theme.monoFont(10))
                .foregroundStyle(Theme.inkMuted)
                .lineLimit(1)
                .truncationMode(.tail)
                .help("The site's wrapper around the item (scene group set edits it)")
        }
    }

    /// ".enter · enter=object": its class and data-* other than the coordinates.
    static func attributes(_ g: SceneGroupInfo) -> String? {
        var parts: [String] = []
        if let c = g.attrs["class"], !c.isEmpty { parts.append("." + c.replacingOccurrences(of: " ", with: ".")) }
        for (k, v) in g.attrs.sorted(by: { $0.key < $1.key }) where k.hasPrefix("data-") && !["data-col", "data-row", "data-x", "data-y"].contains(k) {
            parts.append(v.isEmpty ? String(k.dropFirst(5)) : "\(k.dropFirst(5))=\(v)")
        }
        return parts.isEmpty ? nil : parts.joined(separator: " · ")
    }
}
