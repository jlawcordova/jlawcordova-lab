import CoreGraphics
import Foundation
import Testing

@testable import PixelLab

/// The Agent window's ghost: matching a proposal's render items to the
/// current render's, changed pixels of an object, and the diff lines.
@Suite struct GhostDiffTests {
    func item(_ path: String, _ object: String, tile: [Int]? = nil, px: [Int]? = nil, id: String? = nil) -> RenderItem {
        let at: JSONValue = tile.map { ["tile": .array($0.map { JSONValue($0) })] } ?? ["px": .array((px ?? [0, 0]).map { JSONValue($0) })]
        return RenderItem(path: path, object: object, at: at, id: id, pose: nil, bounds: [0, 0, 4, 4])
    }

    @Test func unchangedItemsMatchEvenWhenPathsShift() {
        let current = [item("0", "pine", tile: [0, 0, 0]), item("1", "pine", tile: [1, 0, 0]), item("2", "house", tile: [2, 2, 0], id: "home")]
        // Two pines added in front: every path after them shifts.
        let proposed = [item("0", "pine", tile: [-2, 0, 0]), item("1", "pine", tile: [-3, 0, 0]),
                        item("2", "pine", tile: [0, 0, 0]), item("3", "pine", tile: [1, 0, 0]), item("4", "house", tile: [2, 2, 0], id: "home")]
        let changes = GhostDiff.compare(current: current, proposed: proposed)
        #expect(changes.map(\.kind) == [.added, .added])
        #expect(changes.map(\.path) == ["0", "1"])
    }

    @Test func aMovedItemPairsWithItsOldPlace() {
        let current = [item("0", "pine", tile: [0, 0, 0]), item("1", "house", tile: [2, 2, 0], id: "home"), item("2", "rock", px: [10, 10])]
        let proposed = [item("0", "pine", tile: [0, 0, 0]), item("1", "house", tile: [3, 2, 0], id: "home"), item("2", "rock", px: [11, 10])]
        let changes = GhostDiff.compare(current: current, proposed: proposed)
        #expect(changes.map(\.kind) == [.moved, .moved])
        #expect(changes.map(\.object) == ["house", "rock"])
    }

    @Test func removedItemsComeFromTheCurrentRender() {
        let current = [item("0", "pine", tile: [0, 0, 0]), item("1", "rock", tile: [1, 0, 0])]
        let proposed = [item("0", "pine", tile: [0, 0, 0])]
        let changes = GhostDiff.compare(current: current, proposed: proposed)
        #expect(changes == [GhostChange(kind: .removed, object: "rock", path: "1", bounds: [0, 0, 4, 4])])
    }

    @Test func anIdMatchesOnlyTheSameId() {
        // A house with an id moved and one without an id added: the id decides.
        let current = [item("0", "house", tile: [0, 0, 0], id: "home")]
        let proposed = [item("0", "house", tile: [5, 0, 0]), item("1", "house", tile: [1, 0, 0], id: "home")]
        let changes = GhostDiff.compare(current: current, proposed: proposed)
        #expect(changes.map(\.kind) == [.added, .moved])
        #expect(changes.map(\.path) == ["0", "1"])
    }

    @Test func identicalRendersHaveNoChanges() {
        let items = [item("0", "pine", tile: [0, 0, 0]), item("1", "pine", tile: [0, 0, 0])]
        #expect(GhostDiff.compare(current: items, proposed: items).isEmpty)
        // Duplicates count: one of two identical pines removed.
        #expect(GhostDiff.compare(current: items, proposed: [items[0]]).map(\.kind) == [.removed])
    }

    @Test func canonicalAtIgnoresKeyOrderAndNumberForm() {
        let a: JSONValue = ["tile": [1, 2, 0], "x": 1.0]
        let b = JSONValue.object(["x": .number(1), "tile": .array([.number(1), .number(2), .number(0)])])
        #expect(GhostDiff.canonical(a) == GhostDiff.canonical(b))
        #expect(GhostDiff.canonical(a) == "{tile:[1,2,0],x:1}")
    }

    func pixels(_ w: Int, _ h: Int, _ set: [(Int, Int, UInt8)]) -> PixelColors {
        var rgba = [UInt8](repeating: 0, count: w * h * 4)
        for (x, y, r) in set {
            let i = (y * w + x) * 4
            rgba[i] = r
            rgba[i + 3] = 255
        }
        return PixelColors(width: w, height: h, rgba: rgba)
    }

    @Test func changedPixelsCompareInScenePixels() {
        let current = pixels(4, 3, [(0, 0, 10), (1, 1, 20)])
        let proposed = pixels(4, 3, [(0, 0, 10), (1, 1, 30), (3, 2, 10)])
        let changed = GhostDiff.changedPixels(current: current, currentOrigin: CGPoint(x: -2, y: -2), proposed: proposed, proposedOrigin: CGPoint(x: -2, y: -2))
        #expect(changed == [PixelPoint(-1, -1), PixelPoint(1, 0)])
        // A bigger proposed map (resized): pixels outside the current one count when painted.
        let wider = pixels(5, 3, [(0, 0, 10), (1, 1, 20), (4, 0, 10)])
        let grown = GhostDiff.changedPixels(current: current, currentOrigin: .zero, proposed: wider, proposedOrigin: .zero)
        #expect(grown == [PixelPoint(4, 0)])
        #expect(GhostDiff.changedPixels(current: current, currentOrigin: .zero, proposed: current, proposedOrigin: .zero).isEmpty)
    }

    @Test func diffLinesAreClassified() {
        let diff = "--- a/scenes/x.json\n+++ b/scenes/x.json\n@@ -1,2 +1,3 @@\n   items: [\n+    { object: 'pine' },\n-    { object: 'rock' },\n"
        let lines = DiffLine.parse(diff)
        #expect(lines.map(\.kind) == [.file, .file, .hunk, .context, .added, .removed])
    }

    @Test func R12_versionsDecodeFromHistoryList() throws {
        let json = #"{"versions":[{"number":3,"startedAt":"2026-10-23T12:00:00.000Z","endedAt":"2026-10-23T12:00:05.000Z","author":{"kind":"agent","name":"claude-code 2.1.0","chat":{"id":"c","title":"Pines"}},"edits":["pine placed"],"files":["scenes/hero-island.json"],"open":true}],"total":1}"#
        let list = try JSONDecoder().decode(HistoryList.self, from: Data(json.utf8))
        let v = try #require(list.versions.first)
        #expect(v.refs == [DocRef(.scene, "hero-island")])
        #expect(v.author.display == "Claude")
        #expect(v.author.chat?.title == "Pines")
        #expect(VersionsModel.spoken(v).hasPrefix("Version 3 at "))
        #expect(VersionsModel.spoken(v).hasSuffix("by Claude in Pines: pine placed, in hero-island"))
        #expect(AgentTime.date("2026-10-19T12:00:00.000Z") != nil)
        #expect(AgentTime.date("2026-10-19T12:00:00Z") != nil)
    }

    @Test func shellQuoting() {
        #expect(AgentModel.shellQuote("/Users/me/pixellab") == "/Users/me/pixellab")
        #expect(AgentModel.shellQuote("/Applications/Pixel Lab.app/Contents/Helpers/pixellab") == "'/Applications/Pixel Lab.app/Contents/Helpers/pixellab'")
        #expect(AgentModel.shellQuote("it's") == "'it'\\''s'")
    }
}
