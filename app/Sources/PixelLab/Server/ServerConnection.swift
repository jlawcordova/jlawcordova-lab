import Foundation

/// How to start the server: `node <cli> serve --stdio --site <site>`.
struct ServerLaunch: Sendable, Equatable {
    var node: URL
    var cli: URL
    var site: String
    /// Extra environment for the server (e.g. PIXELLAB_STATE).
    var environment: [String: String] = [:]
}

/// A JSON-RPC error from the server: `{ code, message, data: { code, details? } }`.
struct RPCError: Error, LocalizedError, Sendable {
    var code: Int
    var message: String
    /// The stable `data.code`: `not_found`, `invalid`, `conflict`, `dirty`, `usage`, …
    var dataCode: String?
    var details: JSONValue?

    var errorDescription: String? { message }
}

enum ServerError: Error, LocalizedError, Sendable {
    case notRunning
    case launchFailed(String)
    case exited(status: Int32, stderr: String)
    case badResponse(String)

    var errorDescription: String? {
        switch self {
        case .notRunning: "The pixellab server isn't running."
        case .launchFailed(let why): "Couldn't start the pixellab server: \(why)"
        case .exited(let status, let stderr):
            "The pixellab server stopped (exit \(status)).\(stderr.isEmpty ? "" : "\n\(stderr.suffix(600))")"
        case .badResponse(let why): "Unexpected reply from the pixellab server: \(why)"
        }
    }
}

