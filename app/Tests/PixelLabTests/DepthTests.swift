import Foundation
import Testing

@testable import PixelLab

/// Draw a scene's items by where they stand (docs/intents/2026-10-depth-order/spec.md):
/// what the app reads from the CLI (depth, a pixel item's level, a group's
/// depth, footprints, check warnings, `reordered`) and how its selection
/// follows the paths a sort moves.
@Suite struct DepthTests {
    private func decode<T: Decodable>(_ type: T.Type, _ json: String) throws -> T {
        try JSONDecoder().decode(type, from: Data(json.utf8))
    }

    @Test func testR14SceneItemsDecodeDepthLevelAndGroupDepth() throws {
        let r = try decode(SceneItemsResult.self, """
        {"scene":"harbor","items":[
          {"path":"0.0","object":"shadow","at":{"tile":[0,0,0]},"group":"0","point":[0,0]},
          {"path":"1","object":"crane-jib","at":{"px":[40,-12],"level":4},"level":4,"id":"jib","point":[40,-12]},
          {"path":"2","object":"cloud-a","at":{"px":[-120,-70]},"level":0,"depth":"front","point":[-120,-70]}
        ],"groups":[{"path":"0","attrs":{"class":"enter"},"items":1,"depth":"back"}],"layers":[]}
        """)
        #expect(r.items.map(\.depthMode) == [.auto, .auto, .front])
        #expect(r.groups.first?.depthMode == .back)
        #expect(r.items[1].at.pxLevel == 4)
        #expect(r.items[1].at.standingLevel == 4)
        #expect(r.items[2].at.standingLevel == 0, "a pixel item stands on level 0 until it says")
        #expect(r.items[0].at.standingLevel == 0)
    }

    @Test func testR14DepthControlValuesTitlesAndHelp() {
        #expect(DepthMode.allCases.map(\.title) == ["Auto", "Front", "Back"])
        #expect(DepthMode.allCases.map(\.rawValue) == ["auto", "front", "back"], "what scene.set --depth takes")
        #expect(DepthMode(wire: nil) == .auto)
        #expect(DepthMode(wire: "front") == .front)
        #expect(DepthMode(wire: "sideways") == .auto)
        #expect(DepthMode.front.detail.contains("over everything"))
        #expect(DepthMode.back.detail.contains("under everything"))
    }

    @Test func testR7APixelItemSpeaksItsLevelOnlyAboveZero() {
        #expect(ItemAt(px: [31, 6]).spoken == "x 31, y 6")
        #expect(ItemAt(px: [31, 6], pxLevel: 0).spoken == "x 31, y 6")
        #expect(ItemAt(px: [40, -12], pxLevel: 4).spoken == "x 40, y -12, level 4")
        #expect(ItemAt(px: [31, 6]).short == "px 31,6")
        #expect(ItemAt(px: [40, -12], pxLevel: 4).short == "px 40,-12 level 4")
        #expect(ItemAt(tile: [3, 4, 2]).standingLevel == 2)
        #expect(ItemAt().standingLevel == nil)
    }

    @Test func testR11CheckWarningsDecodeWithTheirCodes() throws {
        let r = try decode(CheckResult.self, """
        {"ok":true,"checked":9,"problems":[],"warnings":[
          {"file":"scenes/harbor.json","message":"items are not in depth order: 3 would move","text":"scenes/harbor.json: items are not in depth order: 3 would move","code":"out_of_order","scene":"harbor","moves":3},
          {"file":"scenes/harbor.json","message":"boat is drawn 4 times","text":"scenes/harbor.json: boat is drawn 4 times","code":"copies","scene":"harbor","item":"boat","copies":4},
          {"file":"scenes/harbor.json","message":"off grid","text":"scenes/harbor.json: off grid","code":"off_grid","scene":"harbor"}
        ]}
        """)
        let warnings = try #require(r.warnings)
        #expect(warnings.map(\.kind) == [.outOfOrder, .copies, .offGrid])
        #expect(warnings[0].moves == 3)
        #expect(warnings[1].item == "boat" && warnings[1].copies == 4)
        // An older server sends no warnings at all.
        #expect(try decode(CheckResult.self, #"{"ok":true,"problems":[]}"#).warnings == nil)
    }

    @Test func testR8ObjectShowReadsAFootprintForSpritesOnly() throws {
        let sprite = try decode(ObjectShowResult.self, """
        {"name":"house","kind":"sprite","size":[40,32],"layers":[],"frames":1,"colors":4,"anchor":[20,31],"footprint":[2,1],"usedColors":[]}
        """)
        #expect(sprite.footprint == [2, 1])
        let block = try decode(ObjectShowResult.self, """
        {"name":"grass","kind":"block","size":[32,24],"blockSize":[1,1,1],"layers":[],"frames":1,"colors":3,"anchor":[16,8],"usedColors":[]}
        """)
        #expect(block.footprint == nil, "a block's size is its footprint")
    }

