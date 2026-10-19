import SwiftUI

/// What the stage draws over the art in scene mode: the Place tool's tile
/// and ghost, a dragged item's ghost, and the selection (board 03: the
/// accent, square and dashed, with a small name tag).
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
            if let drag = editor.drag, drag.moved, let item = render.item(at: drag.path), let rect = render.sceneRect(of: item) {
                dragGhost(item: item, rect: rect, drag: drag)
            }
            if let path = doc.selectedPath, let item = render.item(at: path), let rect = render.sceneRect(of: item) {
                SelectionOutline(rect: geometry.viewRect(fromScene: rect), name: doc.selectedItem?.id ?? item.object)
            }
        }
        .frame(width: geometry.contentSize.width, height: geometry.contentSize.height, alignment: .topLeading)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
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

/// The selection: a square, dashed accent outline a point outside the item's
/// box, and its name on a small accent tag above it.
struct SelectionOutline: View {
    var rect: CGRect
    var name: String

    var body: some View {
        let r = rect.insetBy(dx: -2, dy: -2)
        ZStack(alignment: .topLeading) {
            // A dark halo under the dashes keeps them readable over grass.
            Rectangle()
                .stroke(Theme.halo, lineWidth: 3.5)
                .frame(width: r.width, height: r.height)
                .offset(x: r.minX, y: r.minY)
            Rectangle()
                .stroke(Theme.accent, style: StrokeStyle(lineWidth: 1.5, dash: [4, 3]))
                .frame(width: r.width, height: r.height)
                .offset(x: r.minX, y: r.minY)
            Text(name)
                .font(Theme.monoFont(10).weight(.medium))
                .foregroundStyle(Theme.onAccent)
                .lineLimit(1)
                .fixedSize()
                .padding(.horizontal, 5)
                .frame(height: 16)
                .background(Theme.accent)
                .offset(x: r.minX, y: max(r.minY - 18, 0))
        }
    }
}
