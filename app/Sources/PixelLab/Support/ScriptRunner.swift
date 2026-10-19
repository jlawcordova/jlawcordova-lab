import AppKit
import Foundation

/// A test hook: drives the app's model from a script, with no UI automation,
/// and snapshots the window between steps. Set `PIXELLAB_SCRIPT=<file>` (and
/// `PIXELLAB_SITE` to a temp copy of the site: the runner refuses the real
/// checkout, since scripts save and build). One action per line, `#` comments:
///
///     open scene:hero-island      appearance dark|light   size 1440x900
///     tool select|place|pan       library <object>        level <n>
///     select <path>|#<id>|none    click <x> <y>           clicktile <col> <row> <level>
///     hover <col> <row> <level>   place [<col> <row> <level>]
///     drag <dc> <dr> [hold]       dragpx <dx> <dy> [hold] release | cancel
///     key <spec> (right, opt+up, shift+down, delete, return, esc, v, 2, ])
///     move <dc> <dr> [<dl>]       nudge <dx> <dy>         set <col|row|level|x|y> <n>
///     class <text>  id <text>     raise lower duplicate remove snap
///     undo redo save [force] revert build done
///     time <ms>  play  pause  tracks on|off   zoom fit|1-4   sidebar library|items
///     search <text>   wait <ms>   snapshot <file.png>   log <text>
///     expect dirty|selected|object|at|items|problems|build|tool|time <value>
///
/// Each action waits until renders, checks and edits settle. The app logs
/// each step to stderr and quits at the end: exit 0 if every step and
/// expectation passed, 1 if any failed, 2 if it couldn't start.
@MainActor
final class ScriptRunner {
    let url: URL
    let lines: [String]
    let size: CGSize
    private var failures = 0

    init(url: URL, lines: [String], size: CGSize) {
        self.url = url
        self.lines = lines
        self.size = size
    }

    static func fromEnvironment() -> ScriptRunner? {
        let env = ProcessInfo.processInfo.environment
        guard let path = env["PIXELLAB_SCRIPT"], !path.isEmpty else { return nil }
        let url = URL(fileURLWithPath: NSString(string: path).expandingTildeInPath)
        let text = (try? String(contentsOf: url, encoding: .utf8)) ?? ""
        var size = CGSize(width: 1440, height: 900)
        if let s = env["PIXELLAB_SNAPSHOT_SIZE"], case let parts = s.split(separator: "x").compactMap({ Double($0) }), parts.count == 2 {
            size = CGSize(width: parts[0], height: parts[1])
        }
        return ScriptRunner(url: url, lines: text.components(separatedBy: .newlines), size: size)
    }

    func start(model: AppModel) {
        Task { await run(model) }
    }

    private func log(_ s: String) {
        FileHandle.standardError.write(Data("script: \(s)\n".utf8))
    }

    private func run(_ model: AppModel) async {
        let env = ProcessInfo.processInfo.environment
        let site = (env["PIXELLAB_SITE"] ?? "").isEmpty ? nil : URL(fileURLWithPath: env["PIXELLAB_SITE"]!).standardizedFileURL.resolvingSymlinksInPath().path
        let real = URL(fileURLWithPath: Locator.defaultSitePath).standardizedFileURL.resolvingSymlinksInPath().path
        guard let site, site != real else {
            log("refusing to run: set PIXELLAB_SITE to a temp copy of the site (not \(real))")
            exit(2)
        }
        guard !lines.isEmpty else {
            log("no script at \(url.path)")
            exit(2)
        }
        // Wait for the server and the first doc.
        let started = Date()
        while Date().timeIntervalSince(started) < 60 {
            if case .ready = model.phase, model.document != nil { break }
            if case .setup(let p) = model.phase {
                log("setup problem: \([p.node, p.site, p.cli, p.server].compactMap { $0 }.joined(separator: "; "))")
                exit(2)
            }
            try? await Task.sleep(for: .milliseconds(100))
        }
        if let window = Snapshotter.mainWindow() {
            window.setFrame(NSRect(origin: window.frame.origin, size: size), display: true)
            window.center()
        }
        await settle(model)
        for (i, raw) in lines.enumerated() {
            let line = raw.trimmingCharacters(in: .whitespaces)
            if line.isEmpty || line.hasPrefix("#") { continue }
            let words = line.split(separator: " ", omittingEmptySubsequences: true).map(String.init)
            do {
                let said = try await perform(words, model: model)
                await settle(model)
                log("\(i + 1): \(line) → ok\(said.map { " (\($0))" } ?? "")")
            } catch {
                failures += 1
                await settle(model)
                log("\(i + 1): \(line) → FAILED: \(error.localizedDescription)")
            }
        }
        log(failures == 0 ? "done, all steps passed" : "done, \(failures) step\(failures == 1 ? "" : "s") failed")
        await model.shutdown()
        exit(failures == 0 ? 0 : 1)
    }

