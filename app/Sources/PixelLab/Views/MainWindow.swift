import SwiftUI

/// The main window: setup until the server runs, then the document window.
struct MainWindow: View {
    @Environment(AppModel.self) private var model

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
                StageView()
                if let notice = model.notice {
                    NoticeBar(text: notice) { model.notice = nil }
                }
                TimelineStrip()
            }
            .background(Theme.hero)
        }
        .inspector(isPresented: $model.showInspector) {
            InspectorView()
                .inspectorColumnWidth(min: 230, ideal: 264, max: 360)
        }
        .navigationTitle(model.document?.title ?? "Pixel Lab")
        .navigationSubtitle(model.document == nil ? "" : (model.document?.isDirty == true ? "Edited" : "Saved"))
        .toolbar { DocumentToolbar() }
        .toolbarBackground(Theme.hero, for: .windowToolbar)
        .toolbarBackgroundVisibility(.visible, for: .windowToolbar)
        .background(Theme.page)
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
            .help("Edit scenes or objects")
        }
        ToolbarItemGroup(placement: .primaryAction) {
            ControlGroup {
                Button {} label: { Label("Select", systemImage: "cursorarrow") }
                    .help("Select (V) — comes with scene editing")
                Button {} label: { Label("Place", systemImage: "plus.square.on.square") }
                    .help("Place (A) — comes with scene editing")
                Button {} label: { Label("Pan", systemImage: "hand.raised") }
                    .help("Pan (H) — comes with scene editing")
            }
            .disabled(true)

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
            .help("Zoom")
            .accessibilityLabel("Zoom, \(zoomLabel)")

            Button {
                model.showGrid.toggle()
            } label: {
                Label("Grid", systemImage: model.showGrid ? "square.grid.3x3.fill" : "square.grid.3x3")
            }
            .help(model.showGrid ? "Hide the iso grid" : "Show the iso grid")
            .disabled(model.mode != .scene)

            Button {
                Task { await model.save() }
            } label: {
                Text("Save")
            }
            .buttonStyle(AccentButtonStyle())
            .disabled(!model.canSave)
            .help("Save the source (⌘S)")
        }
        ToolbarItem(placement: .primaryAction) {
            Button {
                model.showInspector.toggle()
            } label: {
                Label("Inspector", systemImage: "sidebar.right")
            }
            .help(model.showInspector ? "Hide the inspector" : "Show the inspector")
        }
    }

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
    @Environment(\.isEnabled) private var isEnabled

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 13, weight: .semibold))
            .foregroundStyle(Theme.onAccent)
            .padding(.horizontal, 14)
            .frame(height: 28)
            .background(Theme.accent, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
            .opacity(isEnabled ? (configuration.isPressed ? 0.8 : 1) : 0.45)
            .contentShape(Rectangle())
    }
}
