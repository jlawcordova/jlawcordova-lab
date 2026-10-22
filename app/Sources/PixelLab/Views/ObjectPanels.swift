import SwiftUI

// Object mode's panels (the web lab's Palette, Layers, Animation, Object and
// Preview panels, and its status bar): the palette in the sidebar, the rest
// in the inspector.

/// PALETTE: the world palette a row per material ramp, then ink, cream and
/// the skin tones, and the outfit colors for an object that extends
/// character; the current color, and the colors-used meter against the cap.
struct PalettePanel: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        let objects = model.objectEditor
        VStack(alignment: .leading, spacing: 10) {
            currentColor(objects)
            ColorMeter(used: objects.usedHexes.count, cap: ObjectEditor.colorCap)
            SectionLabel(title: "World").padding(.top, 4)
            ForEach(worldRows, id: \.title) { row in
                if row.ramp {
                    HStack(spacing: 4) {
                        Text(row.title)
                            .font(Theme.monoFont(10))
                            .foregroundStyle(Theme.inkMuted)
                            .lineLimit(1)
                            .frame(width: 74, alignment: .leading)
                        ForEach(row.colors) { c in Swatch(color: c) }
                        Spacer(minLength: 0)
                    }
                    .accessibilityElement(children: .contain)
                    .accessibilityLabel(row.title)
                } else {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(row.title).font(Theme.monoFont(10)).foregroundStyle(Theme.inkMuted)
                        swatchGrid(row.colors)
                    }
                }
            }
            if objects.info?.character == true {
                SectionLabel(title: "Outfit").padding(.top, 4)
                swatchGrid(model.palette.filter { $0.tier == "outfit" })
            }
        }
        .padding(.horizontal, 6)
        .padding(.top, 10)
    }

    private func swatchGrid(_ colors: [PaletteColor]) -> some View {
        LazyVGrid(columns: Array(repeating: GridItem(.fixed(Swatch.side), spacing: 4), count: 7), alignment: .leading, spacing: 4) {
            ForEach(colors) { c in Swatch(color: c) }
        }
    }

    private func currentColor(_ objects: ObjectEditor) -> some View {
        let c = model.paletteByName[objects.color]
        return HStack(spacing: 10) {
            RoundedRectangle(cornerRadius: 6, style: .continuous)
                .fill(c.map { Theme.hexColor($0.hex) } ?? Theme.card)
                .frame(width: 34, height: 34)
                .overlay(RoundedRectangle(cornerRadius: 6, style: .continuous).strokeBorder(Theme.border, lineWidth: 1))
            VStack(alignment: .leading, spacing: 2) {
                Text(objects.color).font(Theme.nameFont(14)).foregroundStyle(Theme.ink).lineLimit(1)
                Text([c?.hex, c?.tier].compactMap { $0 }.joined(separator: " · "))
                    .font(Theme.monoFont(10.5)).foregroundStyle(Theme.inkMuted)
            }
            Spacer(minLength: 0)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Current color \(objects.color)")
    }

    struct Row {
        var title: String
        var colors: [PaletteColor]
        var ramp: Bool
    }

    /// The world palette's rows: each 4-shade ramp (name-1…name-4), then the rest (palette-panel.ts worldRows).
    private var worldRows: [Row] {
        var order: [String] = []
        var ramps: [String: [PaletteColor]] = [:]
        for c in model.palette where c.tier == "world" {
            let ramp = c.name.range(of: "-[0-9]$", options: .regularExpression).map { String(c.name[..<$0.lowerBound]) } ?? ""
            if ramps[ramp] == nil { order.append(ramp) }
            ramps[ramp, default: []].append(c)
        }
        var rows: [Row] = []
        var rest: [PaletteColor] = []
        for ramp in order {
            let colors = ramps[ramp] ?? []
            if !ramp.isEmpty, colors.count == 4 { rows.append(Row(title: ramp, colors: colors, ramp: true)) } else { rest.append(contentsOf: colors) }
        }
        if !rest.isEmpty { rows.append(Row(title: "ink, cream, skin", colors: rest, ramp: false)) }
        return rows
    }
}

/// One palette color: click to paint with it. Off when the object may not use it.
struct Swatch: View {
    var color: PaletteColor
    @Environment(AppModel.self) private var model
    static let side: CGFloat = 26

