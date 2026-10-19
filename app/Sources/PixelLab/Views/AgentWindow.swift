import AppKit
import SwiftUI

/// The Agent window (Window › Agent, View › Show Agent, ⌥⌘A): how to connect
/// an agent over MCP, the PROPOSALS it saved (the edits, a diff, Approve and
/// Reject, the ghost on the main window's stage) and the live ACTIVITY feed
/// of its tool calls. Board 03, night and day.
///
/// Keys: ↑ / ↓ move through the proposals, ⌘↩ approves, ⌫ rejects (after a
/// confirmation), Esc hides the ghost.
struct AgentWindow: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        let agent = model.agent
        Group {
            if model.phase == .ready {
                VStack(spacing: 0) {
                    AgentHeader(agent: agent)
                    Rectangle().fill(Theme.border).frame(height: 1)
                    VSplitView {
                        ProposalsSection(agent: agent)
                            .frame(minHeight: 300, idealHeight: 460, maxHeight: .infinity)
                        ActivitySection(agent: agent)
                            .frame(minHeight: 140, idealHeight: 220, maxHeight: .infinity)
                    }
                }
                .background(keys(agent))
            } else {
                ContentUnavailableView {
                    Label("Agent", systemImage: "sparkles")
                } description: {
                    Text("Pixel Lab is starting. Proposals and activity show here once the pixellab server runs.")
                }
            }
        }
        .foregroundStyle(Theme.ink)
        .frame(minWidth: 700, maxWidth: .infinity, minHeight: 560, maxHeight: .infinity)
        .background(Theme.page)
        .tint(Theme.accent)
        .background(WindowAccessor { window in
            model.agent.window = window
            if window.frameAutosaveName.isEmpty { window.setFrameAutosaveName("PixelLabAgent") }
        })
    }

    /// ↑ / ↓ and Esc as window key equivalents (the window has no text fields).
    private func keys(_ agent: AgentModel) -> some View {
        ZStack {
            Button("Previous Proposal") { agent.move(by: -1) }.keyboardShortcut(.upArrow, modifiers: [])
            Button("Next Proposal") { agent.move(by: 1) }.keyboardShortcut(.downArrow, modifiers: [])
            Button("Hide Ghost") { agent.setGhostShown(false) }.keyboardShortcut(.escape, modifiers: [])
        }
        .opacity(0)
        .frame(width: 0, height: 0)
        .accessibilityHidden(true)
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
                if let working = agent.working {
                    ProgressView().controlSize(.small)
                    Text(working).font(.callout).foregroundStyle(Theme.inkMuted)
                } else if let result = agent.lastResult {
                    Text(result).font(.callout).foregroundStyle(Theme.inkMuted).lineLimit(1).truncationMode(.middle)
                }
            }
            Grid(alignment: .leading, horizontalSpacing: 12, verticalSpacing: 8) {
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
                    CodeChip(text: AgentModel.abbreviate(agent.stateDir))
                    Button {
                        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: agent.stateDir)])
                    } label: {
                        Label("Reveal", systemImage: "folder")
                    }
                    .help("Show the state dir (proposals/ and activity.jsonl) in the Finder")
                    .accessibilityLabel("Reveal the state folder in the Finder")
                }
            }
            Text("Agents edit in their own session. Their saves land here as proposals; the site changes only when you approve.")
                .font(.callout)
                .foregroundStyle(Theme.inkMuted)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 14)
        .background(Theme.hero)
    }
}

/// A line of code on the card: SF Mono, selectable.
struct CodeChip: View {
    var text: String

    var body: some View {
        Text(text)
            .font(Theme.monoFont(11))
            .foregroundStyle(Theme.ink)
            .textSelection(.enabled)
            .lineLimit(2)
            .truncationMode(.middle)
            .padding(.horizontal, 8)
            .padding(.vertical, 5)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Theme.card, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 6, style: .continuous).strokeBorder(Theme.border, lineWidth: 1))
    }
}

// MARK: Proposals

struct ProposalsSection: View {
    var agent: AgentModel

