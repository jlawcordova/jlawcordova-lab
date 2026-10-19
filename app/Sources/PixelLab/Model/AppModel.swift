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
        await loadLists()
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
            guard let doc = document, doc.ref.id == status.doc else { return }
            doc.status = status
            requestRender()
            requestCheck()
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
            await loadLists()
            requestRender()
            requestCheck()
        case .notification:
            break // proposals and activity: the Agent window (slice 6)
        case .restarted(let info):
            sessionInfo = info
            notice = "The pixellab server restarted."
            thumbnails.reset(connection: connection)
            if let doc = document {
                _ = try? await connection?.call("doc.open", ["doc": .string(doc.ref.id)])
                requestRender()
                requestCheck()
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
        if snapshot == nil { UserDefaults.standard.set(ref.id, forKey: SettingsKey.lastDoc) }
        do {
            doc.status = try await connection.call("doc.open", ["doc": .string(ref.id)], as: DocStatus.self)
        } catch {
            doc.error = error.localizedDescription
        }
        requestRender()
        requestCheck()
    }

    /// The Scene/Object segmented control: show the last doc of that kind.
    func switchMode(to kind: DocRef.Kind) {
        mode = kind
        guard document?.ref.kind != kind else { return }
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
                do {
                    let method = doc.ref.kind == .scene ? "scene.render" : "object.render"
                    let r: RenderResult = try await connection.call(method, ["name": .string(doc.ref.name), "scale": 1])
                    if let image = PixelImage(png: r.png, renderScale: r.scale) {
                        doc.render = StageImage(
                            image: image,
                            viewBox: CGRect(x: r.viewBox[0], y: r.viewBox[1], width: r.viewBox[2], height: r.viewBox[3]),
                            scale: r.scale,
                            items: r.items,
                            anchor: r.anchor.map { CGPoint(x: $0[0], y: $0[1]) },
                            time: r.time
                        )
                        doc.error = nil
                    }
                } catch {
                    doc.error = error.localizedDescription
                }
                doc.renderCount += 1
            } while doc.needsRender && document === doc
            doc.rendering = false
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

    func save() async {
        guard let doc = document, let connection else { return }
        do {
            let r: SaveResult = try await connection.call("doc.save", ["doc": .string(doc.ref.id)])
            doc.status = DocStatus(doc: r.doc, dirty: r.dirty, canUndo: r.canUndo, canRedo: r.canRedo)
            notice = nil
        } catch let e as RPCError {
            notice = "Couldn't save \(doc.ref.name): \(e.message)"
        } catch {
            notice = "Couldn't save \(doc.ref.name): \(error.localizedDescription)"
        }
    }

    func revert() async { await docCommand("doc.revert") }
    func undo() async { await docCommand("doc.undo") }
    func redo() async { await docCommand("doc.redo") }

    private func docCommand(_ method: String) async {
        guard let doc = document, let connection else { return }
        do {
            let r: UndoResult = try await connection.call(method, ["doc": .string(doc.ref.id)])
            doc.status = DocStatus(doc: r.doc, dirty: r.dirty, canUndo: r.canUndo, canRedo: r.canRedo)
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
