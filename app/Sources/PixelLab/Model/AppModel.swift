import AppKit
import Observation
import SwiftUI

/// Stage zoom: Fit, or a whole number of points per art pixel.
enum Zoom: Hashable, Sendable {
    case fit
    case level(Int)

    /// Scenes: 1×–4×. Objects: 4×–16×.
    static func levels(for kind: DocRef.Kind) -> [Int] { kind == .scene ? [1, 2, 3, 4] : [4, 8, 12, 16] }
}

/// Why the app can't open the site yet; the setup screen shows it.
struct SetupProblem: Equatable {
    var node: String?
    var site: String?
    var cli: String?
    var server: String?
    var isEmpty: Bool { node == nil && site == nil && cli == nil && server == nil }
}

/// The app's state: the server connection, the site's scenes and objects, the
/// open doc and the stage's view settings. One per app; the views read it from
/// the environment.
@MainActor @Observable
final class AppModel {
    static let shared = AppModel()

    enum Phase: Equatable {
        case starting(String)
        case setup(SetupProblem)
        case ready
    }

    var phase: Phase = .starting("Starting…")
    var sessionInfo: SessionInfo?
    var scenes: [SceneSummary] = []
    var objects: [ObjectSummary] = []
    var document: DocumentModel?
    /// The Scene/Object segmented control.
    var mode: DocRef.Kind = .scene
    var sceneZoom: Zoom = .fit
    var objectZoom: Zoom = .fit
    var showGrid: Bool = UserDefaults.standard.object(forKey: SettingsKey.showGrid) as? Bool ?? true {
        didSet { UserDefaults.standard.set(showGrid, forKey: SettingsKey.showGrid) }
    }
    var showInspector = true
    var searchText = ""
    /// The stage's visible size, for Fit.
    var stageViewport: CGSize = .zero
    /// The sidebar row or cell under the pointer ("scene:x" / "object:y").
    var hovered: String?
    /// Settings changed the site or node path since the server started.
    var settingsNeedRestart = false
    /// A short-lived message (a failed save, a server restart) for the status line.
    var notice: String?

    let thumbnails = ThumbnailStore()
    let editor = SceneEditor()
    let objectEditor = ObjectEditor()
    let timeline = TimelineModel()
    /// The Agent window: proposals, activity and the ghost preview.
    let agent = AgentModel()
    /// The palette (`palette.get`), in its order, and by name and hex.
    var palette: [PaletteColor] = []
    var paletteByName: [String: PaletteColor] = [:]
    var paletteByHex: [String: String] = [:]
    /// The New Scene / New Object sheet.
    var newDoc: NewDocDraft?
    /// The last edit's summary (the status line under the stage).
    var lastEdit: String?
    /// Open docs with unsaved changes (from `doc.changed`), for Save/Don't Save on quit.
    var dirtyDocs: Set<String> = []
    /// A save that failed in a way the user decides about.
    var saveAlert: SaveAlert?
    /// The Build art sheet.
    var build: BuildState?
    /// The user chose Don't Save: close and quit without asking again.
    @ObservationIgnored var discardConfirmed = false
    @ObservationIgnored weak var mainWindow: NSWindow?
    @ObservationIgnored let script = ScriptRunner.fromEnvironment()

    init() {
        editor.app = self
        objectEditor.app = self
        timeline.app = self
        agent.app = self
    }
    @ObservationIgnored private(set) var connection: ServerConnection?
    @ObservationIgnored private var eventsTask: Task<Void, Never>?
    @ObservationIgnored let snapshot = SnapshotRequest.fromEnvironment()
    @ObservationIgnored private var lastScene: String?
    @ObservationIgnored private var lastObject: String?

    // MARK: Settings

    var sitePath: String {
        if let env = ProcessInfo.processInfo.environment["PIXELLAB_SITE"], !env.isEmpty { return NSString(string: env).expandingTildeInPath }
        let saved = UserDefaults.standard.string(forKey: SettingsKey.site) ?? ""
        return saved.isEmpty ? Locator.defaultSitePath : NSString(string: saved).expandingTildeInPath
    }

    var appearanceSetting: AppearanceSetting {
        if let forced = snapshot?.appearance { return forced }
        return AppearanceSetting(rawValue: UserDefaults.standard.string(forKey: SettingsKey.appearance) ?? "") ?? .auto
    }

