import AppKit
import SwiftUI

/// The Agent window (Window › Agent Window, ⇧⌥⌘A): Claude Code's status (the Agent tab's chat), how to connect
/// an agent over MCP, and the live ACTIVITY feed of its tool calls. Agents' edits apply as they're made, so
/// there's nothing to approve here (docs/intents/2026-10-autosave/spec.md, R19): File › Revert To › Browse All
/// Versions… shows what they changed. Board 03, night and day.
struct AgentWindow: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        let agent = model.agent
        Group {
            if model.phase == .ready {
                VStack(spacing: 0) {
                    AgentHeader(agent: agent)
                    if let retired = agent.retired {
                        Rectangle().fill(Theme.border).frame(height: 1)
                        RetiredNoticeRow(notice: retired, agent: agent)
                    }
                    Rectangle().fill(Theme.border).frame(height: 1)
                    ActivitySection(agent: agent)
                        .frame(maxHeight: .infinity)
                }
            } else {
                ContentUnavailableView {
                    Label("Agent", systemImage: "sparkles")
                } description: {
                    Text("Pixel Lab is starting. Agents' activity shows here once the pixellab server runs.")
                }
            }
        }
        .foregroundStyle(Theme.ink)
        .frame(minWidth: 700, maxWidth: .infinity, minHeight: 420, maxHeight: .infinity)
        .background(Theme.page)
        .tint(Theme.accent)
        .background(WindowAccessor { window in
            // SwiftUI saves the Window scene's frame itself (autosave name "agent").
            model.agent.window = window
        })
    }
}

/// The one-time notice (R21): proposals that were waiting when edits started to apply were set aside.
struct RetiredNoticeRow: View {
    var notice: AgentModel.RetiredNotice
    var agent: AgentModel

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "tray.full").foregroundStyle(Theme.gold).accessibilityHidden(true)
            Text("\(notice.count) waiting proposal\(notice.count == 1 ? " was" : "s were") set aside when edits started to apply. Their files are in \(AgentModel.abbreviate(notice.dir)).")
                .font(.callout)
                .foregroundStyle(Theme.ink)
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 8)
            Button("Show in Finder") { agent.showRetired() }
            Button("Dismiss") { agent.dismissRetired() }
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 10)
        .background(Theme.card)
        .accessibilityElement(children: .contain)
    }
}

// MARK: Header

/// How to connect: the `claude mcp add` command for this app's CLI, and the state dir.
struct AgentHeader: View {
    var agent: AgentModel

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                Image(systemName: "sparkles").foregroundStyle(Theme.accent).font(.system(size: 15, weight: .semibold))
                Text("Agent").font(Theme.nameFont(17))
                Spacer()
                if let result = agent.lastResult {
                    Text(result).font(.callout).foregroundStyle(Theme.inkMuted).lineLimit(1).truncationMode(.middle)
                }
            }
            Grid(alignment: .leading, horizontalSpacing: 12, verticalSpacing: 8) {
                ClaudeCodeRow()
                GridRow {
                    SectionLabel(title: "Connect").gridColumnAlignment(.leading)
                    CodeChip(text: agent.mcpCommand)
                    Button {
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.setString(agent.mcpCommand, forType: .string)
                        agent.lastResult = "Copied the claude mcp add command"
                    } label: {
                        Label("Copy", systemImage: "doc.on.doc")
                    }
                    .help("Copy the command that adds Pixel Lab's MCP server to Claude Code")
                    .accessibilityLabel("Copy the MCP setup command")
                }
                GridRow {
                    SectionLabel(title: "State")
                    CodeChip(text: AgentModel.abbreviate(agent.stateDir), full: agent.stateDir)
                    Button {
                        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: agent.stateDir)])
                    } label: {
                        Label("Reveal", systemImage: "folder")
                    }
                    .help("Show the state dir (the version history and activity.jsonl) in the Finder")
                    .accessibilityLabel("Reveal the state folder in the Finder")
                }
            }
            Text("Chats in the main window and agents you connect from a terminal edit the document as they go, and you see each edit on the stage. File › Revert To › Browse All Versions… shows what they changed, and takes it back.")
                .font(.callout)
                .foregroundStyle(Theme.inkMuted)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 14)
        .background(Theme.hero)
    }
}

