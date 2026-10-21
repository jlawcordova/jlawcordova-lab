import SwiftUI

/// What the stage draws over the art in scene mode: the Place tool's tile
/// and ghost, a dragged item's (or selection's) ghost, the selection (board
/// 03: the accent, square, dashed for a layer and solid for an item, with a
/// small name tag), what a click would pick, and the box of the layer you're
/// inside, in gold (docs/intents/2026-10-scene-layers/spec.md, D6, D7).
struct StageOverlay: View {
    var geometry: StageGeometry
    var doc: DocumentModel
    var render: StageImage
    @Environment(AppModel.self) private var model
    @Environment(\.displayScale) private var displayScale

    var body: some View {
        let editor = model.editor
        ZStack(alignment: .topLeading) {
            if let object = editor.dropObject ?? (editor.effectiveTool == .place ? editor.placeObject : nil) {
                placeGhost(object: object, editor: editor)
            } else if editor.effectiveTool == .place, let tile = editor.hoverTile ?? editor.cursorTile {
                TileMark(geometry: geometry, tile: tile)
            }
            let layers = model.layers
            if let inside = layers.inside, let rect = box(of: .layer(inside)) {
                Rectangle()
                    .stroke(Theme.gold.opacity(0.7), lineWidth: 1)
                    .frame(width: rect.width + 6, height: rect.height + 6)
                    .offset(x: rect.minX - 3, y: rect.minY - 3)
            }
            if let drag = editor.drag, drag.moved, let item = render.item(at: drag.path), let rect = render.sceneRect(of: item) {
                dragGhost(item: item, rect: rect, drag: drag)
            }
            if let drag = layers.drag, drag.moved {
                selectionGhost(drag)
            }
            if let hover = layers.hover, !doc.selection.contains(hover), editor.drag == nil, layers.drag == nil, editor.effectiveTool == .select,
               let rect = box(of: hover) {
                SelectionOutline(rect: rect, name: layers.label(hover), style: .hover)
            }
            if layers.drag?.moved != true {
                ForEach(Array(doc.selection.enumerated()), id: \.element) { i, target in
                    if let rect = box(of: target) {
                        SelectionOutline(rect: rect, name: tag(for: target, index: i, layers: layers), style: target.layerID == nil ? .item : .layer)
                    }
                }
            }
        }
        .frame(width: geometry.contentSize.width, height: geometry.contentSize.height, alignment: .topLeading)
        .allowsHitTesting(false)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(accessibilitySummary(editor: model.editor))
        .accessibilityHidden(accessibilitySummary(editor: model.editor).isEmpty)
    }

    /// A target's box in view points: the union of its drawn items' boxes.
    private func box(of target: LayerTarget, offset: CGPoint? = nil) -> CGRect? {
        var union: CGRect?
        for path in doc.layerTree.paths(of: target) {
            guard let item = render.item(at: path), let r = render.sceneRect(of: item) else { continue }
            union = union.map { $0.union(r) } ?? r
        }
        guard var u = union else { return nil }
        if let offset { u = u.offsetBy(dx: offset.x, dy: offset.y) }
        return geometry.viewRect(fromScene: u)
    }

    /// The tag over a selected target: "Railway · 12", "pine"; past six
    /// selected, only the first has one.
    private func tag(for target: LayerTarget, index: Int, layers: LayersModel) -> String? {
        if index > 0, doc.selection.count > 6 { return nil }
        switch target {
        case .layer(let id): return "\(doc.layerTree.name(of: id)) · \(doc.layerTree.items(inside: id).count)"
        case .item: return layers.label(target)
        }
    }

