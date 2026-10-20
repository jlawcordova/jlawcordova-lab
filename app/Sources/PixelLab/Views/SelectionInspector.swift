import SwiftUI

/// SELECTED and POSITION for the selected scene item: its object and
/// thumbnail, Col/Row/Level (or X/Y for a pixel-placed item) bound to
/// `scene.set` and `scene.move`, Class and ID, and Raise, Lower, Snap to
/// tile, Duplicate, Remove.
struct SelectionInspector: View {
    var doc: DocumentModel
    @Environment(AppModel.self) private var model
    @Environment(\.displayScale) private var displayScale

    var body: some View {
        let editor = model.editor
        VStack(alignment: .leading, spacing: 18) {
            InspectorSection("Selected") {
                if let item = doc.selectedItem {
                    HStack(spacing: 10) {
                        Thumb(image: model.thumbnails.images[item.object], side: 44)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(item.object)
                                .font(Theme.nameFont(14))
                                .foregroundStyle(Theme.ink)
                                .lineLimit(1)
                                .truncationMode(.middle)
                            Text([item.id.map { "#\($0)" }, "item \(item.path)"].compactMap { $0 }.joined(separator: " · "))
                                .font(Theme.monoFont(10.5))
                                .foregroundStyle(Theme.inkMuted)
                                .lineLimit(1)
                        }
                    }
                    .accessibilityElement(children: .combine)
                    .accessibilityLabel("Selected \(item.object), \(item.at.spoken)")
                    // Its layer (Select <layer>) and its SVG group (docs/intents/2026-10-scene-layers/spec.md, D12).
                    ItemLayerLines(doc: doc, item: item)
                } else {
                    Text(doc.items.isEmpty && !doc.itemsLoaded ? "Loading items…" : "Nothing selected")
                        .font(.callout)
                        .foregroundStyle(Theme.inkMuted)
                    Text("Click the stage to pick a layer, or a row in Layers. Double-click a layer to work inside it.")
                        .font(.caption)
                        .foregroundStyle(Theme.inkMuted.opacity(0.85))
                        .lineLimit(4)
                        .layoutPriority(1)
                }
            }

            InspectorSection("Position") {
                if let item = doc.selectedItem {
                    position(item, editor: editor)
                    textField("Class", text: Binding(get: { editor.classDraft }, set: { editor.classDraft = $0 }), prompt: "none") {
                        Task { await editor.setClass(editor.classDraft) }
                    }
                    textField("ID", text: Binding(get: { editor.idDraft }, set: { editor.idDraft = $0 }), prompt: "none") {
                        Task { await editor.setID(editor.idDraft) }
                    }
                    actions(item, editor: editor)
                } else {
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
            }
        }
    }

    @ViewBuilder
    private func position(_ item: SceneItemInfo, editor: SceneEditor) -> some View {
        if let t = item.at.tile, t.count == 3 {
            HStack(spacing: 6) {
                CoordField(label: "Col", value: t[0]) { v in Task { await editor.set("col", v) } } step: { d in Task { await editor.move(dc: d, dr: 0, dl: 0) } }
                CoordField(label: "Row", value: t[1]) { v in Task { await editor.set("row", v) } } step: { d in Task { await editor.move(dc: 0, dr: d, dl: 0) } }
                CoordField(label: "Level", value: t[2]) { v in Task { await editor.set("level", v) } } step: { d in Task { await editor.move(dc: 0, dr: 0, dl: d) } }
            }
        } else if let p = item.at.px, p.count == 2 {
            HStack(spacing: 6) {
                CoordField(label: "X", value: p[0]) { v in Task { await editor.set("x", v) } } step: { d in Task { await editor.nudge(dx: d, dy: 0) } }
                CoordField(label: "Y", value: p[1]) { v in Task { await editor.set("y", v) } } step: { d in Task { await editor.nudge(dx: 0, dy: d) } }
            }
            Text("Placed by pixel. Snap to tile puts it on the level-0 tile under it.")
                .font(.caption)
                .foregroundStyle(Theme.inkMuted)
                .lineLimit(3)
        }
    }

    private func textField(_ label: String, text: Binding<String>, prompt: String, submit: @escaping () -> Void) -> some View {
        HStack(spacing: 8) {
            Text(label).font(.caption).foregroundStyle(Theme.inkMuted).frame(width: 34, alignment: .leading)
            TextField(label, text: text, prompt: Text(prompt))
                .textFieldStyle(.plain)
                .font(Theme.monoFont(12))
                .foregroundStyle(Theme.ink)
                .padding(.horizontal, 7)
                .frame(height: 24)
                .background(Theme.card, in: RoundedRectangle(cornerRadius: 5, style: .continuous))
                .onSubmit(submit)
                .accessibilityLabel(label)
        }
    }

