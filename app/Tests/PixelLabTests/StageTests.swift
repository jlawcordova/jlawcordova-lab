import CoreGraphics
import Foundation
import ImageIO
import Testing

@testable import PixelLab

@Suite struct StageGeometryTests {
    let origin = CGPoint(x: 0, y: 8)

    @Test func testTileToSceneMatchesIsoMjs() {
        // iso.mjs tileToPx([14, 6, 1], [0, 8]) = [(14-6)*16, (14+6)*8 - 16 + 8]
        #expect(StageGeometry.tileToScene(col: 14, row: 6, level: 1, origin: origin) == CGPoint(x: 128, y: 152))
        #expect(StageGeometry.tileToScene(col: 0, row: 0, level: 0, origin: origin) == origin)
        #expect(StageGeometry.tileToScene(col: -2, row: 3, level: 0, origin: .zero) == CGPoint(x: -80, y: 8))
    }

    @Test func testEveryTopFacePixelMapsBackToItsTile() {
        for level in [0, 1, 3] {
            for col in -3...3 {
                for row in -3...3 {
                    let c = StageGeometry.tileToScene(col: col, row: row, level: level, origin: origin)
                    // Pixels whose centers are inside the top face: |dx|/16 + |dy|/8 < 1.
                    for y in Int(c.y) - 8..<Int(c.y) + 8 {
                        for x in Int(c.x) - 16..<Int(c.x) + 16 {
                            let dx = abs(CGFloat(x) + 0.5 - c.x), dy = abs(CGFloat(y) + 0.5 - c.y)
                            guard dx / 16 + dy / 8 < 1 else { continue }
                            let t = StageGeometry.sceneToTile(x: x, y: y, level: level, origin: origin)
                            #expect([t.col, t.row, t.level] == [col, row, level], Comment(rawValue: "pixel \(x),\(y)"))
                        }
                    }
                }
            }
        }
    }

    @Test func testViewSceneRoundTripAndCentering() {
        let g = StageGeometry(viewBox: CGRect(x: -182, y: -92, width: 446, height: 264), zoom: 2, viewport: CGSize(width: 1000, height: 700), margin: 32, tileOrigin: origin)
        #expect(g.contentSize == CGSize(width: 1000, height: 700))
        #expect(g.artOrigin == CGPoint(x: 54, y: 86))
        let p = CGPoint(x: 300.5, y: 211)
        let s = g.scenePoint(fromView: p)
        #expect(abs(g.viewPoint(fromScene: s).x - p.x) < 1e-9)
        #expect(abs(g.viewPoint(fromScene: s).y - p.y) < 1e-9)
        #expect(g.viewPoint(fromScene: CGPoint(x: -182, y: -92)) == g.artOrigin)
        let px = g.scenePixel(fromView: CGPoint(x: 55.9, y: 87.9))
        #expect(px.x == -182)
        #expect(px.y == -92)
    }

    @Test func testArtBiggerThanViewportScrollsWithMargin() {
        let g = StageGeometry(viewBox: CGRect(x: 0, y: 0, width: 400, height: 300), zoom: 4, viewport: CGSize(width: 800, height: 600), margin: 32, tileOrigin: nil)
        #expect(g.contentSize == CGSize(width: 1664, height: 1264))
        #expect(g.artOrigin == CGPoint(x: 32, y: 32))
        #expect((g.tile(atView: .zero)) == nil)
        #expect(g.gridSegments(in: CGRect(origin: .zero, size: g.contentSize)).isEmpty)
    }

    @Test func testOutputBoundsToViewRect() {
        let g = StageGeometry(viewBox: CGRect(x: -10, y: -20, width: 100, height: 100), zoom: 3, viewport: CGSize(width: 300, height: 300), margin: 0, tileOrigin: nil)
        // Bounds at render scale 2: [x, y, w, h] = [20, 40, 8, 6] → scene (0, 0, 4, 3).
        let r = g.viewRect(fromOutputBounds: [20, 40, 8, 6], scale: 2)
        #expect(r == CGRect(x: 30, y: 60, width: 12, height: 9))
        #expect((g.viewRect(fromOutputBounds: [1, 2, 3], scale: 1)) == nil)
    }

