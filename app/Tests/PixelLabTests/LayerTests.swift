import CoreGraphics
import Foundation
import Testing

@testable import PixelLab

/// Scene layers' pure rules (docs/intents/2026-10-scene-layers/spec.md): the
/// tree's rows, what a click picks inside and outside layers, locks, where
/// placing goes, and Moved by. A small scene: Railway holds Train, a track
/// and the tunnel's front; Train holds a locomotive and a car; a pine is at
/// the top level, and the track sits in an SVG group wrapper (path 2.0).
@Suite struct LayerTests {
    let items = [
        SceneItemInfo(path: "0", object: "pine", at: ItemAt(tile: [0, 0, 1])),
        SceneItemInfo(path: "1", object: "locomotive", at: ItemAt(px: [180, 50]), id: "train-loco", layer: "train"),
        SceneItemInfo(path: "2.0", object: "track", at: ItemAt(tile: [3, 1, 0]), layer: "railway", group: "2"),
        SceneItemInfo(path: "3", object: "boxcar", at: ItemAt(tile: [4, 1, 0]), layer: "train"),
        SceneItemInfo(path: "4", object: "tunnel-front", at: ItemAt(tile: [5, 1, 0]), layer: "railway"),
    ]
    let layers = [
        SceneLayerInfo(id: "railway", name: "Railway", items: 4),
        SceneLayerInfo(id: "train", name: "Train", parent: "railway", items: 2),
        SceneLayerInfo(id: "mine", name: "Mine", items: 0),
    ]
    var tree: LayerTree { LayerTree(items: items, layers: layers) }

    @Test func testR4WithoutLayersEveryItemIsAtTheTopLevelInPaintOrderAndWrappersAreLeftOut() {
        let plain = LayerTree(items: items.map { var i = $0; i.layer = nil; return i }, layers: [])
        #expect(plain.isEmpty)
        #expect(plain.rows(expanded: []).map(\.id) == ["0", "1", "2.0", "3", "4"])
        #expect(plain.rows(expanded: []).allSatisfy { $0.depth == 0 })
        #expect(plain.target(ofItem: "2.0", inside: nil) == .item("2.0"), "an item in a wrapper is picked as itself")
    }

    @Test func testR7RowsListChildLayersThenItemsAndOnlyOpenLayersShowWhatTheyHold() {
        #expect(tree.rows(expanded: []).map(\.id) == ["layer:railway", "layer:mine", "0"])
        let open = tree.rows(expanded: ["railway", "train"])
        #expect(open.map(\.id) == ["layer:railway", "layer:train", "1", "3", "2.0", "4", "layer:mine", "0"])
        #expect(open.map(\.depth) == [0, 1, 2, 2, 1, 1, 0, 0])
        #expect(tree.rows(expanded: ["train"]).map(\.id) == ["layer:railway", "layer:mine", "0"], "a closed parent hides an open child")
        #expect(tree.reveal(.item("3")) == ["railway", "train"])
        #expect(tree.reveal(.layer("train")) == ["railway"])
        let found = tree.rows(matching: "rail", items: Dictionary(uniqueKeysWithValues: items.map { ($0.path, $0) }))
        #expect(found.map(\.id) == ["layer:railway"])
        #expect(tree.rows(matching: "loco", items: Dictionary(uniqueKeysWithValues: items.map { ($0.path, $0) })).map(\.id) == ["1"], "ids match too")
    }

    @Test func testR8AClickPicksTheOutermostLayerUnderThePointerOrTheItem() {
        #expect(tree.target(ofItem: "1", inside: nil) == .layer("railway"))
        #expect(tree.target(ofItem: "2.0", inside: nil) == .layer("railway"))
        #expect(tree.target(ofItem: "0", inside: nil) == .item("0"))
        #expect(tree.paths(of: .layer("railway")) == ["1", "2.0", "3", "4"])
        #expect(tree.paths(of: [.layer("train"), .item("1"), .item("0")]) == ["0", "1", "3"], "each once, in paint order")
    }

    @Test func testR9InsideALayerClicksPickTheNextLevelDownAndNothingOutsideIt() {
        #expect(tree.target(ofItem: "1", inside: "railway") == .layer("train"))
        #expect(tree.target(ofItem: "4", inside: "railway") == .item("4"))
        #expect(tree.target(ofItem: "1", inside: "train") == .item("1"))
        #expect(tree.target(ofItem: "0", inside: "railway") == nil)
        #expect(tree.target(ofItem: "4", inside: "train") == nil)
        #expect(tree.chain(ofLayer: "train") == ["railway", "train"])
        #expect(tree.isWithin(.item("3"), "railway"))
        #expect(!tree.isWithin(.item("0"), "railway"))
    }

    @Test func testR11LockedLayersArePassedByExceptFromInsideThemAndHiddenOnesDontDraw() {
        #expect(!tree.pickable("1", inside: nil, locked: ["train"]))
        #expect(tree.pickable("4", inside: nil, locked: ["train"]), "a locked layer's sibling")
        #expect(!tree.pickable("4", inside: nil, locked: ["railway"]))
        #expect(tree.pickable("4", inside: "railway", locked: ["railway"]), "inside the locked layer")
        #expect(!tree.pickable("1", inside: "railway", locked: ["train"]), "a locked layer inside the one you're in")
        #expect(tree.pickable("1", inside: "train", locked: ["railway", "train"]), "inside a layer within the locked one")
        #expect(tree.isHidden("3", hidden: ["railway"]))
        #expect(!tree.isHidden("0", hidden: ["railway"]))
    }

