import AppKit
import Foundation
import Observation
import UniformTypeIdentifiers

/// The library import sheet's state (docs/intents/2026-10-object-libraries/spec.md, D5): the program's dry-run plan,
/// and the color each missing color is converted to.
struct LibraryDraft: Identifiable {
    var path: String
    var plan: LibraryPlan
    /// Missing color → the document's color to convert it to (R15): the nearest to start with.
    var conversions: [String: String]
    /// Why the last Import went wrong, in the sheet.
    var error: String?
    var id: String { path }

    var name: String { URL(fileURLWithPath: path).lastPathComponent }
    /// Colors that differ, or a `character` that will be swapped (R13, R14).
    var hasDifferences: Bool { !plan.differing.isEmpty || plan.character.swapped }
    /// The sheet is needed: a rename, a difference or a missing color (D5). Otherwise the import goes ahead at once.
    var needsSheet: Bool { !plan.renames.isEmpty || hasDifferences || !plan.missing.isEmpty }
    var buttonTitle: String { hasDifferences || !plan.missing.isEmpty ? "Import Anyway" : "Import" }
    /// What the objects that will be imported are called.
    var importing: Int { plan.count }

    /// The document's colors of a tier a missing color can convert to, the nearest first (R15).
    static func options(for missing: LibraryPlan.Missing, palette: [PaletteColor]) -> [PaletteColor] {
        let sameTier = palette.filter { $0.tier == missing.tier }
        guard let nearest = missing.nearest?.name else { return sameTier }
        return sameTier.filter { $0.name == nearest } + sameTier.filter { $0.name != nearest }
    }

    /// "wood-3 is darker here": how the document's color looks against the library's (R14).
    static func lookWord(document: String, library: String) -> String {
        let d = luminance(document), l = luminance(library)
        if abs(d - l) < 0.03 { return "a different shade" }
        return d < l ? "darker" : "lighter"
    }

    static func luminance(_ hex: String) -> Double {
        let v = UInt32(hex.trimmingCharacters(in: CharacterSet(charactersIn: "#")), radix: 16) ?? 0
        return (0.2126 * Double((v >> 16) & 255) + 0.7152 * Double((v >> 8) & 255) + 0.0722 * Double(v & 255)) / 255
    }
}

/// The palette import sheet's state (D6): the plan, which differing colors take theirs, and which new colors to add.
struct PaletteDraft: Identifiable {
    var path: String
    var plan: PalettePlan
    /// Differing colors set to Use Theirs; the rest are Keep Mine, the default (R22).
    var useTheirs: Set<String> = []
    /// The new colors to add (all, to start with).
    var selected: Set<String>
    var error: String?
    var id: String { path }

    var name: String { URL(fileURLWithPath: path).lastPathComponent }

    init(path: String, plan: PalettePlan) {
        self.path = path
        self.plan = plan
        selected = Set(plan.new.map(\.name))
    }

    /// How many new colors of the tier are selected.
    func adding(_ tier: String) -> Int { plan.new.filter { $0.tier == tier && selected.contains($0.name) }.count }
    /// How many colors the chosen ones would put over a tier's cap (R23).
    func over(_ tier: String) -> Int {
        let t = tier == "world" ? plan.tiers.world : plan.tiers.outfit
        return max(t.have + adding(tier) - t.cap, 0)
    }
    var overTotal: Int { over("world") + over("outfit") }
    /// Some new colors are off the list: the command is narrowed with `only`.
    var narrowed: Bool { selected.count != plan.new.count }
    var canImport: Bool { overTotal == 0 && (!selected.isEmpty || !plan.differing.isEmpty) }
}

/// Importing and exporting libraries and palette files: the File menu's items, the open and save panels, the two sheets,
/// opening a library as a document (R18), and Edit › Undo Import (docs/intents/2026-10-object-libraries/spec.md, D5, D6, D10).
@MainActor @Observable
final class ImportModel {
    @ObservationIgnored weak var app: AppModel?

    /// The sheets, when up.
    var library: LibraryDraft?
    var palette: PaletteDraft?
    /// A command is on its way (scripts settle on it).
    var working = 0
    /// The last thing said, for the script hook.
    var lastMessage: String?
    /// The last offer question (R18), and the script's answer to it.
    var lastOffer: String?
    @ObservationIgnored var scriptOffer: Bool?

    /// What Edit › Undo Import takes back, while nothing has followed it.
    enum Undoable: Equatable {
        case library(version: Int)
        case palette
    }
    var undoable: Undoable?
    /// The palette doc's undo label as the import left it: another edit changes it.
    @ObservationIgnored private var paletteLabel: String?
    @ObservationIgnored private var expectingPalette = false

    var busy: Bool { working > 0 }

