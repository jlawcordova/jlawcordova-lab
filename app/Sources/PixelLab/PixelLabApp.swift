import AppKit
import SwiftUI

@main
struct PixelLabApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    // A plain reference, not @State: the Command Line Tools ship without the
    // SwiftUI macro plugin, and the model is a singleton anyway.
    private let model = AppModel.shared

    var body: some Scene {
        Window("Pixel Lab", id: WindowID.main) {
            MainWindow()
                .environment(model)
                .frame(minWidth: 960, minHeight: 600)
        }
        .defaultSize(width: 1440, height: 900)
        .windowToolbarStyle(.unifiedCompact)
        .commands { PixelLabCommands(model: model) }

        // A single window SwiftUI remembers the frame of. Its automatic Window
        // menu item is replaced by Window › Agent Window (⇧⌥⌘A), since Window ›
        // Agent (⌥⌘A) shows the main window's Agent tab
        // (docs/intents/2026-10-agent-chat/spec.md, D13).
        Window("Agent", id: WindowID.agent) {
            AgentWindow().environment(model)
        }
        .defaultSize(width: 880, height: 820)
        .defaultPosition(.trailing)
        .commandsRemoved()

        Settings {
            SettingsView().environment(model)
        }
    }
}

enum WindowID {
    static let main = "main"
    static let agent = "agent"
}

/// File, Edit and View menu commands. Plain 1–4 and 0 also zoom while the stage
/// has focus.
struct PixelLabCommands: Commands {
    var model: AppModel
    @Environment(\.openWindow) private var openWindow

    var body: some Commands {
        CommandGroup(replacing: .newItem) {
            Button("New Scene…") { model.showNew(.scene) }
                .keyboardShortcut("n")
                .disabled(model.phase != .ready)
            Button("New Object…") { model.showNew(.object) }
                .keyboardShortcut("n", modifiers: [.command, .shift])
                .disabled(model.phase != .ready)
            Divider()
            Button("New Document…") { Task { await model.newDocument() } }
                .keyboardShortcut("n", modifiers: [.command, .option])
            Button("Open Document…") { Task { await model.openDocument() } }
                .keyboardShortcut("o")
        }
        CommandGroup(replacing: .saveItem) {
            Button("Save") { Task { await model.save() } }
                .keyboardShortcut("s")
                .disabled(!model.canSave)
            Button("Save As…") { Task { await model.saveAs() } }
                .keyboardShortcut("s", modifiers: [.command, .shift])
                .disabled(model.phase != .ready)
            Button("Revert to Saved") { Task { await model.revert() } }
                .disabled(!model.canSave)
            Divider()
            Button("Publish") { Task { await model.publish() } }
                .keyboardShortcut("b")
                .disabled(model.phase != .ready || model.build?.running == true)
            Button("Publish to Folder…") { Task { await model.publish(askFolder: true) } }
                .disabled(model.phase != .ready || model.build?.running == true)
        }
        CommandGroup(after: .pasteboard) {
            Divider()
            Button("Duplicate") { Task { await model.editor.duplicate() } }
                .keyboardShortcut("d")
                .disabled(!hasSelection)
            Button("Remove Item (⌫ on the stage)") { Task { await model.editor.remove() } }
                .disabled(!hasSelection)
            Button("Snap to Tile") { Task { await model.editor.snap() } }
                .disabled(model.document?.selectedItem?.at.isTile ?? true)
            // Every stage key, so the keys can be found (docs/intents/2026-10-scene-canvas/spec.md, R19).
            // Only ⌘↓ is a shortcut here: the others are the stage's own keys, which a text field must keep.
            Menu("Move") {
                Button("Select Next Item (Tab)") { stageKey(.tab) }
                Button("Select Previous Item (⇧Tab)") { stageKey(.tab, shift: true) }
                Divider()
                Button("Column −1 (←)") { stageKey(.left) }
                Button("Column +1 (→)") { stageKey(.right) }
                Button("Row −1 (↑)") { stageKey(.up) }
                Button("Row +1 (↓)") { stageKey(.down) }
                Divider()
                Button("Up a Level (⇧↑, Page Up)") { stageKey(.up, shift: true) }
                Button("Down a Level (⇧↓, Page Down)") { stageKey(.down, shift: true) }
                Button("Drop onto What Is Below") { dropOrText() }
                    .keyboardShortcut(.downArrow, modifiers: .command)
                Divider()
                Button("Nudge a Pixel (⌥ + arrows)") { stageKey(.right, option: true) }
                Button("Place at the Cursor, on Top of a Block (Return)") { stageKey(.returnKey) }
            }
            .disabled(!(model.mode == .scene && model.document != nil))
            Button("Edit Object") { if let o = model.document?.selectedItem?.object { Task { await model.editObject(o) } } }
                .keyboardShortcut(.return, modifiers: .command)
                .disabled(!(hasSelection && model.mainIsKey))
            Button("Select None") { model.layers.select([]) }
                .disabled(!(model.mode == .scene && !(model.document?.selection.isEmpty ?? true)))
            // ⌥-click's keyboard way (docs/intents/2026-10-agent-chat/spec.md, D6).
            Button("Select Every Use") { if let o = model.document?.selectedItem?.object { model.layers.selectEveryUse(of: o, add: false) } }
                .disabled(!(model.mode == .scene && model.document?.selectedItem != nil))
            Divider()
            // Scene layers (docs/intents/2026-10-scene-layers/spec.md, D9, D10).
            Button("Group") { Task { await model.layers.group() } }
                .keyboardShortcut("g")
                .disabled(!(model.mode == .scene && !(model.document?.selection.isEmpty ?? true)))
            Button("Ungroup") { Task { await model.layers.ungroup() } }
                .keyboardShortcut("g", modifiers: [.command, .shift])
                .disabled(!(model.mode == .scene && model.document?.selectedLayer != nil))
        }
        CommandGroup(replacing: .undoRedo) {
            Button("Undo") { undoOrText(Selector(("undo:"))) { await model.undo() } }
                .keyboardShortcut("z")
            Button("Redo") { undoOrText(Selector(("redo:"))) { await model.redo() } }
                .keyboardShortcut("z", modifiers: [.command, .shift])
        }
        CommandGroup(after: .toolbar) {
            Divider()
            Button(model.mode == .object && model.backTarget != nil ? "Back to \(model.backTarget ?? "")" : "Back to Scene") { Task { await model.backToScene() } }
                .keyboardShortcut("[")
                .disabled(model.mode != .object || model.backTarget == nil)
            Divider()
            Button("Zoom In") { model.stepZoom(by: 1) }
                .keyboardShortcut("=")
                .disabled(model.document == nil)
            Button("Zoom Out") { model.stepZoom(by: -1) }
                .keyboardShortcut("-")
                .disabled(model.document == nil)
            Button("Zoom to Fit") { model.zoom = .fit }
                .keyboardShortcut("0")
            ForEach(0..<4, id: \.self) { i in
                Button("Zoom \(Zoom.levels(for: model.mode)[i])×") { model.setZoomLevel(index: i) }
                    .keyboardShortcut(KeyEquivalent(Character(String(i + 1))))
            }
            Button(gridShown ? "Hide Grid" : "Show Grid") {
                if model.mode == .object { model.objectEditor.pixelGrid.toggle() } else { model.showGrid.toggle() }
            }
                .keyboardShortcut("'")
            Button(model.showFrame ? "Hide Published Frame" : "Show Published Frame") { model.showFrame.toggle() }
                .disabled(model.mode != .scene)
            Divider()
        }
        // The chat in the main window, and the review window (spec D13).
        CommandGroup(before: .windowList) {
            Button(model.agent.pendingCount > 0 ? "Agent (\(model.agent.pendingCount) to review)" : "Agent") { model.showAgentTab(focusInput: true) }
                .keyboardShortcut("a", modifiers: [.command, .option])
                .disabled(model.phase != .ready)
            Button("Agent Window") { openWindow(id: WindowID.agent) }
                .keyboardShortcut("a", modifiers: [.command, .option, .shift])
            Divider()
        }
    }