/// CLAUDE CODE (docs/intents/2026-10-agent-chat/spec.md, R20): its version and sign-in, how many
/// chats, Open chats and Sign out (or Set up).
struct ClaudeCodeRow: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        let chat = model.chat
        let n = chat.documentChats.filter { !$0.messages.isEmpty }.count
        GridRow {
            SectionLabel(title: "Claude Code")
            HStack(spacing: 8) {
                Circle().fill(chat.setup == .ready ? Theme.accent : Theme.gold).frame(width: 7, height: 7).accessibilityHidden(true)
                Text(chat.setup == .ready ? "\(chat.install?.version ?? "") · signed in with your plan · \(n) chat\(n == 1 ? "" : "s")" : chat.statusLine)
                    .font(.system(size: 12)).foregroundStyle(Theme.ink).lineLimit(1)
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 8).padding(.vertical, 5)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Theme.card, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 6, style: .continuous).strokeBorder(Theme.border, lineWidth: 1))
            .accessibilityElement(children: .combine)
            HStack(spacing: 6) {
                Button(chat.setup == .ready ? "Open chats" : "Set up") {
                    model.showAgentTab()
                    if chat.setup == .ready { chat.showList() }
                }
                .help("The Agent tab in the main window (⌥⌘A)")
                if chat.setup == .ready {
                    Button("Sign out") { Task { await chat.signOut() } }
                }
            }
        }
        .onAppear { chat.start() }
    }
}

/// A line of code on the card: SF Mono, one line truncated in the middle
/// (long paths keep their start and end), the whole text in the tooltip, the
/// accessibility value and Copy (context menu).
struct CodeChip: View {
    var text: String
    /// What the tooltip shows and Copy copies, when the chip shows a shorter form.
    var full: String?

    var body: some View {
        let whole = full ?? text
        Text(text)
            .font(Theme.monoFont(11))
            .foregroundStyle(Theme.ink)
            .lineLimit(1)
            .truncationMode(.middle)
            .help(whole)
            .contextMenu {
                Button("Copy") {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(whole, forType: .string)
                }
            }
            .accessibilityLabel(whole)
            .padding(.horizontal, 8)
            .padding(.vertical, 5)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Theme.card, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 6, style: .continuous).strokeBorder(Theme.border, lineWidth: 1))
    }
}

// MARK: Diff

/// A unified diff in SF Mono on the site's code ground, with the site's
/// syntax colors (src/styles/syntax.css): inserted and deleted line tints,
/// gold file headers, comment-brown hunk headers.
struct DiffView: View {
    var lines: [DiffLine]
    static let maxLines = 4000

    enum Code {
        /// --color-code and --color-on-code: the same ink ground in both themes.
        static let ground = Theme.hexColor("#2E2418")
        static let text = Theme.hexColor("#F4EDE0")
        static let file = Theme.hexColor("#D8B66A")
        static let hunk = Theme.hexColor("#A8957A")
        static let added = Theme.hexColor("#B5C79C")
        static let removed = Theme.hexColor("#E6B8A8")
        static let addedBack = Color(.sRGB, red: 159 / 255, green: 208 / 255, blue: 138 / 255, opacity: 0.2)
        static let removedBack = Color(.sRGB, red: 201 / 255, green: 147 / 255, blue: 156 / 255, opacity: 0.2)
    }

