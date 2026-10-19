import AppKit
import Foundation

/// A test hook: drives the app's model from a script, with no UI automation,
/// and snapshots the window between steps. Set `PIXELLAB_SCRIPT=<file>` (and
/// `PIXELLAB_DOC` to a temp copy of a document, or `PIXELLAB_SITE` to a temp
/// copy of the site: the runner refuses the real checkout and anything inside
/// it, since scripts save and publish). One action per line, `#` comments:
///
///     open scene:hero-island      appearance dark|light   size 1440x900
///     tool select|place|pan       library <object>        level <n>
///     select <path>|#<id>|none    click <x> <y>           clicktile <col> <row> <level>
///     hover <col> <row> <level>   place [<col> <row> <level>]
///     drag <dc> <dr> [hold]       dragpx <dx> <dy> [hold] release | cancel
///     key <spec> (right, opt+up, shift+down, delete, return, esc, v, 2, ])
///     move <dc> <dr> [<dl>]       nudge <dx> <dy>         set <col|row|level|x|y> <n>
///     class <text>  id <text>     raise lower duplicate remove snap
///     undo redo save [force] revert publish [<dir>] done   saveas <path.pixellab>
///     time <ms>  play  pause  tracks on|off   zoom fit|1-4   sidebar library|items
///     search <text>   wait <ms>   snapshot <file.png>   log <text>
///     expect dirty|selected|object|at|items|problems|build|outputs|publishdir|document|tool|time <value>
///     expect scenes|objects|zoom|badge|theme|ghosttime <value>   expect scenepixel <x>,<y> <#RRGGBB|empty>
///
/// Object mode (pixels are object.render pixels at scale 1: a sprite's map
/// column and row, a block's render pixel):
///
///     mode scene|object           tool pencil|eraser|fill|picker   color <name>
///     stroke x,y x,y …            (a pencil or eraser stroke through the points, gaps filled)
///     erase x,y …                 px <x> <y> [opt] [shift]  (a click with the tool)
///     fill <x> <y>   pick <x> <y>   grid on|off   onion on|off   resize <w> <h>
///     layer <n>|add|duplicate|delete|up|down|hide <n>|show <n>
///     frame <n>|next|prev|add|duplicate|delete|earlier|later
///     animate [ff|wf|hf]   still   play   pause   sidebar palette|library|items
///     new scene <name>   new object <name> [block|<w>x<h>|extends]
///     newdialog scene|object [<name>] [block|<w>x<h>|extends]   newcancel   newsubmit
///     mouse down|drag|up <x> <y> [opt] [shift]   (object pixels in object mode)
///     mousepath x,y x,y …         (a fast real drag: down, a drag every 4 ms, up)
///     expect color|layer|frame|layers|frames|doc|mode|hidden|calls <value>
///     expect pixel <x>,<y> <color name|empty>   (the rendered pixel)
///
/// The Agent window (proposals are numbered from 1, oldest first, as listed):
///
///     agent open|close|size <w>x<h>   proposal select <n>|<id>|none   proposal next|prev|open
///     approve [force]   reject   ghost on|off   answer proceed|force|reject|cancel (the next question)
///     agentkey up|down|esc|cmd+return|delete (a key into the Agent window)
///     snapshot agent <file.png>   menus (logs the View and Window menus)
///     expect proposals|ghost|ghostchanges|base|activity|question|proposal <value>
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
        let given = [env["PIXELLAB_DOC"], env["PIXELLAB_SITE"]].compactMap { $0 }.first { !$0.isEmpty }
        let target = given.map { URL(fileURLWithPath: $0).standardizedFileURL.resolvingSymlinksInPath().path }
        let real = URL(fileURLWithPath: Locator.defaultSitePath).standardizedFileURL.resolvingSymlinksInPath().path
        guard let target, target != real, !target.hasPrefix(real + "/") else {
            log("refusing to run: set PIXELLAB_DOC (or PIXELLAB_SITE) to a temp copy of the document or site, not \(real) or anything inside it")
            exit(2)
        }
        guard lines.contains(where: { !$0.trimmingCharacters(in: .whitespaces).isEmpty && !$0.hasPrefix("#") }) else {
            log("no script at \(url.path)")
            exit(2)
        }
        // Wait for the server and the first doc.
        let started = Date()
        while Date().timeIntervalSince(started) < 60 {
            if case .ready = model.phase, model.document != nil { break }
            if case .setup(let p) = model.phase {
                log("setup problem: \([p.node, p.document, p.cli, p.server].compactMap { $0 }.joined(separator: "; "))")
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
            if let t = PaintTool(rawValue: try arg(words, 1)) {
                model.objectEditor.setTool(t)
                return nil
            }
            guard let t = StageTool(rawValue: try arg(words, 1)) else { throw Failure("tool select|place|pan|pencil|eraser|fill|picker") }
            editor.setTool(t)
            return nil
        case "mode", "color", "stroke", "erase", "px", "fill", "pick", "grid", "onion", "resize", "layer", "frame", "animate", "still",
             "new", "newdialog", "newcancel", "newsubmit":
            return try await performObject(words, model: model)
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
                case .char("b"): await model.publish()
                case .char("]"): await editor.raise()
                case .char("["): await editor.lower()
                default: throw Failure("no menu command for \(words[1])")
                }
                return nil
            }
            let used = model.document?.ref.kind == .object ? model.objectEditor.handleKey(k) : editor.handleKey(k)
            guard used else { throw Failure("the stage doesn't use \(words[1])") }
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
        case "build", "publish":
            // `publish [dir]`: into the document's publish folder, or the given one (no panel opens under a script).
            await model.publish(dir: words.count > 1 ? words[1] : nil)
            guard let b = model.build else { throw Failure("no publish") }
            return b.result.map { "ok=\($0.ok) \(Int($0.ms)) ms, \($0.outputs.count) outputs to \($0.publishDir), \($0.problems.count) problems" } ?? b.error
        case "saveas":
            // `saveas <path>`: document.copy, then the app switches to the copy.
            let to = try arg(words, 1)
            await model.saveAs(to: to)
            let expected = to.hasSuffix(".pixellab") ? to : "\(to).pixellab"
            let started = Date()
            while Date().timeIntervalSince(started) < 30 {
                if case .ready = model.phase, model.sessionInfo?.document?.root == expected { break }
                if model.notice != nil { break }
                try? await Task.sleep(for: .milliseconds(100))
            }
            guard case .ready = model.phase, model.sessionInfo?.document?.root == expected else {
                throw Failure(model.notice ?? "the copy didn't open; still on \(model.sessionInfo?.document?.root ?? "?")")
            }
            return "now \(expected)"
        case "done":
            model.build = nil
            model.saveAlert = nil
            return nil
        case "time":
            model.timeline.pause()
            model.timeline.scrub(to: Double(try int(words, 1)))
            return nil
        case "play" where model.document?.ref.kind == .object:
            model.objectEditor.play()
            return model.objectEditor.playing ? "playing" : "nothing to play"
        case "pause" where model.document?.ref.kind == .object:
            model.objectEditor.stopPlaying()
            return "paused on \(model.objectEditor.frameName(model.objectEditor.frame)), \(model.document?.renderCount ?? 0) renders so far"
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
            switch try arg(words, 1) {
            case "palette": model.objectEditor.sidebarTab = .palette
            case "items": editor.sidebarTab = .items
            default:
                editor.sidebarTab = .library
                model.objectEditor.sidebarTab = .library
            }
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
            // Object mode: object pixels (render pixels at scale 1); scenes: scene pixels.
            let origin = model.document?.ref.kind == .object ? geometry.viewBox.origin : .zero
            let scene = CGPoint(x: origin.x + Double(try int(words, 2)) + 0.5, y: origin.y + Double(try int(words, 3)) + 0.5)
            let inWindow = view.convert(geometry.viewPoint(fromScene: scene), to: nil)
            let type: NSEvent.EventType = phase == "down" ? .leftMouseDown : phase == "drag" ? .leftMouseDragged : .leftMouseUp
            var flags: NSEvent.ModifierFlags = []
            if words.dropFirst(4).contains("opt") { flags.insert(.option) }
            if words.dropFirst(4).contains("shift") { flags.insert(.shift) }
            guard let event = NSEvent.mouseEvent(with: type, location: inWindow, modifierFlags: flags,
                                                 timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber, context: nil,
                                                 eventNumber: 0, clickCount: 1, pressure: 1) else { throw Failure("no event") }
            switch type {
            case .leftMouseDown: view.mouseDown(with: event)
            case .leftMouseDragged: view.mouseDragged(with: event)
            default: view.mouseUp(with: event)
            }
            let scroll = view.enclosingScrollView?.contentView.bounds.origin
            return "view \(Int(geometry.viewPoint(fromScene: scene).x)),\(Int(geometry.viewPoint(fromScene: scene).y))\(scroll.map { "; scrolled to \(Int($0.x)),\(Int($0.y))" } ?? "; no scroll view")\(editor.drag.map { " drag \($0.dc),\($0.dr) px \($0.dx),\($0.dy)" } ?? "")"
        case "mousepath":
            // A fast real drag: mouse down at the first object pixel, a drag
            // event every 4 ms through the rest, up at the last.
            guard let view = editor.stageView as? StageInputView, let geometry = view.geometry, let window = view.window else { throw Failure("no stage") }
            let pts = words.dropFirst().compactMap { PixelPoint(spec: $0) }
            guard pts.count >= 2 else { throw Failure("mousepath x,y x,y …") }
            for (i, p) in pts.enumerated() {
                let scene = CGPoint(x: geometry.viewBox.minX + Double(p.x) + 0.5, y: geometry.viewBox.minY + Double(p.y) + 0.5)
                let type: NSEvent.EventType = i == 0 ? .leftMouseDown : (i == pts.count - 1 ? .leftMouseUp : .leftMouseDragged)
                guard let event = NSEvent.mouseEvent(with: type, location: view.convert(geometry.viewPoint(fromScene: scene), to: nil), modifierFlags: [],
                                                     timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber, context: nil,
                                                     eventNumber: 0, clickCount: 1, pressure: 1) else { throw Failure("no event") }
                switch type {
                case .leftMouseDown: view.mouseDown(with: event)
                case .leftMouseDragged: view.mouseDragged(with: event)
                default:
                    view.mouseDragged(with: event)
                    view.mouseUp(with: event)
                }
                try? await Task.sleep(for: .milliseconds(4))
            }
            await waitIdle(model.objectEditor)
            return "\(pts.count) samples, \(model.objectEditor.lastStrokeCalls) paint call\(model.objectEditor.lastStrokeCalls == 1 ? "" : "s")"
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
            let agentWindow = words.count > 2 && words[1] == "agent"
            let out = URL(fileURLWithPath: NSString(string: try arg(words, agentWindow ? 2 : 1)).expandingTildeInPath)
            try? await Task.sleep(for: .milliseconds(300))
            let window = agentWindow ? model.agent.window.flatMap { $0.isVisible ? $0 : nil } : Snapshotter.mainWindow()
            guard let window, Snapshotter.capture(window, to: out) else { throw Failure(agentWindow ? "no Agent window (agent open first)" : "couldn't capture") }
            return out.path
        case "agent", "proposal", "approve", "reject", "ghost", "answer", "menus", "agentkey":
            return try await performAgent(words, model: model)
        case "expect":
            return try expect(words, model: model)
        default:
            throw Failure("unknown action \(words[0])")
        }
    }

    /// Object mode's actions and New Scene / New Object.
    private func performObject(_ words: [String], model: AppModel) async throws -> String? {
        let objects = model.objectEditor
        func needEdit(_ r: EditResult?) throws -> String? {
            guard let r else { throw Failure(model.notice ?? "the edit failed") }
            return r.summary
        }
        func points(_ from: Int) throws -> [PixelPoint] {
            let pts = words.dropFirst(from).map { PixelPoint(spec: $0) }
            guard !pts.isEmpty, pts.allSatisfy({ $0 != nil }) else { throw Failure("\(words[0]) needs x,y points") }
            return pts.compactMap { $0 }
        }
        func requireObject() throws {
            guard model.document?.ref.kind == .object, objects.info != nil else { throw Failure("no object open") }
        }
        switch words[0] {
        case "mode":
            guard let kind = DocRef.Kind(rawValue: try arg(words, 1)) else { throw Failure("mode scene|object") }
            model.switchMode(to: kind)
            try? await Task.sleep(for: .milliseconds(100))
            return nil
        case "color":
            let name = try arg(words, 1)
            guard objects.allowed(name) else { throw Failure(objects.whyNot(name)) }
            objects.setColor(name)
            return nil
        case "stroke", "erase":
            try requireObject()
            let pts = try points(1)
            let tool = objects.tool
            if words[0] == "erase" { objects.tool = .eraser } else if tool != .pencil && tool != .eraser { objects.tool = .pencil }
            defer { objects.tool = tool }
            objects.beginStroke(at: pts[0])
            guard objects.stroking else { throw Failure(model.notice ?? "the stroke didn't start") }
            for p in pts.dropFirst() { objects.extendStroke(to: p) }
            objects.endStroke()
            await waitIdle(objects)
            if let n = model.notice { throw Failure(n) }
            return "\(objects.lastStrokeCalls) call\(objects.lastStrokeCalls == 1 ? "" : "s"): \(model.lastEdit ?? "")"
        case "px":
            try requireObject()
            let p = PixelPoint(try int(words, 1), try int(words, 2))
            objects.press(p, option: words.contains("opt"), shift: words.contains("shift"))
            objects.endStroke()
            await waitIdle(objects)
            return model.lastEdit
        case "fill":
            try requireObject()
            return try needEdit(await objects.fill(PixelPoint(try int(words, 1), try int(words, 2))))
        case "pick":
            try requireObject()
            let c = await objects.pick(PixelPoint(try int(words, 1), try int(words, 2)))
            return c.map { "picked \($0)" } ?? "empty"
        case "grid":
            objects.pixelGrid = (try arg(words, 1)) == "on"
            return nil
        case "onion":
            if objects.onion != ((try arg(words, 1)) == "on") { objects.toggleOnion() }
            return nil
        case "resize":
            return try needEdit(await objects.resize(width: try int(words, 1), height: try int(words, 2)))
        case "layer":
            try requireObject()
            switch try arg(words, 1) {
            case "add": return try needEdit(await objects.addLayer())
            case "duplicate": return try needEdit(await objects.duplicateLayer())
            case "delete": return try needEdit(await objects.deleteLayer())
            case "up": return try needEdit(await objects.moveLayer(by: 1))
            case "down": return try needEdit(await objects.moveLayer(by: -1))
            case "hide", "show":
                let n = try int(words, 2)
                if objects.hidden.contains(n) != (words[1] == "hide") { objects.toggleVisible(n) }
                return nil
            default:
                objects.selectLayer(try int(words, 1))
                return objects.label(objects.layer)
            }
        case "frame":
            try requireObject()
            switch try arg(words, 1) {
            case "next": objects.selectFrame(objects.frame + 1)
            case "prev": objects.selectFrame(objects.frame - 1)
            case "add": return try needEdit(await objects.addFrame())
            case "duplicate": return try needEdit(await objects.duplicateFrame())
            case "delete": return try needEdit(await objects.deleteFrame())
            case "earlier": return try needEdit(await objects.moveFrame(by: -1))
            case "later": return try needEdit(await objects.moveFrame(by: 1))
            default: objects.selectFrame(try int(words, 1))
            }
            return objects.frameName(objects.frame)
        case "animate":
            try requireObject()
            let loop = words.count > 1 ? FrameLoop(rawValue: words[1]) : .ff
            guard let loop else { throw Failure("animate ff|wf|hf") }
            return try needEdit(await objects.animate(loop))
        case "still":
            try requireObject()
            return try needEdit(await objects.makeStill())
        case "new", "newdialog":
            guard let kind = DocRef.Kind(rawValue: try arg(words, 1)) else { throw Failure("\(words[0]) scene|object <name>") }
            model.showNew(kind)
            guard let draft = model.newDoc else { throw Failure("no sheet") }
            if words.count > 2 { draft.name = words[2] }
            for w in words.dropFirst(3) {
                if w == "block" { draft.objectKind = .block }
                else if w == "extends" { draft.extendsCharacter = true }
                else {
                    let wh = w.split(separator: "x").map(String.init)
                    guard wh.count == 2 else { throw Failure("unknown option \(w)") }
                    draft.width = wh[0]
                    draft.height = wh[1]
                }
            }
            if words[0] == "newdialog" { return draft.help(taken: model.takenNames) }
            fallthrough
        case "newsubmit":
            guard let draft = model.newDoc else { throw Failure("no New sheet open") }
            guard await model.createNewDoc() else {
                let why = draft.serverError ?? draft.problem(taken: model.takenNames) ?? "not created"
                model.newDoc = nil
                throw Failure(why)
            }
            return "opened \(model.document?.ref.id ?? "?")"
        case "newcancel":
            model.newDoc = nil
            return nil
        default:
            throw Failure("unknown action \(words[0])")
        }
    }

    /// The Agent window's actions.
    private func performAgent(_ words: [String], model: AppModel) async throws -> String? {
        let agent = model.agent
        func result() -> String? { agent.lastResult.map { r in agent.lastQuestion.map { "asked \($0.rawValue); \(r)" } ?? r } }
        switch words[0] {
        case "agent":
            switch try arg(words, 1) {
            case "open":
                agent.showWindow()
                let started = Date()
                while agent.window?.isVisible != true, Date().timeIntervalSince(started) < 5 { try? await Task.sleep(for: .milliseconds(50)) }
                guard let w = agent.window, w.isVisible else { throw Failure("the Agent window didn't open") }
                w.setFrame(NSRect(origin: w.frame.origin, size: CGSize(width: 880, height: 820)), display: true)
                return "Agent window \(Int(w.frame.width))×\(Int(w.frame.height)), autosave \(w.frameAutosaveName.isEmpty ? "none" : w.frameAutosaveName)"
            case "close":
                agent.window?.performClose(nil)
                try? await Task.sleep(for: .milliseconds(200))
                return agent.window?.isVisible == true ? "still open" : "closed"
            case "size":
                let parts = try arg(words, 2).split(separator: "x").compactMap { Double($0) }
                guard parts.count == 2, let w = agent.window else { throw Failure("agent size WxH (agent open first)") }
                w.setFrame(NSRect(origin: w.frame.origin, size: CGSize(width: parts[0], height: parts[1])), display: true)
                return nil
            default: throw Failure("agent open|close|size")
            }
        case "proposal":
            switch try arg(words, 1) {
            case "select":
                let what = try arg(words, 2)
                if what == "none" { agent.select(nil) }
                else if let n = Int(what) {
                    guard agent.proposals.indices.contains(n - 1) else { throw Failure("no proposal \(n) (\(agent.proposals.count) listed)") }
                    agent.select(agent.proposals[n - 1].id)
                } else {
                    guard agent.proposals.contains(where: { $0.id == what }) else { throw Failure("no proposal \(what)") }
                    agent.select(what)
                }
            case "next": agent.move(by: 1)
            case "prev": agent.move(by: -1)
            case "open": await agent.openInEditor()
            default: throw Failure("proposal select|next|prev|open")
            }
            return agent.selected.map { "\($0.id) \($0.doc) from \($0.agentName), \($0.editsLabel)" } ?? "none selected"
        case "approve":
            guard agent.selected != nil else { throw Failure("no proposal selected") }
            agent.lastQuestion = nil
            await agent.approve(force: words.count > 1 && words[1] == "force")
            return result()
        case "reject":
            guard agent.selected != nil else { throw Failure("no proposal selected") }
            agent.lastQuestion = nil
            await agent.reject()
            return result()
        case "ghost":
            agent.setGhostShown((try arg(words, 1)) == "on")
            return nil
        case "answer":
            guard let a = AgentAnswer(rawValue: try arg(words, 1)) else { throw Failure("answer proceed|force|reject|cancel") }
            agent.scriptAnswer = a
            return nil
        case "agentkey":
            // A real key down into the Agent window's key equivalents: up, down, esc, cmd+return, delete.
            guard let w = agent.window, w.isVisible else { throw Failure("agent open first") }
            let spec = try arg(words, 1)
            let keys: [String: (String, UInt16, NSEvent.ModifierFlags)] = [
                "up": (String(UnicodeScalar(0xF700)!), 126, [.numericPad, .function]), "down": (String(UnicodeScalar(0xF701)!), 125, [.numericPad, .function]),
                "esc": ("\u{1b}", 53, []), "cmd+return": ("\r", 36, [.command]), "delete": ("\u{7f}", 51, []), "backspace": ("\u{8}", 51, []),
            ]
            guard let (chars, code, flags) = keys[spec] else { throw Failure("agentkey up|down|esc|cmd+return|delete") }
            w.makeKeyAndOrderFront(nil)
            guard let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
                                               windowNumber: w.windowNumber, context: nil, characters: chars, charactersIgnoringModifiers: chars,
                                               isARepeat: false, keyCode: code) else { throw Failure("no event") }
            agent.lastQuestion = nil
            let used = agent.handleKeyDown(event) || w.performKeyEquivalent(with: event)
            try? await Task.sleep(for: .milliseconds(150))
            guard used else { throw Failure("the Agent window didn't use \(spec)") }
            return result() ?? agent.selected.map { "selected \($0.doc)" } ?? "none selected"
        case "menus":
            for title in ["View", "Window"] {
                guard let menu = NSApp.mainMenu?.items.first(where: { $0.title == title })?.submenu else { continue }
                let items = menu.items.filter { !$0.isSeparatorItem }.map { item in
                    let mods = item.keyEquivalentModifierMask
                    let key = item.keyEquivalent.isEmpty ? "" : " [\(mods.contains(.command) ? "⌘" : "")\(mods.contains(.option) ? "⌥" : "")\(item.keyEquivalent)]"
                    return item.title + key
                }
                log("\(title) menu: \(items.joined(separator: " | "))")
            }
            return nil
        default:
            throw Failure("unknown action \(words[0])")
        }
    }

    /// Waits for a stroke's paint calls to be answered (at most 10 s).
    private func waitIdle(_ objects: ObjectEditor) async {
        let started = Date()
        while objects.busy, Date().timeIntervalSince(started) < 10 {
            try? await Task.sleep(for: .milliseconds(20))
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
        case "tool": got = doc?.ref.kind == .object ? model.objectEditor.tool.rawValue : model.editor.tool.rawValue
        case "time": got = String(model.timeline.renderTime)
        case "alert": got = model.saveAlert.map { "\($0.kind)" } ?? "none"
        case "color": got = model.objectEditor.color
        case "layer": got = String(model.objectEditor.layer)
        case "frame": got = String(model.objectEditor.frame)
        case "layers": got = String(model.objectEditor.layers.count)
        case "frames": got = String(model.objectEditor.frameCount)
        case "hidden": got = model.objectEditor.hidden.sorted().map(String.init).joined(separator: ",")
        case "calls": got = String(model.objectEditor.lastStrokeCalls)
        case "doc": got = doc?.ref.id ?? "none"
        case "mode": got = model.mode.rawValue
        case "proposals": got = String(model.agent.proposals.count)
        case "proposal": got = model.agent.selected?.doc ?? "none"
        case "ghost": got = model.agent.visibleGhost != nil ? "on" : "off"
        case "ghostchanges":
            guard let render = doc?.render, model.agent.visibleGhost != nil else { throw Failure("no ghost on the stage") }
            let c = model.agent.changes(current: render, renderCount: doc?.renderCount ?? 0)
            let n = { (k: GhostChange.Kind) in c.items.filter { $0.kind == k }.count }
            got = String(c.items.count + c.pixels.count)
            guard got == want else { throw Failure("expected ghostchanges \(want), got \(got)") }
            return doc?.ref.kind == .object ? "\(c.pixels.count) pixels change" : "added \(n(.added)), moved \(n(.moved)), removed \(n(.removed)): \(c.items.map { "\($0.kind.rawValue) \($0.object) \($0.path ?? "")" }.joined(separator: ", "))"
        case "base": got = model.agent.selected.map { $0.baseMatches ? "matches" : "changed" } ?? "none"
        case "activity": got = String(model.agent.visibleActivity.count)
        case "question": got = model.agent.lastQuestion?.rawValue ?? "none"
        case "ghosttime": got = model.agent.visibleGhost?.image.time.map { String(Int($0)) } ?? "none"
        case "scenes": got = String(model.scenes.count)
        case "objects": got = String(model.objects.count)
        case "zoom": got = String(model.effectiveZoom)
        case "badge": got = NSApp.dockTile.badgeLabel ?? "none"
        case "theme": got = model.stageDark ? "dark" : "light"
        case "outputs": got = model.build?.result.map { String($0.outputs.count) } ?? "none"
        case "publishdir": got = model.build?.result?.publishDir ?? model.sessionInfo?.document?.publishDir ?? "none"
        case "document": got = model.sessionInfo?.document?.name ?? "none"
        case "documentkind": got = model.sessionInfo?.document?.kind ?? "none"
        case "scenepixel":
            // A pixel of the stage's scene render, in scene pixels: "#RRGGBB" or empty.
            let parts = words.dropFirst(2)
            guard let p = parts.first.flatMap(PixelPoint.init(spec:)), let render = doc?.render else { throw Failure("expect scenepixel x,y <#RRGGBB|empty>") }
            let colors = PixelColors(cgImage: render.image.cgImage, scale: render.scale)
            let q = PixelPoint(p.x - Int(render.viewBox.minX), p.y - Int(render.viewBox.minY))
            got = colors?.hex(q) ?? "empty"
            let wantColor = parts.dropFirst().joined(separator: " ").uppercased()
            guard got == wantColor else { throw Failure("expected scenepixel \(p) \(wantColor), got \(got)") }
            return "scene pixel \(p) = \(got)"
        case "pixel":
            let parts = words.dropFirst(2)
            guard let p = parts.first.flatMap(PixelPoint.init(spec:)) else { throw Failure("expect pixel x,y <color|empty>") }
            let hex = model.objectEditor.renderColors?.hex(p)
            let name = hex.flatMap { model.objectEditor.colorName(hex: $0) } ?? hex ?? "empty"
            let wantColor = parts.dropFirst().joined(separator: " ")
            guard name == wantColor else { throw Failure("expected pixel \(p) \(wantColor), got \(name)") }
            return "pixel \(p) = \(name)"
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
                if model.editor.pendingEdits > 0 || model.build?.running == true || model.objectEditor.busy { return true }
                if model.newDoc?.creating == true { return true }
                if model.agent.busy { return true }
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
