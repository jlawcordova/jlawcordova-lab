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

/// Board 03's document window: unified toolbar, sidebar, stage with the
/// timeline strip under it, inspector.
struct DocumentWindow: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        @Bindable var model = model
        NavigationSplitView {
            SidebarView()
                .navigationSplitViewColumnWidth(min: 220, ideal: 256, max: 340)
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
        .inspector(isPresented: $model.showInspector) {
            // Inspector | ✦ Agent (docs/intents/2026-10-agent-chat/spec.md, D12).
            RightColumn()
                .inspectorColumnWidth(min: 260, ideal: 340, max: 520)
        }
        // The layer menu, over the whole window (docs/intents/2026-10-scene-layers/spec.md, D9).
        .overlay { LayerMenuLayer() }
        .navigationTitle(model.windowTitle)
        .navigationSubtitle(model.document == nil ? "" : (model.document?.isDirty == true ? "Edited" : "Saved"))
        .toolbar { DocumentToolbar() }
        .toolbarBackground(Theme.hero, for: .windowToolbar)
        .toolbarBackgroundVisibility(.visible, for: .windowToolbar)
        .background(Theme.page)
        .background(WindowAccessor { window in
            model.mainWindow = window
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
                Button("Merge") { Task { await model.mergeFromDisk() } }
                Button("Revert") { Task { await model.revert() } }
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
                Text("\(alert.doc.file) changed on disk since you opened it. Merge brings that change into your copy and keeps your edits; Revert loads the file as it is now and drops your changes; Save Anyway overwrites it.")
            }
        }
        .alert(
            "Both you and the file changed the same lines",
            isPresented: Binding(get: { model.mergeAlert != nil }, set: { if !$0 { model.mergeAlert = nil } }),
            presenting: model.mergeAlert
        ) { _ in
            Button("Reload", role: .destructive) { Task { await model.reloadFromDisk() } }
            Button("Keep Mine") { model.keepMine() }
            Button("Cancel", role: .cancel) {}
        } message: { _ in
            Text("Nothing was merged. Reload loads the file and drops your unsaved edits (Undo brings them back); Keep Mine keeps your copy, and saving it later asks before it overwrites the file.")
        }
        .sheet(isPresented: Binding(get: { model.build != nil }, set: { if !$0 { model.build = nil } })) {
            PublishSheet().environment(model)
        }
        .sheet(item: Binding(get: { model.newDoc }, set: { model.newDoc = $0 })) { draft in
            NewDocSheet(draft: draft).environment(model)
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

struct DocumentToolbar: ToolbarContent {
    @Environment(AppModel.self) private var model

    var body: some ToolbarContent {
        ToolbarItem(placement: .principal) {
            Picker("Mode", selection: Binding(get: { model.mode }, set: { model.switchMode(to: $0) })) {
                Text("Scene").tag(DocRef.Kind.scene)
                Text("Object").tag(DocRef.Kind.object)
            }
            .pickerStyle(.segmented)
            .frame(width: 150)
            .help("Edit scenes or objects (Object with an item selected opens its object)")
        }
        ToolbarItemGroup(placement: .primaryAction) {
            if model.mode == .object {
                Picker("Tool", selection: Binding(get: { model.objectEditor.tool }, set: { model.objectEditor.setTool($0); model.editor.focusStage() })) {
                    ForEach(PaintTool.allCases) { tool in
                        Label("\(tool.title) (\(String(tool.key).uppercased()))", systemImage: tool.symbol)
                            .labelStyle(.iconOnly)
                            .tag(tool)
                            .accessibilityLabel("\(tool.title) tool")
                    }
                }
                .pickerStyle(.segmented)
                .help("Pencil (B) · Eraser (E) · Fill (G) · Picker (I); Option-click with the pencil picks; Shift-click draws a line; hold Space to pan")
            } else {
                Picker("Tool", selection: Binding(get: { model.editor.tool }, set: { model.editor.setTool($0); model.editor.focusStage() })) {
                    ForEach(StageTool.allCases) { tool in
                        Label("\(tool.title) (\(String(tool.key).uppercased()))", systemImage: tool.symbol)
                            .labelStyle(.iconOnly)
                            .tag(tool)
                            .accessibilityLabel("\(tool.title) tool")
                    }
                }
                .pickerStyle(.segmented)
                .help("Select (V) · Place (A) · Pan (H); hold Space to pan")
            }

            Menu {
                Button("Fit") { model.zoom = .fit }
                Divider()
                ForEach(Zoom.levels(for: model.mode), id: \.self) { n in
                    Button("\(n)×") { model.zoom = .level(n) }
                }
            } label: {
                Text(zoomLabel).monospacedDigit().frame(minWidth: 52)
            }
            .menuIndicator(.visible)
            .fixedSize()
            .help(model.mode == .object ? "Zoom: Fit (⌘0) or 4×, 8×, 12×, 16× (⌘1–⌘4)" : "Zoom: Fit (⌘0) or 1×–4× (⌘1–⌘4)")
            .accessibilityLabel("Zoom, \(zoomLabel)")

            Button {
                if model.mode == .object { model.objectEditor.pixelGrid.toggle() } else { model.showGrid.toggle() }
            } label: {
                Label("Grid", systemImage: gridOn ? "square.grid.3x3.fill" : "square.grid.3x3")
            }
            .help(model.mode == .object
                  ? (gridOn ? "Hide the pixel grid (⌘')" : "Show the pixel grid, from 8× (⌘')")
                  : (gridOn ? "Hide the iso grid (⌘')" : "Show the iso grid (⌘')"))
            .accessibilityLabel(model.mode == .object ? "Pixel grid" : "Grid")

            Button {
                Task { await model.publish() }
            } label: {
                Label("Publish", systemImage: "square.and.arrow.up.on.square")
            }
            .help("Publish: compile every scene and write the SVGs to the document's publish folder (⌘B)")
            .accessibilityLabel("Publish")
            .disabled(model.build?.running == true)

            Button {
                Task { await model.save() }
            } label: {
                Text("Save")
            }
            .buttonStyle(AccentButtonStyle())
            .disabled(!model.canSave)
            .help("Save the source (⌘S)")
            .accessibilityLabel("Save")
        }
        ToolbarItem(placement: .primaryAction) {
            AgentToolbarButton(agent: model.agent)
        }
        ToolbarItem(placement: .primaryAction) {
            Button {
                model.showInspector.toggle()
            } label: {
                Label("Inspector", systemImage: "sidebar.right")
            }
            .help(model.showInspector ? "Hide the inspector (⌥⌘I)" : "Show the inspector (⌥⌘I)")
            .accessibilityLabel(model.showInspector ? "Hide Inspector" : "Show Inspector")
        }
    }

    private var gridOn: Bool { model.mode == .object ? model.objectEditor.pixelGrid : model.showGrid }

    private var zoomLabel: String {
        switch model.zoom {
        case .fit: "Fit \(model.effectiveZoom)×"
        case .level(let n): "\(n)×"
        }
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
/// rather than turning gray, so the toolbar keeps board 03's single accent.
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

/// The toolbar's way into the Agent tab (docs/intents/2026-10-agent-chat/spec.md, D13), with
/// the count of proposals waiting (a small accent badge; nothing when there are none).
struct AgentToolbarButton: View {
    var agent: AgentModel
    @Environment(AppModel.self) private var model

    var body: some View {
        let n = agent.pendingCount
        Button {
            model.showAgentTab()
        } label: {
            HStack(spacing: 4) {
                Image(systemName: "sparkles")
                if n > 0 {
                    Text("\(n)")
                        .font(.system(size: 10, weight: .bold).monospacedDigit())
                        .foregroundStyle(Theme.onAccent)
                        .padding(.horizontal, 5)
                        .frame(minWidth: 16, minHeight: 16)
                        .background(Theme.accent, in: Capsule())
                }
            }
        }
        .help(n == 0 ? "Agent: chat with Claude about the scene (⌥⌘A)" : "Agent: \(n) proposal\(n == 1 ? "" : "s") to review (⌥⌘A)")
        .accessibilityLabel(n == 0 ? "Agent" : "Agent, \(n) proposal\(n == 1 ? "" : "s") to review")
    }
}