    /// Dragging layers or several things: each moved item's art follows at 65%
    /// (up to 300 of them), each target's box moves, and the first tag says how
    /// far ("Railway  +1 col  −2 row", "3 things  +1 col  0 row").
    @ViewBuilder
    private func selectionGhost(_ drag: SelectionDrag) -> some View {
        let arts = model.thumbnails.arts
        if drag.paths.count <= 300 {
            ForEach(drag.paths, id: \.self) { path in
                if let item = render.item(at: path), let rect = render.sceneRect(of: item), let art = arts[item.pose ?? item.object] {
                    let topLeft = geometry.viewPoint(fromScene: art.imageOrigin(forItemBoxOrigin: rect.offsetBy(dx: drag.sceneOffset.x, dy: drag.sceneOffset.y).origin))
                    art.image.view(pointsPerPixel: geometry.zoom, displayScale: displayScale)
                        .opacity(0.65)
                        .offset(x: topLeft.x, y: topLeft.y)
                }
            }
        }
        ForEach(Array(drag.targets.enumerated()), id: \.element) { i, target in
            if let rect = box(of: target, offset: drag.sceneOffset) {
                let what = drag.targets.count > 1 ? "\(drag.targets.count) things" : model.layers.label(target)
                SelectionOutline(rect: rect, name: i == 0 ? "\(what)  \(Self.signed(drag.dc)) col  \(Self.signed(drag.dr)) row" : nil,
                                 style: target.layerID == nil ? .item : .layer)
            }
        }
    }

    /// +1, −2, 0.
    static func signed(_ n: Int) -> String { n > 0 ? "+\(n)" : n < 0 ? "−\(-n)" : "0" }

    /// What VoiceOver reads for the overlay: the selection and where the Place tool would put its object.
    private func accessibilitySummary(editor: SceneEditor) -> String {
        var parts: [String] = []
        if let item = doc.selectedItem {
            parts.append("Selected \(item.id.map { "\(item.object) (\($0))" } ?? item.object) at \(item.at.spoken)")
        } else if !doc.selection.isEmpty {
            parts.append("Selected \(model.layers.selectionLabel ?? "")")
        }
        if let inside = model.layers.inside { parts.append("Inside \(doc.layerTree.name(of: inside))") }
        if let object = editor.dropObject ?? (editor.effectiveTool == .place ? editor.placeObject : nil),
           let tile = editor.hoverTile ?? editor.cursorTile {
            parts.append("Place \(object) at column \(tile.col), row \(tile.row), level \(tile.level)")
        }
        return parts.joined(separator: ". ")
    }

    /// The library object, half see-through, where a click would place it.
    @ViewBuilder
    private func placeGhost(object: String, editor: SceneEditor) -> some View {
        if editor.placeByPixel, let p = editor.hoverPixel {
            let point = CGPoint(x: floor(p.x), y: floor(p.y))
            ghost(object: object, placementPoint: point)
        } else if let tile = editor.hoverTile ?? (editor.dropObject == nil ? editor.cursorTile : nil), let origin = geometry.tileOrigin {
            TileMark(geometry: geometry, tile: tile)
            ghost(object: object, placementPoint: StageGeometry.tileToScene(col: tile.col, row: tile.row, level: tile.level, origin: origin))
        }
    }

    @ViewBuilder
    private func ghost(object: String, placementPoint: CGPoint) -> some View {
        if let art = model.thumbnails.arts[object] {
            let topLeft = geometry.viewPoint(fromScene: art.imageOrigin(forPlacementPoint: placementPoint))
            art.image.view(pointsPerPixel: geometry.zoom, displayScale: displayScale)
                .opacity(0.6)
                .offset(x: topLeft.x, y: topLeft.y)
        } else {
            Color.clear.frame(width: 1, height: 1).onAppear { model.thumbnails.request(object) }
        }
    }

    @ViewBuilder
    private func dragGhost(item: RenderItem, rect: CGRect, drag: ItemDrag) -> some View {
        let moved = rect.offsetBy(dx: drag.sceneOffset.x, dy: drag.sceneOffset.y)
        if let art = model.thumbnails.arts[item.pose ?? item.object] {
            let topLeft = geometry.viewPoint(fromScene: art.imageOrigin(forItemBoxOrigin: moved.origin))
            art.image.view(pointsPerPixel: geometry.zoom, displayScale: displayScale)
                .opacity(0.65)
                .offset(x: topLeft.x, y: topLeft.y)
        }
        let r = geometry.viewRect(fromScene: moved)
        Rectangle()
            .stroke(Theme.accent.opacity(0.7), style: StrokeStyle(lineWidth: 1, dash: [3, 3]))
            .frame(width: r.width, height: r.height)
            .offset(x: r.minX, y: r.minY)
    }
}

/// A tile's top face, outlined: where the Place tool puts the object.
struct TileMark: View {
    var geometry: StageGeometry
    var tile: Tile

