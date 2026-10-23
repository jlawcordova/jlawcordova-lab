import Foundation
import Testing

@testable import PixelLab

/// The Versions panel (docs/intents/2026-10-autosave/spec.md, R12, D5): versions under day headings,
/// their times and the words VoiceOver reads.
@Suite struct VersionsTests {
    func version(_ n: Int, _ ended: String, kind: String = "owner", edits: [String] = ["Placed pine"], files: [String] = ["scenes/a.json"]) -> VersionSummary {
        VersionSummary(number: n, startedAt: ended, endedAt: ended, author: VersionAuthor(kind: kind), authors: nil, edits: edits, files: files, open: nil, label: nil)
    }

    @Test func R12_versionsGroupUnderTheirDayInListOrder() {
        let now = ISO8601DateFormatter().string(from: Date())
        let older = "2026-10-01T09:00:00.000Z"
        let groups = VersionsSheet.days([version(3, now), version(2, now), version(1, older)])
        #expect(groups.count == 2)
        #expect(groups[0].day == "Today")
        #expect(groups[0].versions.map(\.number) == [3, 2])
        #expect(groups[1].versions.map(\.number) == [1])
    }

    @Test func R12_aVersionsDocsAreItsFilesAsRefs() {
        let v = version(1, "2026-10-01T09:00:00.000Z", files: ["scenes/harbor.json", "objects/pine.json", "palette.json"])
        #expect(v.refs == [DocRef(.scene, "harbor"), DocRef(.object, "pine")])
        #expect(DocRef(file: "scenes/harbor.json") == DocRef(.scene, "harbor"))
        #expect(DocRef(file: "notes.txt") == nil)
    }

    @Test func D5_voiceOverReadsWhoWhenWhatAndWhere() {
        let v = version(4, "2026-10-01T09:00:00.000Z", kind: "cli", edits: ["Placed pine", "Moved rock"])
        let said = VersionsModel.spoken(v)
        #expect(said.hasPrefix("Version 4 at "))
        #expect(said.contains("by The CLI: Placed pine and 1 more, in a"))
    }

    @Test func R10_aThinnedVersionIsSeveralAuthors() throws {
        let json = #"{"number":9,"startedAt":"2026-09-01T09:00:00.000Z","endedAt":"2026-09-02T09:00:00.000Z","author":{"kind":"several"},"authors":[{"kind":"owner"},{"kind":"agent","name":"claude-code"}],"edits":["a","b"],"files":[]}"#
        let v = try JSONDecoder().decode(VersionSummary.self, from: Data(json.utf8))
        #expect(v.author.display == "Several authors")
        #expect(v.authors?.count == 2)
    }
}
