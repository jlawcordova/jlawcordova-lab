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

/// What the zoom readout shows: the level, and a number that tells one showing from the next.
struct ZoomReadoutValue: Equatable, Sendable {
    var level: Int
    var id: Int
    var text: String { "\(level)×" }
}

/// Why the app can't open the document yet; the setup screen shows it.
struct SetupProblem: Equatable {
    var document: String?
    /// The program is missing from the app.
    var program: String?
    var server: String?
    /// The document is in format 1 (R9): the setup screen offers Migrate….
    var migrate = false
    var isEmpty: Bool { document == nil && program == nil && server == nil }
}

/// The app's state: the server connection, the document's scenes and objects,
/// the open doc and the stage's view settings. One per app; the views read it
/// from the environment.
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
    /// View › Show Published Frame: the outline of the box a scene publishes at.
    var showFrame: Bool = UserDefaults.standard.object(forKey: SettingsKey.showFrame) as? Bool ?? true {
        // Remembered, except under the script hook or a snapshot run, which must leave the owner's settings alone.
        didSet { if script == nil, snapshot == nil { UserDefaults.standard.set(showFrame, forKey: SettingsKey.showFrame) } }
    }
    var showInspector = true
    /// The Library popover over the stage is open (docs/intents/2026-10-floating-library/spec.md, R8).
    var libraryOpen = false

    func toggleLibrary() {
        libraryOpen.toggle()
        editor.sidebarTab = libraryOpen ? .library : .layers
    }
    var searchText = ""
    /// The stage's visible size, for Fit.
    var stageViewport: CGSize = .zero {
        didSet { if oldValue != stageViewport { ensureWorkspace() } }
    }
    /// The sidebar row or cell under the pointer ("scene:x" / "object:y").
    var hovered: String?
    /// Settings changed the document path since the server started.
    var settingsNeedRestart = false
    /// Migrate… asked for (R9): the alert is up.
    var migrateAsking = false
    /// `document migrate` is running, then the server restarts on the migrated document.
    var migrating = false
    /// A line under the setup screen's rows: why the migration failed.
    var migrateMessage: String?
    /// A short-lived message (a failed save, a server restart) for the status line.
    var notice: String?

    let thumbnails = ThumbnailStore()
    let editor = SceneEditor()
    /// Scene layers: inside, hidden, locked, the menu (docs/intents/2026-10-scene-layers/spec.md).
    let layers = LayersModel()
    let objectEditor = ObjectEditor()
    let timeline = TimelineModel()
    /// The Agent window: proposals, activity and the ghost preview.
    let agent = AgentModel()
    /// The Agent tab's chat with Claude Code (docs/intents/2026-10-agent-chat/spec.md).
    let chat = ChatModel()
    /// The right column's tab (spec D12): remembered, except under the script hook or a snapshot.
    var rightTab: RightTab = RightTab(rawValue: UserDefaults.standard.string(forKey: SettingsKey.rightTab) ?? "") ?? .inspector {
        didSet { if script == nil, snapshot == nil { UserDefaults.standard.set(rightTab.rawValue, forKey: SettingsKey.rightTab) } }
    }
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
    /// Open docs with unsaved changes whose file changed on disk (`doc.stale`): the stage shows a banner.
    var staleDocs: Set<String> = []
    /// Stale docs the owner chose Keep Mine for: no banner until the file changes again.
    var keptStale: Set<String> = []
    /// A merge from disk that clashed: the alert's text.
    var mergeAlert: String?
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
        layers.app = self
        objectEditor.app = self
        timeline.app = self
        agent.app = self
        chat.app = self
        // The script hook and a snapshot start on the default tab, not the one the owner left open.
        if script != nil || snapshot != nil { rightTab = .inspector }
    }
    @ObservationIgnored private(set) var connection: ServerConnection?
    @ObservationIgnored private var eventsTask: Task<Void, Never>?
    @ObservationIgnored let snapshot = SnapshotRequest.fromEnvironment()
    @ObservationIgnored private var lastScene: String?
    @ObservationIgnored private var lastObject: String?

    // MARK: Settings

    /// The document the server opens: `PIXELLAB_DOC`, else `PIXELLAB_SITE`
    /// (the script hook and snapshots), else Settings, else the default site's.
    var documentLocation: DocumentLocation {
        if let opened = openedDocument { return .document(opened) }
        return DocumentLocation.resolve(environment: ProcessInfo.processInfo.environment, saved: UserDefaults.standard.string(forKey: SettingsKey.document))
    }

    /// A document opened in this run (Open, New, Save As, Finder): it beats the
    /// environment, so the script hook's `saveas` switches too.
    @ObservationIgnored private var openedDocument: String?

    /// The path the setup screen and Settings show.
    var documentPath: String { Locator.path(of: documentLocation) }

    /// The document's name (its manifest's, else its folder's), once the server is up.
    var documentName: String? { sessionInfo?.document?.name }

    /// The main window's title: the open doc, then the document.
    var windowTitle: String {
        let parts = [document?.title, documentName].compactMap { $0 }
        return parts.isEmpty ? "Pixel Lab" : parts.joined(separator: " — ")
    }

    var appearanceSetting: AppearanceSetting {
        if let forced = snapshot?.appearance { return forced }
        return AppearanceSetting(rawValue: UserDefaults.standard.string(forKey: SettingsKey.appearance) ?? "") ?? .auto
    }

    func applyAppearance() {
        NSApp.appearance = appearanceSetting.appearance
    }

    /// The night appearance: scenes render as the site's dark theme does (the
    /// hero island's drop shadow takes the page color, `scene.render --theme dark`).
    var stageDark = false
    var stageTheme: JSONValue { .string(stageDark ? "dark" : "light") }
    @ObservationIgnored private var appearanceObservation: NSKeyValueObservation?

    /// Follows the effective appearance (the setting, or the system's in Auto).
    func watchAppearance() {
        updateStageTheme()
        appearanceObservation = NSApp.observe(\.effectiveAppearance, options: [.new]) { _, _ in
            Task { @MainActor in AppModel.shared.updateStageTheme() }
        }
    }

    func updateStageTheme() {
        let dark = NSApp.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        guard dark != stageDark else { return }
        stageDark = dark
        timeline.invalidate()
        requestRender()
        agent.syncGhost()
    }

    func setHovered(_ id: String, _ inside: Bool) {
        if inside { hovered = id } else if hovered == id { hovered = nil }
    }

    // MARK: Server

    /// Finds the program and the document, starts the server and loads the document.
    func bootstrap() async {
        phase = .starting("Starting…")
        if let old = connection {
            eventsTask?.cancel()
            await old.stop()
            connection = nil
        }
        let location = documentLocation
        var problem = SetupProblem()
        problem.document = Locator.documentProblem(location)
        problem.migrate = Locator.needsMigration(location)
        let program = Locator.findProgram()
        if program == nil { problem.program = "The pixellab program is missing from the app (Contents/Helpers/pixellab)." }
        guard problem.isEmpty, let program else {
            phase = .setup(problem)
            return
        }

        phase = .starting("Starting the pixellab server…")
        let conn = ServerConnection(launch: ServerLaunch(program: program, location: location))
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
        chat.documentChanged()
        if rightTab == .agent { chat.start() }
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
            if status.dirty { dirtyDocs.insert(status.doc) } else {
                dirtyDocs.remove(status.doc)
                staleDocs.remove(status.doc)
                keptStale.remove(status.doc)
            }
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
                if file == "palette.json" { everything = true }
                else if file.hasPrefix("objects/"), file.hasSuffix(".json") {
                    objectsChanged.append(String(file.dropFirst("objects/".count).dropLast(".json".count)))
                }
            }
            if everything { thumbnails.invalidateAll() } else { thumbnails.invalidate(objectsChanged) }
            timeline.forgetObjects(everything ? nil : objectsChanged)
            if everything { await loadPalette() }
            if let doc = document, doc.ref.kind == .object, everything || objectsChanged.contains(doc.ref.name) { objectEditor.docChanged() }
            await loadLists()
            timeline.invalidate()
            requestRender()
            requestCheck()
            requestItems()
        case .notification(.docStale(let doc, _)):
            staleDocs.insert(doc)
            keptStale.remove(doc)
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
                staleDocs = []
                keptStale = []
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
        layers.reset()
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
                // Hidden layers and the layer you're inside (scene.render --hide, --focus).
                let layerParams = isScene ? layers.renderParams : [:]
                if isScene { layers.renderedParams = layerParams }
                let focus = layerParams["focus"]?.stringValue
                let t = timeline.renderTime
                let generation = timeline.generation
                // A scene's size and the workspace around it (docs/intents/2026-10-scene-canvas/spec.md, R9).
                if isScene { await refreshFrame(doc, connection: connection) }
                let window = isScene ? workspace(for: doc) : nil
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
                if animated, focus == nil, let frame = timeline.cached(t), frame.viewBox == window, let image = PixelImage(png: frame.png, renderScale: frame.scale) {
                    doc.render = StageImage(image: image, viewBox: frame.viewBox, scale: frame.scale, items: frame.items, anchor: nil, time: Double(t))
                    doc.renderCount += 1
                    continue
                }
                do {
                    let method = isScene ? "scene.render" : "object.render"
                    var params: [String: JSONValue] = ["name": .string(doc.ref.name), "scale": 1]
                    if animated { params["time"] = JSONValue(t) }
                    if isScene { params["theme"] = stageTheme }
                    if let window { params["view"] = window.sceneView }
                    params.merge(layerParams) { _, b in b }
                    if !isScene { params.merge(objects.renderParams(frame: objectFrame)) { _, b in b } }
                    let r: RenderResult = try await connection.call(method, params)
                    let viewBox = CGRect(x: r.viewBox[0], y: r.viewBox[1], width: r.viewBox[2], height: r.viewBox[3])
                    if animated, focus == nil {
                        timeline.store(t, .init(png: r.png, scale: r.scale, viewBox: viewBox, items: r.items), generation: generation)
                    }
                    if let image = PixelImage(png: r.png, renderScale: r.scale) {
                        let stage = StageImage(
                            image: image,
                            viewBox: viewBox,
                            scale: r.scale,
                            items: r.items,
                            anchor: r.anchor.map { CGPoint(x: $0[0], y: $0[1]) },
                            time: r.time,
                            focusImage: r.focusPng.flatMap { PixelImage(png: $0, renderScale: r.scale) },
                            focusLayer: r.focusPng == nil ? nil : focus
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
            // A scene just opened: if nothing has the keyboard, the stage does, so Tab, ⌘↓, the arrows and
            // Return work without a click first (a text field or a list the owner is in keeps it).
            if document === doc, !doc.stageFocused, doc.render != nil, doc.ref.kind == .scene {
                doc.stageFocused = true
                // The stage's view is made by the layout after this render lands: wait for it.
                Task {
                    for _ in 0..<20 {
                        try? await Task.sleep(for: .milliseconds(50))
                        guard document === doc else { return }
                        if let w = editor.stageView?.window {
                            if w.firstResponder === w { editor.focusStage() }
                            return
                        }
                    }
                }
            }
            if doc.ref.kind == .object, document === doc, !objectEditor.playing { objectEditor.requestExtras() }
            if document === doc { agent.syncGhost() }
        }
    }

    /// `scene.frame`: the box the open scene publishes at, asked again once per
    /// change of the doc (the timeline's generation counts them).
    func refreshFrame(_ doc: DocumentModel, connection: ServerConnection) async {
        let generation = timeline.generation
        if doc.frameInfo != nil, doc.frameStamp == generation { return }
        if let r: SceneFrameResult = try? await connection.call("scene.frame", ["name": .string(doc.ref.name)]), r.viewBox.count == 4 {
            doc.frameInfo = r
            doc.frameStamp = generation
        }
    }

    /// `scene.frame` with params (fit, box, margin) on the open scene; the doc
    /// changes, so the stage renders again.
    @discardableResult
    func frameCommand(_ params: [String: JSONValue]) async -> SceneFrameResult? {
        guard let doc = document, doc.ref.kind == .scene, let connection else { return nil }
        var p = params
        p["name"] = .string(doc.ref.name)
        do {
            let r: SceneFrameResult = try await connection.call("scene.frame", p)
            if document === doc {
                doc.frameInfo = r
                doc.frameStamp = nil
                doc.syncFrameDraft()
                timeline.invalidate()
                requestRender()
            }
            if let s = r.summary { editor.announce(s); lastEdit = s }
            notice = nil
            return r
        } catch let e as RPCError {
            notice = e.message
        } catch {
            notice = error.localizedDescription
        }
        return nil
    }

    /// Fit to content: remove the scene's fixed size.
    func fitFrame() async { await frameCommand(["fit": true]) }

    /// Fix the size to the box typed in the inspector ("x, y, w, h").
    func applyFrameDraft() async {
        guard let doc = document else { return }
        let n = doc.frameDraft.split(whereSeparator: { ", ".contains($0) }).compactMap { Int($0) }
        guard n.count == 4, n[2] > 0, n[3] > 0 else {
            notice = "The published box is four whole numbers: x, y, width, height."
            return
        }
        await frameCommand(["box": .array(n.map { JSONValue($0) })])
    }

    /// What the stage must hold: the published box, where the cursor has been
    /// and a proposal's ghost.
    func neededRect(for doc: DocumentModel) -> CGRect {
        var r = doc.publishBox
        if let reach = doc.reach { r = r.union(reach) }
        if let ghost = agent.visibleGhost, ghost.ref == doc.ref { r = r.union(ghost.image.viewBox) }
        return r
    }

    /// The window the stage renders now, growing the doc's if the box, the
    /// cursor, the zoom or the viewport need more room.
    func workspace(for doc: DocumentModel) -> CGRect {
        let pad = StageWindow.pad(viewport: stageViewport, zoom: effectiveZoom)
        let window = StageWindow.grow(current: doc.window, needed: neededRect(for: doc), pad: pad)
        if doc.window != window { doc.window = window }
        return window
    }

    /// The zoom, the viewport or the cursor changed: render again if the
    /// workspace has to grow to hold what's on stage.
    func ensureWorkspace() {
        guard let doc = document, doc.ref.kind == .scene, let window = doc.window else { return }
        let pad = StageWindow.pad(viewport: stageViewport, zoom: effectiveZoom)
        if StageWindow.grow(current: window, needed: neededRect(for: doc), pad: pad) != window { requestRender() }
    }

    /// The stage cursor moved: the workspace holds its tile.
    func reachTile(_ t: Tile) {
        guard let doc = document, doc.ref.kind == .scene, let origin = doc.origin else { return }
        let c = StageGeometry.tileToScene(col: t.col, row: t.row, level: t.level, origin: origin)
        let r = CGRect(x: c.x - 16, y: c.y - 8 - 16, width: 32, height: 48)
        doc.reach = doc.reach.map { $0.union(r) } ?? r
        ensureWorkspace()
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
                // Taken as it's sent: an edit that comes back later makes this list one from before it.
                let stamp = doc.itemsStamp(editsPending: editor.pendingEdits)
                do {
                    let r: SceneItemsResult = try await connection.call("scene.items", ["name": .string(doc.ref.name)])
                    doc.setItems(r, asOf: stamp)
                    if document === doc { layers.itemsChanged() }
                    thumbnails.prefetch(r.items.map(\.object))
                } catch {
                    notice = "Couldn't list the items of \(doc.ref.name): \(error.localizedDescription)"
                }
                await loadMotion()
            } while doc.needsItems && document === doc
            doc.loadingItems = false
            if document === doc { editor.syncDrafts() }
        }
    }

    /// Lists the open scene's items now, after a layer edit, so what the edit
    /// made can be selected at once (an earlier `requestItems` may still be in flight).
    func reloadItems() async {
        guard let doc = document, doc.ref.kind == .scene, let connection else { return }
        let stamp = doc.itemsStamp(editsPending: editor.pendingEdits)
        if let r: SceneItemsResult = try? await connection.call("scene.items", ["name": .string(doc.ref.name)]), document === doc {
            doc.setItems(r, asOf: stamp)
            layers.itemsChanged()
        }
    }

    /// The open scene's motion: loop length and tracks (`scene.show --doc`).
    func loadMotion() async {
        guard let doc = document, doc.ref.kind == .scene, let connection else { return }
        do {
            let r: SceneShowMotion = try await connection.call("scene.show", ["name": .string(doc.ref.name), "doc": true])
            // The layers of the objects the scene places carry ambient loops and actions too.
            var names = Set<String>()
            Animation.walk(r.doc?.items) { _, item in if let o = item["object"]?.stringValue { names.insert(o) } }
            var objects: [String: [Int: ElementAnimation]] = [:]
            for name in names.sorted() where timeline.objectAnimation[name] == nil {
                let o: ObjectShowResult? = try? await connection.call("object.show", ["name": .string(name), "doc": true])
                objects[name] = Animation.layers(of: o?.doc)
            }
            if document === doc {
                let before = (timeline.duration, timeline.itemAnimation)
                timeline.load(r, objects: objects)
                // A loop that came or went changes what the stage shows at a time.
                if before.0 != timeline.duration || before.1 != timeline.itemAnimation { timeline.invalidate(); requestRender() }
            }
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
                    doc.warnings = r.warnings ?? []
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
            staleDocs.remove(r.doc)
            keptStale.remove(r.doc)
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

    /// ⌘B: `art.build` compiles every scene with an output and writes the SVGs
    /// into the document's publish folder, or into a folder the owner picks
    /// (`askFolder`, or when the manifest names none); the sheet shows the report.
    /// `dir` is the script hook's folder, so no panel opens.
    func publish(askFolder: Bool = false, dir: String? = nil) async {
        guard let connection, build?.running != true else { return }
        var folder = dir
        if folder == nil, askFolder || sessionInfo?.document?.publishDir == nil {
            guard script == nil, snapshot == nil else {
                build = BuildState(running: false, error: "This document has no publish folder; the script passes one with `publish <dir>`.")
                return
            }
            guard let picked = SetupView.chooseFolder(title: "Publish to a folder", message: "The compiled SVGs are written into this folder.", start: sessionInfo?.document?.publishDir ?? documentPath) else { return }
            folder = picked
        }
        build = BuildState(running: true)
        do {
            let params: [String: JSONValue] = folder.map { ["dir": .string($0)] } ?? [:]
            let r: ArtBuildResult = try await connection.call("art.build", params)
            build = BuildState(running: false, result: r)
        } catch {
            build = BuildState(running: false, error: error.localizedDescription)
        }
    }

    // MARK: Documents

    /// New Document…: a save panel for the `.pixellab`, `document new` through
    /// the program (the server needn't be up), then a switch to it.
    func newDocument(at path: String? = nil) async {
        guard let path = path ?? SetupView.chooseNewDocument(title: "New Pixel Lab document", name: "Untitled.pixellab", start: documentPath) else { return }
        guard let program = Locator.findProgram() else {
            notice = "Pixel Lab's program is missing, so it can't make a document."
            return
        }
        do {
            let out = try await Locator.runCli(program: program, ["document", "new", path, "--json"])
            let made = try JSONDecoder().decode(DocumentNewResult.self, from: Data(out.utf8))
            await switchDocument(to: made.root)
        } catch {
            notice = "Couldn't make the document: \(error.localizedDescription)"
            if case .setup(var p) = phase {
                p.document = "Couldn't make the document: \(error.localizedDescription)"
                phase = .setup(p)
            }
        }
    }

    /// The format-1 document's name for the Migrate alert (R9).
    var migrateName: String {
        switch documentLocation {
        case .document(let p): return URL(fileURLWithPath: p).lastPathComponent
        case .site(let p):
            let art = URL(fileURLWithPath: p).appendingPathComponent("src/assets/pixel-art")
            let old = ((try? FileManager.default.contentsOfDirectory(atPath: art.path)) ?? []).sorted().first {
                $0.hasSuffix(".pixellab") && FileManager.default.fileExists(atPath: art.appendingPathComponent("\($0)/palette.mjs").path)
            }
            return old ?? "The document"
        }
    }

    /// Migrate… (the setup screen, R9): asks first.
    func askMigrate() {
        guard case .setup(let p) = phase, p.migrate, !migrating else { return }
        migrateMessage = nil
        migrateAsking = true
    }

    /// The alert's Migrate: `pixellab document migrate` on the document, in place, then the server starts on it.
    func migrate() async {
        migrateAsking = false
        guard case .setup(let p) = phase, p.migrate, !migrating else { return }
        guard let program = Locator.findProgram() else {
            migrateMessage = "Pixel Lab's program is missing, so it can't migrate the document."
            return
        }
        migrating = true
        migrateMessage = nil
        defer { migrating = false }
        do {
            _ = try await Locator.runCli(program: program, ["document", "migrate"] + documentLocation.arguments)
        } catch {
            migrateMessage = "Couldn't migrate: \(error.localizedDescription)"
            return
        }
        await bootstrap()
    }

    /// Open Document… (⌘O): a `.pixellab` folder, or a site checkout holding one.
    func openDocument() async {
        guard let path = SetupView.chooseDocumentFolder(start: documentPath) else { return }
        await switchDocument(to: path)
    }

    /// Save As… (⇧⌘S): `document.copy` the open document whole, then switch to the copy.
    func saveAs(to path: String? = nil) async {
        guard let connection, phase == .ready else { return }
        guard let path = path ?? SetupView.chooseNewDocument(title: "Save a copy of the document", name: "\(documentName ?? "Untitled").pixellab", start: sessionInfo?.document?.root) else { return }
        do {
            let r: DocumentCopyResult = try await connection.call("document.copy", ["to": .string(path)])
            await switchDocument(to: r.to)
        } catch {
            notice = "Couldn't save a copy: \(error.localizedDescription)"
        }
    }

    /// Makes `path` the document (Settings, so it's remembered) and restarts
    /// the server on it, after any unsaved work is dealt with.
    func switchDocument(to path: String) async {
        guard await confirmUnsaved(window: mainWindow) else { return }
        discardConfirmed = false
        openedDocument = path
        // Remembered in Settings, except under the script hook or a snapshot run, which must leave the owner's settings alone.
        if script == nil, snapshot == nil { UserDefaults.standard.set(path, forKey: SettingsKey.document) }
        settingsNeedRestart = false
        await bootstrap()
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

    func revert() async {
        await docCommand("doc.revert")
        if let ref = document?.ref.id, document?.status?.dirty == false { staleDocs.remove(ref); keptStale.remove(ref) }
    }

    // MARK: A doc that changed on disk (docs/intents/2026-10-mcp-gaps/spec.md, D5)

    /// Whether the stage shows the changed-on-disk banner for the open doc.
    var showsStaleBanner: Bool {
        guard let id = document?.ref.id else { return false }
        return Self.staleBannerShown(doc: id, stale: staleDocs, kept: keptStale)
    }

    nonisolated static func staleBannerShown(doc: String, stale: Set<String>, kept: Set<String>) -> Bool {
        stale.contains(doc) && !kept.contains(doc)
    }

    /// Merge: `doc.merge` brings the file's change into the open copy, keeping the unsaved edits.
    /// A clash leaves everything as it was and says so in an alert.
    func mergeFromDisk() async {
        guard let doc = document, let connection else { return }
        let ref = doc.ref
        do {
            let r: UndoResult = try await connection.call("doc.merge", ["doc": .string(ref.id)])
            doc.status = DocStatus(doc: r.doc, dirty: r.dirty, canUndo: r.canUndo, canRedo: r.canRedo)
            if r.dirty { dirtyDocs.insert(r.doc) } else { dirtyDocs.remove(r.doc) }
            staleDocs.remove(ref.id)
            keptStale.remove(ref.id)
            lastEdit = r.summary ?? "Merged \(ref.file)"
        } catch let e as RPCError where e.dataCode == "merge_conflict" || e.dataCode == "merge_invalid" {
            mergeAlert = e.message
        } catch {
            notice = "Couldn't merge \(ref.name): \(error.localizedDescription)"
        }
    }

    /// Reload: the file as it is now, dropping the unsaved edits (one undoable step, as Revert).
    func reloadFromDisk() async { await revert() }

    /// Keep Mine: hide the banner. Saving later still asks before it overwrites the file.
    func keepMine() {
        guard let id = document?.ref.id else { return }
        keptStale.insert(id)
    }
    func undo() async { await docCommand("doc.undo") }
    func redo() async { await docCommand("doc.redo") }

    private func docCommand(_ method: String) async {
        guard let doc = document, let connection else { return }
        // On its way like an edit: items listed meanwhile may hold the paths from before it.
        editor.pendingEdits += 1
        defer { editor.pendingEdits -= 1 }
        do {
            let r: UndoResult = try await connection.call(method, ["doc": .string(doc.ref.id)])
            doc.status = DocStatus(doc: r.doc, dirty: r.dirty, canUndo: r.canUndo, canRedo: r.canRedo)
            if r.dirty { dirtyDocs.insert(r.doc) } else { dirtyDocs.remove(r.doc) }
            if let s = r.summary { lastEdit = s }
            // The step may have given items other paths: the selection follows its items.
            if document === doc { layers.followSort(r.reordered) }
        } catch {
            notice = error.localizedDescription
        }
    }

    // MARK: Zoom

    var zoom: Zoom {
        get { mode == .scene ? sceneZoom : objectZoom }
        set {
            let before = effectiveZoom
            if mode == .scene { sceneZoom = newValue } else { objectZoom = newValue }
            ensureWorkspace()
            let after = effectiveZoom
            if after != before { showZoomReadout(after) }
        }
    }

    /// The readout over the stage's corner after a zoom change ("2×"): it clears itself
    /// after `readoutSeconds` (docs/intents/2026-10-floating-library/spec.md, R3).
    var zoomReadout: ZoomReadoutValue?
    @ObservationIgnored var readoutSeconds = 1.1
    @ObservationIgnored private var readoutCount = 0

    private func showZoomReadout(_ level: Int) {
        readoutCount += 1
        let value = ZoomReadoutValue(level: level, id: readoutCount)
        zoomReadout = value
        editor.announce("Zoom \(level)×")
        Task { [weak self, seconds = readoutSeconds] in
            try? await Task.sleep(for: .seconds(seconds))
            guard let self, self.zoomReadout?.id == value.id else { return }
            self.zoomReadout = nil
        }
    }

    /// One whole level in or out of the open doc's levels (a pinch, ⌘-scroll, ⌘= and ⌘−), starting
    /// from Fit's effective level and never past the kind's first and last (R1, R2). `anchor` is a
    /// point of the stage's content to keep where it is on screen; without one the middle of the
    /// stage stays put. Returns whether the zoom changed.
    @discardableResult
    func stepZoom(by steps: Int, anchor: CGPoint? = nil) -> Bool {
        guard steps != 0, let doc = document else { return false }
        let levels = Zoom.levels(for: doc.ref.kind)
        let current = effectiveZoom
        let at = levels.lastIndex { $0 <= current } ?? 0
        let next = min(max(at + steps, 0), levels.count - 1)
        guard levels[next] != current else { return false }
        (editor.stageView as? StageInputView)?.anchorNextZoom(atView: anchor)
        zoom = .level(levels[next])
        return true
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
            return fitZoom(art: doc.ref.kind == .scene ? doc.publishSize : (doc.render?.artSize ?? .zero), viewport: stageViewport, kind: doc.ref.kind)
        }
    }

    /// The toolbar's sparkle, Window › Agent (⌥⌘A), Ask Claude about this: the Agent tab, shown.
    func showAgentTab(focusInput: Bool = false) {
        showInspector = true
        rightTab = .agent
        chat.start()
        if focusInput { chat.focusRequest += 1 }
        mainWindow?.makeKeyAndOrderFront(nil)
    }

    func setZoomLevel(index: Int) {
        let levels = Zoom.levels(for: mode)
        guard levels.indices.contains(index) else { return }
        zoom = .level(levels[index])
    }
}

/// The right column's tabs (docs/intents/2026-10-agent-chat/spec.md, D12).
enum RightTab: String, Sendable { case inspector, agent }

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
