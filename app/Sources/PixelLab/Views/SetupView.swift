import AppKit
import SwiftUI

/// Shown when the program or the document can't be found, or the server
/// won't start: what's wrong, and a way to fix it without a terminal.
struct SetupView: View {
    @Environment(AppModel.self) private var model
    var problem: SetupProblem
    @AppStorage(SettingsKey.document) private var documentPath = ""

    var body: some View {
        VStack(spacing: 0) {
            Spacer(minLength: 24)
            VStack(alignment: .leading, spacing: 18) {
                HStack(spacing: 14) {
                    if let icon = AppResources.appIcon ?? NSApp.applicationIconImage {
                        Image(nsImage: icon).resizable().interpolation(.high).frame(width: 56, height: 56)
                    }
                    VStack(alignment: .leading, spacing: 3) {
                        Text("Set up Pixel Lab").font(Theme.nameFont(22))
                        Text("Pixel Lab edits pixel art in a .pixellab document.")
                            .font(.callout).foregroundStyle(Theme.inkMuted)
                    }
                }

                row(
                    title: "Document",
                    symbol: "doc.richtext",
                    value: model.documentPath,
                    problem: problem.document,
                    choose: chooseDocument,
                    extra: {
                        if problem.migrate {
                            Button("Migrate…") { model.askMigrate() }
                                .buttonStyle(AccentButtonStyle())
                                .disabled(model.migrating)
                                .accessibilityLabel("Migrate the document to the new format")
                        }
                        Button("New…") { Task { await model.newDocument() } }
                    }
                )
                if model.migrating {
                    HStack(spacing: 8) {
                        ProgressView().controlSize(.small)
                        Text("Migrating…").font(.callout).foregroundStyle(Theme.inkMuted)
                    }
                    .accessibilityElement(children: .combine)
                }
                if let message = model.migrateMessage { problemText(message) }
                if let program = problem.program { problemText(program) }
                if let server = problem.server {
                    VStack(alignment: .leading, spacing: 6) {
                        Label("The pixellab server didn't start", systemImage: "exclamationmark.triangle.fill")
                            .font(.callout.weight(.semibold))
                            .foregroundStyle(Theme.ink)
                        ScrollView {
                            Text(server)
                                .font(Theme.monoFont(11))
                                .foregroundStyle(Theme.inkMuted)
                                .textSelection(.enabled)
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }
                        .frame(maxHeight: 160)
                        .padding(8)
                        .background(Theme.card, in: RoundedRectangle(cornerRadius: 6))
                    }
                }

                HStack {
                    Spacer()
                    Button("Try Again") { Task { await model.bootstrap() } }
                        .buttonStyle(AccentButtonStyle())
                        .keyboardShortcut(.defaultAction)
                }
            }
            .padding(28)
            .frame(maxWidth: 560)
            .alert(
                "Migrate \(model.migrateName) to the new format?",
                isPresented: Binding(get: { model.migrateAsking }, set: { model.migrateAsking = $0 })
            ) {
                Button("Migrate") { Task { await model.migrate() } }
                Button("Cancel", role: .cancel) {}
            } message: {
                Text("Its sources become JSON files. The art doesn't change. You can undo this with git.")
            }
            .background(Theme.sidebar, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(Theme.border))
            Spacer(minLength: 24)
        }
        .foregroundStyle(Theme.ink)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(24)
        .background(Theme.page)
    }

    private func row<Extra: View>(title: String, symbol: String, value: String, problem: String?, choose: @escaping () -> Void, @ViewBuilder extra: () -> Extra) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            SectionLabel(title: title)
            HStack(spacing: 8) {
                Image(systemName: problem == nil ? symbol : "exclamationmark.triangle.fill")
                    .foregroundStyle(problem == nil ? Theme.accent : Theme.gold)
                    .frame(width: 18)
                Text(value)
                    .font(Theme.monoFont(12))
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .frame(maxWidth: .infinity, alignment: .leading)
                extra()
                Button("Choose…", action: choose)
            }
            .padding(8)
            .background(Theme.card, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
            if let problem { problemText(problem) }
        }
    }

    private func problemText(_ text: String) -> some View {
        Text(text).font(.callout).foregroundStyle(Theme.inkMuted).fixedSize(horizontal: false, vertical: true)
    }

    private func chooseDocument() {
        if let path = SetupView.chooseDocumentFolder(start: model.documentPath) {
            documentPath = path
            Task { await model.bootstrap() }
        }
    }

    /// An open panel for a `.pixellab` document (a folder bundle), or a site
    /// checkout holding one.
    @MainActor static func chooseDocumentFolder(start: String) -> String? {
        let panel = NSOpenPanel()
        panel.title = "Open a Pixel Lab document"
        panel.message = "Choose a .pixellab document, or a site checkout that holds one."
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.treatsFilePackagesAsDirectories = false
        panel.directoryURL = URL(fileURLWithPath: start).deletingLastPathComponent()
        return panel.runModal() == .OK ? panel.url?.path : nil
    }

    /// An open panel for a plain folder (the publish folder).
    @MainActor static func chooseFolder(title: String, message: String, start: String) -> String? {
        let panel = NSOpenPanel()
        panel.title = title
        panel.message = message
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.canCreateDirectories = true
        panel.allowsMultipleSelection = false
        panel.directoryURL = URL(fileURLWithPath: start)
        return panel.runModal() == .OK ? panel.url?.path : nil
    }

    /// A save panel for a new `.pixellab` document, or a copy of the open one.
    @MainActor static func chooseNewDocument(title: String, name: String, start: String?) -> String? {
        let panel = NSSavePanel()
        panel.title = title
        panel.message = "Pixel Lab documents are folders named <name>.pixellab."
        panel.nameFieldStringValue = name.hasSuffix(".pixellab") ? name : "\(name).pixellab"
        panel.canCreateDirectories = true
        panel.treatsFilePackagesAsDirectories = false
        if let start { panel.directoryURL = URL(fileURLWithPath: start).deletingLastPathComponent() }
        return panel.runModal() == .OK ? panel.url?.path : nil
    }
}

/// Settings: the document and appearance.
struct SettingsView: View {
    @Environment(AppModel.self) private var model
    @AppStorage(SettingsKey.document) private var documentPath = ""
    @AppStorage(SettingsKey.appearance) private var appearance = AppearanceSetting.auto.rawValue

    var body: some View {
        Form {
            Section {
                LabeledContent("Document") {
                    HStack {
                        TextField("Document", text: $documentPath, prompt: Text("The document in \(Locator.defaultSitePath)"))
                            .labelsHidden()
                            .font(Theme.monoFont(12))
                        Button("Choose…") {
                            if let p = SetupView.chooseDocumentFolder(start: model.documentPath) { documentPath = p }
                        }
                    }
                }
                HStack {
                    Text(model.settingsNeedRestart ? "Restart the server to use the new document." : "Leave the document empty for the one in the default site checkout.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Spacer()
                    Button("Restart Server") {
                        model.settingsNeedRestart = false
                        Task { await model.bootstrap() }
                    }
                }
            }
            Section {
                Picker("Appearance", selection: $appearance) {
                    ForEach(AppearanceSetting.allCases) { Text($0.title).tag($0.rawValue) }
                }
                .pickerStyle(.segmented)
            }
        }
        .formStyle(.grouped)
        .frame(width: 560)
        .fixedSize(horizontal: false, vertical: true)
        .onChange(of: appearance) { model.applyAppearance() }
        .onChange(of: documentPath) { model.settingsNeedRestart = true }
    }
}
