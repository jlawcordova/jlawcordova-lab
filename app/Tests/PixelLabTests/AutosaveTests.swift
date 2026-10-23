import Foundation
import Testing

@testable import PixelLab

/// Autosave in the app (docs/intents/2026-10-autosave/spec.md, R2–R4, R20, D1, D2): what a doc's save
/// state means for the window, and the statuses `doc.changed` sends.
@Suite struct AutosaveTests {
    @Test func R1_aPendingSaveIsUnsavedButNotBlocked() throws {
        let s = try JSONDecoder().decode(DocStatus.self, from: Data(#"{"doc":"scene:a","dirty":true,"canUndo":true,"canRedo":false,"saveState":"pending","problems":0}"#.utf8))
        #expect(s.unsaved && !s.blocked)
    }

    @Test func R2_aDocWithProblemsIsBlocked() throws {
        let s = try JSONDecoder().decode(DocStatus.self, from: Data(#"{"doc":"scene:a","dirty":true,"canUndo":true,"canRedo":false,"saveState":"problems","problems":2}"#.utf8))
        #expect(s.blocked && s.problems == 2)
    }

    @Test func R3_aSavedDocIsNeitherAndAnOlderStatusStillDecodes() throws {
        let s = try JSONDecoder().decode(DocStatus.self, from: Data(#"{"doc":"scene:a","dirty":false,"canUndo":true,"canRedo":false,"saveState":"saved"}"#.utf8))
        #expect(!s.unsaved && !s.blocked)
        let old = try JSONDecoder().decode(DocStatus.self, from: Data(#"{"doc":"scene:a","dirty":true,"canUndo":true,"canRedo":false}"#.utf8))
        #expect(old.unsaved && !old.blocked && old.saveState == nil)
    }

    @Test func R20_theStatusNamesWhoseStepItIs() throws {
        let s = try JSONDecoder().decode(DocStatus.self, from: Data(#"{"doc":"scene:a","dirty":true,"canUndo":true,"canRedo":false,"undoLabel":"Place pine (Claude)","redoLabel":null,"author":{"kind":"agent","name":"claude-code"}}"#.utf8))
        #expect(s.undoLabel == "Place pine (Claude)")
        #expect(s.redoLabel == nil)
        #expect(s.author?.display == "Claude")
    }

    @Test func D9_sessionInfoNamesItsOwnerAndTheVersionItOpenedAt() throws {
        let json = #"{"name":"pixellab","version":"0.0.0-dev","protocol":1,"pid":42,"site":"/d","sourceDir":"/d","stateDir":"/s","saves":"direct","openedVersion":7,"owner":{"pid":42,"appPid":41,"socket":"/s/owners/k.sock"},"docs":[]}"#
        let info = try JSONDecoder().decode(SessionInfo.self, from: Data(json.utf8))
        #expect(info.openedVersion == 7)
        #expect(info.owner?.pid == 42 && info.owner?.appPid == 41)
    }
}
