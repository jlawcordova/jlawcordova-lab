import SwiftUI

/// The center stage: the open doc's render, nearest-neighbor at a whole zoom,
/// centered and scrollable, over the iso grid (scenes) or a plain card
/// (objects). The art is rendered at scale 1 and zoomed on screen.
struct StageView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.displayScale) private var displayScale

    var body: some View {
        GeometryReader { proxy in
            content(viewport: proxy.size)
        }
        .background(Theme.hero)
        .onGeometryChange(for: CGSize.self, of: { $0.size }) { model.stageViewport = $0 }
        // Visible focus: the stage takes keys (tools, arrows, ⌫, Return, 1–4, 0).
        .overlay {
            Rectangle()
                .strokeBorder(Theme.accent.opacity(model.editor.stageFocused ? 0.55 : 0), lineWidth: 2)
                .allowsHitTesting(false)
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Stage")
    }

    @ViewBuilder
    private func content(viewport: CGSize) -> some View {
        if let doc = model.document, let render = doc.render {
            let zoom = model.effectiveZoom
            let geometry = StageGeometry(
                viewBox: render.viewBox, zoom: zoom, viewport: viewport,
                margin: AppModel.stageMargin, tileOrigin: doc.ref.kind == .scene ? doc.origin : nil
            )
            ScrollView([.horizontal, .vertical]) {
                ZStack(alignment: .topLeading) {
                    // The input view sits at the bottom: everything drawn over it
                    // lets the pointer through, and AppKit-hosted views drawn
                    // under SwiftUI layers keep the art and overlays in order
                    // (on screen and in cacheDisplay snapshots).
                    StageInput(geometry: geometry, editor: model.editor, objects: model.objectEditor, tool: model.editor.effectiveTool,
                               paintTool: model.objectEditor.tool, isScene: doc.ref.kind == .scene)
                        .frame(width: geometry.contentSize.width, height: geometry.contentSize.height)
                    if doc.ref.kind == .scene {
                        if model.showGrid {
                            IsoGridLayer(geometry: geometry, hairline: 1 / max(displayScale, 1))
                        }
                    } else {
                        ObjectGround(geometry: geometry)
                        if let onion = model.objectEditor.onionImage {
                            // The onion skin: the layer's previous frame at 30%.
                            let o = geometry.viewPoint(fromScene: onion.viewBox.origin)
                            onion.image.view(pointsPerPixel: geometry.zoom, displayScale: displayScale)
                                .opacity(0.3)
                                .offset(x: o.x, y: o.y)
                                .allowsHitTesting(false)
                                .accessibilityHidden(true)
                        }
                    }
                    render.image.view(pointsPerPixel: geometry.zoom, displayScale: displayScale)
                        .offset(x: geometry.artOrigin.x, y: geometry.artOrigin.y)
                        .allowsHitTesting(false)
                        .accessibilityLabel("\(doc.ref.name), \(doc.ref.kind.rawValue) at \(zoom)×")
                    if doc.ref.kind == .object {
                        ObjectStageOverlay(geometry: geometry, objects: model.objectEditor, focused: model.editor.stageFocused,
                                           hairline: 1 / max(displayScale, 1))
                        if !model.objectEditor.isBlock { AnchorMark(geometry: geometry) }
                    } else {
                        StageOverlay(geometry: geometry, doc: doc, render: render)
                    }
                }
                .frame(width: geometry.contentSize.width, height: geometry.contentSize.height, alignment: .topLeading)
            }
            .scrollIndicators(.automatic)
            .defaultScrollAnchor(.center)
            .id(doc.ref)
        } else if let doc = model.document, let error = doc.error {
            ContentUnavailableView {
                Label("Couldn't render \(doc.ref.name)", systemImage: "exclamationmark.triangle")
            } description: {
                Text(error).font(Theme.monoFont(11))
            }
            .foregroundStyle(Theme.ink)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if model.document != nil {
            ProgressView().controlSize(.small).frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            ContentUnavailableView("No document", systemImage: "square.stack.3d.up", description: Text("Choose a scene or an object in the sidebar."))
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }
}

/// The level-0 iso lattice (32×16 diamonds at the art's 2:1 slope), aligned to
/// the scene's tile origin, across the whole stage.
struct IsoGridLayer: View {
    var geometry: StageGeometry
    var hairline: CGFloat

    var body: some View {
        Canvas(opaque: false, rendersAsynchronously: false) { context, size in
            var path = Path()
            for (a, b) in geometry.gridSegments(in: CGRect(origin: .zero, size: size)) {
                path.move(to: a)
                path.addLine(to: b)
            }
            context.stroke(path, with: .color(Theme.grid), lineWidth: hairline)
        }
        .frame(width: geometry.contentSize.width, height: geometry.contentSize.height)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// An object's plain card ground.
struct ObjectGround: View {
    var geometry: StageGeometry

    var body: some View {
        let pad = max(geometry.zoom * 2, 12)
        let card = geometry.artRect.insetBy(dx: -pad, dy: -pad)
        ZStack(alignment: .topLeading) {
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(Theme.card)
                .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).strokeBorder(Theme.border, lineWidth: 1))
                .frame(width: card.width, height: card.height)
                .offset(x: card.minX, y: card.minY)
        }
        .frame(width: geometry.contentSize.width, height: geometry.contentSize.height, alignment: .topLeading)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// A small gold cross at an object's anchor: (0, 0) in its render
/// coordinates, the point a scene places it at.
struct AnchorMark: View {
    var geometry: StageGeometry

    var body: some View {
        let p = geometry.viewPoint(fromScene: .zero)
        Path { path in
            path.move(to: CGPoint(x: p.x - 7, y: p.y)); path.addLine(to: CGPoint(x: p.x + 7, y: p.y))
            path.move(to: CGPoint(x: p.x, y: p.y - 7)); path.addLine(to: CGPoint(x: p.x, y: p.y + 7))
        }
        .stroke(Theme.gold, lineWidth: 1.5)
        .frame(width: geometry.contentSize.width, height: geometry.contentSize.height, alignment: .topLeading)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// What the stage draws over an object: pixels painted but not rendered yet
/// (so a stroke shows at once), the pixel grid from 8×, the dashed bounds and
/// the pixel under the pointer (or the keyboard cursor).
struct ObjectStageOverlay: View {
    var geometry: StageGeometry
    var objects: ObjectEditor
    var focused: Bool
    var hairline: CGFloat
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let overlay = objects.visibleOverlay
        let grid = objects.pixelGrid && geometry.zoom >= 8
        let mark = objects.hover ?? (focused ? objects.cursor : nil)
        let art = geometry.artRect
        Canvas(opaque: false, rendersAsynchronously: false) { context, _ in
            for (p, hex) in overlay {
                let r = StrokeMath.viewRect(of: p, geometry: geometry)
                context.fill(Path(r), with: .color(hex.map(Theme.hexColor) ?? Theme.card))
            }
            if grid {
                var path = Path()
                let w = Int(geometry.viewBox.width), h = Int(geometry.viewBox.height)
                for x in 1..<max(w, 1) {
                    let vx = art.minX + CGFloat(x) * geometry.zoom
                    path.move(to: CGPoint(x: vx, y: art.minY))
                    path.addLine(to: CGPoint(x: vx, y: art.maxY))
                }
                for y in 1..<max(h, 1) {
                    let vy = art.minY + CGFloat(y) * geometry.zoom
                    path.move(to: CGPoint(x: art.minX, y: vy))
                    path.addLine(to: CGPoint(x: art.maxX, y: vy))
                }
                context.stroke(path, with: .color(Theme.ink.opacity(0.15)), lineWidth: hairline)
            }
            context.stroke(Path(art.insetBy(dx: -0.5, dy: -0.5)), with: .color(Theme.ink.opacity(0.35)),
                           style: StrokeStyle(lineWidth: 1, dash: [4, 3]))
            if let mark {
                let r = StrokeMath.viewRect(of: mark, geometry: geometry).insetBy(dx: 1, dy: 1)
                context.stroke(Path(r), with: .color(Theme.halo), lineWidth: 3)
                context.stroke(Path(r), with: .color(Theme.ink), lineWidth: 1.5)
            }
        }
        .frame(width: geometry.contentSize.width, height: geometry.contentSize.height)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}