    func applyAppearance() {
        NSApp.appearance = appearanceSetting.appearance
    }

    func setHovered(_ id: String, _ inside: Bool) {
        if inside { hovered = id } else if hovered == id { hovered = nil }
    }

    // MARK: Server

    /// Finds node, the CLI and the site, starts the server and loads the site.
    func bootstrap() async {
        phase = .starting("Finding Node.js…")
        if let old = connection {
            eventsTask?.cancel()
            await old.stop()
            connection = nil
        }
        let nodeSetting = UserDefaults.standard.string(forKey: SettingsKey.node)
        let node = await Task.detached { Locator.findNode(setting: nodeSetting) }.value
        let site = sitePath
        var problem = SetupProblem()
        if node == nil {
            problem.node = (nodeSetting ?? "").isEmpty
                ? "Pixel Lab needs Node.js 22 or later, and couldn't find node in your shell's PATH, /opt/homebrew/bin or /usr/local/bin."
                : "There's no node at \(nodeSetting ?? "")."
        }
        problem.site = Locator.siteProblem(site)
        let cli = Locator.findCLI()
        if cli == nil { problem.cli = "The pixellab CLI is missing from the app (Contents/Resources/cli)." }
        guard problem.isEmpty, let node, let cli else {
            phase = .setup(problem)
            return
        }

        phase = .starting("Starting the pixellab server…")
        let conn = ServerConnection(launch: ServerLaunch(node: node, cli: cli, site: site))
        do {
            sessionInfo = try await conn.start()
        } catch {
            let diag = conn.diagnostics.trimmingCharacters(in: .whitespacesAndNewlines)
            await conn.stop()
            phase = .setup(SetupProblem(server: [error.localizedDescription, diag].filter { !$0.isEmpty }.joined(separator: "\n\n")))
            return
        }
        connection = conn
        thumbnails.reset(connection: conn)
        listen(to: conn)
        await loadPalette()
        await loadLists()
        await agent.load()
        document = nil
        phase = .ready
        await openInitialDoc()
    }

    /// Stops the server (quit, or before restarting with new settings).
    func shutdown() async {
        eventsTask?.cancel()
        eventsTask = nil
        if let connection { await connection.stop() }
        connection = nil
    }

    private func listen(to conn: ServerConnection) {
        eventsTask?.cancel()
        eventsTask = Task { [weak self] in
            for await event in conn.events {
                guard let self, !Task.isCancelled else { return }
                await self.handle(event)
            }
        }
    }

    private func handle(_ event: ServerEvent) async {
        switch event {
        case .notification(.docChanged(let status)):
            if status.dirty { dirtyDocs.insert(status.doc) } else { dirtyDocs.remove(status.doc) }
            guard let doc = document, doc.ref.id == status.doc else { return }
            doc.status = status
            if doc.ref.kind == .object {
                objectEditor.docChanged()
                // The library and scenes show the working copy.
                thumbnails.invalidate([doc.ref.name])
            }
            timeline.invalidate()
            requestRender()
            requestCheck()
            requestItems()
        case .notification(.filesChanged(let files)):
            var objectsChanged: [String] = []
            var everything = false
            for file in files {
                if file == "palette.mjs" { everything = true }
                else if file.hasPrefix("objects/"), file.hasSuffix(".mjs") {
                    objectsChanged.append(String(file.dropFirst("objects/".count).dropLast(".mjs".count)))
                }
            }
            if everything { thumbnails.invalidateAll() } else { thumbnails.invalidate(objectsChanged) }
            if everything { await loadPalette() }
            if let doc = document, doc.ref.kind == .object, everything || objectsChanged.contains(doc.ref.name) { objectEditor.docChanged() }
            await loadLists()
            timeline.invalidate()
            requestRender()
            requestCheck()
            requestItems()
        case .notification(.proposalsChanged(let proposals)):
            agent.proposalsChanged(proposals)
        case .notification(.activityAppended(let entries)):
            agent.activityAppended(entries)
        case .notification:
            break
        case .restarted(let info):
            sessionInfo = info
            notice = "The pixellab server restarted."
            thumbnails.reset(connection: connection)
            await agent.load()
            if let doc = document {
                _ = try? await connection?.call("doc.open", ["doc": .string(doc.ref.id)])
                dirtyDocs = []
                if doc.ref.kind == .object { objectEditor.docChanged() }
                timeline.invalidate()
                requestRender()
                requestCheck()
                requestItems()
            }
        case .stopped(let reason):
            phase = .setup(SetupProblem(server: reason))
        }
    }