    struct Failure: LocalizedError {
        var errorDescription: String?
        init(_ s: String) { errorDescription = s }
    }

    private func int(_ words: [String], _ i: Int) throws -> Int {
        guard i < words.count, let n = Int(words[i]) else { throw Failure("\(words[0]) needs a whole number at argument \(i)") }
        return n
    }

    private func arg(_ words: [String], _ i: Int) throws -> String {
        guard i < words.count else { throw Failure("\(words[0]) needs argument \(i)") }
        return words[i]
    }

    /// Runs one action; returns a note for the log.
    private func perform(_ words: [String], model: AppModel) async throws -> String? {
        let editor = model.editor
        let doc = model.document
        func needEdit(_ r: EditResult?) throws -> String? {
            guard let r else { throw Failure(model.notice ?? "the edit failed") }
            return [r.summary, r.path.map { "path \($0)" }].compactMap { $0 }.joined(separator: ", ")
        }
        switch words[0] {
        case "open":
            guard let ref = DocRef(id: try arg(words, 1)) else { throw Failure("open scene:<name> or object:<name>") }
            await model.open(ref)
            return nil
        case "appearance":
            NSApp.appearance = AppearanceSetting(rawValue: try arg(words, 1))?.appearance
            return nil
        case "size":
            let parts = try arg(words, 1).split(separator: "x").compactMap { Double($0) }
            guard parts.count == 2, let window = Snapshotter.mainWindow() else { throw Failure("size WxH") }
            window.setFrame(NSRect(origin: window.frame.origin, size: CGSize(width: parts[0], height: parts[1])), display: true)
            window.center()
            return nil
        case "tool":
            guard let t = StageTool(rawValue: try arg(words, 1)) else { throw Failure("tool select|place|pan") }
            editor.setTool(t)
            return nil
        case "library":
            editor.choosePlaceObject(try arg(words, 1))
            _ = await model.thumbnails.art(try arg(words, 1))
            return nil
        case "level":
            editor.placeLevel = try int(words, 1)
            return nil
        case "focus":
            editor.focusStage()
            return nil
        case "select":
            let what = try arg(words, 1)
            if what == "none" { editor.select(nil) } else if what.hasPrefix("#") { editor.select(id: String(what.dropFirst())) } else { editor.select(what) }
            return doc?.selectedPath.map { "selected \($0)" } ?? "nothing selected"
        case "click", "clicktile":
            guard let render = doc?.render else { throw Failure("nothing rendered") }
            var x = 0, y = 0
            if words[0] == "click" {
                x = try int(words, 1)
                y = try int(words, 2)
            } else {
                guard let origin = doc?.origin else { throw Failure("no tile origin") }
                let p = StageGeometry.tileToScene(col: try int(words, 1), row: try int(words, 2), level: try int(words, 3), origin: origin)
                x = Int(p.x)
                y = Int(p.y)
            }
            // Objects the scene uses are prefetched; make sure the candidates' art is in.
            for item in render.items { _ = await model.thumbnails.art(item.pose ?? item.object) }
            let path = editor.hitTest(scenePixel: x, y, render: render)
            editor.select(path)
            return "pixel \(x),\(y) → \(path ?? "nothing")"
        case "hover":
            let t = Tile(col: try int(words, 1), row: try int(words, 2), level: try int(words, 3))
            if editor.tool != .place { editor.setTool(.place) }
            editor.placeLevel = t.level
            editor.hoverTile = t
            editor.cursorTile = t
            if let origin = doc?.origin { editor.hoverPixel = StageGeometry.tileToScene(col: t.col, row: t.row, level: t.level, origin: origin) }
            return nil
        case "place":
            guard let object = editor.placeObject else { throw Failure("choose an object with library <name> first") }
            if words.count >= 4 {
                return try needEdit(await editor.place(object, at: Tile(col: try int(words, 1), row: try int(words, 2), level: try int(words, 3))))
            }
            await editor.placeAtCursor()
            return doc?.selectedPath.map { "selected \($0)" }
        case "drag", "dragpx":
            guard let item = doc?.selectedItem else { throw Failure("nothing selected") }
            var d = ItemDrag(path: item.path, startView: .zero, startScene: .zero, byPixel: words[0] == "dragpx")
            d.moved = true
            if d.byPixel { d.dx = try int(words, 1); d.dy = try int(words, 2) } else { d.dc = try int(words, 1); d.dr = try int(words, 2) }
            editor.drag = d
            if words.count > 3, words[3] == "hold" { return "holding the drag" }
            editor.mouseUp()
            return nil
        case "release":
            editor.mouseUp()
            return nil
        case "cancel":
            editor.drag = nil
            return nil
        case "key":
            guard let k = KeyInput(spec: try arg(words, 1)) else { throw Failure("unknown key \(words[1])") }
            if k.command {
                // Menu shortcuts the script can name.
                switch k.key {
                case .char("d"): await editor.duplicate()
                case .char("z"): if k.shift { await model.redo() } else { await model.undo() }
                case .char("s"): await model.save()
                case .char("b"): await model.buildArt()
                case .char("]"): await editor.raise()
                case .char("["): await editor.lower()
                default: throw Failure("no menu command for \(words[1])")
                }
                return nil
            }
            guard editor.handleKey(k) else { throw Failure("the stage doesn't use \(words[1])") }
            return nil
        case "move":
            return try needEdit(await editor.move(dc: try int(words, 1), dr: try int(words, 2), dl: words.count > 3 ? try int(words, 3) : 0))
        case "nudge":
            return try needEdit(await editor.nudge(dx: try int(words, 1), dy: try int(words, 2)))
        case "set":
            return try needEdit(await editor.set(try arg(words, 1), try int(words, 2)))
        case "class":
            await editor.setClass(words.dropFirst().joined(separator: " "))
            return nil
        case "id":
            await editor.setID(words.dropFirst().joined(separator: " "))
            return nil
        case "raise": return try needEdit(await editor.raise())
        case "lower": return try needEdit(await editor.lower())
        case "duplicate": return try needEdit(await editor.duplicate())
        case "remove": return try needEdit(await editor.remove())
        case "snap": return try needEdit(await editor.snap())
        case "undo": await model.undo(); return model.lastEdit
        case "redo": await model.redo(); return model.lastEdit
        case "save":
            let ok = await model.save(force: words.count > 1 && words[1] == "force")
            if !ok, let a = model.saveAlert { return "refused (\(a.kind)): \(a.message)" }
            return ok ? model.lastEdit : "not saved"
        case "revert": await model.revert(); return nil
        case "build":
            await model.buildArt()
            guard let b = model.build else { throw Failure("no build") }
            return b.result.map { "ok=\($0.ok) code=\($0.code.map(String.init) ?? "-") \(Int($0.ms)) ms, \($0.report.count) report lines" } ?? b.error
        case "done":
            model.build = nil
            model.saveAlert = nil
            return nil
        case "time":
            model.timeline.pause()
            model.timeline.scrub(to: Double(try int(words, 1)))
            return nil
        case "play": model.timeline.play(); return nil
        case "pause":
            model.timeline.pause()
            return "paused at \(TimelineModel.label(model.timeline.time)), \(model.document?.renderCount ?? 0) renders so far"
        case "tracks": model.timeline.showTracks = (try arg(words, 1)) == "on"; return nil
        case "zoom":
            let z = try arg(words, 1)
            if z == "fit" { model.zoom = .fit } else { model.zoom = .level(try int(words, 1)) }
            return nil
        case "sidebar":
            editor.sidebarTab = (try arg(words, 1)) == "items" ? .items : .library
            return nil
        case "search":
            model.searchText = words.dropFirst().joined(separator: " ")
            return nil
        case "wait":
            try? await Task.sleep(for: .milliseconds(try int(words, 1)))
            return nil
        case "mouse":
            // Real NSEvents into the stage view: mouse down|drag|up <sceneX> <sceneY> [opt]
            guard let view = editor.stageView as? StageInputView, let geometry = view.geometry, let window = view.window else { throw Failure("no stage") }
            let phase = try arg(words, 1)
            let scene = CGPoint(x: Double(try int(words, 2)) + 0.5, y: Double(try int(words, 3)) + 0.5)
            let inWindow = view.convert(geometry.viewPoint(fromScene: scene), to: nil)
            let type: NSEvent.EventType = phase == "down" ? .leftMouseDown : phase == "drag" ? .leftMouseDragged : .leftMouseUp
            guard let event = NSEvent.mouseEvent(with: type, location: inWindow, modifierFlags: words.count > 4 && words[4] == "opt" ? .option : [],
                                                 timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber, context: nil,
                                                 eventNumber: 0, clickCount: 1, pressure: 1) else { throw Failure("no event") }
            switch type {
            case .leftMouseDown: view.mouseDown(with: event)
            case .leftMouseDragged: view.mouseDragged(with: event)
            default: view.mouseUp(with: event)
            }
            let scroll = view.enclosingScrollView?.contentView.bounds.origin
            return "view \(Int(geometry.viewPoint(fromScene: scene).x)),\(Int(geometry.viewPoint(fromScene: scene).y))\(scroll.map { "; scrolled to \(Int($0.x)),\(Int($0.y))" } ?? "; no scroll view")\(editor.drag.map { " drag \($0.dc),\($0.dr) px \($0.dx),\($0.dy)" } ?? "")"
        case "layout":
            guard let w = Snapshotter.mainWindow(), let c = w.contentView else { throw Failure("no window") }
            func walk(_ v: NSView, _ depth: Int) -> [String] {
                guard depth < 26 else { return [] }
                let line = String(repeating: "  ", count: depth) + "\(type(of: v)) \(v.frame.integral)"
                return [line] + v.subviews.flatMap { walk($0, depth + 1) }
            }
            log("window \(w.frame) content \(c.frame) fitting \(c.fittingSize)")
            for l in walk(c, 0).prefix(400) { log(l) }
            return nil
        case "log":
            return words.dropFirst().joined(separator: " ")
        case "snapshot":
            let out = URL(fileURLWithPath: NSString(string: try arg(words, 1)).expandingTildeInPath)
            try? await Task.sleep(for: .milliseconds(300))
            guard let window = Snapshotter.mainWindow(), Snapshotter.capture(window, to: out) else { throw Failure("couldn't capture") }
            return out.path
        case "expect":
            return try expect(words, model: model)
        default:
            throw Failure("unknown action \(words[0])")
        }
    }

