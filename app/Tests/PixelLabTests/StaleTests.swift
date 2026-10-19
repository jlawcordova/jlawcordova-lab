import Foundation
import Testing

@testable import PixelLab

/// The changed-on-disk banner (docs/intents/2026-10-mcp-gaps/spec.md, R5):
/// when it shows, and what it says.
@Suite struct StaleBannerTests {
    @Test func R5_showsForAStaleDocUntilKeepMine() {
        #expect(AppModel.staleBannerShown(doc: "scene:hero-island", stale: ["scene:hero-island"], kept: []))
        #expect(!AppModel.staleBannerShown(doc: "scene:hero-island", stale: ["scene:hero-island"], kept: ["scene:hero-island"]))
        #expect(!AppModel.staleBannerShown(doc: "scene:hero-island", stale: ["object:pine"], kept: []))
    }

    @Test func R5_namesTheFile() {
        #expect(StaleBanner.message(file: "scenes/hero-island.mjs") == "scenes/hero-island.mjs changed on disk. You have unsaved edits here.")
    }

    @Test func R6_decodesTheMergeOfAnApprovedProposal() throws {
        let json = #"{"id":"20261020-120000-abcdef","doc":"scene:x","file":"scenes/x.mjs","wrote":true,"reloaded":false,"merged":true,"summary":"Applied"}"#
        let r = try JSONDecoder().decode(ProposalApplyResult.self, from: Data(json.utf8))
        #expect(r.merged == true)
        #expect(r.conflict == nil)
    }
}