    func loadLists() async {
        guard let connection else { return }
        do {
            async let s: SceneList = connection.call("scene.list")
            async let o: ObjectList = connection.call("object.list")
            let (sl, ol) = try await (s, o)
            scenes = sl.scenes
            objects = ol.objects
            if let doc = document, doc.ref.kind == .scene, let scene = sl.scenes.first(where: { $0.name == doc.ref.name }) {
                doc.origin = CGPoint(x: scene.origin[0], y: scene.origin[1])
            }
        } catch {
            notice = "Couldn't list the site's art: \(error.localizedDescription)"
        }
    }

    /// `palette.get`: the swatches and the hex → name map for the color under the pointer.
    func loadPalette() async {
        guard let connection else { return }
        do {
            let r: PaletteResult = try await connection.call("palette.get")
            palette = r.colors
            paletteByName = Dictionary(r.colors.map { ($0.name, $0) }, uniquingKeysWith: { a, _ in a })
            paletteByHex = Dictionary(r.colors.map { ($0.hex.uppercased(), $0.name) }, uniquingKeysWith: { a, _ in a })
        } catch {
            notice = "Couldn't read the palette: \(error.localizedDescription)"
        }
    }

    // MARK: Docs

    private func openInitialDoc() async {
        if let ref = snapshot?.doc {
            await open(ref)
            return
        }
        if let saved = UserDefaults.standard.string(forKey: SettingsKey.lastDoc), let ref = DocRef(id: saved),
           (ref.kind == .scene ? scenes.contains { $0.name == ref.name } : objects.contains { $0.name == ref.name })
        {
            await open(ref)
            return
        }
        if let name = scenes.first(where: { $0.name == "hero-island" })?.name ?? scenes.first?.name {
            await open(DocRef(.scene, name))
        }
    }

    /// Opens a doc in the session (`doc.open`), then renders and checks it.
    func open(_ ref: DocRef) async {
        guard let connection else { return }
        mode = ref.kind
        if ref.kind == .scene { lastScene = ref.name } else { lastObject = ref.name }
        if document?.ref == ref { return }
        let doc = DocumentModel(ref: ref)
        if ref.kind == .scene, let scene = scenes.first(where: { $0.name == ref.name }) {
            doc.origin = CGPoint(x: scene.origin[0], y: scene.origin[1])
            doc.summary = "\(scene.items) items\(scene.motion ? " · motion" : "")"
        } else if let object = objects.first(where: { $0.name == ref.name }) {
            doc.summary = object.description
        }
        document = doc
        if snapshot == nil, script == nil { UserDefaults.standard.set(ref.id, forKey: SettingsKey.lastDoc) }
        editor.reset()
        objectEditor.reset()
        timeline.reset()
        do {
            let status = try await connection.call("doc.open", ["doc": .string(ref.id)], as: DocStatus.self)
            doc.status = status
            if status.dirty { dirtyDocs.insert(status.doc) }
        } catch {
            doc.error = error.localizedDescription
        }
        await loadMotion()
        if ref.kind == .object { objectEditor.requestInfo() }
        requestRender()
        requestCheck()
        requestItems()
    }

    /// The Scene/Object segmented control: show the last doc of that kind;
    /// Object with a scene item selected opens that item's object.
    func switchMode(to kind: DocRef.Kind) {
        mode = kind
        guard document?.ref.kind != kind else { return }
        if kind == .object, let object = document?.selectedItem?.object, objects.contains(where: { $0.name == object }) {
            Task { await open(DocRef(.object, object)) }
            return
        }
        let name: String? = kind == .scene
            ? (lastScene ?? scenes.first { $0.name == "hero-island" }?.name ?? scenes.first?.name)
            : (lastObject ?? objects.first { $0.name == "house" }?.name ?? objects.first?.name)
        if let name { Task { await open(DocRef(kind, name)) } }
    }

