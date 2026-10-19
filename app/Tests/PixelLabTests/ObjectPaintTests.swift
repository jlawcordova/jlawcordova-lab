import CoreGraphics
import Foundation
import Testing

@testable import PixelLab

@Suite struct StrokeMathTests {
    /// Every step of a line moves to one of the 8 neighbours: no gaps, no repeats.
    private func isConnected(_ pts: [PixelPoint]) -> Bool {
        zip(pts, pts.dropFirst()).allSatisfy { a, b in
            let dx = abs(a.x - b.x), dy = abs(a.y - b.y)
            return max(dx, dy) == 1
        }
    }

    @Test func testLineEndsAndSinglePixel() {
        #expect(StrokeMath.line(PixelPoint(3, 4), PixelPoint(3, 4)) == [PixelPoint(3, 4)])
        let h = StrokeMath.line(PixelPoint(0, 0), PixelPoint(4, 0))
        #expect(h == (0...4).map { PixelPoint($0, 0) })
        let v = StrokeMath.line(PixelPoint(2, 5), PixelPoint(2, 1))
        #expect(v == (1...5).reversed().map { PixelPoint(2, $0) })
        let d = StrokeMath.line(PixelPoint(0, 0), PixelPoint(3, 3))
        #expect(d == [PixelPoint(0, 0), PixelPoint(1, 1), PixelPoint(2, 2), PixelPoint(3, 3)])
    }

    @Test func testFastDragsLeaveNoGaps() {
        let cases: [(PixelPoint, PixelPoint)] = [
            (PixelPoint(0, 0), PixelPoint(17, 5)), (PixelPoint(2, 30), PixelPoint(9, 1)),
            (PixelPoint(-4, 6), PixelPoint(40, -3)), (PixelPoint(10, 10), PixelPoint(-7, 23)),
        ]
        for (a, b) in cases {
            let pts = StrokeMath.line(a, b)
            #expect(pts.first == a)
            #expect(pts.last == b)
            #expect(isConnected(pts), "\(a) → \(b)")
            #expect(pts.count == max(abs(b.x - a.x), abs(b.y - a.y)) + 1)
        }
    }

    @Test func testLineMatchesTheLabsBresenham() {
        // object-mode.ts line([0, 0], [5, 2]).
        #expect(StrokeMath.line(PixelPoint(0, 0), PixelPoint(5, 2)) == [
            PixelPoint(0, 0), PixelPoint(1, 0), PixelPoint(2, 1), PixelPoint(3, 1), PixelPoint(4, 2), PixelPoint(5, 2),
        ])
    }

    @Test func testSpritePixelMapping() {
        // A 46×42 sprite anchored at (21, 41): its viewBox starts at (-21, -41).
        let g = StageGeometry(viewBox: CGRect(x: -21, y: -41, width: 46, height: 42), zoom: 8, viewport: CGSize(width: 800, height: 600), margin: 32, tileOrigin: nil)
        // Map column 0, row 0 is the art's top-left.
        #expect(StrokeMath.pixel(atView: CGPoint(x: g.artOrigin.x + 0.5, y: g.artOrigin.y + 0.5), geometry: g) == PixelPoint(0, 0))
        #expect(StrokeMath.pixel(atView: CGPoint(x: g.artOrigin.x + 8 * 10 + 7.9, y: g.artOrigin.y + 8 * 3), geometry: g) == PixelPoint(10, 3))
        #expect(StrokeMath.pixel(atView: CGPoint(x: g.artOrigin.x - 1, y: g.artOrigin.y), geometry: g) == nil)
        #expect(StrokeMath.pixel(atView: CGPoint(x: g.artOrigin.x + 46 * 8, y: g.artOrigin.y), geometry: g) == nil)
        #expect(StrokeMath.unclampedPixel(atView: CGPoint(x: g.artOrigin.x - 1, y: g.artOrigin.y), geometry: g) == PixelPoint(-1, 0))
        // Round trip: a pixel's square holds the points that map to it.
        for p in [PixelPoint(0, 0), PixelPoint(45, 41), PixelPoint(21, 41)] {
            let r = StrokeMath.viewRect(of: p, geometry: g)
            #expect(r.size == CGSize(width: 8, height: 8))
            #expect(StrokeMath.pixel(atView: CGPoint(x: r.midX, y: r.midY), geometry: g) == p)
        }
    }

    @Test func testBlockPixelMappingUsesTheRenderBox() {
        // A block's render: the box around its pixels, from its first tile's center.
        let g = StageGeometry(viewBox: CGRect(x: -15, y: -8, width: 30, height: 32), zoom: 4, viewport: CGSize(width: 400, height: 400), margin: 32, tileOrigin: nil)
        // Scene pixel (0, 0), the tile's center, is render pixel (15, 8).
        let center = g.viewPoint(fromScene: CGPoint(x: 0.5, y: 0.5))
        #expect(StrokeMath.pixel(atView: center, geometry: g) == PixelPoint(15, 8))
    }

    @Test func testClipKeepsOrderAndDropsOutsideAndRepeats() {
        let pts = [PixelPoint(-1, 0), PixelPoint(0, 0), PixelPoint(1, 0), PixelPoint(0, 0), PixelPoint(4, 0), PixelPoint(3, 2)]
        #expect(StrokeMath.clip(pts, size: CGSize(width: 4, height: 3)) == [PixelPoint(0, 0), PixelPoint(1, 0), PixelPoint(3, 2)])
    }

    @Test func testBatchGathersEachPixelOnce() {
        var b = StrokeBatch()
        #expect(b.isEmpty)
        b.add([PixelPoint(1, 1), PixelPoint(2, 1)])
        b.add([PixelPoint(2, 1), PixelPoint(3, 1)])
        #expect(b.take() == [PixelPoint(1, 1), PixelPoint(2, 1), PixelPoint(3, 1)])
        #expect(b.isEmpty)
        b.add([PixelPoint(2, 1)])
        #expect(b.take() == [PixelPoint(2, 1)])
    }

    @Test func testPixelPointSpec() {
        #expect(PixelPoint(spec: "3,4") == PixelPoint(3, 4))
        #expect(PixelPoint(spec: "-2,10") == PixelPoint(-2, 10))
        #expect(PixelPoint(spec: "3") == nil)
        #expect(PixelPoint(3, 4).json == [3, 4])
    }
}