/// The app's one link to `pixellab serve --stdio`: newline-delimited JSON-RPC
/// 2.0 over the child's stdin and stdout. Requests carry increasing ids and
/// resolve when their response line arrives; notifications (which the server
/// sends before the response of the request that caused them) go to `events`
/// in order. stderr is kept for diagnostics. If node crashes the server is
/// started again (at most 4 times a minute) and `.restarted` is sent, so the
/// app can reopen its docs.
actor ServerConnection {
    let launch: ServerLaunch
    nonisolated let events: AsyncStream<ServerEvent>
    private let eventSink: AsyncStream<ServerEvent>.Continuation
    private let stderrLog = StderrLog()

    private var process: Process?
    private var stdin: FileHandle?
    private var generation = 0
    private var nextID = 1
    private var pending: [Int: CheckedContinuation<Data, Error>] = [:]
    private var stopping = false
    private var restarts: [Date] = []
    private var readers: [Task<Void, Never>] = []
    private(set) var info: SessionInfo?

    init(launch: ServerLaunch) {
        self.launch = launch
        (events, eventSink) = AsyncStream.makeStream(of: ServerEvent.self, bufferingPolicy: .unbounded)
    }

    /// The last of the server's stderr, for error messages and diagnostics.
    nonisolated var diagnostics: String { stderrLog.text }

    var isRunning: Bool { process?.isRunning ?? false }

    // MARK: Lifecycle

    /// Starts node and asks for `session.info`.
    @discardableResult
    func start() async throws -> SessionInfo {
        stopping = false
        try spawn()
        let info: SessionInfo = try await call("session.info")
        self.info = info
        return info
    }

    /// Asks the server to shut down, closes its stdin and waits briefly for it to
    /// exit; kills it if it doesn't. Safe to call more than once.
    func stop() async {
        stopping = true
        guard let process, process.isRunning else {
            eventSink.finish()
            return
        }
        // `shutdown`, but don't wait on it for long.
        let id = nextID
        nextID += 1
        if let line = try? Self.encode(Request(id: id, method: "shutdown", params: [:])) {
            try? stdin?.write(contentsOf: line)
        }
        try? stdin?.close()
        stdin = nil
        let deadline = Date().addingTimeInterval(1.5)
        while process.isRunning, Date() < deadline {
            try? await Task.sleep(for: .milliseconds(25))
        }
        if process.isRunning { process.terminate() }
        failPending(ServerError.notRunning)
        self.process = nil
        eventSink.finish()
    }

    private func spawn() throws {
        generation += 1
        let gen = generation
        let p = Process()
        p.executableURL = launch.node
        p.arguments = [launch.cli.path, "serve", "--stdio", "--site", launch.site]
        var env = ProcessInfo.processInfo.environment
        // node's own folder first, so `art.build` finds the npm next to it.
        let nodeDir = launch.node.deletingLastPathComponent().path
        env["PATH"] = ([nodeDir] + (env["PATH"] ?? "/usr/bin:/bin:/usr/sbin:/sbin").split(separator: ":").map(String.init))
            .joined(separator: ":")
        for (k, v) in launch.environment { env[k] = v }
        p.environment = env
        p.currentDirectoryURL = URL(fileURLWithPath: launch.site, isDirectory: true)

        let inPipe = Pipe(), outPipe = Pipe(), errPipe = Pipe()
        p.standardInput = inPipe
        p.standardOutput = outPipe
        p.standardError = errPipe

        let (lines, lineSink) = AsyncStream.makeStream(of: Data.self, bufferingPolicy: .unbounded)
        let framer = LineFramer(sink: lineSink)
        outPipe.fileHandleForReading.readabilityHandler = { handle in
            let data = handle.availableData
            if data.isEmpty {
                handle.readabilityHandler = nil
                lineSink.finish()
            } else {
                framer.append(data)
            }
        }
        let log = stderrLog
        errPipe.fileHandleForReading.readabilityHandler = { handle in
            let data = handle.availableData
            if data.isEmpty { handle.readabilityHandler = nil } else { log.append(data) }
        }
        p.terminationHandler = { [weak self] proc in
            let status = proc.terminationStatus
            Task { await self?.didTerminate(generation: gen, status: status) }
        }
        do {
            try p.run()
        } catch {
            throw ServerError.launchFailed(error.localizedDescription)
        }
        process = p
        stdin = inPipe.fileHandleForWriting
        readers.append(Task { [weak self] in
            for await line in lines {
                await self?.receive(line, generation: gen)
            }
        })
    }

    private func didTerminate(generation gen: Int, status: Int32) async {
        guard gen == generation else { return }
        process = nil
        stdin = nil
        let stderr = stderrLog.text
        failPending(ServerError.exited(status: status, stderr: stderr))
        if stopping { return }
        let now = Date()
        restarts = restarts.filter { now.timeIntervalSince($0) < 60 } + [now]
        guard restarts.count <= 4 else {
            eventSink.yield(.stopped(reason: ServerError.exited(status: status, stderr: stderr).localizedDescription))
            return
        }
        try? await Task.sleep(for: .milliseconds(400))
        guard !stopping else { return }
        do {
            let info = try await start()
            eventSink.yield(.restarted(info))
        } catch {
            eventSink.yield(.stopped(reason: error.localizedDescription))
        }
    }

    private func failPending(_ error: Error) {
        let waiting = pending
        pending = [:]
        for (_, c) in waiting { c.resume(throwing: error) }
    }

    // MARK: Requests

    /// Calls a method and decodes its result.
    func call<T: Decodable & Sendable>(_ method: String, _ params: [String: JSONValue] = [:], as type: T.Type = T.self) async throws -> T {
        let data = try await send(method, params)
        do {
            return try JSONDecoder().decode(ResultEnvelope<T>.self, from: data).result
        } catch {
            throw ServerError.badResponse("\(method): \(error)")
        }
    }

    /// Calls a method for its effect; the result is returned as raw JSON.
    @discardableResult
    func call(_ method: String, _ params: [String: JSONValue] = [:]) async throws -> JSONValue {
        try await call(method, params, as: JSONValue.self)
    }

    private func send(_ method: String, _ params: [String: JSONValue]) async throws -> Data {
        guard let stdin, process?.isRunning == true else { throw ServerError.notRunning }
        let id = nextID
        nextID += 1
        let line = try Self.encode(Request(id: id, method: method, params: params))
        return try await withCheckedThrowingContinuation { (c: CheckedContinuation<Data, Error>) in
            pending[id] = c
            do {
                try stdin.write(contentsOf: line)
            } catch {
                pending[id] = nil
                c.resume(throwing: ServerError.launchFailed("writing to the server failed: \(error.localizedDescription)"))
            }
        }
    }

    private static func encode(_ request: Request) throws -> Data {
        var data = try JSONEncoder().encode(request)
        data.append(0x0A)
        return data
    }

    private func receive(_ line: Data, generation gen: Int) {
        guard gen == generation else { return }
        guard let head = try? JSONDecoder().decode(Envelope.self, from: line) else {
            stderrLog.append(Data("app: unparseable line from server: \(String(decoding: line.prefix(200), as: UTF8.self))\n".utf8))
            return
        }
        if let id = head.id {
            guard let c = pending.removeValue(forKey: id) else { return }
            if let e = head.error {
                c.resume(throwing: RPCError(code: e.code, message: e.message, dataCode: e.data?["code"]?.stringValue, details: e.data?["details"]))
            } else {
                c.resume(returning: line)
            }
            return
        }
        guard let method = head.method else { return }
        let params = head.params ?? .null
        let note: ServerNotification
        switch method {
        case "doc.changed":
            if let s = try? JSONDecoder().decode(ParamsEnvelope<DocStatus>.self, from: line).params { note = .docChanged(s) }
            else { note = .other(method: method, params: params) }
        case "files.changed":
            note = .filesChanged((params["files"]?.arrayValue ?? []).compactMap(\.stringValue))
        case "proposals.changed": note = .proposalsChanged(params["proposals"] ?? .null)
        case "activity.appended": note = .activityAppended(params["entries"] ?? .null)
        default: note = .other(method: method, params: params)
        }
        eventSink.yield(.notification(note))
    }
}

