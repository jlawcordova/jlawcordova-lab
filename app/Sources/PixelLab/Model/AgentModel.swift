import AppKit
import Observation

/// The Agent window's state (docs/intents/2026-10-mac-app/plan.md, "Agents propose, people approve"):
/// the proposals agents saved over MCP, the activity log of their tool calls,
/// the selected proposal's diff, and its ghost on the main window's stage.
/// Lists load with `proposal.list` and `activity.list` and stay live through
/// `proposals.changed` and `activity.appended`.
@MainActor @Observable
final class AgentModel {
    @ObservationIgnored weak var app: AppModel?

    var proposals: [ProposalSummary] = [] {
        didSet { if proposals.count != oldValue.count { updateDockBadge() } }
    }
    /// Activity rows, oldest first (at most `activityLimit`).
    var activity: [ActivityRow] = []
    /// Clear (view only) hides the rows up to this id.
    var clearedThrough = -1
    var errorsOnly = false
    var selectedID: String?
    var detail: ProposalDetail?
    var detailError: String?
    /// Esc / Hide turns the ghost off; selecting a proposal turns it on.
    var ghostShown = true
    var ghost: ProposalGhost?
    var ghostError: String?
    /// "Approving…" while an apply or reject runs.
    var working: String?
    /// The last approve or reject, for the status line.
    var lastResult: String?
    @ObservationIgnored private var nextRowID = 0
    @ObservationIgnored private var loading = 0
    @ObservationIgnored private var ghostLoading = false
    @ObservationIgnored private var ghostAgain = false
    /// Rendered ghosts by key (proposal version, doc, time or frame, theme), so
    /// a playing timeline's ghost keeps up once a loop has been rendered.
    @ObservationIgnored private var ghostCache: [String: ProposalGhost] = [:]
    @ObservationIgnored private var memo: (key: String, changes: [GhostChange], pixels: [PixelPoint])?

    // Scripts (PIXELLAB_SCRIPT) answer the questions an alert would ask.
    @ObservationIgnored var scriptAnswer: AgentAnswer?
    var lastQuestion: AgentQuestion.Kind?

    /// Opens the Agent window (set by the main window, which has openWindow).
    @ObservationIgnored var openWindowAction: (@MainActor () -> Void)?
    @ObservationIgnored weak var window: NSWindow? {
        didSet { installKeyMonitor() }
    }
    @ObservationIgnored private var keyMonitor: Any?

    static let activityLimit = 500

    struct ActivityRow: Identifiable, Hashable {
        var id: Int
        var entry: ActivityEntry
        var clock: String
    }

    /// Something waits on the server (scripts settle on it).
    var busy: Bool { loading > 0 || ghostLoading || working != nil }

    var selected: ProposalSummary? { selectedID.flatMap { id in proposals.first { $0.id == id } } }
    var pendingCount: Int { proposals.count }

    /// The Dock icon shows how many proposals wait for review (none: no badge).
    func updateDockBadge() {
        NSApp?.dockTile.badgeLabel = pendingCount > 0 ? String(pendingCount) : nil
    }

    var visibleActivity: [ActivityRow] {
        activity.filter { $0.id > clearedThrough && (!errorsOnly || !$0.entry.ok) }
    }

    // MARK: Loading

    func load() async {
        guard let connection = app?.connection else { return }
        loading += 1
        defer { loading -= 1 }
        do {
            async let p: ProposalList = connection.call("proposal.list")
            async let a: ActivityList = connection.call("activity.list", ["limit": 200])
            let (pl, al) = try await (p, a)
            activity = []
            append(al.entries)
            setProposals(pl.proposals)
        } catch {
            app?.notice = "Couldn't read the agent's proposals: \(error.localizedDescription)"
        }
    }

