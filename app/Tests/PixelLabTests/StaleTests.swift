import Foundation
import Testing

@testable import PixelLab

/// The changed-on-disk banner (docs/intents/2026-10-mcp-gaps/spec.md, R5; autosave spec R7):
/// when it shows, and what it says now that a change that merges is merged without asking.
@Suite struct StaleBannerTests {
    @Test func R5_showsForAStaleDocUntilKeepMine() {
        #expect(AppModel.staleBannerShown(doc: "scene:hero-island", stale: ["scene:hero-island"], kept: []))
        #expect(!AppModel.staleBannerShown(doc: "scene:hero-island", stale: ["scene:hero-island"], kept: ["scene:hero-island"]))
        #expect(!AppModel.staleBannerShown(doc: "scene:hero-island", stale: ["object:pine"], kept: []))
    }

    @Test func R5_namesTheFile() {
        #expect(StaleBanner.message(file: "scenes/hero-island.json") == "scenes/hero-island.json changed on disk, on the same lines as your edits here.")
    }

    @Test func R7_aDocThatClashesIsBlockedAndUnsaved() throws {
        let json = #"{"doc":"scene:x","dirty":true,"canUndo":true,"canRedo":false,"saveState":"conflict","problems":0}"#
        let s = try JSONDecoder().decode(DocStatus.self, from: Data(json.utf8))
        #expect(s.blocked && s.unsaved)
    }
}
