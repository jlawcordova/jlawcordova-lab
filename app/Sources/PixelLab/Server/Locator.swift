import Foundation

/// Finds the bundled program and the site checkout.
enum Locator {
    static let defaultSitePath = NSString(string: "~/Projects/jlawcordova.github.io").expandingTildeInPath

    static func isExecutable(_ path: String) -> Bool {
        var dir: ObjCBool = false
        return FileManager.default.fileExists(atPath: path, isDirectory: &dir) && !dir.boolValue
            && FileManager.default.isExecutableFile(atPath: path)
    }

    /// The program: `Contents/Helpers/pixellab` in the app bundle, else
    /// `PIXELLAB_PROGRAM` (tests), else `target/release/pixellab` in a checkout
    /// found above the executable (`swift run`) or the one this file was
    /// compiled from. (It can't sit in `Contents/MacOS/`: on a case-insensitive
    /// volume `pixellab` is the app's own `PixelLab`.)
    static func findProgram(environment env: [String: String] = ProcessInfo.processInfo.environment) -> URL? {
        var candidates: [URL] = []
        if Bundle.main.bundleURL.pathExtension == "app" {
            candidates.append(Bundle.main.bundleURL.appendingPathComponent("Contents/Helpers/pixellab"))
        }
        if let override = env["PIXELLAB_PROGRAM"], !override.isEmpty {
            candidates.append(URL(fileURLWithPath: NSString(string: override).expandingTildeInPath))
        }
        let rel = "target/release/pixellab"
        if let exe = Bundle.main.executableURL?.resolvingSymlinksInPath() {
            var dir = exe.deletingLastPathComponent()
            for _ in 0..<8 {
                candidates.append(dir.appendingPathComponent(rel))
                dir = dir.deletingLastPathComponent()
            }
        }
        // …/app/Sources/PixelLab/Server/Locator.swift → repo root
        var source = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { source = source.deletingLastPathComponent() }
        candidates.append(source.appendingPathComponent(rel))
        return candidates.first { isExecutable($0.path) }
    }

    /// R9's problem for a document still in format 1.
    static func oldFormatProblem(_ name: String) -> String {
        "\(name) uses the old format. Migrate it with `pixellab document migrate`, or in the app."
    }

    /// Why a path isn't a document the program can open, or nil if it is: a
    /// `.pixellab` folder holding `palette.json`, or (`.site`) a site checkout
    /// holding one under `src/assets/pixel-art/`. A `palette.mjs` (format 1,
    /// or the legacy `source/` folder) is R9's problem, which Migrate… fixes
    /// (`needsMigration`).
    static func documentProblem(_ location: DocumentLocation) -> String? {
        let fm = FileManager.default
        let isFolder = { (path: String) -> Bool in
            var isDir: ObjCBool = false
            return fm.fileExists(atPath: path, isDirectory: &isDir) && isDir.boolValue
        }
        switch location {
        case .document(let path):
            let root = URL(fileURLWithPath: NSString(string: path).expandingTildeInPath)
            guard isFolder(root.path) else { return "There's no document at \(path)." }
            if fm.fileExists(atPath: root.appendingPathComponent("palette.json").path) { return nil }
            if fm.fileExists(atPath: root.appendingPathComponent("palette.mjs").path) { return oldFormatProblem(root.lastPathComponent) }
            return "\(root.lastPathComponent) isn't a Pixel Lab document (no palette.json). Choose a .pixellab, or make a new one."
        case .site(let path):
            let root = URL(fileURLWithPath: NSString(string: path).expandingTildeInPath)
            guard isFolder(root.path) else { return "There's no folder at \(path)." }
            let art = root.appendingPathComponent("src/assets/pixel-art")
            let bundles = ((try? fm.contentsOfDirectory(atPath: art.path)) ?? []).filter { $0.hasSuffix(".pixellab") }
            if bundles.contains(where: { fm.fileExists(atPath: art.appendingPathComponent("\($0)/palette.json").path) }) { return nil }
            if let old = bundles.first(where: { fm.fileExists(atPath: art.appendingPathComponent("\($0)/palette.mjs").path) }) {
                return oldFormatProblem(old)
            }
            if fm.fileExists(atPath: art.appendingPathComponent("source/palette.mjs").path) { return oldFormatProblem("source") }
            return "\(root.lastPathComponent) doesn't look like the site checkout: no .pixellab document under src/assets/pixel-art/."
        }
    }

    /// The path a location names, for display and for the panels' start folder.
    static func path(of location: DocumentLocation) -> String {
        switch location {
        case .document(let p), .site(let p): p
        }
    }

    /// Whether a document problem is R9's (a format-1 document), which
    /// Migrate… fixes.
    static func needsMigration(_ location: DocumentLocation) -> Bool {
        documentProblem(location)?.contains("uses the old format") == true
    }

    /// Runs the program once (`pixellab <args>`), for the commands the app
    /// needs before or outside the server: `document new`, `document migrate`.
    /// Returns stdout; throws with stderr on a non-zero exit.
    static func runCli(program: URL, _ args: [String]) async throws -> String {
        try await Task.detached {
            let p = Process()
            p.executableURL = program
            p.arguments = args
            let out = Pipe(), err = Pipe()
            p.standardOutput = out
            p.standardError = err
            p.standardInput = FileHandle.nullDevice
            try p.run()
            let stdout = String(decoding: out.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
            let stderr = String(decoding: err.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
            p.waitUntilExit()
            guard p.terminationStatus == 0 else {
                throw CliError(message: stderr.trimmingCharacters(in: .whitespacesAndNewlines).replacingOccurrences(of: "pixellab: ", with: ""))
            }
            return stdout
        }.value
    }

    struct CliError: Error, LocalizedError {
        var message: String
        var errorDescription: String? { message }
    }
}

/// What the server opens: a `.pixellab` document (`--document`), or the
/// document inside a site checkout (`--site`, which the program resolves to a
/// `*.pixellab` under `src/assets/pixel-art/`).
enum DocumentLocation: Equatable, Sendable {
    case document(String)
    case site(String)

    /// The server's arguments for this location.
    var arguments: [String] {
        switch self {
        case .document(let p): ["--document", p]
        case .site(let p): ["--site", p]
        }
    }

    /// Where the app's document comes from: `PIXELLAB_DOC`, else
    /// `PIXELLAB_SITE` (the script hook and snapshots, legacy), else the saved
    /// Settings path, else the default site checkout.
    static func resolve(environment env: [String: String], saved: String?) -> DocumentLocation {
        let expand = { (p: String) in NSString(string: p).expandingTildeInPath }
        if let doc = env["PIXELLAB_DOC"], !doc.isEmpty { return .document(expand(doc)) }
        if let site = env["PIXELLAB_SITE"], !site.isEmpty { return .site(expand(site)) }
        if let saved, !saved.isEmpty { return .document(expand(saved)) }
        return .site(Locator.defaultSitePath)
    }
}
