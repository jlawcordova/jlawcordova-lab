import AppKit
import Foundation

/// A test hook: drives the app's model from a script, with no UI automation,
/// and snapshots the window between steps. Set `PIXELLAB_SCRIPT=<file>` (and
/// `PIXELLAB_DOC` to a temp copy of a document, or `PIXELLAB_SITE` to a temp
/// copy of the site: the runner refuses the real checkout and anything inside
/// it, since scripts save and publish). One action per line, `#` comments:
///
///     open scene:hero-island      appearance dark|light   size 1440x900
///     tool select|pan|hand        library <object>  (starts placing it)   level <n>
///     select <path>|#<id>|none    click <x> <y>           clicktile <col> <row> <level>
///     hover <col> <row> <level>   place [<col> <row> <level>]
///     drag <dc> <dr> [hold]       dragpx <dx> <dy> [hold] release | cancel
///     key <spec> (right, opt+up, shift+down, delete, return, esc, v, 2)
///     move <dc> <dr> [<dl>]       nudge <dx> <dy>         set <col|row|level|x|y> <n>
///     class <text>  id <text>     duplicate remove snap
///     depth front|back|auto   (the selected item's Depth control: the group's when the item is alone in its SVG group)
///     groupdepth <group path>|selected front|back|auto   (the wrapper line's menu, for a group of several items; `selected` is the item's group)
///     sort   (Checks' Sort button: the scene's items in depth order)   set level <n>  (a pixel item's, too)
///     play <action> [<item>]   (the Inspector's Play for an ACTIONS row of the selected item, or of item path <item>, which it selects;
///         in object mode, of the selected layer. Waits until the action has played once and the stage is back at rest)
///     playat <action> <ms> [<item>]   (renders the action <ms> into it, as a mid-play frame, and holds it on the stage for a snapshot)
///     playend   (releases a held `playat` frame: the stage draws the playhead's frame again)
///     reducemotion on|off|system   (overrides the system's Reduce Motion for Play and the timeline)
///     undo redo save [force] revert publish [<dir>] done   saveas <path.pixellab>
///     flush   wait saved   (doc.flush; wait until the open doc's saveState is saved)   revertto lastopened
///     time <ms>  play  pause  tracks on|off   zoom fit|1-4   sidebar library|layers
///     pinch in|out [<x> <y>]   (one zoom step, kept under that scene pixel or the middle)   zoomkey in|out   (⌘= and ⌘−)
///     librarydrop <name> <col> <row> <level>   (a drop from the Library onto a tile)   expect floatingbar <ids>   expect toolbar none
///     expect inspector shown   (the right column is on screen)
///     editobject [<name>]   back   expect returnscene <name|none>   (Edit Object on the selected item or a named object; Back)
///     expect anchor kept   (the last pinch left its scene point where it was)   expect anchor steady   (and no frame on the way moved it; after wait 600)
///     search <text>   wait <ms>   snapshot <file.png>   log <text>
///     settings open|close   snapshot settings <file.png>   (the Settings window)
///     cli <args…>  (the bundled program on the script's document, from another state dir: an outside edit to the files)
///     cli-owned <args…>  (the bundled program on the script's document, from the app's state dir: it edits through the app)
///     outside <file> <field.path> <json>  (an editor sets one field of a source file, e.g. `outside scenes/harbor.json items.0.object "no-such"`)
///     stale reload|keep     (the changed-on-disk banner's buttons)   secondapp  (a second server on the document: refused, owned)
///     expect dirty|selected|object|at|items|problems|build|outputs|publishdir|document|tool|time <value>
///     expect scenes|objects|zoom|badge|theme|ghosttime <value>   expect scenepixel <x>,<y> <#RRGGBB|empty>
///     expect stale true|false     (the banner shows)   expect mergealert true|false
///     expect savestate saved|pending|problems|conflict   expect subtitle <text>   expect undolabel|redolabel <text|none>
///     expect owner true|false   (the app's server holds the document's lock)
///     expect depth front|back|auto (what the Depth control shows)   expect level <n>   expect groupdepth <path>|selected front|back|auto
///     expect order <entry entry …>   (the scene's items in list order: each entry is a path, `#id`, `selected`,
///         an object name (its first item) or `name@col,row,level`; they must appear in that order)
///     expect ambient <ms|none>   (the longest ambient loop on the scene's items and on the map layers of the objects it uses)
///     expect timeline <ms|none>   (the timeline's length: the longest of motion and ambient loops, at most 60000)
///     expect actions <name,name|none>   (the ACTIONS rows shown for the selected item, or the selected object layer)
///     expect playing <name|none>   (the action on the stage now)   expect lastplay|announced <text>   (what the last Play did / said; contains)
///     expect warning <code>   expect nowarning <code>   (check's warnings: off_grid, out_of_order, copies)
///     expect cli <text>   (the last `cli` step's output contains it)
///     expect footprint <w>,<d>   (the open sprite's)   footprint <w>,<d>   (object mode: the Footprint steppers)
///
/// Scene layers (docs/intents/2026-10-scene-layers/spec.md, D14; targets are
/// `layer:<id>` or an item path):
///
///     select layer:<id>|<path>|#<id> …   select add <target>   click <x> <y> [shift]   dblclick <x> <y>
///     pointer <x> <y>  (the Select tool's hover)   rightclick <x> <y>   rightclick row <target>
///     menu group|ungroup|inside|hide|lock|close   group   ungroup   (also key cmd+g, key shift+cmd+g)
///     inside <id>|done|top   crumb <n>   hide|show|lock|unlock|expand|collapse <id>   rename <name…>
///     movedby <col> <row>   layernudge upleft|upright|downleft|downright   placeinto
///     layerskey <spec> (up, down, shift+down, right, left, return, esc)   library open|close  (sidebar library|layers: the same)
///     (move, drag and the arrow keys act on the whole selection)
///     expect selection <targets,…|none>   expect inside|layername|itemlayer|hover <id|none>   expect scenelayers|rows <n>
///     hide|show|lock|unlock <item path>|selected   (a single item's eye and lock)   expect hiddenitems|lockeditems <paths,…|none>
///     expect hiddencount|lockedcount <n>
///     expect hiddenlayers|lockedlayers <ids,…|none>   expect into <name>   expect crumbs <a›b>   expect movedby <c,r|none>
///     expect menu <enabled items,…|none|closed>   expect tab layers|library   expect inspector layer|item|multi|none   expect faded true|false
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
/// The setup screen's Migrate… (R9; a format-1 document opens the app on the setup screen, and the script runs from there):
///
///     migrate   (clicks Migrate…: the alert comes up)   answer migrate|cancel   (its buttons; Migrate runs `document migrate`, starts the server and opens the document)
///     expect migrate offered|asking|none   (Migrate… is on the setup screen / the alert is up / neither)
///     expect phase setup|starting|ready   expect problem <text>  (the setup screen's document problem contains it)
///     expect migratemessage <text>   (the line under the document row contains it)
///
/// The Agent window and the Versions panel (docs/intents/2026-10-autosave/spec.md, D12; versions by their number):
///
///     agent open|close|size <w>x<h>   snapshot agent <file.png>   menus (logs the File, Edit, View and Window menus)
///     versions open|close   version select <n>|newest|next|prev|none   version restore [doc]   version undo [<n>]
///     version answer yes|no  (the next restore question)   ghost on|off
///     expect versions <n>   expect version <n>|newest author|edits|files|open <value>   expect ghost on|off   expect ghostchanges <n>
///     mcpcall <tool> <json args>  (an MCP client, the bundled program's serve --mcp on the app's state dir: it edits through the app)
///     expect versionresult <text>   (contains)   expect retired <n|none>   expect activity <n>
///
/// The Agent tab's chat (docs/intents/2026-10-agent-chat/spec.md, D15; with PIXELLAB_CLAUDE_HOME
/// and PIXELLAB_CLAUDE_INSTALLER pointing at scripts/fake-claude, never the owner's Claude Code):
///
///     righttab inspector|agent   setup check|install|signin|open|cancel|signout|copy|choose <path>|wait <state>
///     chat new|list|open <n>|type <text…>|send [<text…>]|stop|wait|remember   (expect session same|new|none)   chatkey tab|shift+tab|return|shift+return|esc
///     details on|off   steps expand <n>   choose <answer n> [<question n>]   link <text…>
///     expect setup|righttab|chats|chattitle|messages|steps|steplabel|chatquestion|chosen|cards|cardstatus <value>
///     expect placeholder|draft|draftlines|running|claudeinfo|lastnote|loginurl|details <value>
///     expect lastreply|setupmessage|installlog|context <text>   (contains)
///     chip pin|unpin|remove <target>   savenow   card undo|show [<n>]   altclick <x> <y> [shift]
///     selectuses   activity all|chats|errors   askclaude   agentwindow
///     expect chips|pinned|unsaved|contextline|suggestion|activityrows|banner <value>
///
/// Object libraries and palette files (docs/intents/2026-10-object-libraries/spec.md, D9):
///
///     category Nature/Trees|none   (the open object's Category field, Return)   librarytab <name>|all|others   libraryquery <text>
///     import library|palette <path>   (File › Import…, with the sheet when it has something to ask)   importsheet convert <missing> <existing>
///     importsheet keep|theirs <color>   importsheet add|remove <color>   importsheet import|cancel
///     export library <path> [category]   export palette <path>   openfile <path>  (File › Open Document… on a path: a library asks first)
///     answer offer import|cancel   (that question, before openfile)   dropfile <path>   (a file dropped on the window)
///     expect tabs <a,b|none>   expect libtab <v>   expect headings <a,b|none>   expect libraryobjects <n>   expect libraryorder <a,b>
///     expect category <path|none>   expect categorydraft|categoryerror <v>   expect objectcategory <name> <path|none>   expect palettecolor <name> <#hex|none>
///     expect importsheet sheet|renames|differs|missing|nearest|new|over|count|button|canimport|error <v>   expect importundo <title|none>   expect offer <text>
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
    /// The chat's Claude session when `chat remember` ran, for `expect session same|new`.
    private var rememberedSession: String?
    /// What the last `cli` step printed, for `expect cli`.
    private var lastCLIOutput = ""

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
                log("setup problem: \([p.document, p.program, p.server].compactMap { $0 }.joined(separator: "; "))")
                // A format-1 document waits on Migrate… (R9): the script goes on from the setup screen.
                if p.migrate { break }
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

    /// Whether a word is an item path (digits and dots), as `hide`, `show`, `lock` and `unlock` take for a single item.
    static func isItemPath(_ words: [String], _ i: Int) -> Bool {
        i < words.count && words[i] == "selected" || i < words.count && !words[i].isEmpty && words[i].allSatisfy { $0.isNumber || $0 == "." } && !words[i].hasPrefix(".") && !words[i].hasSuffix(".")
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
            return [r.summary, r.path.map { "path \($0)" }, r.reordered.flatMap { $0.isEmpty ? nil : "reordered \($0.count)" }].compactMap { $0 }.joined(separator: ", ")
        }
        func needFrame(_ r: SceneFrameResult?) throws -> SceneFrameResult {
            guard let r else { throw Failure(model.notice ?? "scene.frame failed") }
            return r
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
            if try arg(words, 1) == "place" { throw Failure("tool place is gone: choose an object with library <name> to start placing") }
            if try arg(words, 1) == "hand" { editor.setTool(.pan); return nil }
            guard let t = StageTool(rawValue: try arg(words, 1)) else { throw Failure("tool select|pan|hand|pencil|eraser|fill|picker") }
            editor.setTool(t)
            return nil
        case "mode", "color", "stroke", "erase", "px", "fill", "pick", "grid", "onion", "resize", "layer", "animate", "still",
             "new", "newdialog", "newcancel", "newsubmit":
            return try await performObject(words, model: model)
        case "library":
            // library open|close: the popover. library <name>: pick that object, which starts placing it.
            switch try arg(words, 1) {
            case "open": if !model.libraryOpen { model.toggleLibrary() }
            case "close": model.closeLibrary()
            default:
                editor.choosePlaceObject(try arg(words, 1))
                _ = await model.thumbnails.art(try arg(words, 1))
            }
            return nil
        case "level":
            editor.setPlaceLevel(try int(words, 1))
            return nil
        case "drop":
            // ⌘↓: onto what is below the selection (docs/intents/2026-10-scene-canvas/spec.md, R16).
            return try needEdit(await editor.dropSelection())
        case "frame":
            // An object's frame (frame next|add…) is the object editor's.
            if model.document?.ref.kind == .object { return try await performObject(words, model: model) }
            // The scene's published size: frame fit | box x,y,w,h | margin n | outline on|off (R8, R10).
            switch try arg(words, 1) {
            case "fit": _ = try needFrame(await model.frameCommand(["fit": true]))
            case "box":
                let n = try arg(words, 2).split(separator: ",").compactMap { Int($0) }
                guard n.count == 4 else { throw Failure("frame box x,y,w,h") }
                _ = try needFrame(await model.frameCommand(["box": .array(n.map { JSONValue($0) })]))
            case "margin": _ = try needFrame(await model.frameCommand(["margin": JSONValue(try int(words, 2))]))
            case "outline": model.showFrame = try arg(words, 2) == "on"
            default: throw Failure("frame fit|box x,y,w,h|margin n|outline on|off")
            }
            return model.document?.frameInfo?.summary
        case "focus":
            editor.focusStage()
            return nil
        case "editobject":
            // Edit Object: the named object, or the selected item's.
            guard let name = words.count > 1 ? words[1] : doc?.selectedItem?.object else { throw Failure("editobject <name>, or select an item first") }
            await model.editObject(name)
            return "editing \(name)"
        case "librarydrop":
            // A drop from the Library popover onto a tile, at a level (R8): `librarydrop <name> <col> <row> <level>`.
            guard let view = editor.stageView as? StageInputView, let geometry = view.geometry else { throw Failure("no stage") }
            let name = try arg(words, 1)
            let level = try int(words, 4)
            editor.setPlaceLevel(level)
            guard let p = geometry.viewPoint(ofTile: try int(words, 2), try int(words, 3), level) else { throw Failure("no tile origin") }
            _ = await model.thumbnails.art(name)
            editor.drop(name, at: p, geometry: geometry, option: false)
            return "dropped \(name)"
        case "back":
            await model.backToScene()
            return "back to \(model.document?.ref.name ?? "none")"
        case "select":
            let what = try arg(words, 1)
            if what == "add" {
                guard let t = LayerTarget(spec: try arg(words, 2)) else { throw Failure("select add layer:<id>|<path>") }
                if !(doc?.selection.contains(t) ?? true) { model.layers.select((doc?.selection ?? []) + [t]) }
            } else if what == "none" { editor.select(nil) } else if what.hasPrefix("#") { editor.select(id: String(what.dropFirst())) }
            else if words.count > 2 || what.hasPrefix("layer:") {
                let targets = words.dropFirst().compactMap { LayerTarget(spec: $0) }
                guard targets.count == words.count - 1 else { throw Failure("select layer:<id>|<path> …") }
                model.layers.select(targets)
            } else { editor.select(what) }
            let sel = doc?.selection.map(\.description).joined(separator: ",") ?? ""
            return sel.isEmpty ? "nothing selected" : "selected \(sel)"
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
            // The stage's own picking: the outermost layer, the layer you're in, locks (docs/intents/2026-10-scene-layers/spec.md, D6).
            model.layers.click(scenePixel: x, y, shift: words.dropFirst(3).contains("shift"), clickCount: 1)
            let sel = doc?.selection.map(\.description).joined(separator: ",") ?? ""
            return "pixel \(x),\(y) → \(sel.isEmpty ? "nothing" : sel)"
        case "hover":
            let t = Tile(col: try int(words, 1), row: try int(words, 2), level: try int(words, 3))
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
            // `place <col> <row>`: no level, so the item goes on top of what is on the tile (R17).
            if words.count == 3 {
                return try needEdit(await editor.place(object, at: Tile(col: try int(words, 1), row: try int(words, 2), level: 0), stack: true))
            }
            await editor.placeAtCursor()
            return doc?.selectedPath.map { "selected \($0)" }
        case "drag" where doc?.selectedItem == nil && !(doc?.selection.isEmpty ?? true):
            // Layers or several things: by whole tiles (D7).
            guard let doc else { throw Failure("no doc") }
            var d = SelectionDrag(targets: doc.selection, paths: doc.layerTree.paths(of: doc.selection), startView: .zero, startScene: .zero)
            d.moved = true
            d.dc = try int(words, 1)
            d.dr = try int(words, 2)
            model.layers.drag = d
            if words.count > 3, words[3] == "hold" { return "holding the drag of \(doc.selection.count) target\(doc.selection.count == 1 ? "" : "s"), \(d.paths.count) items" }
            editor.mouseUp()
            return nil
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
            model.layers.drag = nil
            return nil
        case "key":
            guard let k = KeyInput(spec: try arg(words, 1)) else { throw Failure("unknown key \(words[1])") }
            // A key press the way the owner makes one: a real NSEvent through NSApp.sendEvent (the window's
            // key equivalents, then the first responder's keyDown), so it fails when the stage isn't the
            // first responder or the key is taken before it. `calls` skips that and calls the handler.
            if words.count < 3 || words[2] != "calls", model.document?.ref.kind == .scene, !k.command || k.key == .down {
                guard let view = editor.stageView, let window = view.window else { throw Failure("no stage") }
                guard window.firstResponder === view else { throw Failure("the stage isn't the first responder (click it, or `focus`): \(String(describing: window.firstResponder))") }
                guard let event = StageInputView.keyEvent(for: k, window: window) else { throw Failure("can't make a key event for \(words[1])") }
                // A ⌘ key goes the way AppKit sends a key equivalent: the window's views, then the main menu.
                // NSApp.sendEvent alone skips both when the app isn't frontmost, as under the hook.
                if !k.command || !(window.performKeyEquivalent(with: event) || NSApp.mainMenu?.performKeyEquivalent(with: event) == true) {
                    NSApp.sendEvent(event)
                }
                try? await Task.sleep(for: .milliseconds(60))
                return nil
            }
            if k.command {
                // Menu shortcuts the script can name.
                switch k.key {
                case .down: await editor.dropSelection()
                case .char("d"): await editor.duplicate()
                case .char("g"): if k.shift { await model.layers.ungroup() } else { await model.layers.group() }
                case .char("z"): if k.shift { await model.redo() } else { await model.undo() }
                case .char("s"): await model.save()
                case .char("b"): await model.publish()
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
        case "depth":
            guard let mode = DepthMode(rawValue: try arg(words, 1)) else { throw Failure("depth front|back|auto") }
            guard doc?.selectedItem != nil else { throw Failure("nothing selected") }
            if let doc, let item = doc.selectedItem, doc.depthMode(of: item) == mode { return "already \(mode.rawValue)" }
            return try needEdit(await editor.setDepth(mode))
        case "groupdepth":
            guard let mode = DepthMode(rawValue: try arg(words, 2)) else { throw Failure("groupdepth <group path>|selected front|back|auto") }
            // `selected`: the wrapper of the selected item.
            let path = words[1] == "selected" ? (doc?.selectedItem?.group ?? "") : words[1]
            guard doc?.groups.contains(where: { $0.path == path }) == true else { throw Failure("no group at \(path.isEmpty ? "the selected item" : path)") }
            if doc?.groups.first(where: { $0.path == path })?.depthMode == mode { return "already \(mode.rawValue)" }
            return try needEdit(await editor.setGroupDepth(path, mode))
        case "sort":
            guard let r = await editor.sort() else { throw Failure(model.notice ?? "the sort failed") }
            return [r.summary, r.reordered.flatMap { $0.isEmpty ? nil : "reordered \($0.count)" }].compactMap { $0 }.joined(separator: ", ")
        case "footprint":
            let parts = try arg(words, 1).split(separator: ",").compactMap { Int($0) }
            guard parts.count == 2 else { throw Failure("footprint <w>,<d>") }
            guard model.objectEditor.footprint != nil else { throw Failure("no sprite open") }
            let r = await model.objectEditor.setFootprint(width: parts[0], depth: parts[1])
            guard r != nil else { throw Failure(model.notice ?? "the footprint didn't change") }
            return r?.summary
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
        case "revertto":
            guard try arg(words, 1) == "lastopened" else { throw Failure("revertto lastopened") }
            await model.restoreLastOpened()
            return model.lastEdit ?? model.notice
        case "flush":
            return await model.flush() ? "saved" : "not everything saved"
        case "wait":
            guard words.count > 1, words[1] == "saved" else {
                try? await Task.sleep(for: .milliseconds(try int(words, 1)))
                return nil
            }
            let started = Date()
            while Date().timeIntervalSince(started) < 10 {
                if model.document?.status?.saveState == "saved" { return "saved" }
                try? await Task.sleep(for: .milliseconds(50))
            }
            throw Failure("not saved after 10 s: \(model.document?.status?.saveState ?? "no status")")
        case "cli-owned":
            // The bundled program on the document, as a terminal would run it: the app owns the document, so it edits through it (R22).
            guard let program = Locator.findProgram() else { throw Failure("no program") }
            let out = try await Locator.runCli(program: program, Array(words.dropFirst()) + model.documentLocation.arguments)
            try? await Task.sleep(for: .milliseconds(300))
            lastCLIOutput = out
            return out.split(separator: "\n").last.map(String.init)
        case "outside":
            // An editor outside Pixel Lab changes one field of a file in the document, valid or not.
            guard let root = model.sessionInfo?.document?.root else { throw Failure("no document") }
            let url = URL(fileURLWithPath: root).appendingPathComponent(try arg(words, 1))
            let path = try arg(words, 2).split(separator: ".").map(String.init)
            let value = try JSONSerialization.jsonObject(with: Data(words.dropFirst(3).joined(separator: " ").utf8), options: .fragmentsAllowed)
            let data = try Data(contentsOf: url)
            guard var top = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { throw Failure("\(url.lastPathComponent) isn't an object") }
            func set(_ node: Any, _ keys: ArraySlice<String>) throws -> Any {
                guard let key = keys.first else { return value }
                if var a = node as? [Any], let i = Int(key), a.indices.contains(i) { a[i] = try set(a[i], keys.dropFirst()); return a }
                if var o = node as? [String: Any] { o[key] = try set(o[key] ?? [String: Any](), keys.dropFirst()); return o }
                throw Failure("no \(key) in \(url.lastPathComponent)")
            }
            top = try set(top, path[...]) as? [String: Any] ?? top
            try JSONSerialization.data(withJSONObject: top, options: [.prettyPrinted, .sortedKeys]).write(to: url, options: .atomic)
            // The server's watcher sees it and the app follows.
            try? await Task.sleep(for: .milliseconds(900))
            return "wrote \(url.lastPathComponent)"
        case "mcpcall":
            // One tool call from an MCP client, as Claude Code would make it while the app has the document open (R22).
            guard let program = Locator.findProgram() else { throw Failure("no program") }
            let tool = try arg(words, 1)
            let args = words.dropFirst(2).joined(separator: " ")
            let reply = try await Self.mcpCall(program: program, location: model.documentLocation, tool: tool, arguments: args.isEmpty ? "{}" : args)
            try? await Task.sleep(for: .milliseconds(300))
            lastCLIOutput = reply
            if reply.contains("\"isError\":true") { throw Failure("\(tool) failed: \(reply.prefix(300))") }
            return String(reply.prefix(200))
        case "secondapp":
            // A second server on the same document, as a second copy of the app would start (R26): it's refused.
            guard let program = Locator.findProgram() else { throw Failure("no program") }
            let second = ServerConnection(launch: ServerLaunch(program: program, location: model.documentLocation))
            do {
                _ = try await second.start()
                await second.stop()
                throw Failure("the second server opened the document")
            } catch let e as RPCError where e.dataCode == "owned" {
                await second.stop()
                let pid = e.details?["appPid"]?.doubleValue.map { String(Int($0)) } ?? "?"
                guard pid == String(ProcessInfo.processInfo.processIdentifier) else { throw Failure("owned, but by app \(pid)") }
                return "refused: owned by this app (\(pid))"
            }
        case "cli":
            // An outside editor: the bundled program, one-shot, on the document the script runs on. Its own state
            // dir, so it doesn't find the app that owns the document and writes the files itself, as git or an editor would.
            guard let program = Locator.findProgram() else { throw Failure("no program") }
            let elsewhere = FileManager.default.temporaryDirectory.appendingPathComponent("pixellab-outside-\(ProcessInfo.processInfo.processIdentifier)").path
            let out = try await Locator.runCli(program: program, Array(words.dropFirst()) + model.documentLocation.arguments, environment: ["PIXELLAB_STATE": elsewhere])
            // Give the server's watcher time to see the file and tell the app.
            let started = Date()
            while Date().timeIntervalSince(started) < 2 {
                try? await Task.sleep(for: .milliseconds(100))
                if let id = model.document?.ref.id, model.staleDocs.contains(id) || model.document?.isDirty == false { break }
            }
            try? await Task.sleep(for: .milliseconds(300))
            lastCLIOutput = out
            return out.split(separator: "\n").last.map(String.init)
        case "stale":
            switch try arg(words, 1) {
            case "reload": await model.reloadFromDisk(); return model.lastEdit
            case "keep": await model.keepMine(); return model.lastEdit
            default: throw Failure("stale reload|keep")
            }
        case "build", "publish":
            // `publish [dir]`: into the document's publish folder, or the given one (no panel opens under a script).
            await model.publish(dir: words.count > 1 ? words[1] : nil)
            guard let b = model.build else { throw Failure("no publish") }
            return b.result.map { "ok=\($0.ok) \(Int($0.ms)) ms, \($0.outputs.count) outputs to \($0.publishDir), \($0.problems.count) problems" } ?? b.error
        case "saveas":
            // `saveas <path>`: document.copy, then the app switches to the copy.
            let to = try arg(words, 1)
            await model.saveAs(to: to)
            // Compared as standardized paths, so "…/T//copy.pixellab" (a $TMPDIR with a trailing /) matches.
            let expected = ((to.hasSuffix(".pixellab") ? to : "\(to).pixellab") as NSString).standardizingPath
            let opened = { model.sessionInfo?.document.map { ($0.root as NSString).standardizingPath } }
            let started = Date()
            while Date().timeIntervalSince(started) < 30 {
                if case .ready = model.phase, opened() == expected { break }
                if model.notice != nil { break }
                try? await Task.sleep(for: .milliseconds(100))
            }
            guard case .ready = model.phase, opened() == expected else {
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
        case "play" where words.count > 1:
            let row = try actionRow(words, nameAt: 1, itemAt: 2, model: model)
            await model.timeline.playAction(row)
            return model.timeline.lastPlay
        case "playat":
            let row = try actionRow(words, nameAt: 1, itemAt: 3, model: model)
            await model.timeline.playAction(row, hold: try int(words, 2))
            return model.timeline.reduceMotion ? model.timeline.lastPlay : "\(row.name) held at \(words[2]) ms"
        case "playend":
            model.timeline.endAction(render: true)
            return nil
        case "reducemotion":
            switch try arg(words, 1) {
            case "on": model.timeline.reduceMotionOverride = true
            case "off": model.timeline.reduceMotionOverride = false
            case "system": model.timeline.reduceMotionOverride = nil
            default: throw Failure("reducemotion on|off|system")
            }
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
        case "pinch":
            // One pinch step (a magnify event's effect) anchored at a scene pixel, else the middle of the stage.
            guard let view = editor.stageView as? StageInputView, let geometry = view.geometry else { throw Failure("no stage") }
            let steps = try arg(words, 1) == "in" ? 1 : -1
            let at: CGPoint? = words.count >= 4 ? geometry.viewPoint(fromScene: CGPoint(x: try int(words, 2), y: try int(words, 3))) : nil
            view.zoomStep(steps, atView: at)
            return "zoom \(model.zoomText)"
        case "pinchby":
            // A pinch by a factor (a magnify event's effect), anchored at a scene pixel or the middle: pinchby 1.1 [<x> <y>].
            guard let view = editor.stageView as? StageInputView, let geometry = view.geometry, let factor = Double(try arg(words, 1)) else { throw Failure("pinchby <factor> [<x> <y>]") }
            let at: CGPoint? = words.count >= 4 ? geometry.viewPoint(fromScene: CGPoint(x: try int(words, 2), y: try int(words, 3))) : nil
            // The scroll view's own magnification and the end of the gesture, as a pinch makes them.
            view.simulatePinch(factor: CGFloat(factor), atView: at)
            return "zoom \(model.zoomText)"
        case "zoomkey":
            // ⌘= / ⌘−, centered on the stage.
            model.stepZoom(by: try arg(words, 1) == "in" ? 1 : -1)
            return "zoom \(model.zoomText)"
        case "sidebar":
            // The sidebar has no Library any more: `sidebar library` opens the popover, the others close it.
            switch try arg(words, 1) {
            case "library": if !model.libraryOpen { model.toggleLibrary() }
            default: model.closeLibrary()
            }
            return nil
        case "search":
            model.searchText = words.dropFirst().joined(separator: " ")
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
        case "settings":
            // The Settings window (⌘,): `settings open` shows it, `settings close` hides it.
            switch try arg(words, 1) {
            case "open":
                // The app menu's Settings… item (⌘,), as a click would.
                let item = NSApp.mainMenu?.items.first?.submenu?.items.first { $0.keyEquivalent == "," }
                if let item, let action = item.action { NSApp.sendAction(action, to: item.target, from: item) }
                try? await Task.sleep(for: .milliseconds(500))
                guard Self.settingsWindow() != nil else {
                    throw Failure("the Settings window didn't open (windows: \(NSApp.windows.map { "\($0.title)|\($0.identifier?.rawValue ?? "")|\($0.isVisible)" }.joined(separator: ", ")))")
                }
            case "close": Self.settingsWindow()?.close()
            default: throw Failure("settings open|close")
            }
            return nil
        case "snapshot":
            let agentWindow = words.count > 2 && words[1] == "agent"
            let settingsWindow = words.count > 2 && words[1] == "settings"
            let out = URL(fileURLWithPath: NSString(string: try arg(words, agentWindow || settingsWindow ? 2 : 1)).expandingTildeInPath)
            try? await Task.sleep(for: .milliseconds(300))
            let window = agentWindow ? model.agent.window.flatMap { $0.isVisible ? $0 : nil } : settingsWindow ? Self.settingsWindow() : Snapshotter.mainWindow()
            guard let window, Snapshotter.capture(window, to: out) else {
                throw Failure(agentWindow ? "no Agent window (agent open first)" : settingsWindow ? "no Settings window (settings open first)" : "couldn't capture")
            }
            return out.path
        case "category", "librarytab", "libraryquery", "import", "importsheet", "export", "openfile", "dropfile":
            return try await performLibraries(words, model: model)
        case "answer" where words.count > 1 && words[1] == "offer":
            // The question File › Open Document… asks about a library (R18): `answer offer import|cancel`, before `openfile`.
            model.imports.scriptOffer = try arg(words, 2) == "import"
            return nil
        case "migrate":
            // The setup screen's Migrate… button: the alert comes up (`answer migrate|cancel` settles it).
            guard case .setup(let p) = model.phase, p.migrate else { throw Failure("there's no Migrate… on screen") }
            model.askMigrate()
            return nil
        case "agent", "versions", "version", "ghost", "answer", "menus":
            return try await performAgent(words, model: model)
        case "righttab", "setup", "chat", "chatkey", "details", "steps", "choose", "link", "chip", "savenow", "card", "altclick", "selectuses",
             "activity", "askclaude", "agentwindow":
            return try await performChat(words, model: model)
        case "dblclick", "pointer", "rightclick", "menu", "group", "ungroup", "inside", "crumb", "hide", "show", "lock", "unlock",
             "expand", "collapse", "rename", "movedby", "layernudge", "placeinto", "layerskey":
            return try await performLayers(words, model: model)
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

    /// Scene layers' actions (docs/intents/2026-10-scene-layers/spec.md, D14).
    private func performLayers(_ words: [String], model: AppModel) async throws -> String? {
        let layers = model.layers
        let doc = model.document
        func layer(_ i: Int) throws -> String {
            let ref = try arg(words, i)
            guard let tree = doc?.layerTree else { throw Failure("no scene open") }
            if tree.byID[ref] != nil { return ref }
            if let l = tree.layers.first(where: { $0.name == ref }) { return l.id }
            throw Failure("no layer \(ref) (\(tree.layers.map(\.id).joined(separator: ", ")))")
        }
        func selection() -> String {
            let s = doc?.selection.map(\.description).joined(separator: ",") ?? ""
            return s.isEmpty ? "nothing selected" : "selected \(s)"
        }
        func sceneEvent(_ type: NSEvent.EventType, _ x: Int, _ y: Int) throws -> (StageInputView, NSEvent, CGPoint) {
            guard let view = model.editor.stageView as? StageInputView, let geometry = view.geometry, let window = view.window else { throw Failure("no stage") }
            let viewPoint = geometry.viewPoint(fromScene: CGPoint(x: Double(x) + 0.5, y: Double(y) + 0.5))
            guard let event = NSEvent.mouseEvent(with: type, location: view.convert(viewPoint, to: nil), modifierFlags: [],
                                                 timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber, context: nil,
                                                 eventNumber: 0, clickCount: 1, pressure: 1) else { throw Failure("no event") }
            return (view, event, viewPoint)
        }
        switch words[0] {
        case "dblclick":
            let (x, y) = (try int(words, 1), try int(words, 2))
            if let render = doc?.render { for item in render.items { _ = await model.thumbnails.art(item.pose ?? item.object) } }
            layers.click(scenePixel: x, y, shift: false, clickCount: 1)
            layers.click(scenePixel: x, y, shift: false, clickCount: 2)
            return "inside \(layers.inside ?? "none"), \(selection())"
        case "pointer":
            guard let view = model.editor.stageView as? StageInputView, let geometry = view.geometry else { throw Failure("no stage") }
            if let render = doc?.render { for item in render.items { _ = await model.thumbnails.art(item.pose ?? item.object) } }
            model.editor.pointerMoved(geometry.viewPoint(fromScene: CGPoint(x: Double(try int(words, 1)) + 0.5, y: Double(try int(words, 2)) + 0.5)),
                                      geometry: geometry, option: false)
            return "hover \(layers.hover?.description ?? "none")"
        case "rightclick":
            layers.closeMenu()
            if try arg(words, 1) == "row" {
                // A real right-click, posted to the app's queue so it's dispatched as the
                // pointer's would be, through the window's hit-testing onto the row's catcher.
                guard let t = LayerTarget(spec: try arg(words, 2)) else { throw Failure("rightclick row layer:<id>|<path>") }
                layers.reveal(t)
                layers.scrollRequest = t
                try? await Task.sleep(for: .milliseconds(300))
                var frame = layers.rowFrames[t.description]
                let started = Date()
                while frame == nil, Date().timeIntervalSince(started) < 2 {
                    try? await Task.sleep(for: .milliseconds(50))
                    frame = layers.rowFrames[t.description]
                }
                guard let frame, let window = Snapshotter.mainWindow(), let content = window.contentView else { throw Failure("row \(t) isn't on screen") }
                let at = CGPoint(x: frame.minX + 60, y: content.bounds.height - frame.midY)
                for type in [NSEvent.EventType.rightMouseDown, .rightMouseUp] {
                    guard let event = NSEvent.mouseEvent(with: type, location: at, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                                         windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1) else { throw Failure("no event") }
                    NSApp.postEvent(event, atStart: false)
                }
                let posted = Date()
                while layers.menu == nil, Date().timeIntervalSince(posted) < 2 { try? await Task.sleep(for: .milliseconds(50)) }
                if layers.menu == nil {
                    let hit = content.superview?.hitTest(at).map { "\(type(of: $0)) \($0.identifier?.rawValue ?? "")" } ?? "nothing"
                    throw Failure("no menu opened: row \(t) at \(frame), clicked \(at) in a \(content.bounds.size) window, which hits \(hit)")
                }
            } else {
                let (x, y) = (try int(words, 1), try int(words, 2))
                if let render = doc?.render { for item in render.items { _ = await model.thumbnails.art(item.pose ?? item.object) } }
                let (view, event, _) = try sceneEvent(.rightMouseDown, x, y)
                view.rightMouseDown(with: event)
            }
            try? await Task.sleep(for: .milliseconds(100))
            guard layers.menu != nil else { throw Failure("no menu opened (\(selection()))") }
            return "menu: \(layers.menuItems.filter(\.enabled).map(\.title).joined(separator: ", ")); \(selection())"
        case "menu":
            let which = try arg(words, 1)
            if which == "close" { layers.closeMenu(); return nil }
            guard layers.menu != nil else { throw Failure("no menu open (rightclick first)") }
            guard let action = LayerMenuAction(rawValue: which) else { throw Failure("menu group|ungroup|inside|hide|lock|close") }
            guard layers.menuItems.first(where: { $0.action == action })?.enabled == true else { throw Failure("\(which) is off in the menu") }
            await layers.perform(action)
            switch action {
            case .group, .ungroup: return model.lastEdit.map { "\($0); \(selection())" } ?? selection()
            default:
                let set = { (s: Set<String>) in s.isEmpty ? "none" : s.sorted().joined(separator: ",") }
                return "\(selection()); inside \(layers.inside ?? "none"); hidden \(set(layers.hidden)); locked \(set(layers.locked))"
            }
        case "group":
            guard await layers.group() != nil else { throw Failure(model.notice ?? "nothing grouped") }
            return model.lastEdit
        case "ungroup":
            guard await layers.ungroup() != nil else { throw Failure(model.notice ?? "nothing ungrouped") }
            return "\(model.lastEdit ?? ""); \(selection())"
        case "inside":
            switch try arg(words, 1) {
            case "done": layers.done()
            case "top": layers.goTo(level: nil)
            default: layers.goInside(try layer(1))
            }
            return "inside \(layers.inside ?? "none")"
        case "crumb":
            let n = try int(words, 1)
            guard layers.crumbs.indices.contains(n) else { throw Failure("no crumb \(n) (\(layers.crumbs.count))") }
            layers.goTo(level: layers.crumbs[n].level)
            return "inside \(layers.inside ?? "none")"
        case "hide", "show":
            if Self.isItemPath(words, 1) {
                // An item path (like 3 or 5.2), or `selected`: a single item's eye (R19).
                guard let path = words[1] == "selected" ? doc?.selectedPath : words[1] else { throw Failure("select one item first") }
                if (layers.hiddenItems[path] != nil) != (words[0] == "hide") { layers.toggleItemHidden(path) }
                return "item \(path)"
            }
            let id = try layer(1)
            if layers.hidden.contains(id) != (words[0] == "hide") { layers.toggleHidden(id) }
            return nil
        case "lock", "unlock":
            if Self.isItemPath(words, 1) {
                guard let path = words[1] == "selected" ? doc?.selectedPath : words[1] else { throw Failure("select one item first") }
                if (layers.lockedItems[path] != nil) != (words[0] == "lock") { layers.toggleItemLocked(path) }
                return "item \(path)"
            }
            let id = try layer(1)
            if layers.locked.contains(id) != (words[0] == "lock") { layers.toggleLocked(id) }
            return nil
        case "expand", "collapse":
            let id = try layer(1)
            if layers.expanded.contains(id) != (words[0] == "expand") { layers.toggleExpanded(id) }
            return nil
        case "rename":
            guard doc?.selectedLayer != nil else { throw Failure("select a layer first") }
            layers.nameDraft = words.dropFirst().joined(separator: " ")
            guard let r = await layers.rename(layers.nameDraft) else { throw Failure(model.notice ?? "not renamed") }
            return r.summary
        case "movedby":
            guard doc?.selectedLayer != nil else { throw Failure("select a layer first") }
            let r = await layers.setMovedBy(col: try int(words, 1), row: try int(words, 2))
            return r?.summary ?? "already there"
        case "layernudge":
            let by: (Int, Int) = switch try arg(words, 1) {
            case "upleft": (-1, 0)
            case "upright": (0, -1)
            case "downleft": (0, 1)
            case "downright": (1, 0)
            default: throw Failure("layernudge upleft|upright|downleft|downright")
            }
            guard let r = await layers.move(dc: by.0, dr: by.1, dl: 0) else { throw Failure(model.notice ?? "nothing moved") }
            return r.summary
        case "placeinto":
            guard doc?.selectedLayer != nil else { throw Failure("select a layer first") }
            layers.placeInto()
            return "placing into \(layers.placeTargetName)"
        case "layerskey":
            guard let k = KeyInput(spec: try arg(words, 1)) else { throw Failure("unknown key \(words[1])") }
            let query = model.searchText.trimmingCharacters(in: .whitespaces).lowercased()
            guard layers.treeKey(k, query: query) else { throw Failure("the Layers tree doesn't use \(words[1])") }
            return "\(selection()), inside \(layers.inside ?? "none")"
        default:
            throw Failure("unknown action \(words[0])")
        }
    }

    /// The `expect`s for scene layers; nil for anything else.
    private func expectLayers(_ what: String, model: AppModel) -> String? {
        let layers = model.layers
        let doc = model.document
        let tree = doc?.layerTree ?? .empty
        func list(_ ids: [String]) -> String { ids.isEmpty ? "none" : ids.joined(separator: ",") }
        switch what {
        case "selection": return list(doc?.selection.map(\.description) ?? [])
        case "inside": return layers.inside ?? "none"
        case "scenelayers": return String(tree.layers.count)
        case "layername": return doc?.selectedLayer.map { tree.name(of: $0) } ?? "none"
        case "itemlayer": return doc?.selectedPath.flatMap { tree.layerOfItem[$0] } ?? "none"
        case "hiddenitems": return list(layers.hiddenItems.keys.sorted())
        case "lockeditems": return list(layers.lockedItems.keys.sorted())
        case "hiddencount": return String(layers.hiddenItems.count)
        case "lockedcount": return String(layers.lockedItems.count)
        case "hiddenlayers": return list(layers.hidden.sorted())
        case "lockedlayers": return list(layers.locked.sorted())
        case "into": return layers.placeTargetName
        case "crumbs": return layers.crumbs.map(\.label).joined(separator: "›")
        case "rows": return String(layers.visibleRows(query: model.searchText.trimmingCharacters(in: .whitespaces).lowercased()).count)
        case "movedby": return layers.movedBy.map { "\($0.col),\($0.row)" } ?? "none"
        case "menu": return layers.menu == nil ? "closed" : list(layers.menuItems.filter(\.enabled).map(\.title))
        case "tab": return model.libraryOpen ? "library" : "layers"
        case "library": return model.libraryOpen ? "open" : "closed"
        case "inspector":
            guard let doc else { return "none" }
            if doc.selectedLayer != nil { return "layer" }
            if doc.selection.count > 1 { return "multi" }
            return doc.selectedItem != nil ? "item" : "none"
        case "faded": return String(doc?.render.map { $0.focusImage != nil && $0.focusLayer == layers.inside && layers.inside != nil } ?? false)
        case "hover": return layers.hover?.description ?? "none"
        default: return nil
        }
    }

    /// The Agent tab's chat (docs/intents/2026-10-agent-chat/spec.md, D15).
    private func performChat(_ words: [String], model: AppModel) async throws -> String? {
        let chat = model.chat
        func waitFor(_ what: String, seconds: Double = 60, _ done: () -> Bool) async throws {
            let started = Date()
            while !done() {
                if Date().timeIntervalSince(started) > seconds { throw Failure("still waiting for \(what) after \(Int(seconds)) s") }
                try? await Task.sleep(for: .milliseconds(50))
            }
        }
        switch words[0] {
        case "righttab":
            switch try arg(words, 1) {
            case "agent": model.showAgentTab()
            case "inspector": model.rightTab = .inspector
            default: throw Failure("righttab inspector|agent")
            }
            if model.rightTab == .agent { try await waitFor("the setup check", seconds: 20) { chat.setup != .checking } }
            return chat.setup.rawValue
        case "setup":
            switch try arg(words, 1) {
            case "check":
                await chat.checkAgain()
            case "install":
                chat.runInstaller()
            case "signin":
                chat.signIn()
            case "open":
                chat.openLoginPage()
                return "opened the sign-in page \(chat.openedLoginPage) time(s)"
            case "cancel":
                chat.cancelSignIn()
            case "signout":
                await chat.signOut()
            case "copy":
                chat.copyInstallCommand()
                return NSPasteboard.general.string(forType: .string)
            case "choose":
                await chat.choose(path: NSString(string: words.dropFirst(2).joined(separator: " ")).expandingTildeInPath)
            case "wait":
                let want = try arg(words, 2)
                try await waitFor("setup \(want)", seconds: 30) { chat.setup.rawValue == want }
            default:
                throw Failure("setup check|install|signin|open|cancel|signout|copy|choose <path>|wait <state>")
            }
            return chat.setup.rawValue
        case "chat":
            switch try arg(words, 1) {
            case "new":
                chat.newChat()
            case "list":
                chat.showList()
            case "open":
                let n = try int(words, 2)
                let list = chat.documentChats
                guard list.indices.contains(n - 1) else { throw Failure("no chat \(n) (there are \(list.count))") }
                chat.open(list[n - 1].id)
                return list[n - 1].title
            case "type":
                chat.draft = words.dropFirst(2).joined(separator: " ")
            case "send":
                let text = words.dropFirst(2).joined(separator: " ")
                if let why = chat.cantSend { throw Failure("can't send: \(why)") }
                await chat.send(text.isEmpty ? nil : text)
                guard chat.isRunning else { throw Failure("the turn didn't start: \(chat.current?.messages.last?.text ?? "")") }
            case "stop":
                guard chat.isRunning else { throw Failure("no turn is running") }
                chat.stop()
                try await waitFor("the turn to stop", seconds: 10) { !chat.isRunning }
            case "remember":
                rememberedSession = chat.current?.session
                return rememberedSession ?? "no session yet"
            case "wait":
                try await waitFor("the turn to end") { !chat.isRunning }
                return chat.current?.messages.last.map { "\($0.kind.rawValue): \(($0.text ?? $0.human ?? "").prefix(80))" }
            default:
                throw Failure("chat new|list|open <n>|type <text>|send [<text>]|stop|wait")
            }
            return nil
        case "chatkey":
            guard let view = chat.inputView, let window = view.window else { throw Failure("no chat input on screen (righttab agent)") }
            window.makeFirstResponder(view)
            let spec = try arg(words, 1)
            let (code, chars, flags): (UInt16, String, NSEvent.ModifierFlags) = switch spec {
            case "tab": (48, "\t", [])
            case "shift+tab": (48, "\t", .shift)
            case "return": (36, "\r", [])
            case "shift+return": (36, "\r", .shift)
            case "esc": (53, "\u{1b}", [])
            default: throw Failure("chatkey tab|shift+tab|return|shift+return|esc")
            }
            guard let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
                                               windowNumber: window.windowNumber, context: nil, characters: chars, charactersIgnoringModifiers: chars, isARepeat: false, keyCode: code)
            else { throw Failure("couldn't make the key") }
            view.keyDown(with: event)
            try? await Task.sleep(for: .milliseconds(150))
            return "draft \"\(chat.draft)\", focus \(window.firstResponder === view ? "input" : "moved")"
        case "details":
            chat.details = try arg(words, 1) == "on"
            return nil
        case "steps":
            guard try arg(words, 1) == "expand" else { throw Failure("steps expand <n>") }
            let n = try int(words, 2)
            let runs = ChatRow.rows(chat.current?.messages ?? [], typing: false).filter { if case .steps = $0.kind { return true }; return false }
            guard runs.indices.contains(n - 1) else { throw Failure("no steps row \(n) (there are \(runs.count))") }
            chat.expanded.insert(runs[n - 1].id)
            if case .steps(let s) = runs[n - 1].kind { return s.compactMap(\.human).joined(separator: " | ") }
            return nil
        case "choose":
            let option = try int(words, 1)
            let questions = (chat.current?.messages ?? []).filter { $0.kind == .question }
            let q = words.count > 2 ? questions[safe: (try int(words, 2)) - 1] : questions.last
            guard let q, let options = q.options, options.indices.contains(option - 1) else { throw Failure("no answer \(option) to choose") }
            await chat.answer(q.id, options[option - 1])
            return options[option - 1]
        case "chip":
            guard let t = LayerTarget(spec: try arg(words, 2)) else { throw Failure("chip pin|unpin|remove layer:<id>|<path>") }
            switch try arg(words, 1) {
            case "pin": if !chat.pins.contains(t) { chat.togglePin(t) }
            case "unpin": if chat.pins.contains(t) { chat.togglePin(t) }
            case "remove": chat.removeChip(t)
            default: throw Failure("chip pin|unpin|remove <target>")
            }
            await chat.refreshContext(force: true)
            return chat.chips.map { "\($0.label) \($0.sub)\($0.pinned ? " (pinned)" : "")" }.joined(separator: ", ")
        case "savenow":
            await chat.saveNow()
            return chat.context.map { "unsaved: \($0.unsaved.isEmpty ? "none" : $0.unsaved.joined(separator: ","))" }
        case "card":
            // A change card's buttons (R18): Undo These Changes, Show in Versions.
            let cards = (chat.current?.messages ?? []).filter { $0.kind == .change }
            let card = words.count > 2 ? cards[safe: (try int(words, 2)) - 1] : cards.last
            guard let card else { throw Failure("no change card") }
            switch try arg(words, 1) {
            case "undo": await chat.undoChanges(card)
            case "show": await chat.showInVersions(card)
            default: throw Failure("card undo|show [<n>]")
            }
            let now = chat.current?.messages.first { $0.id == card.id }
            return "versions \(card.versions?.map(String.init).joined(separator: ",") ?? "none"): \(now.map(chat.statusText) ?? "?")"
        case "altclick":
            // A real ⌥-click through the stage: mouse down and up without moving (D6).
            guard let view = model.editor.stageView as? StageInputView, let geometry = view.geometry else { throw Failure("no stage") }
            if let render = model.document?.render { for item in render.items { _ = await model.thumbnails.art(item.pose ?? item.object) } }
            let p = geometry.viewPoint(fromScene: CGPoint(x: Double(try int(words, 1)) + 0.5, y: Double(try int(words, 2)) + 0.5))
            model.editor.mouseDown(p, geometry: geometry, option: true, shift: words.dropFirst(3).contains("shift"), clickCount: 1)
            model.editor.mouseUp()
            return model.document?.selection.map(\.description).joined(separator: ",")
        case "selectuses":
            guard let o = model.document?.selectedItem?.object else { throw Failure("select one item first") }
            model.layers.selectEveryUse(of: o, add: false)
            return model.document?.selection.map(\.description).joined(separator: ",")
        case "activity":
            guard let f = AgentModel.ActivityFilter(rawValue: try arg(words, 1)) else { throw Failure("activity all|chats|errors") }
            model.agent.filter = f
            return "\(model.agent.visibleActivity.count) rows"
        case "askclaude":
            guard let doc = model.document, !doc.selection.isEmpty else { throw Failure("nothing selected: the button isn't there") }
            model.showAgentTab(focusInput: true)
            try await waitFor("the setup check", seconds: 20) { chat.setup != .checking }
            return nil
        case "agentwindow":
            model.agent.showWindow()
            let started = Date()
            while model.agent.window?.isVisible != true, Date().timeIntervalSince(started) < 5 { try? await Task.sleep(for: .milliseconds(50)) }
            return nil
        case "link":
            let text = words.dropFirst().joined(separator: " ")
            guard let reply = chat.current?.messages.last(where: { $0.kind == .claude })?.text,
                  let seg = chat.segments(reply).first(where: { $0.text == text && $0.target != nil }), let t = seg.target
            else { throw Failure("no link \"\(text)\" in the last reply") }
            chat.select(t)
            return t.description
        default:
            throw Failure("unknown chat action")
        }
    }

    private func expectChat(_ what: String, model: AppModel) -> String? {
        let chat = model.chat
        let messages = chat.current?.messages ?? []
        switch what {
        case "setup": return chat.setup.rawValue
        case "righttab": return model.rightTab.rawValue
        case "chats": return String(chat.documentChats.filter { !$0.messages.isEmpty }.count)
        case "chattitle": return chat.current?.title ?? "none"
        case "messages": return String(messages.count)
        case "steps": return String(messages.filter { $0.kind == .step }.count)
        case "steplabel":
            let runs = ChatRow.rows(messages, typing: false).compactMap { r -> [ChatMessage]? in if case .steps(let s) = r.kind { return s }; return nil }
            return runs.last.map(ChatRow.stepsLabel) ?? "none"
        case "chatquestion": return messages.last(where: { $0.kind == .question })?.text ?? "none"
        case "chosen": return messages.last(where: { $0.kind == .question })?.chosen ?? "none"
        case "cards": return String(messages.filter { $0.kind == .change }.count)
        case "cardstatus": return messages.last(where: { $0.kind == .change }).map(chat.statusText) ?? "none"
        case "cardtitle": return messages.last(where: { $0.kind == .change })?.text ?? "none"
        case "placeholder": return chat.placeholder
        case "draft": return chat.draft
        case "draftlines": return String(chat.draft.components(separatedBy: "\n").count)
        case "running": return String(chat.isRunning)
        case "claudeinfo": return chat.statusLine
        case "lastnote": return messages.last(where: { $0.kind == .note })?.text ?? "none"
        case "loginurl": return chat.loginURL?.absoluteString ?? "none"
        case "details": return chat.details ? "on" : "off"
        // R3: the open chat's Claude session against the one `chat remember` noted.
        case "session":
            guard let now = chat.current?.session else { return "none" }
            return now == rememberedSession ? "same" : "new"
        case "chips": return chat.chips.isEmpty ? "none" : chat.chips.map { "\($0.label) \($0.sub)" }.joined(separator: ",")
        case "pinned": return chat.pins.isEmpty ? "none" : chat.pins.map(\.description).joined(separator: ",")
        case "unsavedcount": return String(chat.context?.unsaved.count ?? 0)
        case "unsaved": return (chat.context?.unsaved ?? []).isEmpty ? "none" : (chat.context?.unsaved ?? []).joined(separator: ",")
        case "contextline": return chat.contextSummary
        case "suggestion": return chat.suggestion ?? "none"
        case "activityrows": return String(model.agent.visibleActivity.count)
        case "banner":
            guard let v = model.versions.selected, model.versions.previewing, model.versions.ghostShown else { return "none" }
            return "Version \(v.number) · \(v.author.display)"
        default: return nil
        }
    }

    /// The Agent window's and the Versions panel's actions.
    private func performAgent(_ words: [String], model: AppModel) async throws -> String? {
        let agent = model.agent
        let versions = model.versions
        switch words[0] {
        case "agent":
            switch try arg(words, 1) {
            case "open":
                agent.showWindow()
                let started = Date()
                while agent.window?.isVisible != true, Date().timeIntervalSince(started) < 5 { try? await Task.sleep(for: .milliseconds(50)) }
                guard let w = agent.window, w.isVisible else { throw Failure("the Agent window didn't open") }
                w.setFrame(NSRect(origin: w.frame.origin, size: CGSize(width: 880, height: 640)), display: true)
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
        case "versions":
            switch try arg(words, 1) {
            case "open": await versions.open()
            case "close": versions.close()
            default: throw Failure("versions open|close")
            }
            return "\(versions.versions.count) versions"
        case "version":
            switch try arg(words, 1) {
            case "select":
                let what = try arg(words, 2)
                switch what {
                case "none": versions.select(nil)
                case "newest": versions.select(versions.versions.first?.number)
                case "next": versions.move(by: 1)
                case "prev": versions.move(by: -1)
                default:
                    guard let n = Int(what), versions.versions.contains(where: { $0.number == n }) else { throw Failure("no version \(what) (\(versions.versions.map(\.number).map(String.init).joined(separator: ",")))") }
                    versions.select(n)
                }
                return versions.selected.map { "version \($0.number) by \($0.author.display): \($0.edits.first ?? "")" } ?? "none selected"
            case "restore":
                guard versions.selected != nil else { throw Failure("no version selected") }
                versions.lastQuestion = nil
                await versions.restore(only: words.count > 2 && words[2] == "doc" ? model.document?.ref : nil)
                return [versions.lastQuestion.map { "asked: \($0)" }, versions.lastResult].compactMap { $0 }.joined(separator: "; ")
            case "undo":
                let n = words.count > 2 ? try int(words, 2) : nil
                guard n != nil || versions.selected != nil else { throw Failure("no version selected") }
                _ = await versions.undoChanges(n)
                return versions.lastResult
            case "answer":
                versions.scriptAnswer = (try arg(words, 2)) == "yes"
                return nil
            default: throw Failure("version select|restore|undo|answer")
            }
        case "ghost":
            versions.setGhostShown((try arg(words, 1)) == "on")
            return nil
        case "answer":
            // The Migrate alert's buttons (R9), when it is up.
            if model.migrateAsking, let which = words.dropFirst().first, which == "migrate" || which == "cancel" {
                if which == "migrate" { await model.migrate() } else { model.migrateAsking = false }
                return nil
            }
            throw Failure("answer migrate|cancel (with the Migrate alert up)")
        case "menus":
            for title in ["File", "Edit", "View", "Window"] {
                guard let menu = NSApp.mainMenu?.items.first(where: { $0.title == title })?.submenu else { continue }
                let items = menu.items.filter { !$0.isSeparatorItem }.map { item in
                    let mods = item.keyEquivalentModifierMask
                    let key = item.keyEquivalent.isEmpty ? "" : " [\(mods.contains(.command) ? "⌘" : "")\(mods.contains(.option) ? "⌥" : "")\(item.keyEquivalent)]"
                    if let sub = item.submenu {
                        let inner = sub.items.filter { !$0.isSeparatorItem }.map { i in
                            i.title + (i.keyEquivalent.isEmpty ? "" : " [\(i.keyEquivalentModifierMask.contains(.command) ? "⌘" : "")\(i.keyEquivalent == String(UnicodeScalar(NSDownArrowFunctionKey)!) ? "↓" : i.keyEquivalent)]")
                        }
                        return item.title + " { " + inner.joined(separator: "; ") + " }"
                    }
                    return item.title + key
                }
                log("\(title) menu: \(items.joined(separator: " | "))")
            }
            return nil
        default:
            throw Failure("unknown action \(words[0])")
        }
    }

    /// Runs `pixellab serve --mcp` on the document, initializes, makes one tool call and returns its result line.
    nonisolated static func mcpCall(program: URL, location: DocumentLocation, tool: String, arguments: String) async throws -> String {
        try await Task.detached {
            let p = Process()
            p.executableURL = program
            p.arguments = ["serve", "--mcp"] + location.arguments
            let input = Pipe(), output = Pipe()
            p.standardInput = input
            p.standardOutput = output
            p.standardError = FileHandle.nullDevice
            try p.run()
            let lines = [
                #"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"claude-code","version":"2.1.0"}}}"#,
                #"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"\#(tool)","arguments":\#(arguments)}}"#,
            ]
            input.fileHandleForWriting.write(Data((lines.joined(separator: "\n") + "\n").utf8))
            try? input.fileHandleForWriting.close()
            let out = String(decoding: output.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
            p.waitUntilExit()
            return out.split(separator: "\n").last.map(String.init) ?? ""
        }.value
    }

    /// The Settings window, when it is up.
    static func settingsWindow() -> NSWindow? {
        NSApp.windows.first { $0.isVisible && ($0.identifier?.rawValue.lowercased().contains("settings") == true || $0.title == "Settings" || $0.title.hasSuffix(" Settings")) }
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
        case "movemenu":
            // Edit › Move lists every stage key (R19): passes when its items hold the text.
            guard let edit = NSApp.mainMenu?.items.first(where: { $0.title == "Edit" })?.submenu,
                  let move = edit.items.first(where: { $0.title == "Move" })?.submenu else { throw Failure("no Edit › Move menu") }
            let titles = move.items.filter { !$0.isSeparatorItem }.map { i in
                i.title + (i.keyEquivalent == String(UnicodeScalar(NSDownArrowFunctionKey)!) && i.keyEquivalentModifierMask.contains(.command) ? " [⌘↓]" : "")
            }.joined(separator: " | ")
            guard titles.contains(want) else { throw Failure("Edit › Move has no “\(want)”: \(titles)") }
            return "Edit › Move has \(want)"
        case "frame": got = doc?.ref.kind == .object ? String(model.objectEditor.frame) : (doc?.frameInfo?.frame ?? "none")
        case "size": got = doc.map { "\(Int($0.publishSize.width)),\(Int($0.publishSize.height))" } ?? "none"
        case "outline": got = model.showFrame ? "on" : "off"
        case "window": got = doc?.window.map { "\(Int($0.minX)),\(Int($0.minY)),\(Int($0.width)),\(Int($0.height))" } ?? "none"
        case "cursor": got = model.editor.cursorTile.map(\.description) ?? "none"
        case "visible":
            // The cursor tile (or "selected": the selected item) is inside what the stage shows (R18).
            guard let o = doc?.origin, let view = model.editor.stageView as? StageInputView, let g = view.geometry else { got = "none"; break }
            var tile: Tile?
            let which = words.count > 2 ? words[2] : "cursor"
            let wantVisible = words.count > 3 ? words[3] : "true"
            if which == "selected" { tile = doc?.selectedItem?.at.tile.flatMap { $0.count == 3 ? Tile(col: $0[0], row: $0[1], level: $0[2]) : nil } } else { tile = model.editor.cursorTile }
            guard let tile else { got = "none"; break }
            let c = StageGeometry.tileToScene(col: tile.col, row: tile.row, level: tile.level, origin: o)
            got = view.visibleRect.contains(g.viewRect(fromScene: CGRect(x: c.x - 16, y: c.y - 8, width: 32, height: 16))) ? "true" : "false"
            guard got == wantVisible else { throw Failure("expected visible \(which) \(wantVisible), got \(got)") }
            return "visible \(which) = \(got)"
        case "at": got = doc?.selectedItem?.at.short ?? "none"
        case "class": got = doc?.selectedItem?.class ?? ""
        case "items": got = String(doc?.items.count ?? 0)
        case "problems": got = String(doc?.problems?.count ?? -1)
        case "build": got = model.build?.result.map { $0.ok ? "ok" : "failed" } ?? "none"
        case "tool":
            got = doc?.ref.kind == .object ? (model.objectEditor.handHeld ? "hand" : model.objectEditor.tool.rawValue)
                : (model.editor.isPlacing ? "place" : model.editor.tool.rawValue)
        case "time": got = String(model.timeline.renderTime)
        case "ambient": got = model.timeline.ambientDuration.map { String(Int($0)) } ?? "none"
        case "timeline": got = model.timeline.duration.map { String(Int($0)) } ?? "none"
        case "actions":
            let rows = doc?.ref.kind == .object ? model.objectEditor.actionRows : (doc?.selectedPath.map { model.timeline.actionRows(forItem: $0) } ?? [])
            got = rows.isEmpty ? "none" : rows.map(\.name).joined(separator: ",")
        case "playing": got = model.timeline.playingAction ?? "none"
        case "lastplay", "announced":
            let said = what == "announced" ? model.timeline.lastAnnouncement : model.timeline.lastPlay
            guard said.contains(want) else { throw Failure("expected \(what) to contain \"\(want)\", got \"\(said)\"") }
            return "\(what) = \(said)"
        case "alert": got = model.saveAlert.map { "\($0.kind)" } ?? "none"
        case "stale": got = String(model.showsStaleBanner)
        case "mergealert": got = String(model.mergeAlert != nil)
        case "color": got = model.objectEditor.color
        case "layer": got = String(model.objectEditor.layer)
        case "frame": got = String(model.objectEditor.frame)
        case "layers": got = String(model.objectEditor.layers.count)
        case "frames": got = String(model.objectEditor.frameCount)
        case "hidden": got = model.objectEditor.hidden.sorted().map(String.init).joined(separator: ",")
        case "calls": got = String(model.objectEditor.lastStrokeCalls)
        // What the Depth control shows: the group's depth when the item is alone in its SVG group.
        case "depth": got = doc.flatMap { d in d.selectedItem.map { d.depthMode(of: $0).rawValue } } ?? "none"
        case "level": got = doc?.selectedItem?.at.standingLevel.map(String.init) ?? "none"
        case "groupdepth":
            let parts = words.dropFirst(2)
            guard let given = parts.first, case let path = given == "selected" ? (doc?.selectedItem?.group ?? "") : given,
                  let group = doc?.groups.first(where: { $0.path == path }) else { throw Failure("expect groupdepth <group path>|selected front|back|auto") }
            let wantMode = parts.dropFirst().joined(separator: " ")
            guard group.depthMode.rawValue == wantMode else { throw Failure("expected groupdepth \(path) \(wantMode), got \(group.depthMode.rawValue)") }
            return "groupdepth \(path) = \(wantMode)"
        case "order":
            guard let doc, !doc.items.isEmpty else { throw Failure("no items") }
            // Spaces or commas separate the entries (a `name@col,row,level` keeps its commas).
            let entries = want.split(separator: " ").flatMap { $0.contains("@") ? [String($0)] : $0.split(separator: ",").map(String.init) }
            guard entries.count >= 2 else { throw Failure("expect order <entry,entry,…> (two or more)") }
            var positions: [Int] = []
            for entry in entries {
                guard let i = Self.position(of: entry, in: doc) else { throw Failure("expect order: no item matches \(entry)") }
                positions.append(i)
            }
            guard zip(positions, positions.dropFirst()).allSatisfy({ $0 < $1 }) else {
                throw Failure("expected order \(want), but they stand at list positions \(positions.map(String.init).joined(separator: ","))")
            }
            return "order \(want) (positions \(positions.map(String.init).joined(separator: ",")))"
        case "cli":
            guard lastCLIOutput.contains(want) else { throw Failure("expected the last cli output to contain \"\(want)\", got: \(lastCLIOutput.split(separator: "\n").last.map(String.init) ?? "nothing")") }
            return "cli output has \"\(want)\""
        case "warning", "nowarning":
            let has = doc?.warnings.contains { $0.code == want } ?? false
            guard has == (what == "warning") else {
                throw Failure("expected \(what) \(want), but the warnings are \(doc?.warnings.map(\.code).joined(separator: ",") ?? "unknown")")
            }
            return "\(what) \(want)"
        case "footprint":
            got = model.objectEditor.footprint.map { "\($0.width),\($0.depth)" } ?? "none"
        case "doc": got = doc?.ref.id ?? "none"
        case "mode": got = model.mode.rawValue
        case "versions": got = String(model.versions.versions.count)
        case "version":
            // expect version <n> author|edits|files|open <value>
            let n = words.count > 2 ? (words[2] == "newest" ? model.versions.versions.first?.number : Int(words[2])) : nil
            guard words.count >= 4, let n, let v = model.versions.versions.first(where: { $0.number == n }) else { throw Failure("expect version <n>|newest author|edits|files|open <value> (no such version)") }
            let field = words[3]
            let value = words.dropFirst(4).joined(separator: " ")
            let have: String
            switch field {
            case "author": have = v.author.kind + (v.author.chat.map { " in \($0.title)" } ?? "")
            case "edits": have = v.edits.joined(separator: " | ")
            case "files": have = v.files.joined(separator: ",")
            case "open": have = String(v.open == true)
            default: throw Failure("expect version <n> author|edits|files|open <value>")
            }
            guard field == "edits" ? have.contains(value) : have == value else { throw Failure("expected version \(n) \(field) \(value), got \(have)") }
            return "version \(n) \(field) = \(have)"
        case "versionresult":
            let said = model.versions.lastResult ?? ""
            guard said.contains(want) else { throw Failure("expected the last restore or undo to say \"\(want)\", got \"\(said)\"") }
            return said
        case "ghost": got = model.versions.visibleGhost != nil ? "on" : "off"
        case "ghostchanges":
            guard let render = doc?.render, model.versions.visibleGhost != nil else { throw Failure("no ghost on the stage") }
            let c = model.versions.changes(current: render, renderCount: doc?.renderCount ?? 0)
            let n = { (k: GhostChange.Kind) in c.items.filter { $0.kind == k }.count }
            got = String(c.items.count + c.pixels.count)
            guard got == want else { throw Failure("expected ghostchanges \(want), got \(got)") }
            return doc?.ref.kind == .object ? "\(c.pixels.count) pixels differ" : "added \(n(.added)), moved \(n(.moved)), removed \(n(.removed)): \(c.items.map { "\($0.kind.rawValue) \($0.object) \($0.path ?? "")" }.joined(separator: ", "))"
        case "retired": got = model.agent.retired.map { String($0.count) } ?? "none"
        case "activity": got = String(model.agent.visibleActivity.count)
        case "savestate": got = doc?.status?.saveState ?? (doc?.isDirty == true ? "pending" : "saved")
        case "subtitle": got = model.saveStateText
        case "undolabel": got = model.document?.status?.undoLabel ?? "none"
        case "redolabel": got = model.document?.status?.redoLabel ?? "none"
        case "owner": got = String(model.sessionInfo?.owner?.pid == model.sessionInfo?.pid && model.sessionInfo?.owner != nil)
        case "ghosttime": got = model.versions.visibleGhost?.image.time.map { String(Int($0)) } ?? "none"
        case "scenes": got = String(model.scenes.count)
        case "objects": got = String(model.objects.count)
        case "zoom": got = model.zoomText
        case "returnscene": got = model.returnScene?.name ?? "none"
        case "floatingbar":
            // The bar's buttons by id: select,pan,library in a scene; pencil,eraser,fill,picker,pan in an object (R6).
            guard let kind = doc?.ref.kind else { got = "none"; break }
            got = FloatingBarItems.ids(for: kind).joined(separator: ",")
        case "toolbar":
            // The window has no toolbar items (R12).
            let items = Snapshotter.mainWindow()?.toolbar?.items.filter {
                $0.itemIdentifier != .flexibleSpace && $0.itemIdentifier != .space && !$0.itemIdentifier.rawValue.contains("splitViewSeparator")
            } ?? []
            got = items.isEmpty ? "none" : items.map(\.itemIdentifier.rawValue).joined(separator: ",")
        case "inspector" where want == "shown":
            // The right column is on screen at its least width or more (R13).
            got = model.rightColumnWidth >= 260 ? "shown" : "hidden (\(Int(model.rightColumnWidth)) pt)"
        case "anchor" where want == "steady":
            // No frame drawn during the last zoom showed the anchor away from where it belongs (within a point).
            // The zoom is watched for half a second: `wait 600` first.
            guard let worst = (model.editor.stageView as? StageInputView)?.traceWorst else { throw Failure("no anchored zoom yet") }
            got = worst <= 1 ? "steady" : "strayed \(Int(worst.rounded())) points"
        case "anchor":
            // The last anchored zoom left its scene point where it was on screen (within a point).
            guard let drift = (model.editor.stageView as? StageInputView)?.anchorDrift else { throw Failure("no anchored zoom yet") }
            got = drift <= 1 ? "kept" : "moved \(drift) points"
        case "badge": got = NSApp.dockTile.badgeLabel ?? "none"
        case "theme": got = model.stageDark ? "dark" : "light"
        case "outputs": got = model.build?.result.map { String($0.outputs.count) } ?? "none"
        case "publishdir": got = model.build?.result?.publishDir ?? model.sessionInfo?.document?.publishDir ?? "none"
        case "migrate":
            if model.migrateAsking { got = "asking" }
            else if case .setup(let p) = model.phase, p.migrate { got = "offered" }
            else { got = "none" }
        case "phase":
            got = switch model.phase { case .starting: "starting"; case .setup: "setup"; case .ready: "ready" }
        case "problem", "migratemessage":
            var text = ""
            if what == "problem" { if case .setup(let p) = model.phase { text = p.document ?? "" } } else { text = model.migrateMessage ?? "" }
            guard text.contains(want) else { throw Failure("expected \(what) to contain \"\(want)\", got \"\(text)\"") }
            return "\(what) has \"\(want)\""
        case _ where expectLibraries(words, model: model) != nil:
            let value = expectLibraries(words, model: model) ?? ""
            // importsheet <what>, objectcategory <name> and palettecolor <name> carry one more word before the value.
            let skip = ["importsheet", "objectcategory", "palettecolor"].contains(what) ? 3 : 2
            let wanted = words.dropFirst(skip).joined(separator: " ")
            let contains = what == "categoryerror" || what == "offer" || (what == "importsheet" && words[safe: 2] == "error")
            guard contains ? (wanted == "none" ? value == "none" : value.contains(wanted)) : value == wanted else {
                throw Failure("expected \(words.dropFirst().prefix(skip - 1).joined(separator: " ")) \(contains ? "to contain " : "")\(wanted), got \(value)")
            }
            return "\(what) = \(value)"
        case "document": got = model.sessionInfo?.document?.name ?? "none"
        case "documentkind": got = model.sessionInfo?.document?.kind ?? "none"
        case _ where expectLayers(what, model: model) != nil: got = expectLayers(what, model: model) ?? ""
        case "lastreply", "setupmessage", "installlog", "context":
            let text: String = switch what {
            case "lastreply": model.chat.current?.messages.last(where: { $0.kind == .claude })?.text ?? ""
            case "setupmessage": model.chat.setupMessage ?? ""
            case "context": model.chat.context?.text ?? ""
            default: model.chat.installLog.joined(separator: "\n")
            }
            guard text.contains(want) else { throw Failure("expected \(what) to contain \"\(want)\", got \"\(text.suffix(300))\"") }
            return "\(what) has \"\(want)\""
        case _ where expectChat(what, model: model) != nil: got = expectChat(what, model: model) ?? ""
        case "scenepixel":
            // A pixel of the stage's scene render, in scene pixels: "#RRGGBB" or empty.
            let parts = words.dropFirst(2)
            guard let p = parts.first.flatMap(PixelPoint.init(spec:)), let render = doc?.render else { throw Failure("expect scenepixel x,y <#RRGGBB|empty>") }
            // One pixel of the render (the stage's window can be millions of pixels).
            let sc = max(render.scale, 1)
            let q = PixelPoint(p.x - Int(render.viewBox.minX), p.y - Int(render.viewBox.minY))
            let cell = render.image.cgImage.cropping(to: CGRect(x: q.x * sc, y: q.y * sc, width: sc, height: sc))
            got = cell.flatMap { PixelColors(cgImage: $0, scale: sc) }?.hex(PixelPoint(0, 0)) ?? "empty"
            let wantColor = parts.dropFirst().joined(separator: " ").uppercased()
            guard got.uppercased() == wantColor else { throw Failure("expected scenepixel \(p) \(wantColor), got \(got)") }
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

    /// The ACTIONS row `play`/`playat` names: for the selected item (or `<item>`, which it selects),
    /// or the selected layer of the open object.
    private func actionRow(_ words: [String], nameAt: Int, itemAt: Int, model: AppModel) throws -> ActionRow {
        let name = try arg(words, nameAt)
        var rows: [ActionRow]
        if model.document?.ref.kind == .object {
            rows = model.objectEditor.actionRows
        } else {
            if words.count > itemAt { model.editor.select(words[itemAt]) }
            guard let path = model.document?.selectedPath else { throw Failure("select an item first, or give its path") }
            rows = model.timeline.actionRows(forItem: path)
        }
        guard let row = rows.first(where: { $0.name == name }) else {
            throw Failure("no action \(name) in ACTIONS (\(rows.map(\.name).joined(separator: ", ")))")
        }
        return row
    }

    /// Where an entry of `expect order` stands in the scene's items (list order):
    /// a path, `#id`, `selected`, an object name (its first item) or `name@col,row,level`.
    static func position(of entry: String, in doc: DocumentModel) -> Int? {
        let items = doc.items
        if entry == "selected" { return doc.selectedPath.flatMap { p in items.firstIndex { $0.path == p } } }
        if entry.hasPrefix("#") { return items.firstIndex { $0.id == String(entry.dropFirst()) } }
        if LayerTarget(spec: entry) != nil, let i = items.firstIndex(where: { $0.path == entry }) { return i }
        let parts = entry.split(separator: "@", maxSplits: 1).map(String.init)
        guard let name = parts.first else { return nil }
        if parts.count == 2 {
            return items.firstIndex { $0.object == name && $0.at.tile.map { $0.map(String.init).joined(separator: ",") } == parts[1] }
        }
        return items.firstIndex { $0.object == name }
    }

    // MARK: Object libraries and palette files (docs/intents/2026-10-object-libraries/spec.md, D9)

    /// `category`, `librarytab`, `import`, `importsheet`, `export`, `openfile` and `dropfile`.
    private func performLibraries(_ words: [String], model: AppModel) async throws -> String? {
        let imports = model.imports
        switch words[0] {
        case "category":
            // category Nature/Trees | category none: the open object's Category field, Return.
            guard model.document?.ref.kind == .object else { throw Failure("category needs an open object") }
            let typed = words.dropFirst().joined(separator: " ")
            await model.objectEditor.setCategory(typed: typed.lowercased() == "none" ? "" : typed)
            return model.objectEditor.categoryError ?? model.lastEdit
        case "libraryquery":
            // The Library popover's search (the sidebar's is `search`); empty clears it.
            model.libraryQuery = words.dropFirst().joined(separator: " ")
            model.libraryCursor = 0
            return nil
        case "librarytab":
            let name = words.dropFirst().joined(separator: " ")
            let tabs = model.libraryTabNames
            let want = name.lowercased() == "all" ? LibraryCategories.all : name.lowercased() == "others" ? LibraryCategories.others : name
            guard let tab = tabs.first(where: { $0.lowercased() == want.lowercased() }) else {
                throw Failure("no tab “\(name)” (tabs: \(tabs.isEmpty ? "none" : tabs.joined(separator: ", ")))")
            }
            model.libraryTab = tab
            model.libraryCursor = 0
            return tab
        case "import":
            let path = NSString(string: try arg(words, 2)).expandingTildeInPath
            switch try arg(words, 1) {
            case "library": await imports.startLibrary(path)
            case "palette": await imports.startPalette(path)
            default: throw Failure("import library|palette <path>")
            }
            if imports.library == nil, imports.palette == nil, let n = model.notice, n.hasPrefix("Couldn't") { throw Failure(n) }
            return imports.library != nil ? "library sheet" : imports.palette != nil ? "palette sheet" : imports.lastMessage
        case "importsheet":
            switch try arg(words, 1) {
            case "convert":
                guard imports.library != nil else { throw Failure("no library import sheet") }
                guard imports.library?.plan.missing.contains(where: { $0.name == words[safe: 2] }) == true else { throw Failure("no missing color \(words[safe: 2] ?? "?")") }
                imports.library?.conversions[try arg(words, 2)] = try arg(words, 3)
            case "keep", "theirs":
                guard imports.palette != nil else { throw Failure("no palette import sheet") }
                guard imports.palette?.plan.differing.contains(where: { $0.name == words[safe: 2] }) == true else { throw Failure("no differing color \(words[safe: 2] ?? "?")") }
                if words[1] == "theirs" { imports.palette?.useTheirs.insert(try arg(words, 2)) } else { imports.palette?.useTheirs.remove(try arg(words, 2)) }
            case "add", "remove":
                guard imports.palette != nil else { throw Failure("no palette import sheet") }
                if words[1] == "add" { imports.palette?.selected.insert(try arg(words, 2)) } else { imports.palette?.selected.remove(try arg(words, 2)) }
            case "import":
                if imports.library != nil { await imports.applyLibrary() }
                else if imports.palette != nil { await imports.applyPalette() }
                else { throw Failure("no import sheet is up") }
                return imports.library?.error ?? imports.palette?.error ?? imports.lastMessage
            case "cancel":
                imports.cancelLibrary()
                imports.cancelPalette()
            default: throw Failure("importsheet convert <missing> <existing>|keep <name>|theirs <name>|add <color>|remove <color>|import|cancel")
            }
            return nil
        case "export":
            let path = NSString(string: try arg(words, 2)).expandingTildeInPath
            switch try arg(words, 1) {
            case "library":
                let category = words.count > 3 ? LibraryCategories.parse(words.dropFirst(3).joined(separator: " ")) : nil
                await imports.exportLibrary(to: path, category: category)
            case "palette": await imports.exportPalette(to: path)
            default: throw Failure("export library <path> [category]|palette <path>")
            }
            if let n = model.notice, n.hasPrefix("Couldn't export") { throw Failure(n) }
            return imports.lastMessage
        case "openfile":
            // File › Open Document… on a path: a library or palette file asks whether to import it (R18).
            let path = NSString(string: try arg(words, 1)).expandingTildeInPath
            if ImportModel.isLibrary(path) || ImportModel.isPalette(path) { await imports.offer(path) } else { await model.switchDocument(to: path) }
            return imports.lastOffer
        case "dropfile":
            // A file dropped on the window.
            let took = await imports.drop([NSString(string: try arg(words, 1)).expandingTildeInPath])
            guard took else { throw Failure("not a .pixellib or .pixelpal") }
            return nil
        default: throw Failure("unknown action \(words[0])")
        }
    }

    /// The `expect`s for categories, tabs and imports; nil for anything else.
    private func expectLibraries(_ words: [String], model: AppModel) -> String? {
        let what = words[1]
        let imports = model.imports
        func list(_ items: [String]) -> String { items.isEmpty ? "none" : items.joined(separator: ",") }
        switch what {
        case "tabs": return list(model.libraryTabNames)
        case "libtab": return model.libraryTab
        case "headings": return list(model.librarySections.compactMap(\.heading))
        case "libraryobjects": return String(LibraryCategories.flattened(model.librarySections).count)
        case "libraryorder": return list(LibraryCategories.flattened(model.librarySections).map(\.name))
        case "category": return model.objectEditor.categoryText.isEmpty ? "none" : model.objectEditor.categoryText
        case "categorydraft": return model.objectEditor.categoryDraft.isEmpty ? "none" : model.objectEditor.categoryDraft
        case "categoryerror": return model.objectEditor.categoryError ?? "none"
        case "objectcategory":
            // objectcategory pine Nature › Trees (an object of the document, whatever is open)
            guard let name = words[safe: 2] else { return "usage" }
            let want = words.dropFirst(3).joined(separator: " ")
            guard let object = model.objects.first(where: { $0.name == name }) else { return want == "missing" ? "missing" : "no object \(name)" }
            return object.category.map { LibraryCategories.text($0) } ?? "none"
        case "palettecolor":
            guard let name = words[safe: 2] else { return "usage" }
            return model.paletteByName[name]?.hex.uppercased() ?? "none"
        case "importundo": return imports.undoTitle ?? "none"
        case "offer": return imports.lastOffer ?? "none"
        case "importsheet":
            let sub = words[safe: 2] ?? ""
            switch sub {
            case "sheet": return imports.library != nil ? "library" : imports.palette != nil ? "palette" : "none"
            case "renames": return list(imports.library?.plan.renames.map { "\($0.from)>\($0.to)" } ?? [])
            case "differs":
                if let l = imports.library { return list(l.plan.differing.map(\.name) + (l.plan.character.swapped ? ["character"] : [])) }
                if let p = imports.palette { return list(p.plan.differing.map { "\($0.name)=\(p.useTheirs.contains($0.name) ? "theirs" : "keep")" }) }
                return "none"
            case "missing": return list(imports.library.map { l in l.plan.missing.map { "\($0.name)>\(l.conversions[$0.name] ?? "?")" } } ?? [])
            case "nearest": return list(imports.library?.plan.missing.map { "\($0.name)>\($0.nearest?.name ?? "none")" } ?? [])
            case "new": return list(imports.palette.map { p in p.plan.new.filter { p.selected.contains($0.name) }.map(\.name) } ?? [])
            case "over": return String(imports.palette?.overTotal ?? 0)
            case "count": return String(imports.library?.plan.count ?? 0)
            case "button": return imports.library?.buttonTitle ?? (imports.palette != nil ? "Import" : "none")
            case "canimport": return String(imports.palette?.canImport ?? (imports.library != nil))
            case "error": return imports.library?.error ?? imports.palette?.error ?? "none"
            default: return "importsheet sheet|renames|differs|missing|nearest|new|over|count|button|canimport|error"
            }
        default: return nil
        }
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
                if model.migrating { return true }
                if case .starting = model.phase { return true }
                if model.agent.busy || model.versions.busy || model.imports.busy { return true }
                if model.chat.setup == .checking, model.rightTab == .agent { return true }
                guard let doc = model.document else { return false }
                // A scene's items as of the last edit's result, so Moved by and the inspector read them.
                if doc.ref.kind == .scene, doc.itemsLoaded, !model.layers.itemsCurrent { return true }
                return doc.rendering || doc.checking || doc.loadingItems || doc.render == nil || !model.thumbnails.isIdle
            }()
            if busy { steadySince = nil } else {
                steadySince = steadySince ?? Date()
                if Date().timeIntervalSince(steadySince!) > 0.35 { return }
            }
            try? await Task.sleep(for: .milliseconds(50))
        }
        let d = model.document
        log("(still busy after 30 s: edits \(model.editor.pendingEdits), build \(model.build?.running == true), rendering \(d?.rendering ?? false), checking \(d?.checking ?? false), items \(d?.loadingItems ?? false), items current \(model.layers.itemsCurrent), thumbnails idle \(model.thumbnails.isIdle))")
    }
}

extension Array {
    subscript(safe i: Int) -> Element? { indices.contains(i) ? self[i] : nil }
}