    @Test func testTileUnderViewPointAndDiamond() {
        let g = StageGeometry(viewBox: CGRect(x: -64, y: -32, width: 128, height: 128), zoom: 2, viewport: CGSize(width: 256, height: 256), margin: 0, tileOrigin: origin)
        let center = g.viewPoint(ofTile: 2, 1, 1)!
        let t = g.tile(atView: center, level: 1)!
        #expect([t.col, t.row, t.level] == [Int]([2, 1, 1]))
        let d = g.tileDiamond(2, 1, 1)!
        #expect(d[0] == CGPoint(x: center.x, y: center.y - 16))
        #expect(d[1] == CGPoint(x: center.x + 32, y: center.y))
    }

    @Test func testGridLinesPassThroughTileCorners() {
        let g = StageGeometry(viewBox: CGRect(x: -64, y: -32, width: 128, height: 128), zoom: 1, viewport: CGSize(width: 128, height: 128), margin: 0, tileOrigin: origin)
        let segments = g.gridSegments(in: CGRect(x: 0, y: 0, width: 128, height: 128))
        #expect(!(segments.isEmpty))
        // The left corner of tile [0, 0, 0]'s top face lies on one of the lines.
        let corner = g.tileDiamond(0, 0, 0)![3]
        let onLine = segments.contains { a, b in
            let cross = (b.x - a.x) * (corner.y - a.y) - (b.y - a.y) * (corner.x - a.x)
            return abs(cross) / hypot(b.x - a.x, b.y - a.y) < 0.01
        }
        #expect(onLine)
    }
}

@Suite struct HitTestingTests {
    /// A 4×4 mask, opaque only in its left half.
    let leftHalf = AlphaMask(width: 4, height: 4, alpha: (0..<16).map { $0 % 4 < 2 ? 255 : 0 })

    @Test func testTopmostCandidateFirst() {
        let items = [
            HitTester.Candidate(path: "0", rect: CGRect(x: 0, y: 0, width: 10, height: 10), object: "block"),
            HitTester.Candidate(path: "1", rect: CGRect(x: 5, y: 5, width: 10, height: 10), object: "pine"),
            HitTester.Candidate(path: "2", rect: CGRect(x: 50, y: 50, width: 4, height: 4), object: "far"),
        ]
        #expect(HitTester.candidates(atPixel: 6, 6, in: items).map(\.path) == ["1", "0"])
        #expect(HitTester.pick(atPixel: 6, 6, in: items) { _, _, _ in true } == "1")
        #expect((HitTester.pick(atPixel: 30, 30, in: items) { _, _, _ in true }) == nil)
    }

    @Test func testTransparentPixelFallsThroughToTheItemBehind() {
        let items = [
            HitTester.Candidate(path: "back", rect: CGRect(x: 0, y: 0, width: 4, height: 4), object: "block"),
            HitTester.Candidate(path: "front", rect: CGRect(x: 0, y: 0, width: 4, height: 4), object: "tree"),
        ]
        let masks = ["block": AlphaMask(width: 4, height: 4, alpha: Array(repeating: 255, count: 16)), "tree": leftHalf]
        let pick = { (x: Int, y: Int) in
            HitTester.pick(atPixel: x, y, in: items) { c, dx, dy in masks[c.object]?.isOpaque(x: dx, y: dy) }
        }
        #expect(pick(1, 1) == "front")
        #expect(pick(3, 1) == "back")
    }

    @Test func testUnknownArtCountsItsBox() {
        let items = [HitTester.Candidate(path: "x", rect: CGRect(x: 0, y: 0, width: 4, height: 4), object: "x")]
        #expect(HitTester.pick(atPixel: 3, 3, in: items) { _, _, _ in nil } == "x")
        #expect((HitTester.pick(atPixel: 3, 3, in: items) { _, _, _ in false }) == nil)
    }

