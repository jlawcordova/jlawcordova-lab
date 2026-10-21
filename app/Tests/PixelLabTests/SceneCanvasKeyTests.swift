import CoreGraphics
import Foundation
import Testing

@testable import PixelLab

/// Keyboard block placement (docs/intents/2026-10-scene-canvas/spec.md, R14–R19). The
/// behavior itself runs against the program in scripts/acceptance/canvas.txt; these pin
/// the key names and the results the app reads.
@Suite struct SceneCanvasKeyTests {
    @Test func testR14TheExistingKeysKeepTheirNames() {
        #expect(KeyInput(spec: "left")?.key == .left)
        #expect(KeyInput(spec: "shift+up") == KeyInput(key: .up, shift: true))
        #expect(KeyInput(spec: "shift+down") == KeyInput(key: .down, shift: true))
        #expect(KeyInput(spec: "pageup")?.key == .pageUp)
        #expect(KeyInput(spec: "pagedown")?.key == .pageDown)
        #expect(KeyInput(spec: "opt+right") == KeyInput(key: .right, option: true))
        #expect(KeyInput(spec: "return")?.key == .returnKey)
        #expect(KeyInput(spec: "delete")?.key == .delete)
    }

    @Test func testR15TabAndShiftTabAreStageKeys() {
        #expect(KeyInput(spec: "tab") == KeyInput(key: .tab))
        #expect(KeyInput(spec: "shift+tab") == KeyInput(key: .tab, shift: true))
    }

    @Test func testR16CommandDownIsTheDropKey() {
        #expect(KeyInput(spec: "cmd+down") == KeyInput(key: .down, command: true))
    }

    @Test func testR17ThePlaceResultsLevelIsRead() throws {
        let json = #"{"doc":"scene:meadow","changed":true,"summary":"lamp placed at column 2, row 3, level 1, on top of grass","path":"1","item":{"object":"lamp","at":{"tile":[2,3,1]}}}"#
        let r = try JSONDecoder().decode(EditResult.self, from: Data(json.utf8))
        #expect(r.itemTile == Tile(col: 2, row: 3, level: 1))
        let none = try JSONDecoder().decode(EditResult.self, from: Data(#"{"doc":"scene:meadow"}"#.utf8))
        #expect(none.itemTile == nil)
    }

    @Test func testR8SceneFrameResultDecodes() throws {
        let json = #"{"scene":"meadow","frame":"fit","size":[272,200],"viewBox":[-136,-56,272,200],"margin":0,"changed":false,"summary":"x"}"#
        let r = try JSONDecoder().decode(SceneFrameResult.self, from: Data(json.utf8))
        #expect(!r.isFixed)
        #expect(r.box == CGRect(x: -136, y: -56, width: 272, height: 200))
    }
}