    /// `proposals.changed`.
    func proposalsChanged(_ value: JSONValue) {
        guard let list = Self.decode([ProposalSummary].self, value) else { return }
        setProposals(list)
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

    private func setProposals(_ list: [ProposalSummary]) {
        let before = selected
        let oldIndex = selectedID.flatMap { id in proposals.firstIndex { $0.id == id } }
        proposals = list
        guard let id = selectedID else { return }
        if let now = list.first(where: { $0.id == id }) {
            // Rewritten by the agent (a new version) or its base changed: reload.
            if now.version != before?.version || now.baseMatches != before?.baseMatches {
                Task { await loadDetail() }
                syncGhost()
            }
        } else {
            // Applied or rejected (here or elsewhere): select the next one.
            let next = list.isEmpty ? nil : list[min(oldIndex ?? 0, list.count - 1)].id
            select(next)
        }
    }

    nonisolated static func decode<T: Decodable>(_ type: T.Type, _ value: JSONValue) -> T? {
        guard let data = try? JSONEncoder().encode(value) else { return nil }
        return try? JSONDecoder().decode(T.self, from: data)
    }

    // MARK: Selection

    func select(_ id: String?) {
        guard id != selectedID || (id != nil && detail?.id != id) else { return }
        selectedID = id
        detail = nil
        detailError = nil
        ghost = nil
        ghostError = nil
        memo = nil
        ghostShown = true
        guard id != nil else { return }
        Task { await loadDetail() }
        syncGhost()
    }

    /// ↑ / ↓ in the Agent window.
    func move(by delta: Int) {
        guard !proposals.isEmpty else { return }
        let i = selectedID.flatMap { id in proposals.firstIndex { $0.id == id } }
        let next = i.map { min(max($0 + delta, 0), proposals.count - 1) } ?? (delta > 0 ? 0 : proposals.count - 1)
        select(proposals[next].id)
    }

    func loadDetail() async {
        guard let id = selectedID, let connection = app?.connection else { return }
        loading += 1
        defer { loading -= 1 }
        do {
            let d: ProposalDetail = try await connection.call("proposal.show", ["id": .string(id)])
            if selectedID == id {
                detail = d
                detailError = nil
            }
        } catch {
            if selectedID == id { detailError = error.localizedDescription }
        }
    }

    // MARK: Ghost

    /// The selected proposal is for the doc on the stage.
    var previewing: Bool {
        guard let p = selected, let doc = app?.document else { return false }
        return p.ref == doc.ref
    }

    /// The ghost the stage draws now, if any.
    var visibleGhost: ProposalGhost? {
        guard ghostShown, previewing, let ghost, ghost.proposalID == selectedID, ghost.ref == app?.document?.ref else { return nil }
        return ghost
    }

    func setGhostShown(_ on: Bool) {
        ghostShown = on
        syncGhost()
    }

    /// The key the ghost should be rendered for now, or nil for no ghost.
    private func wantedGhost() -> (key: String, params: [String: JSONValue], ref: DocRef)? {
        guard ghostShown, let app, let p = selected, let ref = p.ref, let doc = app.document, doc.ref == ref else { return nil }
        var params: [String: JSONValue] = ["id": .string(p.id), "scale": 1]
        var at = ""
        if ref.kind == .scene {
            let t = app.timeline.hasMotion ? app.timeline.renderTime : 0
            params["time"] = JSONValue(t)
            params["theme"] = app.stageTheme
            at = "t\(t)\(app.stageDark ? "d" : "l")"
        } else {
            params.merge(app.objectEditor.renderParams(frame: app.objectEditor.frame)) { _, b in b }
            at = "f" + app.objectEditor.cacheKey(frame: app.objectEditor.frame)
        }
        return ("\(p.version)|\(ref.id)|\(at)", params, ref)
    }

    /// Renders the ghost when what it should show changed (a new selection,
    /// version, doc, time or frame); coalesces like the stage's render.
    func syncGhost() {
        guard let wanted = wantedGhost() else {
            if !ghostShown || !previewing { ghost = nil }
            return
        }
        if ghost?.key == wanted.key { return }
        if let hit = ghostCache[wanted.key] {
            ghost = hit
            memo = nil
            return
        }
        // While the timeline plays, the ghost follows it: one render in flight
        // at a time (the latest time wins), cached per step for the next loop.
        if ghostLoading {
            ghostAgain = true
            return
        }
        guard let connection = app?.connection else { return }
        ghostLoading = true
        Task {
            var next: (key: String, params: [String: JSONValue], ref: DocRef)? = wanted
            while let w = next {
                ghostAgain = false
                do {
                    let r: ProposalRenderResult = try await connection.call("proposal.render", w.params)
                    if let image = PixelImage(png: r.png, renderScale: r.scale) {
                        let viewBox = CGRect(x: r.viewBox[0], y: r.viewBox[1], width: r.viewBox[2], height: r.viewBox[3])
                        let stage = StageImage(image: image, viewBox: viewBox, scale: r.scale, items: r.items,
                                               anchor: r.anchor.map { CGPoint(x: $0[0], y: $0[1]) }, time: r.time)
                        let colors = w.ref.kind == .object ? PixelColors(cgImage: image.cgImage, scale: r.scale) : nil
                        let made = ProposalGhost(key: w.key, proposalID: r.id, ref: w.ref, image: stage, colors: colors)
                        if ghostCache.count > 400 { ghostCache = [:] }
                        ghostCache[w.key] = made
                        ghost = made
                        ghostError = nil
                        memo = nil
                    }
                } catch {
                    ghostError = error.localizedDescription
                    ghost = nil
                }
                let again = wantedGhost()
                next = (ghostAgain || again?.key != ghost?.key) && again?.key != w.key ? again : nil
            }
            ghostLoading = false
        }
    }

    /// What the ghost changes against the stage's current render: items
    /// (scenes) or pixels (objects, in scene pixels).
    func changes(current: StageImage, renderCount: Int) -> (items: [GhostChange], pixels: [PixelPoint]) {
        guard let ghost = visibleGhost else { return ([], []) }
        let key = "\(ghost.key)#\(renderCount)"
        if let memo, memo.key == key { return (memo.changes, memo.pixels) }
        var items: [GhostChange] = []
        var pixels: [PixelPoint] = []
        if ghost.ref.kind == .scene {
            items = GhostDiff.compare(current: current.items, proposed: ghost.image.items)
        } else {
            pixels = GhostDiff.changedPixels(current: app?.objectEditor.renderColors, currentOrigin: current.viewBox.origin,
                                             proposed: ghost.colors, proposedOrigin: ghost.image.viewBox.origin)
        }
        memo = (key, items, pixels)
        return (items, pixels)
    }

    // MARK: Window

    /// ⌫ in the Agent window rejects the selected proposal (after a
    /// confirmation). A local monitor rather than a button shortcut: the Delete
    /// key's characters ("\u{7f}") don't match SwiftUI's `.delete` key equivalent.
    private func installKeyMonitor() {
        guard keyMonitor == nil else { return }
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            nonisolated(unsafe) let e = event
            let used = MainActor.assumeIsolated { AgentModel.shared?.handleKeyDown(e) == true }
            return used ? nil : event
        }
    }