    var body: some View {
        if let d = geometry.tileDiamond(tile.col, tile.row, tile.level) {
            Path { p in
                p.move(to: d[0]); p.addLine(to: d[1]); p.addLine(to: d[2]); p.addLine(to: d[3]); p.closeSubpath()
            }
            .fill(Theme.accent.opacity(0.22))
            .overlay(
                Path { p in
                    p.move(to: d[0]); p.addLine(to: d[1]); p.addLine(to: d[2]); p.addLine(to: d[3]); p.closeSubpath()
                }
                .stroke(Theme.halo, lineWidth: 3)
            )
            .overlay(
                Path { p in
                    p.move(to: d[0]); p.addLine(to: d[1]); p.addLine(to: d[2]); p.addLine(to: d[3]); p.closeSubpath()
                }
                .stroke(Theme.accent, style: StrokeStyle(lineWidth: 1.5, dash: [4, 3]))
            )
            .frame(width: geometry.contentSize.width, height: geometry.contentSize.height, alignment: .topLeading)
        }
    }
}

/// An outline a point outside a box, with its name on a small tag above it:
/// a selected layer dashed in the accent, a selected item solid, and what a
/// click would pick dashed and muted with its name on the halo.
struct SelectionOutline: View {
    enum Style { case layer, item, hover }
    var rect: CGRect
    var name: String?
    var style: Style = .item

    var body: some View {
        let r = rect.insetBy(dx: -2, dy: -2)
        let color = style == .hover ? Theme.inkMuted : Theme.accent
        ZStack(alignment: .topLeading) {
            // A dark halo under the line keeps it readable over grass.
            Rectangle()
                .stroke(Theme.halo, lineWidth: style == .hover ? 3 : 3.5)
                .frame(width: r.width, height: r.height)
                .offset(x: r.minX, y: r.minY)
            Rectangle()
                .stroke(color, style: StrokeStyle(lineWidth: style == .hover ? 1 : 1.5, dash: style == .item ? [] : [4, 3]))
                .frame(width: r.width, height: r.height)
                .offset(x: r.minX, y: r.minY)
            if let name {
                Text(name)
                    .font(Theme.nameFont(10.5, weight: style == .hover ? .regular : .semibold))
                    .foregroundStyle(style == .hover ? Theme.ink : Theme.onAccent)
                    .lineLimit(1)
                    .fixedSize()
                    .padding(.horizontal, 6)
                    .frame(height: 17)
                    .background(style == .hover ? Theme.halo : Theme.accent, in: RoundedRectangle(cornerRadius: 3, style: .continuous))
                    .offset(x: r.minX, y: r.minY >= 20 ? r.minY - 20 : r.maxY + 3)
            }
        }
    }
}

/// A proposal on the stage (the Agent window's preview): the proposed doc's
/// render at 60% over the current one, and what it changes outlined in gold:
/// added and moved items solid with a small tag, removed items dashed, and
/// for objects every changed pixel.
struct ProposalGhostLayer: View {
    var geometry: StageGeometry
    var ghost: ProposalGhost
    /// The stage's current render (removed items' boxes are in it).
    var current: StageImage
    var changes: (items: [GhostChange], pixels: [PixelPoint])
    @Environment(\.displayScale) private var displayScale

    var body: some View {
        let image = ghost.image
        let o = geometry.viewPoint(fromScene: image.viewBox.origin)
        ZStack(alignment: .topLeading) {
            image.image.view(pointsPerPixel: geometry.zoom, displayScale: displayScale)
                .opacity(0.6)
                .offset(x: o.x, y: o.y)
            Canvas(opaque: false, rendersAsynchronously: false) { context, _ in
                for p in changes.pixels {
                    let r = geometry.viewRect(fromScene: CGRect(x: CGFloat(p.x), y: CGFloat(p.y), width: 1, height: 1))
                    let inset = r.insetBy(dx: min(1, r.width / 4), dy: min(1, r.height / 4))
                    context.stroke(Path(inset), with: .color(Theme.halo), lineWidth: 2.5)
                    context.stroke(Path(inset), with: .color(Theme.gold), lineWidth: 1.25)
                }
                for change in changes.items {
                    guard let r = rect(change) else { continue }
                    let path = Path(r.insetBy(dx: -2, dy: -2))
                    context.stroke(path, with: .color(Theme.halo), lineWidth: 3.5)
                    context.stroke(path, with: .color(Theme.gold),
                                   style: StrokeStyle(lineWidth: 1.5, dash: change.kind == .removed ? [4, 3] : []))
                }
            }
            .frame(width: geometry.contentSize.width, height: geometry.contentSize.height)
            ForEach(Array(changes.items.enumerated()), id: \.offset) { _, change in
                if let r = rect(change) {
                    Text(change.kind.rawValue)
                        .font(Theme.monoFont(10).weight(.medium))
                        .foregroundStyle(Theme.hexColor("#2E2418"))
                        .fixedSize()
                        .padding(.horizontal, 5)
                        .frame(height: 15)
                        .background(Theme.gold)
                        .offset(x: r.minX - 2, y: max(r.minY - 19, 0))
                }
            }
        }
        .frame(width: geometry.contentSize.width, height: geometry.contentSize.height, alignment: .topLeading)
        .allowsHitTesting(false)
        .accessibilityElement()
        .accessibilityLabel(summary)
    }