    var body: some View {
        HStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 8) {
                HStack(alignment: .firstTextBaseline) {
                    SectionLabel(title: "Proposals")
                    Spacer()
                    Text("\(agent.proposals.count)").font(Theme.monoFont(11)).foregroundStyle(Theme.inkMuted)
                }
                .padding(.horizontal, 14)
                .padding(.top, 12)
                if agent.proposals.isEmpty {
                    Text("Nothing to review. When an agent saves over MCP, its edits wait here for you.")
                        .font(.callout)
                        .foregroundStyle(Theme.inkMuted)
                        .padding(.horizontal, 14)
                        .fixedSize(horizontal: false, vertical: true)
                    Spacer()
                } else {
                    ScrollViewReader { proxy in
                        ScrollView {
                            VStack(spacing: 2) {
                                ForEach(agent.proposals) { p in
                                    ProposalRow(proposal: p, selected: agent.selectedID == p.id) { agent.select(p.id) }
                                        .id(p.id)
                                }
                            }
                            .padding(.horizontal, 8)
                            .padding(.bottom, 10)
                        }
                        .onChange(of: agent.selectedID) { _, id in
                            if let id { withAnimation(.easeOut(duration: 0.12)) { proxy.scrollTo(id) } }
                        }
                    }
                }
            }
            .frame(width: 260)
            .frame(maxHeight: .infinity, alignment: .top)
            .background(Theme.sidebar)
            Rectangle().fill(Theme.border).frame(width: 1)
            ProposalDetailView(agent: agent)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
    }
}

struct ProposalRow: View {
    var proposal: ProposalSummary
    var selected: Bool
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(alignment: .top, spacing: 9) {
                Image(systemName: proposal.ref?.kind == .object ? "cube" : "square.stack.3d.up")
                    .foregroundStyle(selected ? Theme.accent : Theme.inkMuted)
                    .frame(width: 18)
                    .padding(.top, 1)
                VStack(alignment: .leading, spacing: 2) {
                    Text(proposal.ref?.name ?? proposal.doc).font(Theme.nameFont(13)).foregroundStyle(Theme.ink).lineLimit(1)
                    Text("\(proposal.agentName) · \(proposal.editsLabel)")
                        .font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(1)
                }
                Spacer(minLength: 4)
                VStack(alignment: .trailing, spacing: 3) {
                    Text(AgentTime.short(proposal.createdAt)).font(Theme.monoFont(10)).foregroundStyle(Theme.inkMuted)
                    if !proposal.baseMatches {
                        Image(systemName: "exclamationmark.triangle.fill").font(.system(size: 10)).foregroundStyle(Theme.gold)
                            .help("The file changed since this proposal was made")
                    }
                }
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 7)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(selected ? Theme.selection : Color.clear, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel("\(proposal.ref?.kind.rawValue ?? "doc") \(proposal.ref?.name ?? proposal.doc), from \(proposal.agentName), \(proposal.editsLabel), \(AgentTime.short(proposal.createdAt))\(proposal.baseMatches ? "" : ", the file changed since")")
        .accessibilityAddTraits(selected ? [.isSelected] : [])
    }
}

struct ProposalDetailView: View {
    var agent: AgentModel
    @Environment(AppModel.self) private var model

    var body: some View {
        if let p = agent.selected {
            VStack(alignment: .leading, spacing: 12) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Image(systemName: p.ref?.kind == .object ? "cube" : "square.stack.3d.up").foregroundStyle(Theme.accent)
                    Text(p.ref?.name ?? p.doc).font(Theme.nameFont(16)).lineLimit(1)
                    Text(p.ref?.kind.rawValue ?? "").font(.callout).foregroundStyle(Theme.inkMuted)
                    Spacer()
                    Text("\(p.agentName) · \(AgentTime.short(p.createdAt))").font(.callout).foregroundStyle(Theme.inkMuted).lineLimit(1)
                }
                baseStatus(p)
                VStack(alignment: .leading, spacing: 5) {
                    SectionLabel(title: "Edits")
                    ScrollView {
                        VStack(alignment: .leading, spacing: 3) {
                            ForEach(Array(p.summary.enumerated()), id: \.offset) { _, line in
                                HStack(alignment: .firstTextBaseline, spacing: 6) {
                                    Text("•").foregroundStyle(Theme.inkMuted)
                                    Text(line).font(.callout).fixedSize(horizontal: false, vertical: true)
                                }
                            }
                            if p.summary.isEmpty { Text("No edits listed.").font(.callout).foregroundStyle(Theme.inkMuted) }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .scrollBounceBehavior(.basedOnSize)
                    .frame(maxHeight: 96)
                    .fixedSize(horizontal: false, vertical: true)
                }
                VStack(alignment: .leading, spacing: 5) {
                    HStack(alignment: .firstTextBaseline) {
                        SectionLabel(title: "Diff")
                        if let d = agent.detail { Text(d.file).font(Theme.monoFont(11)).foregroundStyle(Theme.inkMuted) }
                        Spacer()
                        if let d = agent.detail { diffStats(d.diff) }
                    }
                    if let d = agent.detail, d.id == p.id {
                        DiffView(lines: DiffLine.parse(d.diff))
                    } else if let e = agent.detailError {
                        Text(e).font(Theme.monoFont(11)).foregroundStyle(Theme.inkMuted)
                        Spacer()
                    } else {
                        ProgressView().controlSize(.small).frame(maxWidth: .infinity, maxHeight: .infinity)
                    }
                }
                .frame(maxHeight: .infinity, alignment: .top)
                footer(p)
            }
            .padding(16)
        } else {
            ContentUnavailableView {
                Label(agent.proposals.isEmpty ? "No proposals" : "Choose a proposal", systemImage: "tray")
            } description: {
                Text(agent.proposals.isEmpty
                     ? "Edits an agent saves over MCP wait here for you to approve or reject."
                     : "Select one to see its edits and diff, and preview it on the stage.")
            }
            .foregroundStyle(Theme.ink)
        }
    }

