import Foundation

/// Finds the owner's Claude Code and asks it about itself
/// (docs/intents/2026-10-agent-chat/spec.md, D1, D2): where `claude` is, its
/// version, whether it's signed in, and the environment every `claude` the
/// app starts runs with.
enum ClaudeLocator {
    /// Anthropic's official installer, shown on the setup card before it runs.
    static let installCommand = "curl -fsSL https://claude.ai/install.sh | bash"

    /// Where the search looks, and what Install runs. Only the script hook
    /// (`PIXELLAB_SCRIPT`) may change them, so a test never finds or touches
    /// the owner's install: `PIXELLAB_CLAUDE_HOME` replaces `~` (and skips the
    /// Homebrew folders and the login shell), `PIXELLAB_CLAUDE_INSTALLER` runs
    /// instead of the curl command.
    struct Search: Sendable, Equatable {
        var home: String
        var useSystem: Bool
        var installer: String?

        static func fromEnvironment(_ env: [String: String] = ProcessInfo.processInfo.environment) -> Search {
            let scripted = !(env["PIXELLAB_SCRIPT"] ?? "").isEmpty
            if scripted, let home = env["PIXELLAB_CLAUDE_HOME"], !home.isEmpty {
                return Search(home: home, useSystem: false, installer: env["PIXELLAB_CLAUDE_INSTALLER"].flatMap { $0.isEmpty ? nil : $0 })
            }
            return Search(home: FileManager.default.homeDirectoryForCurrentUser.path, useSystem: true, installer: nil)
        }
    }

    struct Install: Sendable, Equatable {
        var url: URL
        var version: String
    }

    /// The places to look, in order: the chosen command, `~/.local/bin`, the Homebrew folders.
    static func candidates(chosen: String?, search: Search) -> [String] {
        var out: [String] = []
        if let chosen, !chosen.trimmingCharacters(in: .whitespaces).isEmpty { out.append(NSString(string: chosen).expandingTildeInPath) }
        out.append((search.home as NSString).appendingPathComponent(".local/bin/claude"))
        if search.useSystem { out += ["/opt/homebrew/bin/claude", "/usr/local/bin/claude"] }
        return out
    }

    /// The first `claude` that answers `--version`; then the login shell's (`zsh -lc 'command -v claude'`).
    static func find(chosen: String?, search: Search) -> Install? {
        for path in candidates(chosen: chosen, search: search) where Locator.isExecutable(path) {
            if let v = version(of: URL(fileURLWithPath: path), search: search) { return Install(url: URL(fileURLWithPath: path), version: v) }
        }
        guard search.useSystem, let shell = loginShellClaude(), Locator.isExecutable(shell) else { return nil }
        return version(of: URL(fileURLWithPath: shell), search: search).map { Install(url: URL(fileURLWithPath: shell), version: $0) }
    }

    /// `claude --version` → "2.1.290".
    static func version(of url: URL, search: Search) -> String? {
        guard let r = run(url, ["--version"], search: search, timeout: 5), r.status == 0 else { return nil }
        return parseVersion(r.stdout)
    }

    /// Why a claude didn't answer `--version`: its exit and last output line.
    static func diagnose(_ url: URL, search: Search) -> String {
        guard Locator.isExecutable(url.path) else { return "there's no claude command at \(url.path)" }
        guard let r = run(url, ["--version"], search: search, timeout: 5) else { return "claude --version didn't answer within 5 s" }
        let last = (r.stderr + r.stdout).split(separator: "\n").last.map(String.init) ?? ""
        return "claude --version exited \(r.status)\(last.isEmpty ? "" : ": \(last)")"
    }

    static func parseVersion(_ text: String) -> String? {
        let first = text.split(whereSeparator: { $0 == " " || $0 == "\n" }).first.map(String.init) ?? ""
        return first.first?.isNumber == true && first.contains(".") ? first : nil
    }

    /// `claude auth status --json` → loggedIn, or nil if it couldn't say.
    static func signedIn(_ url: URL, search: Search) -> Bool? {
        guard let r = run(url, ["auth", "status", "--json"], search: search, timeout: 10) else { return nil }
        return parseSignedIn(r.stdout)
    }

    static func parseSignedIn(_ text: String) -> Bool? {
        guard let data = text.data(using: .utf8), let v = try? JSONDecoder().decode(JSONValue.self, from: data) else { return nil }
        if case .bool(let b) = v["loggedIn"] { return b }
        return nil
    }

