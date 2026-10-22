import CoreGraphics
import Foundation
import Testing

@testable import PixelLab

/// docs/intents/2026-10-scene-canvas/spec.md: the stage with no edge (R9).
@Suite struct SceneCanvasStageTests {
    let box = CGRect(x: -16, y: -8, width: 32, height: 16)

    @Test func testR9WorkspaceHoldsTheBoxWithRoomOnEverySide() {
        let pad = StageWindow.pad(viewport: CGSize(width: 1000, height: 700), zoom: 2)
        let w = StageWindow.grow(current: nil, needed: box, pad: pad)
        #expect(w.contains(box.insetBy(dx: -pad, dy: -pad)))
        // Whole multiples of the grain, so a small move doesn't change the window.
        #expect(w.minX.truncatingRemainder(dividingBy: StageWindow.grain) == 0)
        #expect(w.width.truncatingRemainder(dividingBy: StageWindow.grain) == 0)
    }

    @Test func testR9WorkspaceCoversAViewportAtAnyZoom() {
        let viewport = CGSize(width: 1000, height: 700)
        for zoom in 1...4 {
            let pad = StageWindow.pad(viewport: viewport, zoom: CGFloat(zoom))
            let w = StageWindow.grow(current: nil, needed: box, pad: pad)
            // The art at the center can be scrolled until the edge of the window reaches the viewport's far side.
            #expect(w.width * CGFloat(zoom) >= viewport.width, Comment(rawValue: "zoom \(zoom)"))
            #expect(w.height * CGFloat(zoom) >= viewport.height, Comment(rawValue: "zoom \(zoom)"))
        }
    }

    @Test func testR9WorkspaceGrowsTowardAnEdgeAndNeverShrinks() {
        let pad: CGFloat = 256
        let first = StageWindow.grow(current: nil, needed: box, pad: pad)
        // Content well inside: the same window, so nothing moves under a drag.
        #expect(StageWindow.grow(current: first, needed: box.offsetBy(dx: 40, dy: 20), pad: pad) == first)
        // Content toward the right edge: the window grows there and keeps what it had.
        let far = box.offsetBy(dx: first.maxX - 20, dy: 0)
        let grown = StageWindow.grow(current: first, needed: far, pad: pad)
        #expect(grown.contains(first))
        #expect(grown.contains(far.insetBy(dx: -pad, dy: -pad)))
        // The content comes back: the window stays.
        #expect(StageWindow.grow(current: grown, needed: box, pad: pad) == grown)
    }

    @Test func testR9SceneStageIsAtLeastTheViewportAndCenteredWhenSmaller() {
        let window = CGRect(x: -400, y: -300, width: 800, height: 600)
        let big = StageGeometry(window: window, zoom: 2, viewport: CGSize(width: 1000, height: 700), tileOrigin: .zero)
        #expect(big.contentSize == CGSize(width: 1600, height: 1200))
        #expect(big.artOrigin == .zero)
        let small = StageGeometry(window: CGRect(x: 0, y: 0, width: 100, height: 100), zoom: 1, viewport: CGSize(width: 1000, height: 700), tileOrigin: .zero)
        #expect(small.contentSize == CGSize(width: 1000, height: 700))
        #expect(small.artOrigin == CGPoint(x: 450, y: 300))
    }

    @Test func testR9HitTestingAndPlacingNeedNoArtUnderThePointer() {
        // A point far outside any art: the tile under it follows from the tile origin alone.
        let window = CGRect(x: -2000, y: -1000, width: 4000, height: 2000)
        let g = StageGeometry(window: window, zoom: 1, viewport: CGSize(width: 800, height: 600), tileOrigin: .zero)
        let center = g.viewPoint(ofTile: 50, -30, 0)
        #expect(center != nil)
        let t = g.tile(atView: center!)
        #expect(t?.col == 50)
        #expect(t?.row == -30)
        #expect(t?.level == 0)
        // And back: the diamond of that tile is on the stage.
        let d = g.tileDiamond(50, -30, 0)
        #expect(d?.count == 4)
    }

    @Test func testR9SceneViewParamIsWholePixels() {
        let v = CGRect(x: -1000, y: -500, width: 2048, height: 1024).sceneView
        #expect(v == [JSONValue(-1000), JSONValue(-500), JSONValue(2048), JSONValue(1024)])
    }

    @Test func testR7RenderFrameDecodesANumberAndAWord() throws {
        let n = try JSONDecoder().decode(RenderFrame.self, from: Data("3".utf8))
        #expect(n.number == 3)
        let w = try JSONDecoder().decode(RenderFrame.self, from: Data("\"fit\"".utf8))
        #expect(w.kind == "fit")
    }
}