    var body: some View {
        let objects = model.objectEditor
        let current = objects.color == color.name
        let allowed = objects.allowed(color.name)
        let used = objects.usedHexes.contains(color.hex.uppercased())
        Button {
            objects.setColor(color.name)
            model.editor.focusStage()
        } label: {
            RoundedRectangle(cornerRadius: 5, style: .continuous)
                .fill(Theme.hexColor(color.hex))
                .frame(width: Self.side, height: Self.side)
                .overlay(RoundedRectangle(cornerRadius: 5, style: .continuous).strokeBorder(Theme.border, lineWidth: 1))
                .overlay(alignment: .bottomTrailing) {
                    if used { Circle().fill(Theme.ink.opacity(0.75)).frame(width: 4, height: 4).padding(3) }
                }
                .padding(2)
                .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).strokeBorder(current ? Theme.accent : .clear, lineWidth: 2))
                .opacity(allowed ? 1 : 0.28)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(allowed ? "\(color.name), \(color.hex)\(used ? " (used)" : "")" : "\(color.name), \(color.hex): \(objects.whyNot(color.name))")
        .accessibilityLabel("\(color.name), \(color.hex)")
        .accessibilityValue(current ? "current color" : (allowed ? (used ? "used" : "") : "not available"))
        .accessibilityAddTraits(current ? .isSelected : [])
    }
}

/// "Colors used: 7 of 12" with a segment per color; gold from 9.
struct ColorMeter: View {
    var used: Int
    var cap: Int

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            HStack {
                Text("Colors used").font(.caption).foregroundStyle(Theme.inkMuted)
                Spacer()
                Text("\(used) of \(cap)").font(Theme.monoFont(11)).foregroundStyle(Theme.ink).monospacedDigit()
            }
            HStack(spacing: 2) {
                ForEach(0..<cap, id: \.self) { i in
                    RoundedRectangle(cornerRadius: 1.5)
                        .fill(i < used ? (used >= 9 ? Theme.gold : Theme.accent) : Theme.border)
                        .frame(height: 5)
                }
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Colors used: \(used) of \(cap)")
    }
}

// MARK: Inspector

/// LAYERS, ANIMATION, OBJECT and PREVIEW for the open object.
struct ObjectInspector: View {
    var doc: DocumentModel
    @Environment(AppModel.self) private var model

    var body: some View {
        let objects = model.objectEditor
        VStack(alignment: .leading, spacing: 18) {
            if objects.info == nil {
                HStack(spacing: 6) {
                    ProgressView().controlSize(.mini)
                    Text("Loading \(doc.ref.name)…").font(.caption).foregroundStyle(Theme.inkMuted)
                }
            } else {
                LayersSection(objects: objects)
                ActionsSection(rows: objects.actionRows)
                if objects.currentLayer?.isLoop == true || (objects.editable && objects.currentLayer != nil) {
                    FramesSection(objects: objects)
                }
                ObjectFactsSection(objects: objects)
                if objects.footprint != nil {
                    FootprintSection(objects: objects)
                }
                InspectorSection("Preview") {
                    PreviewTile(render: doc.render)
                    Text("At 1× and 2×, standing on one tile, at the frame shown.")
                        .font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(2)
                }
            }
        }
    }
}

struct LayersSection: View {
    var objects: ObjectEditor

    var body: some View {
        InspectorSection("Layers") {
            VStack(spacing: 1) {
                ForEach(objects.layers.reversed(), id: \.index) { l in
                    row(l)
                }
            }
            if objects.editable {
                HStack(spacing: 4) {
                    IconAction(symbol: "plus", title: "Add layer", help: "Add a blank layer on top") { Task { await objects.addLayer() } }
                    IconAction(symbol: "plus.square.on.square", title: "Duplicate layer", help: "Duplicate the layer, just above it") { Task { await objects.duplicateLayer() } }
                    IconAction(symbol: "arrow.up", title: "Raise layer", help: "Raise the layer (paint it later)") { Task { await objects.moveLayer(by: 1) } }
                        .disabled(objects.layer >= objects.layers.count - 1)
                    IconAction(symbol: "arrow.down", title: "Lower layer", help: "Lower the layer (paint it earlier)") { Task { await objects.moveLayer(by: -1) } }
                        .disabled(objects.layer == 0)
                    IconAction(symbol: "trash", title: "Delete layer", help: "Delete the layer") { Task { await objects.deleteLayer() } }
                        .disabled(objects.layers.count < 2)
                }
            } else if let info = objects.info {
                Text(info.isBlock
                     ? "A block paints its top surface and its sides as the engine draws them."
                     : "Takes its layers from \(info.extends ?? "its base"); rows painted here override them.")
                    .font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(3)
            }
        }
    }