    /// Renders the open doc; coalesces requests that arrive mid-render.
    func requestRender() {
        guard let doc = document, let connection else { return }
        if doc.rendering {
            doc.needsRender = true
            return
        }
        doc.rendering = true
        Task {
            repeat {
                doc.needsRender = false
                let isScene = doc.ref.kind == .scene
                let animated = isScene && timeline.hasMotion
                let t = timeline.renderTime
                let generation = timeline.generation
                let objects = objectEditor
                let objectFrame = objects.frame
                let objectKey = objects.cacheKey(frame: objectFrame)
                let objectGeneration = objects.cacheGeneration
                let acked = objects.ackedJob
                if !isScene, objects.info != nil, let cached = objects.cached(frame: objectFrame) {
                    doc.render = cached
                    objects.renderColors = PixelColors(cgImage: cached.image.cgImage, scale: cached.scale)
                    objects.reconcile(upTo: acked)
                    doc.renderCount += 1
                    continue
                }
                if animated, let frame = timeline.cached(t), let image = PixelImage(png: frame.png, renderScale: frame.scale) {
                    doc.render = StageImage(image: image, viewBox: frame.viewBox, scale: frame.scale, items: frame.items, anchor: nil, time: Double(t))
                    doc.renderCount += 1
                    continue
                }
                do {
                    let method = isScene ? "scene.render" : "object.render"
                    var params: [String: JSONValue] = ["name": .string(doc.ref.name), "scale": 1]
                    if animated { params["time"] = JSONValue(t) }
                    if !isScene { params.merge(objects.renderParams(frame: objectFrame)) { _, b in b } }
                    let r: RenderResult = try await connection.call(method, params)
                    let viewBox = CGRect(x: r.viewBox[0], y: r.viewBox[1], width: r.viewBox[2], height: r.viewBox[3])
                    if animated {
                        timeline.store(t, .init(png: r.png, scale: r.scale, viewBox: viewBox, items: r.items), generation: generation)
                    }
                    if let image = PixelImage(png: r.png, renderScale: r.scale) {
                        let stage = StageImage(
                            image: image,
                            viewBox: viewBox,
                            scale: r.scale,
                            items: r.items,
                            anchor: r.anchor.map { CGPoint(x: $0[0], y: $0[1]) },
                            time: r.time
                        )
                        doc.render = stage
                        doc.error = nil
                        if !isScene, document === doc {
                            objects.renderColors = PixelColors(cgImage: image.cgImage, scale: r.scale)
                            objects.reconcile(upTo: acked)
                            if objects.info != nil { objects.store(stage, key: objectKey, generation: objectGeneration) }
                        }
                    }
                } catch {
                    doc.error = error.localizedDescription
                }
                doc.renderCount += 1
            } while doc.needsRender && document === doc
            doc.rendering = false
            if doc.ref.kind == .object, document === doc, !objectEditor.playing { objectEditor.requestExtras() }
            if document === doc { agent.syncGhost() }
        }
    }

    /// Loads the open scene's items (the Items tree, the inspector, hit-testing
    /// object art); coalesces like `requestRender`.
    func requestItems() {
        guard let doc = document, doc.ref.kind == .scene, let connection else { return }
        if doc.loadingItems {
            doc.needsItems = true
            return
        }
        doc.loadingItems = true
        Task {
            repeat {
                doc.needsItems = false
                do {
                    let r: SceneItemsResult = try await connection.call("scene.items", ["name": .string(doc.ref.name)])
                    doc.setItems(r)
                    thumbnails.prefetch(r.items.map(\.object))
                } catch {
                    notice = "Couldn't list the items of \(doc.ref.name): \(error.localizedDescription)"
                }
                if timeline.hasMotion || !timeline.tracks.isEmpty { await loadMotion() }
            } while doc.needsItems && document === doc
            doc.loadingItems = false
            if document === doc { editor.syncDrafts() }
        }
    }

    /// The open scene's motion: loop length and tracks (`scene.show --doc`).
    func loadMotion() async {
        guard let doc = document, doc.ref.kind == .scene, let connection else { return }
        do {
            let r: SceneShowMotion = try await connection.call("scene.show", ["name": .string(doc.ref.name), "doc": true])
            if document === doc { timeline.load(r) }
        } catch {
            if document === doc { timeline.load(SceneShowMotion(name: doc.ref.name, doc: nil)) }
        }
    }

