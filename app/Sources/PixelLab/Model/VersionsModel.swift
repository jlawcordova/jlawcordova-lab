import AppKit
import Observation

/// The Versions sheet (docs/intents/2026-10-autosave/spec.md, R12–R14, D5): the
/// document's history, newest first, the selected version's edits and diff, and
/// its ghost on the stage (the doc on stage as it was then). Restore This
/// Version, Restore <doc> Only and Undo These Changes run `history.restore` and
/// `history.undo`, each one undo step and a version of its own. The list loads
/// with `history.list` and follows `history.changed`.
@MainActor @Observable
final class VersionsModel {
    @ObservationIgnored weak var app: AppModel?

    var versions: [VersionSummary] = []
    var total = 0
    /// The sheet is up (File › Revert To › Browse All Versions…, a change card's Show in Versions).
    var sheetShown = false
    var selectedNumber: Int?
    var detail: HistoryShow?
    var detailError: String?
    /// The panel shows the selected version's diff.
    var showDiff = false
    /// Hide (the banner) turns the ghost off; selecting a version turns it on.
    var ghostShown = true
    var ghost: VersionGhost?
    var ghostError: String?
    /// "Restoring…" while a restore or an undo runs.
    var working: String?
    /// The last restore or undo, for the sheet's status line.
    var lastResult: String?
    // Scripts (PIXELLAB_SCRIPT) answer the question an alert would ask.
    @ObservationIgnored var scriptAnswer: Bool?
    var lastQuestion: String?

    @ObservationIgnored private var loading = 0
    @ObservationIgnored private var ghostLoading = false
    @ObservationIgnored private var ghostAgain = false
    @ObservationIgnored private var ghostCache: [String: VersionGhost] = [:]
    @ObservationIgnored private var memo: (key: String, changes: [GhostChange], pixels: [PixelPoint])?

    /// Something waits on the server (scripts settle on it).
    var busy: Bool { loading > 0 || ghostLoading || working != nil }

    var selected: VersionSummary? { selectedNumber.flatMap { n in versions.first { $0.number == n } } }

    // MARK: Loading

    func load() async {
        guard let connection = app?.connection else { return }
        loading += 1
        defer { loading -= 1 }
        do {
            let r: HistoryList = try await connection.call("history.list", ["limit": 500])
            set(r)
        } catch {
            app?.notice = "Couldn't read the version history: \(error.localizedDescription)"
        }
    }

    /// `history.changed`: the newest version grew or a new one started.
    func historyChanged(_ value: JSONValue) {
        Task { await load() }
    }

    private func set(_ r: HistoryList) {
        let before = selected
        versions = r.versions
        total = r.total
        if let n = selectedNumber, !versions.contains(where: { $0.number == n }) {
            select(versions.first?.number)
        } else if let now = selected, now != before {
            Task { await loadDetail() }
            syncGhost()
        }
        app?.chat.versionsListed(versions)
    }

    // MARK: The sheet

    /// Opens the sheet, on a version or the newest.
    func open(selecting number: Int? = nil) async {
        sheetShown = true
        ghostShown = true
        await load()
        select(number ?? selectedNumber ?? versions.first?.number)
    }

    func close() {
        sheetShown = false
        ghost = nil
        memo = nil
        app?.ensureWorkspace()
    }

    func select(_ number: Int?) {
        guard number != selectedNumber || (number != nil && detail?.number != number) else { return }
        selectedNumber = number
        detail = nil
        detailError = nil
        ghost = nil
        ghostError = nil
        memo = nil
        ghostShown = true
        guard number != nil else { return }
        Task { await loadDetail() }
        syncGhost()
    }

    /// ↑ / ↓ in the sheet (the list is newest first: ↓ goes back in time).
    func move(by delta: Int) {
        guard !versions.isEmpty else { return }
        let i = selectedNumber.flatMap { n in versions.firstIndex { $0.number == n } }
        let next = i.map { min(max($0 + delta, 0), versions.count - 1) } ?? 0
        select(versions[next].number)
    }