    @Test func testAlphaMaskFromImage() throws {
        let ctx = try #require(CGContext(data: nil, width: 3, height: 2, bitsPerComponent: 8, bytesPerRow: 0,
                                          space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.setFillColor(CGColor(red: 1, green: 0, blue: 0, alpha: 1))
        // CG's origin is bottom-left: this fills the top row's first pixel.
        ctx.fill(CGRect(x: 0, y: 1, width: 1, height: 1))
        let image = try #require(ctx.makeImage())
        let mask = try #require(AlphaMask(cgImage: image))
        #expect(mask.width == 3)
        #expect(mask.isOpaque(x: 0, y: 0))
        #expect(!(mask.isOpaque(x: 1, y: 0)))
        #expect(!(mask.isOpaque(x: 0, y: 1)))
        #expect(!(mask.isOpaque(x: -1, y: 0)))
    }

    @Test func testObjectArtMapsObjectPixelsThroughScaleAndViewBox() throws {
        // A sprite 2×2 anchored at (1, 2): viewBox [-1, -2, 2, 2], rendered at scale 2.
        let mask = AlphaMask(width: 4, height: 4, alpha: [
            255, 255, 0, 0,
            255, 255, 0, 0,
            0, 0, 0, 0,
            0, 0, 0, 0,
        ])
        let png = try #require(Self.png(width: 4, height: 4))
        let pixels = try #require(PixelImage(png: png, renderScale: 2))
        let art = ObjectArt(image: pixels, viewBox: CGRect(x: -1, y: -2, width: 2, height: 2),
                            box: CGRect(x: -1, y: -2, width: 1, height: 1), scale: 2, mask: mask)
        #expect(art.isOpaque(objectX: -1, objectY: -2))
        #expect(!(art.isOpaque(objectX: 0, objectY: -2)))
        #expect(!(art.isOpaque(objectX: -1, objectY: -1)))
        // Placed with its anchor at (10, 20), the PNG's top-left is at (9, 18).
        #expect(art.imageOrigin(forPlacementPoint: CGPoint(x: 10, y: 20)) == CGPoint(x: 9, y: 18))
        #expect(art.imageOrigin(forItemBoxOrigin: CGPoint(x: 9, y: 18)) == CGPoint(x: 9, y: 18))
    }

    static func png(width: Int, height: Int) -> Data? {
        guard let ctx = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue),
              let image = ctx.makeImage() else { return nil }
        let data = NSMutableData()
        guard let dest = CGImageDestinationCreateWithData(data, "public.png" as CFString, 1, nil) else { return nil }
        CGImageDestinationAddImage(dest, image, nil)
        guard CGImageDestinationFinalize(dest) else { return nil }
        return data as Data
    }
}

@Suite struct EditingLogicTests {
    @Test func testProblemLocator() {
        let items = [
            SceneItemInfo(path: "5.2", object: "pine", at: ItemAt(tile: [1, 2, 0])),
            SceneItemInfo(path: "7", object: "crane-jib", at: ItemAt(px: [3, 4]), id: "jib"),
        ]
        #expect(ProblemLocator.path(in: "scenes/x.json: items[5].items[2]: object \"nope\" does not exist") == "5.2")
        #expect(ProblemLocator.path(in: "scenes/x.json: items[12]: at must be") == "12")
        #expect((ProblemLocator.path(in: "scenes/x.json: viewBox must be four whole numbers")) == nil)
        #expect(ProblemLocator.locate("scenes/x.json: items[5].items[2]: class must be", items: items) == "5.2")
        #expect(ProblemLocator.locate("scenes/x.json: motion: track \"jib\": keys must be in order", items: items) == "7")
        #expect((ProblemLocator.locate("scenes/x.json: items[9]: nope", items: items)) == nil)
    }

    @Test func testKeySpecs() {
        #expect(KeyInput(spec: "opt+up") == KeyInput(key: .up, option: true))
        #expect(KeyInput(spec: "shift+down") == KeyInput(key: .down, shift: true))
        #expect(KeyInput(spec: "cmd+d") == KeyInput(key: .char("d"), command: true))
        #expect(KeyInput(spec: "delete")?.key == .delete)
        #expect(KeyInput(spec: "]")?.key == .char("]"))
        #expect((KeyInput(spec: "hyper+x")) == nil)
    }