// MARK: Wire types

private struct Request: Encodable {
    let jsonrpc = "2.0"
    let id: Int
    let method: String
    let params: [String: JSONValue]
}

private struct Envelope: Decodable {
    struct ErrorObject: Decodable {
        let code: Int
        let message: String
        let data: JSONValue?
    }
    let id: Int?
    let method: String?
    let params: JSONValue?
    let error: ErrorObject?

    enum CodingKeys: String, CodingKey { case id, method, params, error }

    // Skip `result` (it can be a large PNG); the caller decodes it as its own type.
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try? c.decodeIfPresent(Int.self, forKey: .id)
        method = try c.decodeIfPresent(String.self, forKey: .method)
        params = method == nil ? nil : try c.decodeIfPresent(JSONValue.self, forKey: .params)
        error = try c.decodeIfPresent(ErrorObject.self, forKey: .error)
    }
}

private struct ResultEnvelope<T: Decodable>: Decodable { let result: T }
private struct ParamsEnvelope<T: Decodable>: Decodable { let params: T }

/// Splits stdout into lines. Readability handlers run one at a time on a
/// FileHandle queue; the lock keeps it honest anyway.
private final class LineFramer: @unchecked Sendable {
    private var buffer = Data()
    private var scanned = 0
    private let lock = NSLock()
    private let sink: AsyncStream<Data>.Continuation

    init(sink: AsyncStream<Data>.Continuation) { self.sink = sink }

    func append(_ data: Data) {
        lock.lock()
        defer { lock.unlock() }
        buffer.append(data)
        while let nl = buffer[(buffer.startIndex + scanned)...].firstIndex(of: 0x0A) {
            let line = Data(buffer[buffer.startIndex..<nl])
            buffer = Data(buffer[(nl + 1)...])
            scanned = 0
            if !line.isEmpty { sink.yield(line) }
        }
        scanned = buffer.count
    }
}

/// The tail of the server's stderr (also echoed to the app's stderr).
final class StderrLog: @unchecked Sendable {
    private var data = Data()
    private let lock = NSLock()
    private let limit = 32_768

    func append(_ chunk: Data) {
        FileHandle.standardError.write(chunk)
        lock.lock()
        data.append(chunk)
        if data.count > limit { data = Data(data.suffix(limit)) }
        lock.unlock()
    }

    var text: String {
        lock.lock()
        defer { lock.unlock() }
        return String(decoding: data, as: UTF8.self)
    }
}