    var body: some View {
        let shown = lines.prefix(Self.maxLines)
        // Scrolled so the first change shows, a few context lines above it.
        let first = lines.first { $0.kind == .added || $0.kind == .removed }.map { max($0.id - 2, 0) }
        ScrollViewReader { proxy in
        ScrollView([.vertical, .horizontal]) {
            VStack(alignment: .leading, spacing: 0) {
                if lines.isEmpty {
                    Text("No change from the file as it is.").foregroundStyle(Code.hunk).padding(.horizontal, 12)
                }
                ForEach(shown) { line in
                    Text(line.text.isEmpty ? " " : line.text)
                        .id(line.id)
                        .foregroundStyle(color(line.kind))
                        .fixedSize()
                        .padding(.horizontal, 12)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(background(line.kind))
                }
                if lines.count > Self.maxLines {
                    Text("… \(lines.count - Self.maxLines) more lines").foregroundStyle(Code.hunk).padding(.horizontal, 12)
                }
            }
            .font(Theme.monoFont(11))
            .padding(.vertical, 8)
            .textSelection(.enabled)
        }
        .defaultScrollAnchor(.topLeading)
        .task(id: lines) {
            // After the first layout, or there's nothing to scroll yet.
            try? await Task.sleep(for: .milliseconds(60))
            if let first { proxy.scrollTo(first, anchor: .topLeading) }
        }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Code.ground, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
        .environment(\.colorScheme, .dark)
        .accessibilityLabel("Diff, \(lines.filter { $0.kind == .added }.count) lines added, \(lines.filter { $0.kind == .removed }.count) removed")
    }

    private func color(_ kind: DiffLine.Kind) -> Color {
        switch kind {
        case .file: Code.file
        case .hunk: Code.hunk
        case .added: Code.added
        case .removed: Code.removed
        case .context: Code.text
        }
    }

    private func background(_ kind: DiffLine.Kind) -> Color {
        switch kind {
        case .added: Code.addedBack
        case .removed: Code.removedBack
        default: .clear
        }
    }
}

// MARK: Activity

struct ActivitySection: View {
    var agent: AgentModel

    var body: some View {
        let rows = agent.visibleActivity
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 10) {
                SectionLabel(title: "Activity")
                Text("\(rows.count)").font(Theme.monoFont(11)).foregroundStyle(Theme.inkMuted)
                Spacer()
                Picker("Show", selection: Binding(get: { agent.filter }, set: { agent.filter = $0 })) {
                    Text("All").tag(AgentModel.ActivityFilter.all)
                    Text("Chats").tag(AgentModel.ActivityFilter.chats)
                    Text("Errors").tag(AgentModel.ActivityFilter.errors)
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .frame(width: 200)
                .help("Show every tool call, the Agent chat's, or only the ones that failed")
                Button {
                    agent.clearActivity()
                } label: {
                    Label("Clear", systemImage: "xmark.circle")
                }
                .disabled(rows.isEmpty)
                .help("Clear the feed (the log file is kept)")
            }
            .padding(.horizontal, 14)
            .padding(.top, 10)
            if rows.isEmpty {
                Text(agent.filter == .errors ? "No failed tool calls." : agent.filter == .chats ? "No tool calls from chats yet." : "No tool calls yet. Every MCP tool call an agent makes shows here as it happens.")
                    .font(.callout)
                    .foregroundStyle(Theme.inkMuted)
                    .padding(.horizontal, 14)
                Spacer()
            } else {
                ScrollViewReader { proxy in
                    ScrollView {
                        LazyVStack(spacing: 0) {
                            ForEach(rows) { row in ActivityRowView(row: row).id(row.id) }
                        }
                        .padding(.horizontal, 8)
                        .padding(.bottom, 8)
                    }
                    .onAppear { if let id = rows.last?.id { proxy.scrollTo(id, anchor: .bottom) } }
                    .onChange(of: agent.activity.last?.id) { _, id in
                        if let id { proxy.scrollTo(id, anchor: .bottom) }
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Theme.page)
    }
}

struct ActivityRowView: View {
    var row: AgentModel.ActivityRow

    var body: some View {
        let e = row.entry
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Text(row.clock).font(Theme.monoFont(11)).foregroundStyle(Theme.inkMuted)
            Image(systemName: e.ok ? "checkmark.circle" : "exclamationmark.triangle.fill")
                .font(.system(size: 11))
                .foregroundStyle(e.ok ? Theme.inkMuted : Theme.gold)
                .frame(width: 14)
            Text(e.tool).font(Theme.monoFont(11)).foregroundStyle(Theme.ink).lineLimit(1).frame(width: 130, alignment: .leading)
            Text(e.who).font(.system(size: 12)).foregroundStyle(Theme.inkMuted).lineLimit(1).truncationMode(.tail).frame(width: 150, alignment: .leading)
            Text(e.summary ?? "").font(.system(size: 12)).foregroundStyle(e.ok ? Theme.inkMuted : Theme.ink).lineLimit(1).truncationMode(.tail)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 4)
        .help(e.summary ?? "")
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("\(row.clock), \(e.tool)\(e.who.isEmpty ? "" : " by \(e.who)"), \(e.ok ? "ok" : "failed"): \(e.summary ?? "")")
    }
}