    /// A change's box in view points: from the proposed render, or the current one for removed items.
    private func rect(_ change: GhostChange) -> CGRect? {
        guard let b = change.bounds, b.count == 4 else { return nil }
        let image = change.kind == .removed ? current : ghost.image
        let s = CGFloat(max(image.scale, 1))
        return geometry.viewRect(fromScene: CGRect(x: image.viewBox.minX + b[0] / s, y: image.viewBox.minY + b[1] / s, width: b[2] / s, height: b[3] / s))
    }

    /// VoiceOver: what the ghost changes, with the objects named (the tags' text).
    private var summary: String {
        if ghost.ref.kind == .object { return "Proposal ghost: \(changes.pixels.count) pixel\(changes.pixels.count == 1 ? "" : "s") change" }
        let parts = [GhostChange.Kind.added, .moved, .removed].compactMap { k -> String? in
            let objects = changes.items.filter { $0.kind == k }.map(\.object)
            guard !objects.isEmpty else { return nil }
            let names = objects.prefix(5).joined(separator: ", ") + (objects.count > 5 ? " and \(objects.count - 5) more" : "")
            return "\(objects.count) \(k.rawValue): \(names)"
        }
        return "Proposal ghost: " + (parts.isEmpty ? "no item changes" : parts.joined(separator: "; "))
    }
}

/// Over the stage while a proposal is ghosted: who proposed it, and Approve,
/// Reject and Hide (Esc).
struct ProposalBanner: View {
    var agent: AgentModel
    var proposal: ProposalSummary

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "sparkles").foregroundStyle(Theme.gold)
            VStack(alignment: .leading, spacing: 1) {
                Text(title).font(.system(size: 13, weight: .semibold)).foregroundStyle(Theme.ink).lineLimit(1)
                Text(detail).font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(1)
            }
            Spacer(minLength: 12)
            Button("Hide") { agent.setGhostShown(false) }
                .keyboardShortcut(.escape, modifiers: [])
                .help("Hide the ghost (Esc)")
            Button("Reject") { Task { await agent.reject() } }
                .disabled(agent.working != nil)
                .help("Reject the proposal (asks first)")
            Button("Approve") { Task { await agent.approve() } }
                .buttonStyle(AccentButtonStyle())
                .disabled(agent.working != nil)
                .help("Approve: write the proposal to \(proposal.ref?.file ?? "the file")")
        }
        .padding(.leading, 12)
        .padding(.trailing, 8)
        .padding(.vertical, 7)
        .frame(maxWidth: 640)
        .background(Theme.card, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).strokeBorder(Theme.border, lineWidth: 1))
        .shadow(color: .black.opacity(0.18), radius: 8, y: 2)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(title)
    }

    private var title: String { proposal.bannerTitle }

    private var detail: String {
        // "2 edits · pine moved to column 4, …" (docs/intents/2026-10-agent-chat/spec.md, D7).
        var parts = [proposal.editsLabel + (proposal.summary.first.map { " · \($0)\(proposal.summary.count > 1 ? ", …" : "")" } ?? "")]
        if !proposal.baseMatches { parts.append("the file changed since") }
        if let e = agent.ghostError { parts.append("couldn't render: \(e)") }
        if let r = agent.working { parts.append(r) }
        return parts.joined(separator: " · ")
    }
}