    @Test func testR11ViewStateIsHiddenAndLockedListsKeyedByDocumentAndScene() throws {
        let all = ["/docs/a.pixellab": ["hero-island": LayerViewState(hidden: ["train"], locked: ["mine"])]]
        let data = try JSONEncoder().encode(all)
        let back = try JSONDecoder().decode([String: [String: LayerViewState]].self, from: data)
        #expect(back == all)
        let text = String(decoding: data, as: UTF8.self)
        #expect(text.contains("\"hidden\":[\"train\"]") && text.contains("\"locked\":[\"mine\"]"))
    }

    @Test func testR16PlacingGoesIntoTheSelectedLayerTheItemsLayerTheOneYoureInOrTheTopLevel() {
        #expect(tree.placeTarget(selection: [.layer("mine")], inside: "railway") == "mine")
        #expect(tree.placeTarget(selection: [.item("3")], inside: nil) == "train")
        #expect(tree.placeTarget(selection: [.item("0")], inside: "railway") == "railway", "a top-level item, inside Railway")
        #expect(tree.placeTarget(selection: [.item("0")], inside: nil) == nil)
        #expect(tree.placeTarget(selection: [.layer("train"), .item("0")], inside: nil) == nil, "several things: the top level")
        #expect(tree.placeTarget(selection: [], inside: "train") == "train")
        #expect(tree.placeTarget(selection: [], inside: nil) == nil)
    }

    @Test func testR14MovedByCountsWholeTilesFromTheLayersFirstItemOnATile() throws {
        let byPath = Dictionary(uniqueKeysWithValues: items.map { ($0.path, $0) })
        let b = try #require(MovedBaseline(layer: "train", tree: tree, items: byPath))
        #expect(b.path == "3", "the first on a tile, not the pixel-placed locomotive")
        #expect(b.moved(items: byPath)! == (0, 0))
        var moved = byPath
        moved["3"]?.at = ItemAt(tile: [5, -1, 0])
        #expect(b.moved(items: moved)! == (1, -2))
        moved["3"] = SceneItemInfo(path: "3", object: "crate", at: ItemAt(tile: [5, -1, 0]))
        #expect(b.moved(items: moved) == nil, "another object took the path")
        #expect(MovedBaseline.tiles(dx: 16, dy: 8)! == (1, 0))
        #expect(MovedBaseline.tiles(dx: -16, dy: 8)! == (0, 1))
        #expect(MovedBaseline.tiles(dx: 0, dy: 16)! == (1, 1))
        #expect(MovedBaseline.tiles(dx: 3, dy: 0) == nil)
        #expect(MovedBaseline(layer: "mine", tree: tree, items: byPath) == nil, "an empty layer")
    }

    @Test func testR14SignedFieldsReadAndWriteSigns() throws {
        let f = SignedFormat()
        #expect(f.format(2) == "+2")
        #expect(f.format(-3) == "−3")
        #expect(f.format(0) == "0")
        #expect(try f.parseStrategy.parse("−3") == -3)
        #expect(try f.parseStrategy.parse("+4") == 4)
        #expect(try f.parseStrategy.parse("-1") == -1)
        #expect(throws: (any Error).self) { try f.parseStrategy.parse("x") }
    }

    @Test func testR10ASelectionDragMovesByWholeTiles() {
        var d = SelectionDrag(targets: [.layer("train")], paths: ["1", "3"], startView: .zero, startScene: .zero)
        #expect(d.isZero)
        d.dc = 1
        d.dr = -2
        #expect(d.sceneOffset == CGPoint(x: 48, y: -8))
        #expect(StageOverlay.signed(d.dc) == "+1" && StageOverlay.signed(d.dr) == "−2")
    }

    @Test func testLayerTargetsReadAsTheHookWritesThem() {
        #expect(LayerTarget(spec: "layer:train") == .layer("train"))
        #expect(LayerTarget(spec: "12") == .item("12"))
        #expect(LayerTarget(spec: "5.2") == .item("5.2"))
        #expect(LayerTarget(spec: "layer:") == nil)
        #expect(LayerTarget(spec: "pine") == nil)
        #expect(LayerTarget(spec: "5.") == nil)
        #expect(LayerTarget.layer("a").description == "layer:a")
    }

    @Test func testABrokenParentOrACycleFallsBackToTheTopLevel() {
        let t = LayerTree(items: [], layers: [
            SceneLayerInfo(id: "a", name: "A", parent: "b", items: 0),
            SceneLayerInfo(id: "b", name: "B", parent: "a", items: 0),
            SceneLayerInfo(id: "c", name: "C", parent: "gone", items: 0),
        ])
        #expect(t.rows(expanded: ["a", "b"]).allSatisfy { $0.depth <= 1 })
        #expect(t.byID["c"]?.parent == nil)
    }

    /// docs/intents/2026-10-floating-library/spec.md, R9: the placing preview names the target the sidebar's chip used to.
    @MainActor @Test("R9: the ghost tag names the layer placing goes into")
    func testR9TheGhostTagNamesTheLayerPlacingGoesInto() {
        let app = AppModel()
        let doc = DocumentModel(ref: DocRef(.scene, "a"))
        doc.layerTree = tree
        app.document = doc
        func tag() -> String { StageOverlay.placeTagText(object: "pine", target: app.layers.placeTargetName) }
        #expect(tag() == "pine → Top level")
        doc.selection = [.layer("mine")]
        #expect(tag() == "pine → Mine", "a selected layer")
        doc.selection = [.item("3")]
        #expect(tag() == "pine → Train", "the selected item's layer")
        doc.selection = []
        app.layers.inside = "railway"
        #expect(tag() == "pine → Railway", "the layer you're inside")
        doc.selection = [.layer("train"), .item("0")]
        app.layers.inside = nil
        #expect(tag() == "pine → Top level", "several things: the top level")
    }
}
