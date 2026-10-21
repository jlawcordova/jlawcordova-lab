import Foundation
import Testing

@testable import PixelLab

/// The document the app opens (docs/intents/2026-10-pixellab-document/spec.md,
/// R3, R10, R11): where it comes from, what counts as one, and the publish
/// result the sheet shows.
@Suite struct DocumentLocationTests {
    @Test("R11: PIXELLAB_DOC wins, then PIXELLAB_SITE, then Settings, then the default site")
    func testResolve() {
        #expect(DocumentLocation.resolve(environment: ["PIXELLAB_DOC": "/a.pixellab", "PIXELLAB_SITE": "/s"], saved: "/b.pixellab") == .document("/a.pixellab"))
        #expect(DocumentLocation.resolve(environment: ["PIXELLAB_SITE": "/s"], saved: "/b.pixellab") == .site("/s"))
        #expect(DocumentLocation.resolve(environment: [:], saved: "/b.pixellab") == .document("/b.pixellab"))
        #expect(DocumentLocation.resolve(environment: ["PIXELLAB_DOC": ""], saved: "") == .site(Locator.defaultSitePath))
        #expect(DocumentLocation.resolve(environment: [:], saved: nil) == .site(Locator.defaultSitePath))
        #expect(DocumentLocation.resolve(environment: [:], saved: "~/x.pixellab") == .document(NSString(string: "~/x.pixellab").expandingTildeInPath))
    }

    @Test("R3: the server gets --document for a document and --site for a checkout")
    func testArguments() {
        #expect(DocumentLocation.document("/a.pixellab").arguments == ["--document", "/a.pixellab"])
        #expect(DocumentLocation.site("/s").arguments == ["--site", "/s"])
        #expect(Locator.path(of: .document("/a.pixellab")) == "/a.pixellab")
    }