    func reset() {
        library = nil
        palette = nil
        undoable = nil
    }

    // MARK: Undo Import

    var undoTitle: String? { canUndo ? "Undo Import" : nil }

    /// A library import is undoable while its version is the newest; a palette import while no other doc has an edit.
    var canUndo: Bool {
        switch undoable {
        case .library(let version)?: app?.versions.versions.first?.number == version
        case .palette?: true
        case nil: false
        }
    }

    /// `doc.changed`: an edit after an import takes Undo Import away (the next ⌘Z undoes that edit instead).
    func docChanged(_ status: DocStatus) {
        if status.doc == "palette:" {
            if expectingPalette { paletteLabel = status.undoLabel }
            else if case .palette? = undoable, status.undoLabel != paletteLabel { undoable = nil }
        } else if undoable != nil, status.unsaved {
            undoable = nil
        }
    }

    func undo() async {
        guard let app, let connection = app.connection, let what = undoable else { return }
        undoable = nil
        working += 1
        defer { working -= 1 }
        switch what {
        case .library(let version):
            if await app.versions.undoChanges(version) { say("Undid the import") } else { app.notice = app.versions.lastResult }
        case .palette:
            do {
                let r: UndoResult = try await connection.call("doc.undo", ["doc": .string("palette:")])
                await refreshAfterPalette()
                say(r.summary ?? "Undid the palette import")
            } catch {
                app.notice = "Couldn't undo the import: \(error.localizedDescription)"
            }
        }
    }

    // MARK: Library import (D5)

    /// File › Import Library…: a panel, then `startLibrary`.
    func chooseLibrary() async {
        guard let app, app.phase == .ready else { return }
        guard let path = Self.chooseFile(title: "Import an object library", message: "Choose a .pixellib. Its objects are copied into this document.", extension: "pixellib", package: true, start: app.documentPath) else { return }
        await startLibrary(path)
    }

    /// Reads the library against the document (`dryRun`); with nothing to ask, imports it at once.
    func startLibrary(_ path: String) async {
        guard let app, let connection = app.connection, app.phase == .ready else { return }
        working += 1
        defer { working -= 1 }
        do {
            let plan: LibraryPlan = try await connection.call("library.import", ["from": .string(path), "dryRun": true])
            var draft = LibraryDraft(path: path, plan: plan, conversions: [:])
            for m in plan.missing { draft.conversions[m.name] = LibraryDraft.options(for: m, palette: app.palette).first?.name ?? m.nearest?.name }
            if draft.needsSheet {
                library = draft
                say("Import \(draft.name): \(plan.count) objects")
            } else {
                await apply(draft)
            }
        } catch {
            app.notice = "Couldn't read \(URL(fileURLWithPath: path).lastPathComponent): \(error.localizedDescription)"
            say(app.notice ?? "")
        }
    }

    /// The sheet's Import / Import Anyway.
    func applyLibrary() async {
        guard let draft = library else { return }
        await apply(draft)
    }

    func cancelLibrary() { library = nil }

    private func apply(_ draft: LibraryDraft) async {
        guard let app, let connection = app.connection else { return }
        var params: [String: JSONValue] = ["from": .string(draft.path)]
        if draft.hasDifferences { params["acceptDifferences"] = true }
        if !draft.plan.missing.isEmpty {
            params["convert"] = .object(draft.conversions.mapValues { JSONValue.string($0) })
        }
        working += 1
        defer { working -= 1 }
        do {
            let r: LibraryPlan = try await connection.call("library.import", params)
            library = nil
            await app.loadLists()
            app.thumbnails.invalidate(r.imported ?? [])
            await app.versions.load()
            if let v = r.version, r.changed == true { undoable = .library(version: v) }
            app.lastEdit = r.summary
            app.notice = nil
            say(r.summary ?? "Imported")
        } catch let e as RPCError {
            // The sheet stays up with the reason; with no sheet (an import at once) it is a notice.
            if library != nil { library?.error = e.message } else { app.notice = e.message }
            say(e.message)
        } catch {
            if library != nil { library?.error = error.localizedDescription } else { app.notice = error.localizedDescription }
        }
    }

    // MARK: Palette import (D6)

    func choosePalette() async {
        guard let app, app.phase == .ready else { return }
        guard let path = Self.chooseFile(title: "Import a palette", message: "Choose a .pixelpal. Its new colors are added to this document's palette.", extension: "pixelpal", package: false, start: app.documentPath) else { return }
        await startPalette(path)
    }