    private func stageKey(_ key: KeyInput.Key, shift: Bool = false, option: Bool = false) {
        _ = model.editor.handleKey(KeyInput(key: key, option: option, shift: shift))
    }

    /// ⌘↓ drops the selection on the stage; in a text field it keeps moving the caret to the end.
    private func dropOrText() {
        if NSApp.keyWindow?.firstResponder is NSTextView {
            NSApp.sendAction(Selector(("moveToEndOfDocument:")), to: nil, from: nil)
        } else {
            Task { await model.editor.dropSelection() }
        }
    }

    private var gridShown: Bool { model.mode == .object ? model.objectEditor.pixelGrid : model.showGrid }

    private var hasSelection: Bool { model.mode == .scene && model.document?.selectedItem != nil }

    /// ⌘Z in a text field undoes typing; elsewhere it undoes the doc.
    private func undoOrText(_ action: Selector, doc: @escaping @MainActor () async -> Void) {
        if NSApp.keyWindow?.firstResponder is NSTextView {
            NSApp.sendAction(action, to: nil, from: nil)
        } else {
            Task { await doc() }
        }
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var terminating = false

    func applicationDidFinishLaunching(_ notification: Notification) {
        // `swift run` starts a bare executable: make it a regular, frontmost app.
        NSApp.setActivationPolicy(.regular)
        Fonts.register()
        let model = AppModel.shared
        model.applyAppearance()
        model.watchAppearance()
        if let request = model.snapshot {
            Snapshotter.start(request, model: model)
        } else if let script = model.script {
            script.start(model: model)
        } else {
            NSApp.activate()
        }
        Task { await model.bootstrap() }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }

    /// A `.pixellab` opened from Finder (double-click, drag onto the icon): it
    /// becomes the document, after any unsaved work is dealt with.
    func application(_ application: NSApplication, open urls: [URL]) {
        guard let url = urls.first(where: { $0.pathExtension == "pixellab" }) ?? urls.first else { return }
        Task { await AppModel.shared.switchDocument(to: url.path) }
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard !terminating else { return .terminateNow }
        terminating = true
        Task {
            let model = AppModel.shared
            guard await model.confirmUnsaved(window: model.mainWindow) else {
                terminating = false
                NSApp.reply(toApplicationShouldTerminate: false)
                return
            }
            model.chat.stopEverything()
            await model.shutdown()
            NSApp.reply(toApplicationShouldTerminate: true)
        }
        return .terminateLater
    }
}
