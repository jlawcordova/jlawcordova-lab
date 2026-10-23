import AppKit
import Observation

/// The Agent window's state: the activity log of agents' tool calls over MCP,
/// and the one-time notice about proposals set aside when edits started to apply
/// (docs/intents/2026-10-autosave/spec.md, R19, R21). The log loads with
/// `activity.list` and stays live through `activity.appended`.
@MainActor @Observable
final class AgentModel {
    @ObservationIgnored weak var app: AppModel?

    /// The list has been read from the server at least once.
    var listed = false
    /// Activity rows, oldest first (at most `activityLimit`).
    var activity: [ActivityRow] = []
    /// Clear (view only) hides the rows up to this id.
    var clearedThrough = -1
    /// The activity filter: All, Chats (calls from the Agent chat), Errors.
    enum ActivityFilter: String, CaseIterable, Sendable { case all, chats, errors }
    var filter: ActivityFilter = .all
    var errorsOnly: Bool {
        get { filter == .errors }
        set { filter = newValue ? .errors : .all }
    }
    /// The last thing the window did (Copy), for its status line.
    var lastResult: String?
    /// Proposals set aside when edits started to apply: how many, and where their files are (R21).
    var retired: RetiredNotice?
    @ObservationIgnored private var nextRowID = 0
    @ObservationIgnored private var loading = 0

    /// Opens the Agent window (set by the main window, which has openWindow).
    @ObservationIgnored var openWindowAction: (@MainActor () -> Void)?
    @ObservationIgnored weak var window: NSWindow?

    static let activityLimit = 500

    struct ActivityRow: Identifiable, Hashable {
        var id: Int
        var entry: ActivityEntry
        var clock: String
    }

    /// `retired-proposals/notice.json` in the state dir.
    struct RetiredNotice: Codable, Equatable, Sendable {
        var count: Int
        var dir: String
        var at: String?
    }

    /// Something waits on the server (scripts settle on it).
    var busy: Bool { loading > 0 }

    var visibleActivity: [ActivityRow] {
        activity.filter { row in
            guard row.id > clearedThrough else { return false }
            switch filter {
            case .all: return true
            case .chats: return row.entry.chat != nil
            case .errors: return !row.entry.ok
            }
        }
    }

    // MARK: Loading

    func load() async {
        guard let connection = app?.connection else { return }
        loading += 1
        defer { loading -= 1 }
        readRetired()
        do {
            let al: ActivityList = try await connection.call("activity.list", ["limit": 200])
            activity = []
            append(al.entries)
            listed = true
        } catch {
            app?.notice = "Couldn't read the agents' activity: \(error.localizedDescription)"
        }
    }

    /// `activity.appended`.
    func activityAppended(_ value: JSONValue) {
        guard let entries = Self.decode([ActivityEntry].self, value) else { return }
        append(entries)
    }

    private func append(_ entries: [ActivityEntry]) {
        for e in entries {
            activity.append(ActivityRow(id: nextRowID, entry: e, clock: AgentTime.clock(e.time)))
            nextRowID += 1
        }
        if activity.count > Self.activityLimit { activity.removeFirst(activity.count - Self.activityLimit) }
    }

    func clearActivity() { clearedThrough = activity.last?.id ?? clearedThrough }

    nonisolated static func decode<T: Decodable>(_ type: T.Type, _ value: JSONValue) -> T? {
        guard let data = try? JSONEncoder().encode(value) else { return nil }
        return try? JSONDecoder().decode(T.self, from: data)
    }

    // MARK: Proposals set aside (R21)

    var noticeFile: URL { URL(fileURLWithPath: stateDir).appendingPathComponent("retired-proposals/notice.json") }

    func readRetired() {
        guard let data = try? Data(contentsOf: noticeFile) else {
            retired = nil
            return
        }
        retired = try? JSONDecoder().decode(RetiredNotice.self, from: data)
    }

    /// Dismiss: the notice goes, the files stay.
    func dismissRetired() {
        try? FileManager.default.removeItem(at: noticeFile)
        retired = nil
    }

    func showRetired() {
        guard let r = retired else { return }
        if app?.script == nil { NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: r.dir)]) }
    }

    // MARK: Window

    func showWindow() {
        if let window, window.isVisible {
            window.makeKeyAndOrderFront(nil)
        } else {
            openWindowAction?()
        }
    }

    // MARK: MCP setup

    /// `claude mcp add …` for the program this app runs (and its site and state dir when they aren't the defaults).
    var mcpCommand: String {
        let program = Locator.findProgram()?.path ?? "<path to>/pixellab"
        var parts = ["claude", "mcp", "add", "pixellab"]
        if let state = app?.sessionInfo?.stateDir, URL(fileURLWithPath: state).standardizedFileURL != Self.defaultStateDir.standardizedFileURL {
            parts += ["-e", "PIXELLAB_STATE=\(Self.shellQuote(state))"]
        }
        parts += ["--", Self.shellQuote(program), "serve", "--mcp"]
        // The document the app runs on, when it isn't the default site's.
        if let location = app?.documentLocation, location != .site(Locator.defaultSitePath) {
            parts += location.arguments.enumerated().map { $0.offset == 1 ? Self.shellQuote($0.element) : $0.element }
        }
        return parts.joined(separator: " ")
    }

    var stateDir: String { app?.sessionInfo?.stateDir ?? Self.defaultStateDir.path }

    static var defaultStateDir: URL {
        FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/Pixel Lab", isDirectory: true)
    }

    nonisolated static func shellQuote(_ s: String) -> String {
        if s.allSatisfy({ $0.isLetter || $0.isNumber || "/._-+=:@~".contains($0) }) { return s }
        return "'" + s.replacingOccurrences(of: "'", with: "'\\''") + "'"
    }

    /// "~/Library/…" for a path under the home folder.
    nonisolated static func abbreviate(_ path: String) -> String { NSString(string: path).abbreviatingWithTildeInPath }
}
