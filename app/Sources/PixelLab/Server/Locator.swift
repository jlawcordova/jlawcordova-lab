import Foundation

/// Finds node, the bundled CLI and the site checkout.
enum Locator {
    static let defaultSitePath = NSString(string: "~/Projects/jlawcordova.github.io").expandingTildeInPath

    /// node: the Settings value, else `zsh -lc 'command -v node'`, else the usual Homebrew spots.
    static func findNode(setting: String?) -> URL? {
        if let setting, !setting.trimmingCharacters(in: .whitespaces).isEmpty {
            let path = NSString(string: setting).expandingTildeInPath
            return isExecutable(path) ? URL(fileURLWithPath: path) : nil
        }
        if let found = loginShellNode(), isExecutable(found) { return URL(fileURLWithPath: found) }
        for path in ["/opt/homebrew/bin/node", "/usr/local/bin/node"] where isExecutable(path) {
            return URL(fileURLWithPath: path)
        }
        return nil
    }

    private static func loginShellNode() -> String? {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/bin/zsh")
        p.arguments = ["-lc", "command -v node"]
        let out = Pipe()
        p.standardOutput = out
        p.standardError = FileHandle.nullDevice
        p.standardInput = FileHandle.nullDevice
        do { try p.run() } catch { return nil }
        // A login shell can hang on a broken profile; give it a few seconds.
        let deadline = Date().addingTimeInterval(5)
        while p.isRunning, Date() < deadline { Thread.sleep(forTimeInterval: 0.02) }
        if p.isRunning { p.terminate(); return nil }
        let text = String(decoding: out.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
        return text.split(separator: "\n").last.map { String($0).trimmingCharacters(in: .whitespaces) }
    }

    static func isExecutable(_ path: String) -> Bool {
        var dir: ObjCBool = false
        return FileManager.default.fileExists(atPath: path, isDirectory: &dir) && !dir.boolValue
            && FileManager.default.isExecutableFile(atPath: path)
    }

    /// The CLI entry point: the app bundle's `Resources/cli`, else
    /// `PIXELLAB_CLI`, else a `cli/` found above the executable (`swift run`),
    /// else the repo this file was compiled from.
    static func findCLI() -> URL? {
        let rel = "cli/bin/pixellab.mjs"
        var candidates: [URL] = []
        if let res = Bundle.main.resourceURL { candidates.append(res.appendingPathComponent(rel)) }
        if let env = ProcessInfo.processInfo.environment["PIXELLAB_CLI"], !env.isEmpty {
            let url = URL(fileURLWithPath: NSString(string: env).expandingTildeInPath)
            candidates.append(url.pathExtension == "mjs" ? url : url.appendingPathComponent("bin/pixellab.mjs"))
        }
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
        return candidates.first { FileManager.default.fileExists(atPath: $0.path) }
    }

    /// Why a path isn't a document the CLI can open, or nil if it is: a
    /// `.pixellab` folder holding `palette.mjs`, or (`.site`) a site checkout
    /// holding one under `src/assets/pixel-art/` (a `*.pixellab`, or the
    /// legacy `source/` folder).
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
            guard fm.fileExists(atPath: root.appendingPathComponent("palette.mjs").path) else {
                return "\(root.lastPathComponent) isn't a Pixel Lab document (no palette.mjs). Choose a .pixellab, or make a new one."
            }
            return nil
        case .site(let path):
            let root = URL(fileURLWithPath: NSString(string: path).expandingTildeInPath)
            guard isFolder(root.path) else { return "There's no folder at \(path)." }
            let art = root.appendingPathComponent("src/assets/pixel-art")
            let bundles = ((try? fm.contentsOfDirectory(atPath: art.path)) ?? []).filter { $0.hasSuffix(".pixellab") }
            if bundles.contains(where: { fm.fileExists(atPath: art.appendingPathComponent("\($0)/palette.mjs").path) }) { return nil }
            if fm.fileExists(atPath: art.appendingPathComponent("source/palette.mjs").path) { return nil }
            return "\(root.lastPathComponent) doesn't look like the site checkout: no .pixellab document or source/ folder under src/assets/pixel-art/."
        }
    }

    /// The path a location names, for display and for the panels' start folder.
    static func path(of location: DocumentLocation) -> String {
        switch location {
        case .document(let p), .site(let p): p
        }
    }

    /// Runs the CLI once (`node <cli> <args>`), for the commands the app needs
    /// before or outside the server: `document new`. Returns stdout; throws
    /// with stderr on a non-zero exit.
    static func runCli(node: URL, cli: URL, _ args: [String]) async throws -> String {
        try await Task.detached {
            let p = Process()
            p.executableURL = node
            p.arguments = [cli.path] + args
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
/// document inside a site checkout (`--site`, which the CLI resolves to a
/// `*.pixellab` under `src/assets/pixel-art/` or its legacy `source/` folder).
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