    func startPalette(_ path: String) async {
        guard let app, let connection = app.connection, app.phase == .ready else { return }
        working += 1
        defer { working -= 1 }
        do {
            let plan: PalettePlan = try await connection.call("palette.import", ["from": .string(path), "dryRun": true])
            if plan.new.isEmpty, plan.differing.isEmpty, plan.otherTier.isEmpty {
                app.lastEdit = "Nothing to import: this document has every color in \(URL(fileURLWithPath: path).lastPathComponent)"
                say(app.lastEdit ?? "")
                return
            }
            palette = PaletteDraft(path: path, plan: plan)
            say("Import \(URL(fileURLWithPath: path).lastPathComponent): \(plan.new.count) new colors")
        } catch {
            app.notice = "Couldn't read \(URL(fileURLWithPath: path).lastPathComponent): \(error.localizedDescription)"
            say(app.notice ?? "")
        }
    }

    func applyPalette() async {
        guard let app, let connection = app.connection, let draft = palette, draft.canImport else { return }
        var params: [String: JSONValue] = ["from": .string(draft.path)]
        let theirs = draft.plan.differing.map(\.name).filter { draft.useTheirs.contains($0) }
        let mine = draft.plan.differing.map(\.name).filter { !draft.useTheirs.contains($0) }
        if !mine.isEmpty { params["keep"] = .array(mine.map { .string($0) }) }
        if !theirs.isEmpty { params["useTheirs"] = .array(theirs.map { .string($0) }) }
        if draft.narrowed { params["only"] = .array(draft.plan.new.map(\.name).filter { draft.selected.contains($0) }.map { .string($0) }) }
        working += 1
        expectingPalette = true
        defer { working -= 1 }
        do {
            let r: PalettePlan = try await connection.call("palette.import", params)
            expectingPalette = false
            palette = nil
            await refreshAfterPalette()
            if r.changed == true { undoable = .palette }
            app.lastEdit = r.summary
            app.notice = nil
            say(r.summary ?? "Imported")
        } catch let e as RPCError {
            expectingPalette = false
            palette?.error = e.message
            say(e.message)
        } catch {
            expectingPalette = false
            palette?.error = error.localizedDescription
        }
    }

    func cancelPalette() { palette = nil }

    /// A palette change (an import, or its undo): swatches, thumbnails and renders follow.
    private func refreshAfterPalette() async {
        guard let app else { return }
        await app.loadPalette()
        app.thumbnails.invalidateAll()
        app.timeline.invalidate()
        app.objectEditor.docChanged()
        app.requestRender()
        app.requestCheck()
        app.requestItems()
    }

    // MARK: Export (D4)

    /// File › Export Library…: a save panel with an Objects menu (All Objects, or one category).
    func exportLibrary(to path: String? = nil, category: [String]? = nil, replace: Bool = false) async {
        guard let app, let connection = app.connection, app.phase == .ready else { return }
        var to = path
        var cat = category
        var replace = replace
        if to == nil {
            guard let chosen = Self.chooseLibraryExport(name: "\(app.documentName ?? "Untitled").pixellib", start: app.sessionInfo?.document?.root, categories: LibraryCategories.categories(app.objects)) else { return }
            to = chosen.path
            cat = chosen.category
            replace = true // the panel asked about replacing
        }
        guard let to else { return }
        var params: [String: JSONValue] = ["to": .string(to)]
        if let cat { params["category"] = .array(cat.map { .string($0) }) }
        if replace { params["replace"] = true }
        working += 1
        defer { working -= 1 }
        await app.flush()
        do {
            let r: ExportResult = try await connection.call("library.export", params)
            app.lastEdit = r.summary
            say(r.summary)
        } catch {
            app.notice = "Couldn't export: \(error.localizedDescription)"
            say(app.notice ?? "")
        }
    }

    func exportPalette(to path: String? = nil, replace: Bool = false) async {
        guard let app, let connection = app.connection, app.phase == .ready else { return }
        var to = path
        var replace = replace
        if to == nil {
            guard let chosen = Self.chooseSave(title: "Export the palette", message: "The palette is written to a .pixelpal.", name: "\(app.documentName ?? "Untitled").pixelpal", extension: "pixelpal", start: app.sessionInfo?.document?.root) else { return }
            to = chosen
            replace = true
        }
        guard let to else { return }
        var params: [String: JSONValue] = ["to": .string(to)]
        if replace { params["replace"] = true }
        working += 1
        defer { working -= 1 }
        do {
            let r: ExportResult = try await connection.call("palette.export", params)
            app.lastEdit = r.summary
            say(r.summary)
        } catch {
            app.notice = "Couldn't export: \(error.localizedDescription)"
            say(app.notice ?? "")
        }
    }

    // MARK: Opening and dropping a file (R12, R18)

    nonisolated static func isLibrary(_ path: String) -> Bool { path.lowercased().hasSuffix(".pixellib") }
    nonisolated static func isPalette(_ path: String) -> Bool { path.lowercased().hasSuffix(".pixelpal") }