    /// The model the key monitor talks to (the app's one).
    static var shared: AgentModel? { AppModel.shared.agent }

    /// Returns whether the Agent window used the key.
    func handleKeyDown(_ event: NSEvent) -> Bool {
        guard let window, event.window === window, window.isKeyWindow, window.attachedSheet == nil else { return false }
        let mods = event.modifierFlags.intersection([.command, .option, .control, .shift])
        guard event.keyCode == 51, mods.isEmpty, !(window.firstResponder is NSTextView), selected != nil, working == nil else { return false }
        Task { await reject() }
        return true
    }

    func showWindow() {
        if let window, window.isVisible {
            window.makeKeyAndOrderFront(nil)
        } else {
            openWindowAction?()
        }
    }

    /// Open in editor: the proposal's doc in the main window, with the ghost on.
    func openInEditor() async {
        guard let app, let ref = selected?.ref else { return }
        ghostShown = true
        if app.document?.ref != ref { await app.open(ref) }
        app.mainWindow?.makeKeyAndOrderFront(nil)
        syncGhost()
    }

    // MARK: Approve and reject

    /// Approve: `proposal.apply`. Warns first when the doc has unsaved changes
    /// in the app; a changed base offers Apply Anyway or Reject; problems are shown.
    func approve(force: Bool = false, warned: Bool = false) async {
        guard let app, let connection = app.connection, let p = selected, working == nil else { return }
        if !warned, app.dirtyDocs.contains(p.doc) {
            let a = await ask(AgentQuestion(
                kind: .dirty,
                title: "\(p.ref?.name ?? p.doc) has unsaved changes",
                message: "Approving writes \(p.ref?.file ?? "the file") with the agent's version and merges it into your open copy, keeping your unsaved changes. If you both changed the same lines, Pixel Lab asks what to keep.",
                buttons: [("Approve Anyway", .proceed), ("Cancel", .cancel)]))
            guard a == .proceed else { return }
        }
        working = "Approving…"
        do {
            var params: [String: JSONValue] = ["id": .string(p.id)]
            if force { params["force"] = true }
            let r: ProposalApplyResult = try await connection.call("proposal.apply", params)
            working = nil
            lastResult = "Applied \(r.id): wrote \(r.file)" + (r.reloaded ? "; the open copy reloaded" : r.merged == true ? "; merged into your open copy" : r.conflict == true ? "; couldn't merge into your open copy, so choose in the editor" : "")
            if let ref = DocRef(id: r.doc) { await app.docWritten(ref) }
            await refreshList()
        } catch let e as RPCError {
            working = nil
            switch e.dataCode {
            case "conflict":
                let a = await ask(AgentQuestion(
                    kind: .conflict,
                    title: "\(p.ref?.file ?? p.doc) changed since the proposal was made",
                    message: "Someone (you, git or another editor) changed the file after \(p.agentName) opened it. Apply Anyway replaces the file with the proposal's version, dropping those changes.",
                    buttons: [("Apply Anyway", .force), ("Reject", .reject), ("Cancel", .cancel)]))
                if a == .force { await approve(force: true, warned: true) } else if a == .reject { await reject(confirmed: true) }
            case "invalid":
                let problems = AppModel.lines(e.details).map(DocumentWindow.trimFile)
                let shown = problems.prefix(8).map { "• " + $0 }.joined(separator: "\n")
                let a = await ask(AgentQuestion(
                    kind: .invalid,
                    title: "The proposal has \(problems.count) problem\(problems.count == 1 ? "" : "s")",
                    message: "Nothing was written.\n\n" + (shown.isEmpty ? e.message : shown) + (problems.count > 8 ? "\n…and \(problems.count - 8) more." : ""),
                    buttons: [("OK", .cancel), ("Reject", .reject)]))
                if a == .reject { await reject(confirmed: true) }
            default:
                lastResult = "Couldn't approve: \(e.message)"
            }
        } catch {
            working = nil
            lastResult = "Couldn't approve: \(error.localizedDescription)"
        }
    }