    /// Runs `check` for the open doc (its file and what it uses).
    func requestCheck() {
        guard let doc = document, let connection else { return }
        if doc.checking {
            doc.needsCheck = true
            return
        }
        doc.checking = true
        Task {
            repeat {
                doc.needsCheck = false
                do {
                    let r: CheckResult = try await connection.call(
                        "check", ["name": .string(doc.ref.name), "kind": .string(doc.ref.kind.rawValue)])
                    doc.problems = r.problems
                    doc.checkedCount = r.checked
                    doc.checkSummary = r.summary
                } catch {
                    doc.problems = [CheckProblem(file: doc.ref.file, message: error.localizedDescription, text: error.localizedDescription)]
                }
            } while doc.needsCheck && document === doc
            doc.checking = false
        }
    }

    // MARK: Commands

    var canSave: Bool { document?.status?.dirty ?? false }
    var canUndo: Bool { document?.status?.canUndo ?? false }
    var canRedo: Bool { document?.status?.canRedo ?? false }

    /// Saves the open doc. `invalid` and `conflict` become an alert the user
    /// answers; returns whether it saved.
    @discardableResult
    func save(force: Bool = false) async -> Bool {
        guard let doc = document else { return false }
        return await save(doc.ref, force: force)
    }

    @discardableResult
    func save(_ ref: DocRef, force: Bool = false) async -> Bool {
        guard let connection else { return false }
        do {
            var params: [String: JSONValue] = ["doc": .string(ref.id)]
            if force { params["force"] = true }
            let r: SaveResult = try await connection.call("doc.save", params)
            let status = DocStatus(doc: r.doc, dirty: r.dirty, canUndo: r.canUndo, canRedo: r.canRedo)
            if let doc = document, doc.ref == ref { doc.status = status }
            if r.dirty { dirtyDocs.insert(r.doc) } else { dirtyDocs.remove(r.doc) }
            notice = nil
            lastEdit = r.wrote == false ? "\(ref.name) was already saved" : "Saved \(r.file ?? ref.name)"
            return true
        } catch let e as RPCError {
            switch e.dataCode {
            case "invalid":
                saveAlert = SaveAlert(kind: .invalid, doc: ref, message: e.message, problems: Self.lines(e.details))
            case "conflict":
                saveAlert = SaveAlert(kind: .conflict, doc: ref, message: e.message, problems: [])
            default:
                notice = "Couldn't save \(ref.name): \(e.message)"
            }
        } catch {
            notice = "Couldn't save \(ref.name): \(error.localizedDescription)"
        }
        return false
    }

    /// An error's `details` as lines (strings, or objects with message/text).
    static func lines(_ details: JSONValue?) -> [String] {
        guard let a = details?.arrayValue else { return details?.stringValue.map { [$0] } ?? [] }
        return a.compactMap { $0.stringValue ?? $0["text"]?.stringValue ?? $0["message"]?.stringValue }
    }

    /// Asks Save / Don't Save / Cancel when open docs have unsaved changes.
    /// Returns whether to go on (close or quit).
    func confirmUnsaved(window: NSWindow?) async -> Bool {
        if discardConfirmed || dirtyDocs.isEmpty || script != nil { return true }
        let refs = dirtyDocs.sorted().compactMap { DocRef(id: $0) }
        let names = refs.map(\.name)
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = names.count == 1
            ? "Do you want to save the changes you made to \(names[0])?"
            : "Do you want to save the changes to \(names.count) documents?"
        alert.informativeText = (names.count == 1 ? "" : names.joined(separator: ", ") + "\n\n") + "Your changes will be lost if you don't save them."
        alert.addButton(withTitle: "Save")
        let dont = alert.addButton(withTitle: "Don't Save")
        dont.hasDestructiveAction = true
        alert.addButton(withTitle: "Cancel")
        let response: NSApplication.ModalResponse
        if let window, window.isVisible { response = await alert.beginSheetModal(for: window) } else { response = alert.runModal() }
        switch response {
        case .alertFirstButtonReturn:
            for ref in refs where !(await save(ref)) { return false }
            discardConfirmed = true
            return true
        case .alertSecondButtonReturn:
            discardConfirmed = true
            return true
        default:
            return false
        }
    }

