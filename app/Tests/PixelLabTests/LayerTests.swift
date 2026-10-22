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

/// docs/intents/2026-10-floating-library/spec.md, R19, D12: hide and lock single items.
@MainActor @Suite struct ItemHideTests {
    let app = AppModel()
    let doc = DocumentModel(ref: DocRef(.scene, "harbor"))
    var layers: LayersModel { app.layers }

    init() {
        app.document = doc
        list([
            SceneItemInfo(path: "0", object: "pine", at: ItemAt(tile: [0, 0, 1])),
            SceneItemInfo(path: "1", object: "house", at: ItemAt(tile: [2, 0, 0])),
            SceneItemInfo(path: "2.0", object: "rock", at: ItemAt(tile: [3, 1, 0]), group: "2"),
        ])
    }

    func list(_ items: [SceneItemInfo]) {
        doc.setItems(SceneItemsResult(scene: "harbor", items: items, groups: [], layers: []))
        layers.itemsChanged()
    }

    @Test("R19: a hidden item isn't drawn or picked, and leaves the selection")
    func testR19AHiddenItemIsNotDrawnOrPickedAndLeavesTheSelection() {
        layers.select([.item("0"), .item("1")], announce: false)
        layers.toggleItemHidden("0")
        #expect(layers.hiddenItems == ["0": "pine"])
        #expect(doc.selection == [.item("1")], "hiding a selected item deselects it")
        #expect(layers.renderParams["hide"] == .array([.string("0")]), "the render leaves it out, by path")
        #expect(!layers.isPickable("0"))
        #expect(layers.isPickable("1"))
        // Its row still selects from the tree.
        layers.selectRow(.item("0"), shift: false)
        #expect(doc.selection == [.item("0")])
        layers.toggleItemHidden("0")
        #expect(layers.hiddenItems.isEmpty)
        #expect(layers.renderParams["hide"] == nil)
        #expect(layers.isPickable("0"))
    }

    @Test("R19: a locked item can't be picked on the stage but selects from the tree")
    func testR19ALockedItemCantBePickedButSelectsFromTheTree() {
        layers.toggleItemLocked("1")
        #expect(layers.lockedItems == ["1": "house"])
        #expect(!layers.isPickable("1"), "the stage passes it by")
        #expect(layers.renderParams["hide"] == nil, "locked items still draw")
        layers.selectRow(.item("1"), shift: false)
        #expect(doc.selection == [.item("1")], "the tree selects it")
        #expect(app.editor.selected?.path == "1", "and the inspector and the keys have it")
        let paths = doc.layerTree.everyUse(of: "house", items: doc.items, inside: nil, hidden: [], locked: [], hiddenItems: [], lockedItems: ["1"])
        #expect(paths.isEmpty, "Select Every Use skips it too")
    }

    @Test("R19: a hide follows a depth sort's reordered path")
    func testR19AHideFollowsADepthSortsReorderedPath() {
        layers.toggleItemHidden("0")
        layers.toggleItemLocked("1")
        layers.followSort(["0": "1", "1": "0"])
        #expect(layers.hiddenItems == ["1": "pine"], "the pine moved to path 1, and its eye with it")
        #expect(layers.lockedItems == ["0": "house"])
        layers.followSort(["1": "5"])
        #expect(layers.hiddenItems == ["5": "pine"])
    }

    @Test("R19: a stale entry is dropped on reload")
    func testR19AStaleEntryIsDroppedOnReload() {
        layers.toggleItemHidden("0")
        layers.toggleItemLocked("2.0")
        #expect(layers.hiddenItems.count == 1 && layers.lockedItems.count == 1)
        // A file changed on disk: path 0 now holds a house, and the rock is gone.
        list([
            SceneItemInfo(path: "0", object: "house", at: ItemAt(tile: [0, 0, 1])),
            SceneItemInfo(path: "1", object: "pine", at: ItemAt(tile: [2, 0, 0])),
        ])
        #expect(layers.hiddenItems.isEmpty, "path 0 holds another object")
        #expect(layers.lockedItems.isEmpty, "path 2.0 holds nothing")
        #expect(layers.renderParams["hide"] == nil)
    }

    @Test("R19: the view state keeps hidden and locked items, and reads a file from before them")
    func testR19ViewStateKeepsItems() throws {
        let state = LayerViewState(hidden: ["a"], locked: [], hiddenItems: [ItemMark(path: "3", object: "pine")], lockedItems: [ItemMark(path: "4", object: "rock")])
        let back = try JSONDecoder().decode(LayerViewState.self, from: try JSONEncoder().encode(state))
        #expect(back == state)
        let old = try JSONDecoder().decode(LayerViewState.self, from: Data(#"{"hidden":["a"],"locked":["b"]}"#.utf8))
        #expect(old.hiddenItems.isEmpty && old.lockedItems.isEmpty)
        #expect(old.hidden == ["a"] && old.locked == ["b"])
    }
}
