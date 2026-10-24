import SwiftUI

/// The main window: setup until the server runs, then the document window.
struct MainWindow: View {
    @Environment(AppModel.self) private var model
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        Group {
            switch model.phase {
            case .ready:
                DocumentWindow()
            case .setup(let problem):
                SetupView(problem: problem)
            case .starting(let message):
                VStack(spacing: 12) {
                    ProgressView().controlSize(.small)
                    Text(message).font(.callout).foregroundStyle(Theme.inkMuted)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(Theme.page)
            }
        }
        .tint(Theme.accent)
        .onAppear {
            let open = openWindow
            model.agent.openWindowAction = { open(id: WindowID.agent) }
        }
    }
}

/// Board 03's document window: a slim title bar with no toolbar, the sidebar, the stage with its
/// floating bar and the timeline strip under it, and the inspector column.
struct DocumentWindow: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        @Bindable var model = model
        NavigationSplitView {
            SidebarView()
                .navigationSplitViewColumnWidth(min: 220, ideal: 256, max: 340)
                .toolbar(removing: .sidebarToggle)
        } detail: {
            VStack(spacing: 0) {
                if model.showsStaleBanner, let file = model.document?.ref.file {
                    StaleBanner(file: file)
                }
                StageView()
                if let notice = model.notice {
                    NoticeBar(text: notice) { model.notice = nil }
                }
                if model.document?.ref.kind == .object {
                    ObjectStatusBar()
                } else {
                    TimelineStrip()
                }
            }
            .background(Theme.hero)
        }
        // Always shown: no button, menu item or key hides it (docs/intents/2026-10-floating-library/spec.md, R13).
        .inspector(isPresented: .constant(true)) {
            // Inspector | ✦ Agent … Publish · Save (docs/intents/2026-10-agent-chat/spec.md, D12; floating-library R14).
            RightColumn()
                .inspectorColumnWidth(min: 260, ideal: 340, max: 520)
        }
        // The layer menu, over the whole window (docs/intents/2026-10-scene-layers/spec.md, D9).
        .overlay { LayerMenuLayer() }
        .navigationTitle(model.windowTitle)
        // Saved, Edited (a save is coming) or why it can't be saved (docs/intents/2026-10-autosave/spec.md, R3).
        .navigationSubtitle(model.document == nil ? "" : model.saveStateText)
        // No toolbar: the title and its Edited / Saved subtitle in a slim title bar (R12).
        .toolbar(removing: .sidebarToggle)
        .toolbarBackground(Theme.hero, for: .windowToolbar)
        .toolbarBackgroundVisibility(.visible, for: .windowToolbar)
        .background(Theme.page)
        .background(WindowAccessor { window in
            model.mainWindow = window
            model.watchKey(of: window)
            WindowCloseGuard.install(on: window)
            window.isDocumentEdited = !model.dirtyDocs.isEmpty
        })
        .onChange(of: model.dirtyDocs) { _, dirty in
            model.mainWindow?.isDocumentEdited = !dirty.isEmpty
        }
        .alert(
            saveAlertTitle,
            isPresented: Binding(get: { model.saveAlert != nil }, set: { if !$0 { model.saveAlert = nil } }),
            presenting: model.saveAlert
        ) { alert in
            switch alert.kind {
            case .invalid:
                Button("OK", role: .cancel) {}
            case .conflict:
                Button("Reload") { Task { await model.reloadFromDisk() } }
                Button("Save Anyway", role: .destructive) { Task { await model.save(alert.doc, force: true) } }
                Button("Cancel", role: .cancel) {}
            }
        } message: { alert in
            switch alert.kind {
            case .invalid:
                let shown = alert.problems.prefix(8).map { "• " + Self.trimFile($0) }.joined(separator: "\n")
                let more = alert.problems.count > 8 ? "\n…and \(alert.problems.count - 8) more (see Checks)." : ""
                Text("Fix these first; nothing was written.\n\n" + (shown.isEmpty ? alert.message : shown) + more)
            case .conflict:
                Text("\(alert.doc.file) changed on disk, on the same lines as your edits. Reload loads the file as it is now and drops your edits; Save Anyway overwrites it.")
            }
        }
        .alert(
            "Both you and the file changed the same lines",
            isPresented: Binding(get: { model.mergeAlert != nil }, set: { if !$0 { model.mergeAlert = nil } }),
            presenting: model.mergeAlert
        ) { _ in
            Button("Reload", role: .destructive) { Task { await model.reloadFromDisk() } }
            Button("Keep Mine") { Task { await model.keepMine() } }
            Button("Cancel", role: .cancel) {}
        } message: { _ in
            Text("Nothing was merged. Reload loads the file and drops your unsaved edits (Undo brings them back); Keep Mine saves your copy over the file.")
        }
        .sheet(isPresented: Binding(get: { model.build != nil }, set: { if !$0 { model.build = nil } })) {
            PublishSheet().environment(model)
        }
        .sheet(item: Binding(get: { model.newDoc }, set: { model.newDoc = $0 })) { draft in
            NewDocSheet(draft: draft).environment(model)
        }
        // File › Import Library… and Import Palette…, when they have something to ask (docs/intents/2026-10-object-libraries/spec.md, D5, D6).
        .sheet(item: Binding(get: { model.imports.library }, set: { if $0 == nil { model.imports.cancelLibrary() } })) { _ in
            LibraryImportSheet().environment(model)
        }
        .sheet(item: Binding(get: { model.imports.palette }, set: { if $0 == nil { model.imports.cancelPalette() } })) { _ in
            PaletteImportSheet().environment(model)
        }
        // A .pixellib or .pixelpal dropped on the window starts its import (R12, R21).
        .onDrop(of: [.fileURL], isTargeted: nil) { providers in
            var took = false
            for provider in providers where provider.hasItemConformingToTypeIdentifier("public.file-url") {
                took = true
                provider.loadItem(forTypeIdentifier: "public.file-url", options: nil) { item, _ in
                    guard let data = item as? Data, let url = URL(dataRepresentation: data, relativeTo: nil) else { return }
                    guard ImportModel.isLibrary(url.path) || ImportModel.isPalette(url.path) else { return }
                    Task { @MainActor in await AppModel.shared.imports.drop([url.path]) }
                }
            }
            return took
        }
    }

    private var saveAlertTitle: String {
        guard let a = model.saveAlert else { return "" }
        switch a.kind {
        case .invalid: return "Can't save \(a.doc.name): \(a.problems.count) problem\(a.problems.count == 1 ? "" : "s")"
        case .conflict: return "\(a.doc.name) changed on disk"
        }
    }

    /// "scenes/x.json: items[3]: rule" → "items[3]: rule".
    static func trimFile(_ line: String) -> String {
        guard let r = line.range(of: ".json: ") else { return line }
        return String(line[r.upperBound...])
    }
}

struct NoticeBar: View {
    var text: String
    var dismiss: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "info.circle").foregroundStyle(Theme.gold)
            Text(text).font(.callout).foregroundStyle(Theme.ink).lineLimit(2)
            Spacer()
            Button(action: dismiss) { Image(systemName: "xmark") }
                .buttonStyle(.borderless)
                .accessibilityLabel("Dismiss")
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
        .background(Theme.card)
    }
}

/// The one accent button (Save): accent fill, dark ink, dimmed when disabled
/// rather than turning gray, so the window keeps board 03's single accent.
struct AccentButtonStyle: ButtonStyle {
    var height: CGFloat = 28
    @Environment(\.isEnabled) private var isEnabled

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: height < 28 ? 12 : 13, weight: .semibold))
            .foregroundStyle(Theme.onAccent)
            .padding(.horizontal, height < 28 ? 10 : 14)
            .frame(height: height)
            .background(Theme.accent, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
            .opacity(isEnabled ? (configuration.isPressed ? 0.8 : 1) : 0.45)
            .contentShape(Rectangle())
    }
}