    /// ⌘B: runs the site's `npm run art` and shows the report.
    func buildArt() async {
        guard let connection, build?.running != true else { return }
        build = BuildState(running: true)
        do {
            let r: ArtBuildResult = try await connection.call("art.build")
            build = BuildState(running: false, result: r)
        } catch {
            build = BuildState(running: false, error: error.localizedDescription)
        }
    }

    /// A doc's file was written outside the editor (an approved proposal):
    /// the open copy reloads if it was clean (the server says so with
    /// doc.changed); render, check and list again either way.
    func docWritten(_ ref: DocRef) async {
        if ref.kind == .object { thumbnails.invalidate([ref.name]) }
        if let doc = document, doc.ref == ref {
            if let status: DocStatus = try? await connection?.call("doc.status", ["doc": .string(ref.id)]) {
                doc.status = DocStatus(doc: status.doc, dirty: status.dirty, canUndo: status.canUndo, canRedo: status.canRedo)
                if status.dirty { dirtyDocs.insert(status.doc) } else { dirtyDocs.remove(status.doc) }
            }
            if ref.kind == .object { objectEditor.docChanged() }
            timeline.invalidate()
            requestRender()
            requestCheck()
            requestItems()
        }
        await loadLists()
    }

    func revert() async { await docCommand("doc.revert") }
    func undo() async { await docCommand("doc.undo") }
    func redo() async { await docCommand("doc.redo") }

    private func docCommand(_ method: String) async {
        guard let doc = document, let connection else { return }
        do {
            let r: UndoResult = try await connection.call(method, ["doc": .string(doc.ref.id)])
            doc.status = DocStatus(doc: r.doc, dirty: r.dirty, canUndo: r.canUndo, canRedo: r.canRedo)
            if r.dirty { dirtyDocs.insert(r.doc) } else { dirtyDocs.remove(r.doc) }
            if let s = r.summary { lastEdit = s }
        } catch {
            notice = error.localizedDescription
        }
    }

    // MARK: Zoom

    var zoom: Zoom {
        get { mode == .scene ? sceneZoom : objectZoom }
        set { if mode == .scene { sceneZoom = newValue } else { objectZoom = newValue } }
    }

    /// Stage padding around the art when it's bigger than the stage, in points.
    static let stageMargin: CGFloat = 32
    /// The least room Fit leaves around the art.
    static let fitMargin: CGFloat = 8

    /// Fit: the largest whole zoom at which the art fits the stage (at least the
    /// kind's smallest level).
    func fitZoom(art: CGSize, viewport: CGSize, kind: DocRef.Kind) -> Int {
        let levels = kind == .scene ? 1...4 : 4...16
        guard art.width > 0, art.height > 0, viewport.width > 0 else { return levels.lowerBound }
        let w = (viewport.width - 2 * Self.fitMargin) / art.width
        let h = (viewport.height - 2 * Self.fitMargin) / art.height
        let fit = Int(floor(min(w, h)))
        return min(max(fit, levels.lowerBound), levels.upperBound)
    }

    /// The zoom the stage draws at now.
    var effectiveZoom: Int {
        guard let doc = document else { return 1 }
        switch zoom {
        case .level(let n): return n
        case .fit:
            if let forced = snapshot?.zoom { return forced }
            return fitZoom(art: doc.render?.artSize ?? .zero, viewport: stageViewport, kind: doc.ref.kind)
        }
    }

    func setZoomLevel(index: Int) {
        let levels = Zoom.levels(for: mode)
        guard levels.indices.contains(index) else { return }
        zoom = .level(levels[index])
    }
}

/// A save the server refused: problems to fix, or a file changed on disk.
struct SaveAlert: Identifiable {
    enum Kind { case invalid, conflict }
    var kind: Kind
    var doc: DocRef
    var message: String
    var problems: [String]
    var id: String { "\(kind)-\(doc.id)" }
}

/// The Build art sheet: running, then the report.
struct BuildState {
    var running: Bool
    var result: ArtBuildResult?
    var error: String?
}