    private func row(_ l: ObjectLayerInfo) -> some View {
        let selected = l.index == objects.layer
        let visible = !objects.hidden.contains(l.index)
        return HStack(spacing: 6) {
            Button {
                objects.toggleVisible(l.index)
            } label: {
                Image(systemName: visible ? "eye" : "eye.slash")
                    .foregroundStyle(visible ? Theme.inkMuted : Theme.inkMuted.opacity(0.5))
                    .frame(width: 20)
            }
            .buttonStyle(.plain)
            .help(visible ? "Hide in the editor (never saved)" : "Show in the editor")
            .accessibilityLabel("Show \(objects.label(l.index))")
            .accessibilityValue(visible ? "shown" : "hidden")
            Button {
                objects.selectLayer(l.index)
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: l.isLoop ? "film" : "square.3.layers.3d.down.right")
                        .imageScale(.small)
                        .foregroundStyle(selected ? Theme.accent : Theme.inkMuted)
                    Text(objects.label(l.index))
                        .font(.system(size: 12, weight: selected ? .semibold : .regular))
                        .foregroundStyle(visible ? Theme.ink : Theme.inkMuted)
                        .lineLimit(1)
                        .truncationMode(.middle)
                    Spacer(minLength: 0)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(objects.label(l.index))
            .accessibilityAddTraits(selected ? .isSelected : [])
        }
        .padding(.horizontal, 6)
        .frame(height: 26)
        .background(RoundedRectangle(cornerRadius: 5, style: .continuous).fill(selected ? Theme.selection : .clear))
    }
}

/// ANIMATION: a still layer's Animate button, or a loop's frame strip,
/// playback, onion skin, loop kind and frame actions.
struct FramesSection: View {
    var objects: ObjectEditor
    @Environment(\.displayScale) private var displayScale