    @Test func testR4EditResultsCarryTheSortsReorderedPaths() throws {
        let edit = try decode(EditResult.self, #"{"doc":"scene:x","changed":true,"summary":"Moved pine","path":"5","reordered":{"3":"4","4":"3"}}"#)
        #expect(edit.reordered == ["3": "4", "4": "3"])
        #expect(try decode(EditResult.self, #"{"doc":"scene:x","changed":false}"#).reordered == nil)
        let layer = try decode(LayerEditResult.self, #"{"doc":"scene:x","summary":"Moved Train","moved":2,"paths":["1","4"],"reordered":{"2":"3"}}"#)
        #expect(layer.paths == ["1", "4"])
        #expect(layer.edit.reordered == ["2": "3"], "the layer edit carries it on as an edit result")
    }

    @Test func testR11SortOfOneSceneDecodesWithoutADocField() throws {
        let r = try decode(SceneSortResult.self, #"{"changed":true,"summary":"Sorted harbor by depth: 3 items moved, 12 pixels change","moved":[{"from":"1","to":"2"}],"changedPixels":12,"scenes":[],"reordered":{"1":"2","2":"1"}}"#)
        #expect(r.changed && r.changedPixels == 12)
        #expect(r.reordered == ["1": "2", "2": "1"])
        #expect(try decode(SceneSortResult.self, #"{"changed":false,"summary":"Already in depth order","moved":[],"changedPixels":0}"#).reordered == nil)
    }

    @Test func testR4UndoResultsCarryReorderedToo() throws {
        let r = try decode(UndoResult.self, #"{"doc":"scene:x","changed":true,"summary":"Undid: pine moved","dirty":false,"canUndo":false,"canRedo":true,"reordered":{"8":"80","80":"8"}}"#)
        #expect(r.reordered == ["8": "80", "80": "8"])
        #expect(try decode(UndoResult.self, #"{"doc":"scene:x","dirty":false,"canUndo":false,"canRedo":false}"#).reordered == nil)
    }

    @MainActor @Test func testR14AnItemAloneInItsGroupReadsAndWritesTheGroupsDepth() {
        let doc = DocumentModel(ref: DocRef(.scene, "harbor"))
        doc.groups = [
            SceneGroupInfo(path: "0", attrs: [:], items: 1, depth: "front"),
            SceneGroupInfo(path: "1", attrs: [:], items: 2, depth: "back"),
        ]
        let alone = SceneItemInfo(path: "0.0", object: "pine", at: ItemAt(tile: [0, 1, 0]), group: "0", depth: "back")
        let shared = SceneItemInfo(path: "1.0", object: "grass", at: ItemAt(tile: [0, 0, 0]), group: "1")
        let bare = SceneItemInfo(path: "2", object: "rock", at: ItemAt(tile: [1, 0, 0]), depth: "front")
        #expect(doc.soleGroup(of: alone)?.path == "0")
        #expect(doc.depthMode(of: alone) == .front, "the group's, not the item's own back")
        #expect(doc.soleGroup(of: shared) == nil)
        #expect(doc.depthMode(of: shared) == .auto, "the item's own, with several in the group")
        #expect(doc.soleGroup(of: bare) == nil)
        #expect(doc.depthMode(of: bare) == .front)
    }

    @MainActor @Test func testR4TheSelectionFollowsItsItemsToTheirNewPaths() {
        let doc = DocumentModel(ref: DocRef(.scene, "harbor"))
        // The pine (3) and the house (4) swap places; layer ids and other paths stay.
        let reordered = ["3": "4", "4": "3"]
        doc.selection = [.item("3")]
        doc.follow(reordered)
        #expect(doc.selectedPath == "4", "one item")

        doc.selection = [.item("3"), .layer("train"), .item("4"), .item("7")]
        doc.follow(reordered)
        #expect(doc.selection == [.item("4"), .layer("train"), .item("3"), .item("7")], "several things, in the order they were picked")

        doc.selection = [.item("3")]
        doc.follow([:])
        #expect(doc.selectedPath == "3", "nothing moved")
    }

    @MainActor @Test func testR4AGroupMemberKeepsItsPlaceAmongTheItemsThatMoved() {
        // A sort that moves the group at 2 to 5 takes its members' paths with it.
        let doc = DocumentModel(ref: DocRef(.scene, "harbor"))
        doc.selection = [.item("2.0"), .item("2.1")]
        doc.follow(["2.0": "5.0", "2.1": "5.1"])
        #expect(doc.selection == [.item("5.0"), .item("5.1")])
    }

    @Test func testR4MovedByKeepsMeasuringTheSameItemAfterASort() {
        let items = [
            "3": SceneItemInfo(path: "3", object: "boxcar", at: ItemAt(tile: [4, 1, 0]), layer: "train"),
            "4": SceneItemInfo(path: "4", object: "tunnel-front", at: ItemAt(tile: [5, 1, 0]), layer: "train"),
        ]
        let tree = LayerTree(items: items.values.sorted { $0.path < $1.path }, layers: [SceneLayerInfo(id: "train", name: "Train", items: 2)])
        let baseline = MovedBaseline(layer: "train", tree: tree, items: items)
        #expect(baseline?.path == "3")
        // The layer moves one column; the sort puts the boxcar at 4 and the tunnel at 3.
        let after = [
            "3": SceneItemInfo(path: "3", object: "tunnel-front", at: ItemAt(tile: [6, 1, 0]), layer: "train"),
            "4": SceneItemInfo(path: "4", object: "boxcar", at: ItemAt(tile: [5, 1, 0]), layer: "train"),
        ]
        #expect(baseline?.moved(items: after) == nil, "the old path now names another object")
        let followed = baseline?.following(["3": "4", "4": "3"])
        #expect(followed?.path == "4")
        #expect(followed?.moved(items: after)?.col == 1)
        #expect(followed?.moved(items: after)?.row == 0)
    }

    /// A train layer holding one track tile, beside another track out of the layer. Moving the
    /// layer two columns puts its tile behind the other one, so the sort swaps their paths.
    @MainActor private struct TwoTracks {
        let app = AppModel()
        let doc = DocumentModel(ref: DocRef(.scene, "harbor"))
        let reordered = ["3": "4", "4": "3"]
        let before = SceneItemsResult(scene: "harbor", items: [
            SceneItemInfo(path: "3", object: "track", at: ItemAt(tile: [4, 1, 0]), layer: "train"),
            SceneItemInfo(path: "4", object: "track", at: ItemAt(tile: [5, 1, 0])),
        ], groups: [], layers: [SceneLayerInfo(id: "train", name: "Train", items: 1)])
        let after = SceneItemsResult(scene: "harbor", items: [
            SceneItemInfo(path: "3", object: "track", at: ItemAt(tile: [5, 1, 0])),
            SceneItemInfo(path: "4", object: "track", at: ItemAt(tile: [6, 1, 0]), layer: "train"),
        ], groups: [], layers: [SceneLayerInfo(id: "train", name: "Train", items: 1)])

        init() {
            app.document = doc
            list(before)
        }

        var layers: LayersModel { app.layers }
        var moved: String { layers.movedBy.map { "\($0.col),\($0.row)" } ?? "none" }

        /// `scene.items` came back with this list.
        func list(_ r: SceneItemsResult) {
            doc.setItems(r)
            layers.itemsChanged()
        }
    }

    // Moved by (docs/intents/2026-10-scene-layers/spec.md, D12) reads the item it measures from
    // the latest `scene.items`, which comes back on its own schedule: before the edit's result
    // (and its `reordered`) or after it. Either way it measures the tile it was given, never the
    // other track that has its old path, so typing 0 in it never moves the layer the wrong way.
    @MainActor @Test func testR4MovedByMeasuresTheSameItemWhenTheEditReturnsBeforeItsItems() {
        let t = TwoTracks()
        t.layers.select([.layer("train")], announce: false)
        #expect(t.moved == "0,0")
        t.app.editor.pendingEdits += 1
        t.layers.followSort(t.reordered)
        t.app.editor.pendingEdits -= 1
        #expect(t.moved == "0,0", "the list is from before the move: path 4 is still the other track")
        t.list(t.after)
        #expect(t.moved == "2,0")
    }

    @MainActor @Test func testR4MovedByMeasuresTheSameItemWhenTheItemsReturnBeforeTheEdit() {
        let t = TwoTracks()
        t.layers.select([.layer("train")], announce: false)
        t.app.editor.pendingEdits += 1
        // The items the edit's doc.changed asked for land while its result is on its way.
        t.list(t.after)
        #expect(t.moved == "0,0", "path 3 is now the other track; the result saying so hasn't come")
        t.layers.followSort(t.reordered)
        t.app.editor.pendingEdits -= 1
        t.list(t.after)
        #expect(t.moved == "2,0")
    }

    @MainActor @Test func testR4MovedByCountsFromWhereTheItemStoodWhenSelectedAfterAnEdit() {
        let t = TwoTracks()
        t.app.editor.pendingEdits += 1
        t.layers.followSort(t.reordered)
        t.app.editor.pendingEdits -= 1
        // Selected before the items from after the move are in.
        t.layers.select([.layer("train")], announce: false)
        t.list(t.after)
        #expect(t.moved == "0,0", "it stands where it was when selected")
        #expect(t.layers.baseline?.path == "4")
    }

    @MainActor @Test func testR14AWarningNamesAnOutOfOrderSceneOnlyForThatScene() {
        let doc = DocumentModel(ref: DocRef(.scene, "harbor"))
        let there = CheckWarning(file: "scenes/harbor.json", message: "m", text: "t", code: "out_of_order", scene: "harbor", moves: 2)
        let other = CheckWarning(file: "scenes/town.json", message: "m", text: "t", code: "out_of_order", scene: "town", moves: 1)
        let copies = CheckWarning(file: "scenes/harbor.json", message: "m", text: "t", code: "copies", scene: "harbor", item: "boat", copies: 4)
        doc.warnings = [copies, other]
        #expect(doc.outOfOrder == nil)
        doc.warnings = [copies, other, there]
        #expect(doc.outOfOrder == there)
    }
}