    /// A library opened as a document (Open Document…, Finder): it isn't one, so ask whether to import it (R18).
    func offer(_ path: String) async {
        guard let app else { return }
        let file = URL(fileURLWithPath: path).lastPathComponent
        guard app.phase == .ready else {
            app.notice = "Open a document first, then import \(file) into it."
            return
        }
        let library = Self.isLibrary(path)
        let title = library ? "“\(file)” is an object library" : "“\(file)” is a palette file"
        let into = app.documentName.map { "“\($0)”" } ?? "the open document"
        let text = library
            ? "It holds objects to copy into a document, not a document to open. Import its objects into \(into)?"
            : "It holds colors to add to a document, not a document to open. Import its colors into \(into)?"
        lastOffer = title
        let yes: Bool
        if app.script != nil {
            yes = scriptOffer ?? true
            scriptOffer = nil
        } else {
            let alert = NSAlert()
            alert.messageText = title
            alert.informativeText = text
            alert.addButton(withTitle: "Import…")
            alert.addButton(withTitle: "Cancel")
            let window = [NSApp.keyWindow, app.mainWindow].compactMap { $0 }.first { $0.isVisible }
            let response = window != nil ? await alert.beginSheetModal(for: window!) : alert.runModal()
            yes = response == .alertFirstButtonReturn
        }
        guard yes else { return }
        if library { await startLibrary(path) } else { await startPalette(path) }
    }

    /// Files dropped on the window: a library or a palette file starts its import. Returns whether any did.
    @discardableResult
    func drop(_ paths: [String]) async -> Bool {
        var took = false
        for path in paths {
            if Self.isLibrary(path) { took = true; await startLibrary(path) }
            else if Self.isPalette(path) { took = true; await startPalette(path) }
        }
        return took
    }

    // MARK: Panels

    private func say(_ text: String) {
        lastMessage = text
        app?.editor.announce(text)
    }

    @MainActor static func chooseFile(title: String, message: String, extension ext: String, package: Bool, start: String) -> String? {
        let panel = NSOpenPanel()
        panel.title = title
        panel.message = message
        panel.canChooseFiles = true
        panel.canChooseDirectories = package
        panel.allowsMultipleSelection = false
        panel.treatsFilePackagesAsDirectories = false
        if let type = UTType(filenameExtension: ext) { panel.allowedContentTypes = [type] }
        panel.directoryURL = URL(fileURLWithPath: start).deletingLastPathComponent()
        return panel.runModal() == .OK ? panel.url?.path : nil
    }

    @MainActor static func chooseSave(title: String, message: String, name: String, extension ext: String, start: String?) -> String? {
        let panel = NSSavePanel()
        panel.title = title
        panel.message = message
        panel.nameFieldStringValue = name.hasSuffix(".\(ext)") ? name : "\(name).\(ext)"
        panel.canCreateDirectories = true
        panel.treatsFilePackagesAsDirectories = false
        if let start { panel.directoryURL = URL(fileURLWithPath: start).deletingLastPathComponent() }
        guard panel.runModal() == .OK, var path = panel.url?.path else { return nil }
        if !path.lowercased().hasSuffix(".\(ext)") { path += ".\(ext)" }
        return path
    }

    /// The export panel, with an Objects menu: All Objects, or one category and everything below it (R11).
    @MainActor static func chooseLibraryExport(name: String, start: String?, categories: [[String]]) -> (path: String, category: [String]?)? {
        let panel = NSSavePanel()
        panel.title = "Export an object library"
        panel.message = "The objects are copied into a .pixellib. The document doesn't change."
        panel.nameFieldStringValue = name
        panel.canCreateDirectories = true
        panel.treatsFilePackagesAsDirectories = false
        if let start { panel.directoryURL = URL(fileURLWithPath: start).deletingLastPathComponent() }
        let menu = NSPopUpButton(frame: .zero, pullsDown: false)
        menu.addItem(withTitle: "All Objects")
        for c in categories { menu.addItem(withTitle: LibraryCategories.text(c)) }
        menu.setAccessibilityLabel("Objects")
        let label = NSTextField(labelWithString: "Objects:")
        let stack = NSStackView(views: [label, menu])
        stack.spacing = 8
        stack.edgeInsets = NSEdgeInsets(top: 8, left: 0, bottom: 8, right: 0)
        stack.frame = NSRect(x: 0, y: 0, width: 320, height: 40)
        panel.accessoryView = stack
        guard panel.runModal() == .OK, var path = panel.url?.path else { return nil }
        if !path.lowercased().hasSuffix(".pixellib") { path += ".pixellib" }
        let index = menu.indexOfSelectedItem
        return (path, index > 0 && index - 1 < categories.count ? categories[index - 1] : nil)
    }
}