    var body: some View {
        InspectorSection("Animation") {
            if let l = objects.currentLayer, l.isLoop {
                if objects.editable {
                    HStack {
                        Text("Loop").font(.caption).foregroundStyle(Theme.inkMuted)
                        Picker("Loop", selection: Binding(get: { l.loop ?? "ff" }, set: { v in
                            if let loop = FrameLoop(rawValue: v), v != l.loop { Task { await objects.animate(loop) } }
                        })) {
                            ForEach(FrameLoop.allCases) { Text($0.title).tag($0.rawValue) }
                            if FrameLoop(rawValue: l.loop ?? "") == nil { Text(l.loop ?? "?").tag(l.loop ?? "?") }
                        }
                        .labelsHidden()
                        .controlSize(.small)
                    }
                }
                strip(l)
                HStack(spacing: 4) {
                    IconAction(symbol: "chevron.left", title: "Previous frame", help: "Previous frame (Page Up)") { objects.selectFrame(objects.frame - 1) }
                    IconAction(symbol: objects.playing ? "pause.fill" : "play.fill", title: objects.playing ? "Pause" : "Play",
                               help: objects.playing ? "Pause" : "Play at the loop's timing") { objects.togglePlay() }
                    IconAction(symbol: "chevron.right", title: "Next frame", help: "Next frame (Page Down)") { objects.selectFrame(objects.frame + 1) }
                    Button {
                        objects.toggleOnion()
                    } label: {
                        Label("Onion", systemImage: objects.onion ? "circle.lefthalf.filled" : "circle.dashed")
                            .font(.system(size: 11.5))
                            .foregroundStyle(objects.onion ? Theme.ink : Theme.inkMuted)
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                    .background(RoundedRectangle(cornerRadius: 5, style: .continuous).fill(objects.onion ? Theme.selection : .clear))
                    .help("Onion skin: the previous frame at 30%")
                    .accessibilityLabel("Onion skin")
                    .accessibilityValue(objects.onion ? "on" : "off")
                }
                if objects.editable {
                    HStack(spacing: 4) {
                        IconAction(symbol: "plus", title: "Add frame", help: "Add a blank frame after this one") { Task { await objects.addFrame() } }
                        IconAction(symbol: "plus.square.on.square", title: "Duplicate frame", help: "Duplicate this frame") { Task { await objects.duplicateFrame() } }
                        IconAction(symbol: "arrow.left", title: "Move earlier", help: "Move the frame earlier") { Task { await objects.moveFrame(by: -1) } }
                            .disabled(objects.frame == 0)
                        IconAction(symbol: "arrow.right", title: "Move later", help: "Move the frame later") { Task { await objects.moveFrame(by: 1) } }
                            .disabled(objects.frame >= objects.frameCount - 1)
                        IconAction(symbol: "trash", title: "Delete frame", help: "Delete the frame") { Task { await objects.deleteFrame() } }
                            .disabled(objects.frameCount < 2)
                    }
                    Button("Stop animating") { Task { await objects.makeStill() } }
                        .controlSize(.small)
                        .help("Make the layer still, keeping the frame shown")
                }
                if let note = objects.loopNote {
                    Text(note).font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(3)
                }
            } else {
                Text("This layer is still. Animate it to paint its frames one by one.")
                    .font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(3)
                Button {
                    Task { await objects.animate(.ff) }
                } label: {
                    Label("Animate this layer", systemImage: "film.stack")
                }
                .controlSize(.small)
                .help("Turn the layer into an ff loop of 4 frames, each a copy to paint")
            }
        }
    }

    private func strip(_ l: ObjectLayerInfo) -> some View {
        ScrollViewReader { proxy in
            ScrollView(.horizontal) {
                HStack(spacing: 4) {
                    ForEach(0..<l.frames, id: \.self) { i in
                        let selected = i == objects.frame
                        Button {
                            objects.selectFrame(i)
                        } label: {
                            VStack(spacing: 2) {
                                ZStack {
                                    RoundedRectangle(cornerRadius: 5, style: .continuous).fill(Theme.card)
                                    if let image = objects.cached(frame: i)?.image {
                                        image.view(pointsPerPixel: LibraryCell.factor(for: image.size, in: 36), displayScale: displayScale)
                                    }
                                }
                                .frame(width: 42, height: 42)
                                .overlay(RoundedRectangle(cornerRadius: 5, style: .continuous)
                                    .strokeBorder(selected ? Theme.accent : Theme.border, lineWidth: selected ? 1.5 : 1))
                                Text(objects.frameName(i)).font(Theme.monoFont(9.5)).foregroundStyle(selected ? Theme.ink : Theme.inkMuted)
                            }
                        }
                        .buttonStyle(.plain)
                        .id(i)
                        .accessibilityLabel("Frame \(objects.frameName(i))")
                        .accessibilityAddTraits(selected ? .isSelected : [])
                    }
                }
                .padding(.vertical, 2)
            }
            .scrollIndicators(.never)
            .onChange(of: objects.frame) { _, f in proxy.scrollTo(f) }
        }
    }
}

/// OBJECT: kind, size, extends, block size, colors, layers and frames, and
/// the size fields for a sprite with layers of its own.
struct ObjectFactsSection: View {
    var objects: ObjectEditor

    var body: some View {
        InspectorSection("Object") {
            if let info = objects.info {
                Grid(alignment: .leading, horizontalSpacing: 10, verticalSpacing: 4) {
                    fact("Kind", info.isBlock ? "block" : (info.character == true ? "sprite · character" : "sprite"))
                    fact("Size", "\(info.size.first ?? 0) × \(info.size.count > 1 ? info.size[1] : 0) px (max 64 × 64)")
                    if let ext = info.extends { fact("Extends", ext) }
                    if let b = info.blockSize, b.count == 3 { fact("Block", "\(b[0]) wide · \(b[1]) deep · \(b[2]) level\(b[2] == 1 ? "" : "s")") }
                    fact("Colors", "\(info.usedColors.count) of \(ObjectEditor.colorCap)")
                    fact("Layers", "\(info.layers.count) · \(info.frames) frame\(info.frames == 1 ? "" : "s")")
                }
                if objects.editable {
                    HStack(spacing: 6) {
                        sizeField("Width", text: Binding(get: { objects.widthDraft }, set: { objects.widthDraft = $0 }))
                        Text("×").foregroundStyle(Theme.inkMuted).padding(.top, 14)
                        sizeField("Height", text: Binding(get: { objects.heightDraft }, set: { objects.heightDraft = $0 }))
                    }
                    Text("Return resizes: pads or crops at the right and bottom.")
                        .font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(2)
                } else if info.extends != nil {
                    Text("Rows painted here override \(info.extends ?? "")'s rows. A row painted back to match is dropped.")
                        .font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(3)
                } else if info.isBlock {
                    Text("Light comes from the island side: the top is the light shade, the left the mid, the right the shadow.")
                        .font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(3)
                }
            }
        }
    }