    @Test func testDragOffsets() {
        var d = ItemDrag(path: "1", startView: .zero, startScene: .zero, byPixel: false)
        d.dc = 1
        d.dr = 0
        #expect(d.sceneOffset == CGPoint(x: 16, y: 8))
        d.dc = 1
        d.dr = 1
        #expect(d.sceneOffset == CGPoint(x: 0, y: 16))
        d.byPixel = true
        d.dx = -3
        d.dy = 2
        #expect(d.sceneOffset == CGPoint(x: -3, y: 2))
        #expect(!(d.isZero))
    }
}

/// Ambient loops on the timeline, and ACTIONS (docs/intents/2026-10-rust-engine/spec.md, R23, D10).
@Suite @MainActor struct AmbientAndActionTests {
    /// A `scene.show --doc` result.
    private func scene(_ json: String) throws -> SceneShowMotion {
        try JSONDecoder().decode(SceneShowMotion.self, from: Data(json.utf8))
    }

    private let cloud = #"{ "object": "cloud", "at": { "tile": [0, 0, 0] }, "ambient": { "duration": 16000, "keys": [{ "t": 0, "move": [0, 0] }] } }"#
    private let frog = #"{ "object": "frog", "at": { "tile": [1, 0, 0] }, "actions": { "hop": { "duration": 480, "keys": [] }, "croak": { "duration": 1500, "keys": [] } } }"#

    @Test func testR23TimelineLengthIsTheLongestOfMotionAndAmbientLoops() {
        #expect(Animation.timelineLength(motion: nil, ambient: []) == nil)
        #expect(Animation.timelineLength(motion: 3000, ambient: []) == 3000)
        #expect(Animation.timelineLength(motion: nil, ambient: [8000, 16000]) == 16000)
        #expect(Animation.timelineLength(motion: 20000, ambient: [8000]) == 20000)
        #expect(Animation.timelineLength(motion: 3000, ambient: [16000]) == 16000)
    }

    @Test func testR23TimelineLengthIsCappedAtSixtySeconds() {
        #expect(Animation.timelineLength(motion: nil, ambient: [600_000]) == 60_000)
        #expect(Animation.timelineLength(motion: 90_000, ambient: [10_000]) == 60_000)
        #expect(Animation.timelineLength(motion: 60_000, ambient: []) == 60_000)
    }

