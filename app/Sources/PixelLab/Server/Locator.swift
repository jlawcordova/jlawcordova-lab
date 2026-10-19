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

    /// Why a folder isn't a site checkout the CLI can use, or nil if it is.
    static func siteProblem(_ path: String) -> String? {
        let root = URL(fileURLWithPath: NSString(string: path).expandingTildeInPath)
        var isDir: ObjCBool = false
        guard FileManager.default.fileExists(atPath: root.path, isDirectory: &isDir), isDir.boolValue else {
            return "There's no folder at \(path)."
        }
        let needed = ["src/lib/pixel-art/engine.mjs", "src/assets/pixel-art/source/palette.mjs", "scripts/pixel-art-preview.mjs"]
        let missing = needed.filter { !FileManager.default.fileExists(atPath: root.appendingPathComponent($0).path) }
        if missing.isEmpty { return nil }
        return "\(root.lastPathComponent) doesn't look like the site checkout (missing \(missing.joined(separator: ", ")))."
    }
}