    func loadDetail() async {
        guard let n = selectedNumber, let connection = app?.connection else { return }
        loading += 1
        defer { loading -= 1 }
        do {
            let d: HistoryShow = try await connection.call("history.show", ["version": JSONValue(n)])
            if selectedNumber == n {
                detail = d
                detailError = nil
            }
        } catch {
            if selectedNumber == n { detailError = error.localizedDescription }
        }
    }

    // MARK: Ghost

    /// The ghost is wanted: the sheet is up with a version selected, for the doc on stage.
    var previewing: Bool { sheetShown && selected != nil && app?.document != nil }

    /// Whether the selected version changed the doc on stage.
    var touchesDocOnStage: Bool {
        guard let v = selected, let ref = app?.document?.ref else { return false }
        return v.refs.contains(ref)
    }

    /// The ghost the stage draws now, if any.
    var visibleGhost: VersionGhost? {
        guard ghostShown, previewing, let ghost, ghost.version == selectedNumber, ghost.ref == app?.document?.ref else { return nil }
        return ghost
    }

    func setGhostShown(_ on: Bool) {
        ghostShown = on
        syncGhost()
    }

    private func wantedGhost() -> (key: String, params: [String: JSONValue], ref: DocRef)? {
        guard ghostShown, sheetShown, let app, let v = selected, let doc = app.document else { return nil }
        let ref = doc.ref
        var params: [String: JSONValue] = ["version": JSONValue(v.number), "doc": .string(ref.id), "scale": 1]
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
        return ("\(v.number)@\(v.endedAt)|\(ref.id)|\(at)", params, ref)
    }