    private func expect(_ words: [String], model: AppModel) throws -> String {
        let what = try arg(words, 1)
        let want = words.dropFirst(2).joined(separator: " ")
        let doc = model.document
        let got: String
        switch what {
        case "dirty": got = String(doc?.isDirty ?? false)
        case "selected": got = doc?.selectedPath ?? "none"
        case "object": got = doc?.selectedItem?.object ?? "none"
        case "at": got = doc?.selectedItem?.at.short ?? "none"
        case "class": got = doc?.selectedItem?.class ?? ""
        case "items": got = String(doc?.items.count ?? 0)
        case "problems": got = String(doc?.problems?.count ?? -1)
        case "build": got = model.build?.result.map { $0.ok ? "ok" : "failed" } ?? "none"
        case "tool": got = model.editor.tool.rawValue
        case "time": got = String(model.timeline.renderTime)
        case "alert": got = model.saveAlert.map { "\($0.kind)" } ?? "none"
        default: throw Failure("expect what? \(what)")
        }
        guard got == want else { throw Failure("expected \(what) \(want), got \(got)") }
        return "\(what) = \(got)"
    }

    /// Waits until edits, renders, checks, item lists and the build are done
    /// and the stage has been steady for a moment (at most 30 s).
    private func settle(_ model: AppModel) async {
        // Playing renders without end: nothing to wait for.
        if model.timeline.playing { return }
        let started = Date()
        var steadySince: Date?
        while Date().timeIntervalSince(started) < 30 {
            let busy: Bool = {
                if model.editor.pendingEdits > 0 || model.build?.running == true { return true }
                guard let doc = model.document else { return false }
                return doc.rendering || doc.checking || doc.loadingItems || doc.render == nil || !model.thumbnails.isIdle
            }()
            if busy { steadySince = nil } else {
                steadySince = steadySince ?? Date()
                if Date().timeIntervalSince(steadySince!) > 0.35 { return }
            }
            try? await Task.sleep(for: .milliseconds(50))
        }
        let d = model.document
        log("(still busy after 30 s: edits \(model.editor.pendingEdits), build \(model.build?.running == true), rendering \(d?.rendering ?? false), checking \(d?.checking ?? false), items \(d?.loadingItems ?? false), thumbnails idle \(model.thumbnails.isIdle))")
    }
}
