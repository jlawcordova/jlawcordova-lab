import Foundation

/// A child process whose output comes back a line at a time: a chat turn,
/// the sign-in, the installer. `stop()` sends SIGINT, then SIGTERM after 2 s.
final class LineProcess: @unchecked Sendable {
    enum Output: Sendable, Equatable {
        case line(String)
        case stderr(String)
        case exited(Int32)
    }

    let output: AsyncStream<Output>
    private let sink: AsyncStream<Output>.Continuation
    private let process = Process()
    private let stdinPipe = Pipe()
    private let lock = NSLock()
    private var outBuffer = Data()
    private var errBuffer = Data()

    /// Starts it. `input` is written to stdin; `closeInput` closes stdin after it.
    init(executable: URL, arguments: [String], environment: [String: String], directory: URL? = nil, input: Data? = nil, closeInput: Bool = true) throws {
        (output, sink) = AsyncStream.makeStream(of: Output.self, bufferingPolicy: .unbounded)
        process.executableURL = executable
        process.arguments = arguments
        process.environment = environment
        if let directory { process.currentDirectoryURL = directory }
        let out = Pipe(), err = Pipe()
        process.standardOutput = out
        process.standardError = err
        process.standardInput = stdinPipe
        out.fileHandleForReading.readabilityHandler = { [weak self] h in self?.take(h.availableData, error: false) }
        err.fileHandleForReading.readabilityHandler = { [weak self] h in self?.take(h.availableData, error: true) }
        process.terminationHandler = { [weak self] p in
            guard let self else { return }
            out.fileHandleForReading.readabilityHandler = nil
            err.fileHandleForReading.readabilityHandler = nil
            self.take(out.fileHandleForReading.readDataToEndOfFile(), error: false)
            self.take(err.fileHandleForReading.readDataToEndOfFile(), error: true)
            self.flush()
            self.sink.yield(.exited(p.terminationStatus))
            self.sink.finish()
        }
        try process.run()
        if let input { try? stdinPipe.fileHandleForWriting.write(contentsOf: input) }
        if closeInput { try? stdinPipe.fileHandleForWriting.close() }
    }

    var isRunning: Bool { process.isRunning }
    var pid: Int32 { process.processIdentifier }

    func stop() {
        guard process.isRunning else { return }
        process.interrupt()
        let p = process
        DispatchQueue.global().asyncAfter(deadline: .now() + 2) { if p.isRunning { p.terminate() } }
    }

    private func take(_ data: Data, error: Bool) {
        guard !data.isEmpty else { return }
        lock.lock()
        if error { errBuffer.append(data) } else { outBuffer.append(data) }
        var lines: [Output] = []
        for isErr in [false, true] {
            while true {
                let buf = isErr ? errBuffer : outBuffer
                guard let nl = buf.firstIndex(of: 0x0A) else { break }
                let line = String(decoding: buf[buf.startIndex..<nl], as: UTF8.self)
                let rest = Data(buf[buf.index(after: nl)...])
                if isErr { errBuffer = rest } else { outBuffer = rest }
                lines.append(isErr ? .stderr(line) : .line(line))
            }
        }
        lock.unlock()
        for l in lines { sink.yield(l) }
    }

    private func flush() {
        lock.lock()
        let out = outBuffer, err = errBuffer
        outBuffer = Data()
        errBuffer = Data()
        lock.unlock()
        if !out.isEmpty { sink.yield(.line(String(decoding: out, as: UTF8.self))) }
        if !err.isEmpty { sink.yield(.stderr(String(decoding: err, as: UTF8.self))) }
    }
}

/// How a chat turn starts `claude` (docs/intents/2026-10-agent-chat/spec.md, D1).
enum ClaudeTurn {
    /// The one pixellab tool that writes files outside a proposal (`document.new`).
    static let disallowed = ["mcp__pixellab__document_new"]

    /// `claude -p …`, checked against `claude --help` of Claude Code 2.1.290.
    static func arguments(mcpConfig: String, prompt: String, resume: String?) -> [String] {
        var args = [
            "-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose",
            "--mcp-config", mcpConfig, "--strict-mcp-config",
            "--tools", "", "--allowedTools", "mcp__pixellab__*", "--disallowedTools", disallowed.joined(separator: ","),
            "--permission-prompts", "none", "--setting-sources", "",
            "--append-system-prompt", prompt,
        ]
        if let resume, !resume.isEmpty { args += ["--resume", resume] }
        return args
    }

    /// The MCP config: the bundled CLI's `serve --mcp` on the app's document and
    /// state dir, with the chat it works for (its proposals and activity record it).
    static func mcpConfig(node: URL, cli: URL, location: DocumentLocation, stateDir: String, chatID: String, title: String) -> String {
        let server: JSONValue = [
            "type": "stdio",
            "command": .string(node.path),
            "args": .array(([cli.path, "serve", "--mcp"] + location.arguments).map { .string($0) }),
            "env": ["PIXELLAB_STATE": .string(stateDir), "PIXELLAB_CHAT": .string(chatID), "PIXELLAB_CHAT_TITLE": .string(title)],
        ]
        let config: JSONValue = ["mcpServers": ["pixellab": server]]
        let data = (try? JSONEncoder().encode(config)) ?? Data()
        return String(decoding: data, as: UTF8.self)
    }

    /// The stdin line: `{"type":"user","message":{"role":"user","content":"<block>\n\n<text>"}}`.
    static func input(block: String, text: String) -> Data {
        let content = block.isEmpty ? text : "\(block)\n\n\(text)"
        let line: JSONValue = ["type": "user", "message": ["role": "user", "content": .string(content)]]
        var data = (try? JSONEncoder().encode(line)) ?? Data()
        data.append(0x0A)
        return data
    }

    /// Whether a turn that ended before it started failed to resume its session (spec D1: start a new one once).
    static func resumeFailed(stderr: String, started: Bool) -> Bool {
        !started && stderr.localizedCaseInsensitiveContains("No conversation found")
    }
}