    private func fact(_ term: String, _ value: String) -> some View {
        GridRow {
            Text(term).font(.caption).foregroundStyle(Theme.inkMuted)
            Text(value).font(Theme.monoFont(11)).foregroundStyle(Theme.ink).lineLimit(1)
        }
        .accessibilityElement(children: .combine)
    }

    private func sizeField(_ label: String, text: Binding<String>) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(label).font(.caption).foregroundStyle(Theme.inkMuted)
            TextField(label, text: text)
                .textFieldStyle(.plain)
                .font(Theme.monoFont(12))
                .foregroundStyle(Theme.ink)
                .padding(.horizontal, 7)
                .frame(height: 24)
                .background(Theme.card, in: RoundedRectangle(cornerRadius: 5, style: .continuous))
                .onSubmit {
                    Task { await objects.resize(width: Int(objects.widthDraft) ?? -1, height: Int(objects.heightDraft) ?? -1) }
                }
                .accessibilityLabel(label)
        }
    }
}

/// FOOTPRINT (sprites only): how many tiles the sprite covers, wide and deep,
/// so it sorts against its neighbours by the ground under it
/// (docs/intents/2026-10-depth-order/spec.md, R8, D7). A block's size is its footprint.
struct FootprintSection: View {
    var objects: ObjectEditor