    private func actions(_ item: SceneItemInfo, editor: SceneEditor) -> some View {
        VStack(spacing: 6) {
            HStack(spacing: 6) {
                ActionButton(title: "Raise", symbol: "square.2.layers.3d.top.filled", help: "Paint it one later, in front of the next item (])") {
                    Task { await editor.raise() }
                }
                ActionButton(title: "Lower", symbol: "square.2.layers.3d.bottom.filled", help: "Paint it one earlier, behind the item before it ([)") {
                    Task { await editor.lower() }
                }
                ActionButton(title: "Snap", symbol: "square.grid.3x3.middle.filled", help: "Snap to the level-0 tile under it") {
                    Task { await editor.snap() }
                }
                .disabled(item.at.isTile)
            }
            HStack(spacing: 6) {
                ActionButton(title: "Duplicate", symbol: "plus.square.on.square", help: "Duplicate one column over (⌘D)") {
                    Task { await editor.duplicate() }
                }
                ActionButton(title: "Remove", symbol: "trash", help: "Remove the item (⌫)") {
                    Task { await editor.remove() }
                }
            }
        }
        .padding(.top, 2)
    }
}

/// PLACE: the library object the Place tool puts down, and the level.
struct PlaceInspector: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        let editor = model.editor
        InspectorSection("Place") {
            HStack(spacing: 10) {
                Thumb(image: editor.placeObject.flatMap { model.thumbnails.images[$0] }, side: 36)
                VStack(alignment: .leading, spacing: 2) {
                    Text(editor.placeObject ?? "No object chosen")
                        .font(Theme.nameFont(13))
                        .foregroundStyle(editor.placeObject == nil ? Theme.inkMuted : Theme.ink)
                        .lineLimit(1)
                    // Where it goes (docs/intents/2026-10-scene-layers/spec.md, D13).
                    Text(model.layers.placeTarget == nil ? "into the top level" : "into \(model.layers.placeTargetName)")
                        .font(.caption)
                        .foregroundStyle(Theme.inkMuted)
                        .lineLimit(1)
                    Text(editor.placeObject == nil ? "Choose one in the Library." : "Click a tile, or Return at the cursor. ⌥ places by pixel.")
                        .font(.caption)
                        .foregroundStyle(Theme.inkMuted)
                        .lineLimit(3)
                }
            }
            HStack {
                Text("Level").font(.caption).foregroundStyle(Theme.inkMuted)
                Text("\(editor.placeLevel)").font(Theme.monoFont(12)).foregroundStyle(Theme.ink).frame(minWidth: 18)
                Stepper("Place level", value: Binding(get: { editor.placeLevel }, set: { editor.placeLevel = $0 }), in: -4...12)
                    .labelsHidden()
                    .controlSize(.small)
                Spacer()
                if let t = editor.hoverTile ?? editor.cursorTile {
                    Text("tile \(t.col),\(t.row)").font(Theme.monoFont(10.5)).foregroundStyle(Theme.inkMuted)
                }
            }
            .accessibilityElement(children: .combine)
            .accessibilityLabel("Place level \(editor.placeLevel)")
        }
    }
}

struct InspectorSection<Content: View>: View {
    var title: String
    var content: Content

    init(_ title: String, @ViewBuilder content: () -> Content) {
        self.title = title
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            SectionLabel(title: title)
            content
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// An object thumbnail on a card, crisp at a whole factor.
struct Thumb: View {
    var image: PixelImage?
    var side: CGFloat
    @Environment(\.displayScale) private var displayScale

    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 6, style: .continuous).fill(Theme.card)
            if let image {
                image.view(pointsPerPixel: LibraryCell.factor(for: image.size, in: side - 6), displayScale: displayScale)
            }
        }
        .frame(width: side, height: side)
        .overlay(RoundedRectangle(cornerRadius: 6, style: .continuous).strokeBorder(Theme.border, lineWidth: 1))
        .accessibilityHidden(true)
    }
}

/// A coordinate: a number field (Return sets it with `scene.set`) and a
/// stepper (moves by one with `scene.move`).
struct CoordField: View {
    var label: String
    var value: Int
    var set: (Int) -> Void
    var step: (Int) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(label).font(.caption).foregroundStyle(Theme.inkMuted)
            HStack(spacing: 0) {
                TextField(label, value: Binding(get: { value }, set: { if $0 != value { set($0) } }), format: IntegerFormatStyle<Int>().grouping(.never))
                    .textFieldStyle(.plain)
                    .font(Theme.monoFont(12))
                    .foregroundStyle(Theme.ink)
                    .padding(.leading, 7)
                    // A new field when the value changes elsewhere (an arrow key,
                    // a drag, undo): a value-bound field keeps showing its old text.
                    .id(value)
                    .accessibilityLabel(label)
                Stepper(label, onIncrement: { step(1) }, onDecrement: { step(-1) })
                    .labelsHidden()
                    .controlSize(.mini)
                    .accessibilityLabel("\(label) stepper")
            }
            .frame(height: 24)
            .background(Theme.card, in: RoundedRectangle(cornerRadius: 5, style: .continuous))
        }
    }
}

struct ActionButton: View {
    var title: String
    var symbol: String
    var help: String
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            Label(title, systemImage: symbol)
                .labelStyle(.titleAndIcon)
                .font(.system(size: 11.5))
                .frame(maxWidth: .infinity)
        }
        .buttonStyle(.bordered)
        .controlSize(.small)
        .help(help)
    }
}