@Suite struct ObjectRulesTests {
    @Test func testNewDocNames() {
        let taken: Set = ["house", "hero-island"]
        #expect(NewDocRules.nameProblem("small-rock", taken: taken) == nil)
        #expect(NewDocRules.nameProblem("rock2", taken: taken) == nil)
        #expect(NewDocRules.nameProblem("", taken: taken) != nil)
        for bad in ["Rock", "small_rock", "small--rock", "-rock", "rock-", "2rock", "small rock"] {
            #expect(NewDocRules.nameProblem(bad, taken: taken)?.hasPrefix("Use lowercase") == true, "\(bad)")
        }
        #expect(NewDocRules.nameProblem("house", taken: taken) == "There's already an object or scene called house.")
        #expect(NewDocRules.nameProblem(String(repeating: "a", count: 65), taken: taken)?.contains("65 characters") == true)
        #expect(NewDocRules.sizeProblem(width: "16", height: "64") == nil)
        #expect(NewDocRules.sizeProblem(width: "0", height: "8") != nil)
        #expect(NewDocRules.sizeProblem(width: "65", height: "8") != nil)
        #expect(NewDocRules.sizeProblem(width: "x", height: "8") != nil)
    }

    @MainActor @Test func testNewDocCommands() {
        let scene = NewDocDraft(kind: .scene)
        scene.name = " meadow "
        #expect(scene.command.method == "scene.new")
        #expect(scene.command.params == ["name": "meadow"])
        let sprite = NewDocDraft(kind: .object)
        sprite.name = "small-rock"
        sprite.width = "12"
        sprite.height = "8"
        #expect(sprite.command.params == ["name": "small-rock", "kind": "sprite", "width": 12, "height": 8])
        sprite.extendsCharacter = true
        #expect(sprite.command.params == ["name": "small-rock", "kind": "sprite", "extends": "character"])
        sprite.width = "0"
        #expect(sprite.problem(taken: []) == nil, "an outfit's size isn't asked")
        let block = NewDocDraft(kind: .object)
        block.name = "tile-sand"
        block.objectKind = .block
        block.width = "999"
        #expect(block.problem(taken: []) == nil)
        #expect(block.command.params == ["name": "tile-sand", "kind": "block"])
    }

    @Test func testLoopTiming() {
        // frames-panel.ts play: CYCLE / frames (LOOP_CYCLES ff 600, wf 1000, hf 3000).
        #expect(FrameLoop.step(loop: "ff", count: 4) == 150)
        #expect(FrameLoop.step(loop: "wf", count: 5) == 200)
        #expect(FrameLoop.step(loop: "hf", count: 6) == 500)
        #expect(FrameLoop.step(loop: "zz", count: 3) == 200)
        #expect(FrameLoop.ff.title == "ff · 4 frames in 0.6 s")
        #expect(FrameLoop.hf.title == "hf · 6 frames in 3 s")
    }

    @Test func testTileShapeIsTheIsoDiamond() {
        // iso.mjs tilePixels(0, 0): a 32×16 diamond, half the box.
        #expect(TileShape.pixels.count == 256)
        let inside = Set(TileShape.pixels)
        #expect(inside.contains(PixelPoint(0, 0)) && inside.contains(PixelPoint(-15, 0)) && inside.contains(PixelPoint(14, 0)) && inside.contains(PixelPoint(-1, -8)))
        #expect(!inside.contains(PixelPoint(-16, 0)) && !inside.contains(PixelPoint(15, 7)))
        #expect(TileShape.pixels.allSatisfy { $0.x >= -16 && $0.x < 16 && $0.y >= -8 && $0.y < 8 })
        #expect(!TileShape.outline.isEmpty && TileShape.outline.count < TileShape.pixels.count)
    }

    @Test func testPixelColorsHex() {
        let c = PixelColors(width: 2, height: 1, rgba: [0x2E, 0x24, 0x18, 255, 0, 0, 0, 0])
        #expect(c.hex(PixelPoint(0, 0)) == "#2E2418")
        #expect(c.hex(PixelPoint(1, 0)) == nil)
        #expect(c.hex(PixelPoint(2, 0)) == nil)
    }
}