    private func baseStatus(_ p: ProposalSummary) -> some View {
        HStack(spacing: 7) {
            Image(systemName: p.baseMatches ? "checkmark.seal" : "exclamationmark.triangle.fill")
                .foregroundStyle(p.baseMatches ? Theme.accent : Theme.gold)
            Text(p.baseMatches ? "Base matches: made from \(p.ref?.file ?? "the file") as it is now" : "\(p.ref?.file ?? "The file") changed since the proposal was made")
                .font(.callout)
            Spacer()
            Text(stageStatus(p)).font(.callout).foregroundStyle(Theme.inkMuted).lineLimit(1)
        }
        .accessibilityElement(children: .combine)
    }

    private func stageStatus(_ p: ProposalSummary) -> String {
        if !agent.previewing { return "Open it in the editor to preview it on the stage" }
        if !agent.ghostShown { return "Ghost hidden" }
        if let e = agent.ghostError { return "Couldn't render: \(e)" }
        return agent.visibleGhost != nil ? "Ghosted on the stage" : "Rendering the ghost…"
    }

    private func diffStats(_ diff: String) -> some View {
        let lines = DiffLine.parse(diff)
        let added = lines.filter { $0.kind == .added }.count, removed = lines.filter { $0.kind == .removed }.count
        return Text("+\(added) −\(removed)").font(Theme.monoFont(11)).foregroundStyle(Theme.inkMuted)
            .accessibilityLabel("\(added) lines added, \(removed) removed")
    }

    private func footer(_ p: ProposalSummary) -> some View {
        HStack(spacing: 8) {
            Button {
                Task { await agent.openInEditor() }
            } label: {
                Label("Open in Editor", systemImage: "arrow.up.forward.app")
            }
            .help("Open \(p.ref?.name ?? "the doc") in the main window with the proposal ghosted on the stage")
            Button {
                agent.setGhostShown(!agent.ghostShown)
            } label: {
                Label(agent.ghostShown ? "Hide Ghost" : "Show Ghost", systemImage: agent.ghostShown ? "eye.slash" : "eye")
            }
            .disabled(!agent.previewing)
            .help(agent.ghostShown ? "Hide the proposal on the stage (Esc)" : "Show the proposal on the stage")
            Spacer()
            Button(role: .destructive) {
                Task { await agent.reject() }
            } label: {
                Text("Reject")
            }
            .keyboardShortcut(.delete, modifiers: [])
            .disabled(agent.working != nil)
            .help("Reject: delete the proposal and leave the site as it is (⌫)")
            Button {
                Task { await agent.approve() }
            } label: {
                Text("Approve")
            }
            .buttonStyle(AccentButtonStyle())
            .keyboardShortcut(.return, modifiers: .command)
            .disabled(agent.working != nil)
            .help("Approve: write the proposal to \(p.ref?.file ?? "the file") (⌘↩)")
        }
    }
}

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
        ScrollView([.vertical, .horizontal]) {
            VStack(alignment: .leading, spacing: 0) {
                if lines.isEmpty {
                    Text("No change from the file as it is.").foregroundStyle(Code.hunk).padding(.horizontal, 12)
                }
                ForEach(shown) { line in
                    Text(line.text.isEmpty ? " " : line.text)
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
                Picker("Show", selection: Binding(get: { agent.errorsOnly }, set: { agent.errorsOnly = $0 })) {
                    Text("All").tag(false)
                    Text("Errors").tag(true)
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .frame(width: 140)
                .help("Show every tool call, or only the ones that failed")
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
                Text(agent.errorsOnly ? "No failed tool calls." : "No tool calls yet. Every MCP tool call an agent makes shows here as it happens.")
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
                    .defaultScrollAnchor(.bottom)
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
            Text(e.tool).font(Theme.monoFont(11)).foregroundStyle(Theme.ink).lineLimit(1).frame(width: 150, alignment: .leading)
            Text(e.summary ?? "").font(.system(size: 12)).foregroundStyle(e.ok ? Theme.inkMuted : Theme.ink).lineLimit(1).truncationMode(.tail)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 4)
        .help(e.summary ?? "")
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("\(row.clock), \(e.tool), \(e.ok ? "ok" : "failed"): \(e.summary ?? "")")
    }
}
