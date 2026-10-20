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
        .windowToolbarStyle(.unified)
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
            Button("Select None") { model.layers.select([]) }
                .disabled(!(model.mode == .scene && !(model.document?.selection.isEmpty ?? true)))
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
            Button(model.showInspector ? "Hide Inspector" : "Show Inspector") { model.showInspector.toggle() }
                .keyboardShortcut("i", modifiers: [.command, .option])
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