    @Test func testR23ASceneWithAmbientLoopsButNoMotionGetsTheTimeline() throws {
        let t = TimelineModel()
        try t.load(scene(#"{ "name": "meadow", "doc": { "items": [\#(cloud)] } }"#))
        #expect(t.hasMotion)
        #expect(t.duration == 16000)
        #expect(t.motionDuration == nil)
        #expect(t.hasAmbient)
        #expect(t.renderTime == 0)
        t.scrub(to: 5230)
        #expect(t.renderTime == 5200)
    }

    @Test func testR23MotionAloneAndNothingAtAll() throws {
        let t = TimelineModel()
        try t.load(scene(#"{ "name": "a", "doc": { "motion": { "duration": 4000, "tracks": [] }, "items": [] } }"#))
        #expect(t.duration == 4000)
        #expect(!t.hasAmbient)
        try t.load(scene(#"{ "name": "a", "doc": { "items": [\#(frog)] } }"#))
        #expect(!t.hasMotion)
        #expect(t.duration == nil)
    }

    @Test func testR23AmbientLoopsOnObjectMapLayersCountToo() throws {
        let t = TimelineModel()
        let tree = #"{ "object": "tree", "at": { "tile": [2, 0, 0] } }"#
        let pine = #"{ "group": { "class": "forest" }, "items": [\#(tree)] }"#
        let sway = Animation.layers(of: try JSONDecoder().decode(JSONValue.self, from: Data(#"{ "layers": [{ "map": ["a"] }, { "map": ["a"], "ambient": { "duration": 4000, "keys": [] } }] }"#.utf8)))
        try t.load(scene(#"{ "name": "a", "doc": { "motion": { "duration": 1000, "tracks": [] }, "items": [\#(pine)] } }"#), objects: ["tree": sway])
        #expect(t.itemObject["0.0"] == "tree")
        #expect(t.ambientDuration == 4000)
        #expect(t.duration == 4000)
    }

    @Test func testD10ActionsRowsForAnItem() throws {
        let t = TimelineModel()
        try t.load(scene(#"{ "name": "a", "doc": { "items": [\#(cloud), \#(frog)] } }"#))
        #expect(t.actionRows(forItem: "0").isEmpty)
        let rows = t.actionRows(forItem: "1")
        #expect(rows.map(\.name) == ["croak", "hop"])
        #expect(rows.map(\.durationLabel) == ["1.5 s", "480 ms"])
    }

    @Test func testD10ActionsRowsJoinTheItemsAndItsObjectsLayersByName() throws {
        let t = TimelineModel()
        let layers = Animation.layers(of: try JSONDecoder().decode(JSONValue.self, from: Data(#"{ "layers": [{ "map": ["a"], "actions": { "hop": { "duration": 600, "keys": [] }, "wave": { "duration": 900, "keys": [] } } }] }"#.utf8)))
        try t.load(scene(#"{ "name": "a", "doc": { "items": [\#(frog)] } }"#), objects: ["frog": layers])
        let rows = t.actionRows(forItem: "0")
        #expect(rows.map(\.name) == ["croak", "hop", "wave"])
        #expect(rows.first { $0.name == "hop" }?.duration == 600, "a name both carry shows once, as long as the longer")
        #expect(rows.first { $0.name == "wave" }?.source == "layer 1")
    }

    @Test func testD10ActionsRowsForAnObjectLayer() throws {
        let app = AppModel()
        let doc = DocumentModel(ref: DocRef(.object, "frog"))
        app.document = doc
        let source = try JSONDecoder().decode(JSONValue.self, from: Data(#"{ "layers": [{ "map": ["a"] }, { "map": ["a"], "actions": { "hop": { "duration": 480, "keys": [] } } }] }"#.utf8))
        app.objectEditor.info = ObjectShowResult(name: "frog", kind: "sprite", size: [1, 1], layers: [ObjectLayerInfo(index: 0, frames: 1), ObjectLayerInfo(index: 1, frames: 1)], frames: 1, colors: 1, anchor: [0, 0], usedColors: [], doc: source)
        app.objectEditor.layer = 0
        #expect(app.objectEditor.actionRows.isEmpty)
        app.objectEditor.layer = 1
        #expect(app.objectEditor.actionRows.map(\.name) == ["hop"])
    }

    @Test func testD10ActionSampleTimesStayInsideTheAction() {
        #expect(Animation.sampleTimes(duration: 480) == Array(stride(from: 0, to: 480, by: 40)))
        #expect(Animation.sampleTimes(duration: 30) == [0])
        let long = Animation.sampleTimes(duration: 600_000)
        #expect(long.count <= Animation.maxFrames)
        #expect((long.last ?? 0) < 600_000)
    }

    @Test func testR23ReduceMotionShowsTheRestFrameAndSaysSo() async throws {
        let app = AppModel()
        let doc = DocumentModel(ref: DocRef(.scene, "a"))
        app.document = doc
        let t = app.timeline
        try t.load(scene(#"{ "name": "a", "doc": { "items": [\#(frog)] } }"#))
        t.reduceMotionOverride = true
        await t.playAction(ActionRow(name: "hop", duration: 480, source: "item"))
        #expect(t.lastAnnouncement == "hop: shown at rest")
        #expect(t.lastPlay == "hop: shown at rest")
        #expect(!t.actionActive, "no frames play: the stage stays on the rest frame")
        #expect(t.playingAction == nil)
        #expect(t.renderTime == 0)
    }
}