    /// Renders the ghost when what it should show changed (a new selection, doc,
    /// time or frame); coalesces like the stage's render.
    func syncGhost() {
        guard let wanted = wantedGhost() else {
            if !ghostShown || !previewing { ghost = nil }
            return
        }
        if ghost?.key == wanted.key { return }
        if let hit = ghostCache[wanted.key] {
            ghost = hit
            memo = nil
            app?.ensureWorkspace()
            return
        }
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
                    let r: HistoryRenderResult = try await connection.call("history.render", w.params)
                    if let image = PixelImage(png: r.png, renderScale: r.scale) {
                        let viewBox = CGRect(x: r.viewBox[0], y: r.viewBox[1], width: r.viewBox[2], height: r.viewBox[3])
                        let stage = StageImage(image: image, viewBox: viewBox, scale: r.scale, items: r.items,
                                               anchor: r.anchor.map { CGPoint(x: $0[0], y: $0[1]) }, time: r.time)
                        let colors = w.ref.kind == .object ? PixelColors(cgImage: image.cgImage, scale: r.scale) : nil
                        let made = VersionGhost(key: w.key, version: r.version, ref: w.ref, image: stage, colors: colors)
                        if ghostCache.count > 400 { ghostCache = [:] }
                        ghostCache[w.key] = made
                        ghost = made
                        ghostError = nil
                        memo = nil
                        app?.ensureWorkspace()
                    }
                } catch let e as RPCError where e.dataCode == "not_found" {
                    // The doc on stage didn't exist then: no ghost, and say so.
                    ghostError = "\(w.ref.name) didn't exist at version \(selectedNumber ?? 0)"
                    ghost = nil
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

    /// What the ghost shows changed against the stage's current render.
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

    // MARK: Going back (R13, R14)

    /// Restore This Version (the whole document) or Restore <doc> Only, after a question.
    func restore(only ref: DocRef? = nil, asked: Bool = false) async {
        guard let app, let connection = app.connection, let v = selected, working == nil else { return }
        if !asked {
            let what = ref.map { "\($0.name)" } ?? "the whole document"
            guard await confirm("Restore \(what) to \(Self.time(v.endedAt))?", "It goes back as it was after version \(v.number). You can undo this.", button: "Restore") else { return }
        }
        working = "Restoring…"
        defer { working = nil }
        var params: [String: JSONValue] = ["version": JSONValue(v.number)]
        if let ref { params["doc"] = .string(ref.id) }
        do {
            let r: HistoryActionResult = try await connection.call("history.restore", params)
            lastResult = r.summary
            await written(r)
        } catch {
            lastResult = "Couldn't restore: \(error.localizedDescription)"
        }
    }

    /// Undo These Changes: takes back what a version changed, keeping everything since. No question: it's one ⌘Z away.
    func undoChanges(_ number: Int? = nil) async -> Bool {
        guard let app, let connection = app.connection, let n = number ?? selectedNumber, working == nil else { return false }
        working = "Undoing…"
        defer { working = nil }
        do {
            let r: HistoryActionResult = try await connection.call("history.undo", ["version": JSONValue(n)])
            lastResult = r.summary
            await written(r)
            return true
        } catch let e as RPCError where e.dataCode == "clash" {
            lastResult = e.message
            if app.script == nil {
                _ = await confirm("Version \(n)'s changes can't be undone", e.message, button: nil)
            }
            return false
        } catch {
            lastResult = "Couldn't undo: \(error.localizedDescription)"
            return false
        }
    }

    /// The docs a restore or undo wrote: the stage and lists follow.
    private func written(_ r: HistoryActionResult) async {
        guard let app else { return }
        for id in r.written {
            if let ref = DocRef(id: id) { await app.docWritten(ref) }
        }
        if !r.removed.isEmpty { await app.loadLists() }
        await load()
    }

    /// An alert (a sheet on the window in front), or the script's answer. `button` nil: just OK.
    func confirm(_ title: String, _ message: String, button: String?) async -> Bool {
        lastQuestion = title
        if app?.script != nil {
            defer { scriptAnswer = nil }
            return scriptAnswer ?? true
        }
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = title
        alert.informativeText = message
        if let button {
            alert.addButton(withTitle: button)
            alert.addButton(withTitle: "Cancel")
        } else {
            alert.addButton(withTitle: "OK")
        }
        let window = [NSApp.keyWindow, app?.mainWindow].compactMap { $0 }.first { $0.isVisible }
        let response = window != nil ? await alert.beginSheetModal(for: window!) : alert.runModal()
        return button != nil && response == .alertFirstButtonReturn
    }

    // MARK: Words

    /// "3:02 PM", or "Oct 21, 3:02 PM" for another day.
    nonisolated static func time(_ iso: String, now: Date = Date()) -> String {
        guard let d = AgentTime.date(iso) else { return iso }
        let f = DateFormatter()
        f.dateFormat = Calendar.current.isDate(d, inSameDayAs: now) ? "h:mm a" : "MMM d, h:mm a"
        return f.string(from: d)
    }

    /// The day heading a version goes under: "Today", "Yesterday", "Tuesday, Oct 21".
    nonisolated static func day(_ iso: String, now: Date = Date()) -> String {
        guard let d = AgentTime.date(iso) else { return "" }
        let cal = Calendar.current
        if cal.isDate(d, inSameDayAs: now) { return "Today" }
        if let y = cal.date(byAdding: .day, value: -1, to: now), cal.isDate(d, inSameDayAs: y) { return "Yesterday" }
        let f = DateFormatter()
        f.dateFormat = "EEEE, MMM d"
        return f.string(from: d)
    }

    /// VoiceOver's words for a row: "Version at 3:02 PM by Claude in Pine forest: placed 3 pines, in Hero Island".
    nonisolated static func spoken(_ v: VersionSummary) -> String {
        let by = v.author.display
        let chat = v.author.chat.map { " in \($0.title)" } ?? ""
        let edits = v.edits.isEmpty ? "" : ": " + (v.edits.count == 1 ? v.edits[0] : "\(v.edits[0]) and \(v.edits.count - 1) more")
        let docs = v.refs.map(\.name).joined(separator: ", ")
        return "Version \(v.number) at \(time(v.endedAt)) by \(by)\(chat)\(edits)\(docs.isEmpty ? "" : ", in \(docs)")"
    }
}