    var body: some View {
        InspectorSection("Footprint") {
            if let f = objects.footprint {
                HStack(spacing: 14) {
                    tiles("Wide", value: f.width) { w in Task { await objects.setFootprint(width: w, depth: f.depth) } }
                    tiles("Deep", value: f.depth) { d in Task { await objects.setFootprint(width: f.width, depth: d) } }
                }
                Text("Tiles it covers from the tile it's placed on, 1 to 16 each way. A house or a locomotive sorts against its neighbours by this ground.")
                    .font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(4)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private func tiles(_ label: String, value: Int, set: @escaping (Int) -> Void) -> some View {
        HStack(spacing: 6) {
            Text(label).font(.caption).foregroundStyle(Theme.inkMuted)
            Text("\(value)")
                .font(Theme.monoFont(12))
                .foregroundStyle(Theme.ink)
                .frame(minWidth: 20, alignment: .trailing)
            Stepper(label, value: Binding(get: { value }, set: { if $0 != value { set($0) } }), in: ObjectEditor.footprintRange)
                .labelsHidden()
                .controlSize(.regular)
                .accessibilityLabel("Footprint \(label.lowercased())")
                .accessibilityValue("\(value) tile\(value == 1 ? "" : "s")")
        }
        .accessibilityElement(children: .contain)
    }
}

/// PREVIEW: the object standing on one tile, at 1× and 2×, at the frame shown.
struct PreviewTile: View {
    var render: StageImage?

    var body: some View {
        if let render {
            ViewThatFits(in: .horizontal) {
                HStack(alignment: .bottom, spacing: 14) { tiles(render) }
                VStack(alignment: .leading, spacing: 10) { tiles(render) }
            }
            .padding(10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Theme.card, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
            .accessibilityElement(children: .ignore)
            .accessibilityLabel("Preview at 1× and 2× on a tile")
        }
    }

    @ViewBuilder
    private func tiles(_ render: StageImage) -> some View {
        PreviewCanvas(render: render, scale: 1)
        PreviewCanvas(render: render, scale: 2)
    }
}

struct PreviewCanvas: View {
    var render: StageImage
    var scale: CGFloat
    @Environment(\.displayScale) private var displayScale

    var body: some View {
        // Object pixels with the anchor (a block's first tile center) at the
        // tile's center (0, 0); the box holds the art and the tile.
        let art = render.viewBox
        let tile = CGRect(x: -16, y: -8, width: 32, height: 16)
        let box = art.union(tile).insetBy(dx: -2, dy: -2)
        ZStack(alignment: .topLeading) {
            Canvas { context, _ in
                for p in TileShape.outline {
                    context.fill(Path(CGRect(x: (CGFloat(p.x) - box.minX) * scale, y: (CGFloat(p.y) - box.minY) * scale, width: scale, height: scale)),
                                 with: .color(Theme.ink.opacity(0.25)))
                }
            }
            render.image.view(pointsPerPixel: scale, displayScale: displayScale)
                .offset(x: (art.minX - box.minX) * scale, y: (art.minY - box.minY) * scale)
        }
        .frame(width: box.width * scale, height: box.height * scale, alignment: .topLeading)
    }
}

/// A small icon button for the panels' actions.
struct IconAction: View {
    var symbol: String
    var title: String
    var help: String
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol).frame(width: 18, height: 16)
        }
        .buttonStyle(.bordered)
        .controlSize(.small)
        .help(help)
        .accessibilityLabel(title)
    }
}

// MARK: Status bar

/// Under the stage in object mode: the tool and color, the pixel under the
/// pointer (or the cursor) with its color, the layer and frame, the zoom and
/// the last edit.
struct ObjectStatusBar: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        let objects = model.objectEditor
        let at = objects.hover ?? objects.cursor
        let hex = objects.renderColors?.hex(at)
        let under = hex.flatMap { objects.colorName(hex: $0) } ?? (hex ?? "empty")
        HStack(spacing: 14) {
            HStack(spacing: 6) {
                Image(systemName: objects.tool.symbol).foregroundStyle(Theme.accent)
                Text(objects.tool.title).font(.callout).foregroundStyle(Theme.ink)
                RoundedRectangle(cornerRadius: 3)
                    .fill(model.paletteByName[objects.color].map { Theme.hexColor($0.hex) } ?? Theme.card)
                    .frame(width: 12, height: 12)
                    .overlay(RoundedRectangle(cornerRadius: 3).strokeBorder(Theme.border, lineWidth: 1))
                Text(objects.color).font(Theme.monoFont(11)).foregroundStyle(Theme.inkMuted).lineLimit(1)
            }
            .accessibilityElement(children: .combine)
            divider
            HStack(spacing: 6) {
                Text("x \(at.x) · y \(at.y)").font(Theme.monoFont(11)).foregroundStyle(Theme.ink).monospacedDigit()
                RoundedRectangle(cornerRadius: 3)
                    .fill(hex.map(Theme.hexColor) ?? .clear)
                    .frame(width: 12, height: 12)
                    .overlay(RoundedRectangle(cornerRadius: 3).strokeBorder(Theme.border, lineWidth: 1))
                Text(under).font(Theme.monoFont(11)).foregroundStyle(Theme.inkMuted).lineLimit(1)
            }
            .accessibilityElement(children: .combine)
            .accessibilityLabel("x \(at.x), y \(at.y): \(under)")
            divider
            Text(layerText(objects)).font(Theme.monoFont(11)).foregroundStyle(Theme.inkMuted).lineLimit(1)
            divider
            Text("\(model.zoomText)×").font(Theme.monoFont(11)).foregroundStyle(Theme.inkMuted)
            Spacer(minLength: 8)
            if let edit = model.lastEdit {
                Text(edit).font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(1).truncationMode(.middle)
            }
        }
        .padding(.horizontal, 14)
        .frame(height: 40)
        .background(Theme.sidebar)
        .overlay(alignment: .top) { Rectangle().fill(Theme.border).frame(height: 1) }
    }

    private var divider: some View { Rectangle().fill(Theme.border).frame(width: 1, height: 16) }

    private func layerText(_ objects: ObjectEditor) -> String {
        let name = objects.label(objects.layer).components(separatedBy: " · ").first ?? ""
        return objects.currentLayer?.isLoop == true ? "\(name) · \(objects.frameName(objects.frame))" : name
    }
}