    /// Reject: `proposal.reject` (deletes it), after a confirmation.
    func reject(confirmed: Bool = false) async {
        guard let app, let connection = app.connection, let p = selected, working == nil else { return }
        if !confirmed {
            let a = await ask(AgentQuestion(
                kind: .reject,
                title: "Reject the proposal from \(p.agentName)?",
                message: "Its \(p.editsLabel) to \(p.ref?.name ?? p.doc) are deleted. The site stays as it is.",
                buttons: [("Reject", .proceed), ("Cancel", .cancel)], destructive: 0))
            guard a == .proceed else { return }
        }
        working = "Rejecting…"
        do {
            let _: JSONValue = try await connection.call("proposal.reject", ["id": .string(p.id)])
            working = nil
            lastResult = "Rejected \(p.id) (\(p.ref?.name ?? p.doc))"
            await refreshList()
        } catch {
            working = nil
            lastResult = "Couldn't reject: \(error.localizedDescription)"
        }
    }

    private func refreshList() async {
        guard let connection = app?.connection else { return }
        if let r: ProposalList = try? await connection.call("proposal.list") { setProposals(r.proposals) }
    }

    /// An alert (a sheet on the window in front), or the script's answer.
    func ask(_ q: AgentQuestion) async -> AgentAnswer {
        lastQuestion = q.kind
        if app?.script != nil {
            defer { scriptAnswer = nil }
            if let a = scriptAnswer { return a }
            return q.kind == .dirty || q.kind == .reject ? .proceed : .cancel
        }
        let alert = NSAlert()
        alert.alertStyle = q.kind == .invalid ? .informational : .warning
        alert.messageText = q.title
        alert.informativeText = q.message
        for (i, b) in q.buttons.enumerated() {
            let button = alert.addButton(withTitle: b.title)
            if i == q.destructive { button.hasDestructiveAction = true }
        }
        let window = [NSApp.keyWindow, self.window, app?.mainWindow].compactMap { $0 }.first { $0.isVisible }
        let response = window != nil ? await alert.beginSheetModal(for: window!) : alert.runModal()
        let index = response.rawValue - NSApplication.ModalResponse.alertFirstButtonReturn.rawValue
        return q.buttons.indices.contains(index) ? q.buttons[index].answer : .cancel
    }

    // MARK: MCP setup

    /// `claude mcp add …` for the CLI this app runs (and its site and state dir when they aren't the defaults).
    var mcpCommand: String {
        let cli = Locator.findCLI()?.path ?? "<path to>/cli/bin/pixellab.mjs"
        var parts = ["claude", "mcp", "add", "pixellab"]
        if let state = app?.sessionInfo?.stateDir, URL(fileURLWithPath: state).standardizedFileURL != Self.defaultStateDir.standardizedFileURL {
            parts += ["-e", "PIXELLAB_STATE=\(Self.shellQuote(state))"]
        }
        parts += ["--", "node", Self.shellQuote(cli), "serve", "--mcp"]
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

/// What an Approve or Reject asks before it goes on.
struct AgentQuestion {
    enum Kind: String { case dirty, conflict, invalid, reject }
    var kind: Kind
    var title: String
    var message: String
    var buttons: [(title: String, answer: AgentAnswer)]
    var destructive: Int? = nil
}

enum AgentAnswer: String { case proceed, force, reject, cancel }