    /// The environment for a `claude` the app starts: the app's own, without
    /// every `CLAUDE*` and `ANTHROPIC_*` variable (a parent Claude Code
    /// session's redirect its auth; an API key would bill an API account
    /// instead of the owner's plan), HOME as the search's, and a PATH of
    /// `claude`'s folder, then the login shell's PATH (so an npm-installed
    /// `claude`, a Node script, finds its `node` without Pixel Lab looking
    /// for one), then the system's. `loginPath` is for tests; nil resolves it
    /// (once) unless the search is the script hook's.
    static func environment(base: [String: String], claude: URL?, search: Search, loginPath: [String]? = nil) -> [String: String] {
        var env = base.filter { key, _ in !(key.hasPrefix("CLAUDE") || key.hasPrefix("ANTHROPIC_")) }
        if !search.useSystem { env["HOME"] = search.home }
        var path: [String] = []
        for dir in [claude?.deletingLastPathComponent().path].compactMap({ $0 }) where !path.contains(dir) {
            path.append(dir)
        }
        for dir in (loginPath ?? (search.useSystem ? loginShellPath : [])) + ["/usr/bin", "/bin", "/usr/sbin", "/sbin"] where !dir.isEmpty && !path.contains(dir) {
            path.append(dir)
        }
        env["PATH"] = path.joined(separator: ":")
        return env
    }

    struct Output: Sendable {
        var status: Int32
        var stdout: String
        var stderr: String
    }

    /// Runs `claude` once and waits (at most `timeout` seconds; then it's ended and nil).
    static func run(_ url: URL, _ args: [String], search: Search, timeout: TimeInterval) -> Output? {
        let p = Process()
        p.executableURL = url
        p.arguments = args
        p.environment = environment(base: ProcessInfo.processInfo.environment, claude: url, search: search)
        let out = Pipe(), err = Pipe()
        p.standardOutput = out
        p.standardError = err
        p.standardInput = FileHandle.nullDevice
        do { try p.run() } catch { return nil }
        let collector = PipeCollector(out: out, err: err)
        let deadline = Date().addingTimeInterval(timeout)
        while p.isRunning, Date() < deadline { Thread.sleep(forTimeInterval: 0.02) }
        if p.isRunning {
            p.terminate()
            return nil
        }
        let (stdout, stderr) = collector.finish()
        return Output(status: p.terminationStatus, stdout: stdout, stderr: stderr)
    }

    /// What the login shell prints for `command`, or nil (5 s at most: a login shell can hang on a broken profile).
    private static func loginShell(_ command: String) -> String? {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/bin/zsh")
        p.arguments = ["-lc", command]
        let out = Pipe()
        p.standardOutput = out
        p.standardError = FileHandle.nullDevice
        p.standardInput = FileHandle.nullDevice
        do { try p.run() } catch { return nil }
        let deadline = Date().addingTimeInterval(5)
        while p.isRunning, Date() < deadline { Thread.sleep(forTimeInterval: 0.02) }
        if p.isRunning { p.terminate(); return nil }
        let text = String(decoding: out.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
        return text.split(separator: "\n").last.map { String($0).trimmingCharacters(in: .whitespaces) }
    }

    private static func loginShellClaude() -> String? { loginShell("command -v claude") }

    /// The login shell's PATH (`zsh -lc 'echo $PATH'`), resolved once.
    static let loginShellPath: [String] = parsePath(loginShell("echo $PATH"))

    /// "/a:/b::/c" → ["/a", "/b", "/c"].
    static func parsePath(_ text: String?) -> [String] {
        (text ?? "").split(separator: ":").map { String($0).trimmingCharacters(in: .whitespacesAndNewlines) }.filter { !$0.isEmpty }
    }
}

/// Reads two pipes as they fill, so a chatty child never blocks on a full pipe.
final class PipeCollector: @unchecked Sendable {
    private let lock = NSLock()
    private var out = Data()
    private var err = Data()
    private let outPipe: Pipe
    private let errPipe: Pipe

    init(out: Pipe, err: Pipe) {
        outPipe = out
        errPipe = err
        out.fileHandleForReading.readabilityHandler = { [weak self] h in
            let d = h.availableData
            self?.lock.withLock { self?.out.append(d) }
        }
        err.fileHandleForReading.readabilityHandler = { [weak self] h in
            let d = h.availableData
            self?.lock.withLock { self?.err.append(d) }
        }
    }

    func finish() -> (String, String) {
        outPipe.fileHandleForReading.readabilityHandler = nil
        errPipe.fileHandleForReading.readabilityHandler = nil
        let restOut = outPipe.fileHandleForReading.readDataToEndOfFile()
        let restErr = errPipe.fileHandleForReading.readDataToEndOfFile()
        return lock.withLock {
            out.append(restOut)
            err.append(restErr)
            return (String(decoding: out, as: UTF8.self), String(decoding: err, as: UTF8.self))
        }
    }
}
