import AppKit
import SwiftUI

/// Shown when node, the CLI or the site can't be found, or the server won't
/// start: what's wrong, and a way to fix it without a terminal.
struct SetupView: View {
    @Environment(AppModel.self) private var model
    var problem: SetupProblem
    @AppStorage(SettingsKey.site) private var sitePath = ""
    @AppStorage(SettingsKey.node) private var nodePath = ""

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
                        Text("Pixel Lab edits the pixel art in your site checkout with Node.js.")
                            .font(.callout).foregroundStyle(Theme.inkMuted)
                    }
                }

                row(
                    title: "Site checkout",
                    symbol: "folder",
                    value: model.sitePath,
                    problem: problem.site,
                    choose: chooseSite
                )
                row(
                    title: "Node.js",
                    symbol: "terminal",
                    value: nodePath.isEmpty ? "Found automatically" : nodePath,
                    problem: problem.node,
                    choose: chooseNode
                )
                if let cli = problem.cli { problemText(cli) }
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
                    if !nodePath.isEmpty {
                        Button("Find Node Automatically") { nodePath = "" }
                    }
                    Spacer()
                    Button("Try Again") { Task { await model.bootstrap() } }
                        .buttonStyle(AccentButtonStyle())
                        .keyboardShortcut(.defaultAction)
                }
            }
            .padding(28)
            .frame(maxWidth: 560)
            .background(Theme.sidebar, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(Theme.border))
            Spacer(minLength: 24)
        }
        .foregroundStyle(Theme.ink)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(24)
        .background(Theme.page)
    }

    private func row(title: String, symbol: String, value: String, problem: String?, choose: @escaping () -> Void) -> some View {
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

    private func chooseSite() {
        if let path = SetupView.chooseFolder(start: sitePath.isEmpty ? Locator.defaultSitePath : sitePath) {
            sitePath = path
            Task { await model.bootstrap() }
        }
    }

    private func chooseNode() {
        if let path = SetupView.chooseNodeBinary() {
            nodePath = path
            Task { await model.bootstrap() }
        }
    }

    @MainActor static func chooseFolder(start: String) -> String? {
        let panel = NSOpenPanel()
        panel.title = "Choose the site checkout"
        panel.message = "Choose your jlawcordova.github.io checkout."
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.directoryURL = URL(fileURLWithPath: start).deletingLastPathComponent()
        return panel.runModal() == .OK ? panel.url?.path : nil
    }

    @MainActor static func chooseNodeBinary() -> String? {
        let panel = NSOpenPanel()
        panel.title = "Choose node"
        panel.message = "Choose the node executable (Node.js 22 or later)."
        panel.canChooseDirectories = false
        panel.canChooseFiles = true
        panel.allowsMultipleSelection = false
        panel.treatsFilePackagesAsDirectories = true
        panel.showsHiddenFiles = true
        panel.directoryURL = URL(fileURLWithPath: "/opt/homebrew/bin")
        return panel.runModal() == .OK ? panel.url?.path : nil
    }
}

/// Settings: the site checkout, node, and appearance.
struct SettingsView: View {
    @Environment(AppModel.self) private var model
    @AppStorage(SettingsKey.site) private var sitePath = ""
    @AppStorage(SettingsKey.node) private var nodePath = ""
    @AppStorage(SettingsKey.appearance) private var appearance = AppearanceSetting.auto.rawValue

    var body: some View {
        Form {
            Section {
                LabeledContent("Site checkout") {
                    HStack {
                        TextField("Site checkout", text: $sitePath, prompt: Text(Locator.defaultSitePath))
                            .labelsHidden()
                            .font(Theme.monoFont(12))
                        Button("Choose…") {
                            if let p = SetupView.chooseFolder(start: sitePath.isEmpty ? Locator.defaultSitePath : sitePath) { sitePath = p }
                        }
                    }
                }
                LabeledContent("Node.js") {
                    HStack {
                        TextField("Node.js", text: $nodePath, prompt: Text("Found automatically"))
                            .labelsHidden()
                            .font(Theme.monoFont(12))
                        Button("Choose…") {
                            if let p = SetupView.chooseNodeBinary() { nodePath = p }
                        }
                    }
                }
                HStack {
                    Text(model.settingsNeedRestart ? "Restart the server to use the new paths." : "Leave Node.js empty to find it in your shell's PATH.")
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
        .onChange(of: sitePath) { model.settingsNeedRestart = true }
        .onChange(of: nodePath) { model.settingsNeedRestart = true }
    }
}