    @Test("R11: a document is a folder with palette.json; a site holds one under src/assets/pixel-art")
    func testDocumentProblem() throws {
        let base = FileManager.default.temporaryDirectory.appendingPathComponent("pixellab-doc-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: base) }
        let doc = base.appendingPathComponent("art.pixellab")
        try FileManager.default.createDirectory(at: doc, withIntermediateDirectories: true)
        #expect(Locator.documentProblem(.document(base.appendingPathComponent("nowhere").path))?.hasPrefix("There's no document at") == true)
        #expect(Locator.documentProblem(.document(doc.path))?.contains("no palette.json") == true)
        try "{}\n".write(to: doc.appendingPathComponent("palette.json"), atomically: true, encoding: .utf8)
        #expect(Locator.documentProblem(.document(doc.path)) == nil)

        let site = base.appendingPathComponent("site")
        try FileManager.default.createDirectory(at: site.appendingPathComponent("src/assets/pixel-art"), withIntermediateDirectories: true)
        #expect(Locator.documentProblem(.site(site.path))?.contains("no .pixellab document under") == true)
        let bundle = site.appendingPathComponent("src/assets/pixel-art/site.pixellab")
        try FileManager.default.createDirectory(at: bundle, withIntermediateDirectories: true)
        #expect(Locator.documentProblem(.site(site.path)) != nil, "a .pixellab without palette.json doesn't count")
        try "{}\n".write(to: bundle.appendingPathComponent("palette.json"), atomically: true, encoding: .utf8)
        #expect(Locator.documentProblem(.site(site.path)) == nil)
    }

    @Test("R9: a palette.mjs is the old format, which Migrate… fixes; a legacy source/ folder too")
    func testOldFormatProblem() throws {
        let base = FileManager.default.temporaryDirectory.appendingPathComponent("pixellab-doc-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: base) }
        let doc = base.appendingPathComponent("art.pixellab")
        try FileManager.default.createDirectory(at: doc, withIntermediateDirectories: true)
        try "export default {};\n".write(to: doc.appendingPathComponent("palette.mjs"), atomically: true, encoding: .utf8)
        #expect(Locator.documentProblem(.document(doc.path)) == "art.pixellab uses the old format. Migrate it with `pixellab document migrate`, or in the app.")
        #expect(Locator.needsMigration(.document(doc.path)))

        let site = base.appendingPathComponent("site")
        let legacy = site.appendingPathComponent("src/assets/pixel-art/source")
        try FileManager.default.createDirectory(at: legacy, withIntermediateDirectories: true)
        try "export default {};\n".write(to: legacy.appendingPathComponent("palette.mjs"), atomically: true, encoding: .utf8)
        #expect(Locator.needsMigration(.site(site.path)))
        let bundle = site.appendingPathComponent("src/assets/pixel-art/site.pixellab")
        try FileManager.default.createDirectory(at: bundle, withIntermediateDirectories: true)
        try "export default {};\n".write(to: bundle.appendingPathComponent("palette.mjs"), atomically: true, encoding: .utf8)
        #expect(Locator.documentProblem(.site(site.path))?.hasPrefix("site.pixellab uses the old format") == true)
        // Once it is migrated, the problem is gone, and a missing document is not R9's.
        try FileManager.default.removeItem(at: bundle.appendingPathComponent("palette.mjs"))
        try "{}\n".write(to: bundle.appendingPathComponent("palette.json"), atomically: true, encoding: .utf8)
        #expect(Locator.documentProblem(.site(site.path)) == nil)
        #expect(!Locator.needsMigration(.document(base.appendingPathComponent("nowhere").path)))
    }

    @Test("R21: the program is found through PIXELLAB_PROGRAM")
    func testFindProgram() throws {
        let base = FileManager.default.temporaryDirectory.appendingPathComponent("pixellab-prog-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: base) }
        try FileManager.default.createDirectory(at: base, withIntermediateDirectories: true)
        let fake = base.appendingPathComponent("pixellab")
        try "#!/bin/sh\n".write(to: fake, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: fake.path)
        #expect(Locator.findProgram(environment: ["PIXELLAB_PROGRAM": fake.path])?.resolvingSymlinksInPath().path == fake.resolvingSymlinksInPath().path)
    }
}

@Suite struct PublishResultTests {
    @Test("R10: the publish result decodes, and its report is the CLI's lines with problems and over-budget outputs marked")
    func testArtBuildResult() throws {
        let json = """
        { "ok": false, "publishDir": "/out", "ms": 912.5, "unsaved": ["scene:x"],
          "outputs": [
            { "file": "a.svg", "scene": "a", "raw": 100, "gzip": 20, "over": false, "report": "1.0 KB / 0.2 KB gzip → 0.1 KB / 0.0 KB gzip, lossless" },
            { "file": "big.svg", "scene": "big", "raw": 999999, "gzip": 99999, "over": true, "report": "… lossless — OVER BUDGET" }
          ],
          "previewOnly": [ { "file": null, "scene": "demo", "raw": 10, "gzip": 5, "over": false, "report": "… lossless" } ],
          "problems": [] }
        """
        let r = try JSONDecoder().decode(ArtBuildResult.self, from: Data(json.utf8))
        #expect(r.ok == false)
        #expect(r.publishDir == "/out")
        #expect(r.outputs.count == 2 && r.outputs[1].file == "big.svg")
        #expect(r.previewOnly.first?.file == nil && r.previewOnly.first?.scene == "demo")
        #expect(r.report == [
            "a.svg: 1.0 KB / 0.2 KB gzip → 0.1 KB / 0.0 KB gzip, lossless",
            "big.svg: … lossless — OVER BUDGET",
            "demo (preview only, not written): … lossless",
        ])
        #expect(r.marked == ["big.svg: … lossless — OVER BUDGET"])
        #expect(r.overBudget.map(\.scene) == ["big"])

        let failed = try JSONDecoder().decode(ArtBuildResult.self, from: Data("""
        { "ok": false, "publishDir": "/out", "outputs": [], "previewOnly": [], "problems": ["objects/x.json: row 1 is 3 wide, expected 4"], "ms": 3, "unsaved": [] }
        """.utf8))
        #expect(failed.report == ["objects/x.json: row 1 is 3 wide, expected 4"])
        #expect(failed.marked == ["objects/x.json: row 1 is 3 wide, expected 4"])
    }

    @Test("R4: session.info carries the document, and still the old site field")
    func testSessionInfo() throws {
        let json = """
        { "name": "pixellab", "version": "0.2.0", "protocol": 1, "pid": 1,
          "document": { "root": "/d/art.pixellab", "name": "art", "kind": "document", "publishDir": "/d", "siteRoot": null },
          "site": "/d/art.pixellab", "sourceDir": "/d/art.pixellab", "stateDir": "/s", "saves": "direct", "docs": [] }
        """
        let info = try JSONDecoder().decode(SessionInfo.self, from: Data(json.utf8))
        #expect(info.document == DocumentInfo(root: "/d/art.pixellab", name: "art", kind: "document", publishDir: "/d", siteRoot: nil))
        #expect(info.site == "/d/art.pixellab")
    }
}
