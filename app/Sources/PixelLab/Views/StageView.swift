import SwiftUI

/// The center stage: the open doc's render, nearest-neighbor at a whole zoom,
/// centered and scrollable, over the iso grid (scenes) or a plain card
/// (objects). The art is rendered at scale 1 and zoomed on screen.
struct StageView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.displayScale) private var displayScale
    @FocusState private var focused: Bool

    var body: some View {
        GeometryReader { proxy in
            content(viewport: proxy.size)
        }
        .background(Theme.hero)
        .onGeometryChange(for: CGSize.self, of: { $0.size }) { model.stageViewport = $0 }
        .focusable()
        .focused($focused)
        .focusEffectDisabled()
        .onKeyPress(characters: CharacterSet(charactersIn: "012345"), phases: .down) { press in
            // Plain 1–4 zoom and 0 fits while the stage has focus (the View menu has ⌘1–⌘4, ⌘0).
            guard press.modifiers.isEmpty, let n = Int(press.characters) else { return .ignored }
            if n == 0 { model.zoom = .fit } else if n <= 4 { model.setZoomLevel(index: n - 1) } else { return .ignored }
            return .handled
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
                    if doc.ref.kind == .scene {
                        if model.showGrid {
                            IsoGridLayer(geometry: geometry, hairline: 1 / max(displayScale, 1))
                        }
                    } else {
                        ObjectGround(geometry: geometry)
                    }
                    render.image.view(pointsPerPixel: geometry.zoom, displayScale: displayScale)
                        .offset(x: geometry.artOrigin.x, y: geometry.artOrigin.y)
                        .accessibilityLabel("\(doc.ref.name), \(doc.ref.kind.rawValue) at \(zoom)×")
                    if doc.ref.kind == .object {
                        AnchorMark(geometry: geometry)
                    }
                }
                .frame(width: geometry.contentSize.width, height: geometry.contentSize.height, alignment: .topLeading)
                .contentShape(Rectangle())
                .onTapGesture { focused = true }
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
