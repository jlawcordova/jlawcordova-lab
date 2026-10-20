import SwiftUI

/// The right column's Agent tab (docs/intents/2026-10-agent-chat/spec.md, D12):
/// Claude Code's setup until it's ready, then the chat or the Chats list.
struct AgentTab: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        let chat = model.chat
        Group {
            if chat.setup == .ready {
                if chat.showingList { ChatListView(chat: chat) } else { ChatView(chat: chat) }
            } else {
                ScrollView { SetupCard(chat: chat).padding(16) }
                    .scrollBounceBehavior(.basedOnSize)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Theme.sidebar)
        .onAppear { chat.start() }
    }
}

// MARK: Setup (D2)

struct SetupCard: View {
    var chat: ChatModel

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack(spacing: 12) {
                ClaudeAvatar(size: 44)
                VStack(alignment: .leading, spacing: 3) {
                    Text("Set up the Agent").font(Theme.nameFont(14)).foregroundStyle(Theme.ink)
                    Text("Claude Code on this Mac, signed in with your Claude plan. Its edits come back as proposals.")
                        .font(.system(size: 12)).foregroundStyle(Theme.inkMuted).fixedSize(horizontal: false, vertical: true)
                }
            }
            VStack(alignment: .leading, spacing: 12) {
                step(0, "Install Claude Code", done: chat.install.map { "claude \($0.version) at \(chat.displayPath($0.url))" }, todo: "The claude command, from Anthropic")
                step(1, "Sign in with your Claude plan", done: "Signed in", todo: "Pro or Max, through your browser")
                step(2, "Chat about your art", done: "", todo: "Select things on the stage, then ask")
            }
            .accessibilityElement(children: .contain)
            .accessibilityLabel("Setup steps")
            stateCard
            if let m = chat.setupMessage {
                Text(m).font(.system(size: 12)).foregroundStyle(Theme.inkMuted).fixedSize(horizontal: false, vertical: true)
                    .accessibilityAddTraits(.updatesFrequently)
            }
        }
    }

    /// Which step the flow is on: 0 install, 1 sign in, 3 done.
    private var at: Int {
        switch chat.setup {
        case .checking, .missing, .installing: chat.install == nil ? 0 : 1
        case .signedOut, .waiting: 1
        case .ready: 3
        }
    }

    private func step(_ k: Int, _ title: String, done: String?, todo: String) -> some View {
        let isDone = k < at, active = k == at
        return HStack(alignment: .top, spacing: 10) {
            ZStack {
                if isDone {
                    Circle().fill(Theme.accent)
                    Image(systemName: "checkmark").font(.system(size: 11, weight: .bold)).foregroundStyle(Theme.onAccent)
                } else {
                    Circle().strokeBorder(active ? Theme.accent : Theme.border, lineWidth: active ? 2 : 1)
                    Text("\(k + 1)").font(.system(size: 12, weight: .semibold)).foregroundStyle(active ? Theme.accent : Theme.inkMuted)
                }
            }
            .frame(width: 24, height: 24)
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.system(size: 13, weight: .semibold)).foregroundStyle(Theme.ink)
                Text(isDone ? (done ?? todo) : todo).font(.system(size: 12)).foregroundStyle(Theme.inkMuted).fixedSize(horizontal: false, vertical: true)
            }
            .padding(.top, 2)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Step \(k + 1), \(title), \(isDone ? "done" : active ? "now" : "next")")
    }

    @ViewBuilder
    private var stateCard: some View {
        switch chat.setup {
        case .checking:
            card {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text("Looking for Claude Code…").font(.system(size: 12)).foregroundStyle(Theme.inkMuted)
                }
            }
        case .missing:
            card {
                Text("Claude Code isn’t installed").font(Theme.nameFont(13)).foregroundStyle(Theme.ink)
                Text("Pixel Lab looked in ~/.local/bin and on your PATH and didn’t find the claude command. It can run Anthropic’s installer for you:")
                    .font(.system(size: 12)).foregroundStyle(Theme.inkMuted).fixedSize(horizontal: false, vertical: true)
                Text(ClaudeLocator.installCommand)
                    .font(Theme.monoFont(11)).foregroundStyle(Theme.ink).textSelection(.enabled)
                    .padding(.horizontal, 10).padding(.vertical, 8)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Theme.sidebar, in: RoundedRectangle(cornerRadius: 6))
                    .accessibilityLabel("Installer command: \(ClaudeLocator.installCommand)")
                HStack(spacing: 8) {
                    Button("Install Claude Code") { chat.runInstaller() }.buttonStyle(AccentButtonStyle(height: 32))
                    Button(chat.copied ? "Copied" : "Copy command for Terminal") { chat.copyInstallCommand() }.controlSize(.large)
                }
                FlowLayout(spacing: 14) {
                    Button("I installed it, check again") { Task { await chat.checkAgain() } }.buttonStyle(AccentLinkStyle())
                    Button("Choose the claude command…") { chat.chooseClaude() }.buttonStyle(AccentLinkStyle())
                }
            }
        case .installing:
            card {
                HStack(spacing: 8) {
                    PulseDot()
                    Text("Installing Claude Code…").font(Theme.nameFont(13)).foregroundStyle(Theme.ink)
                }
                VStack(alignment: .leading, spacing: 3) {
                    ForEach(Array(chat.installLog.suffix(12).enumerated()), id: \.offset) { _, l in
                        Text(l).font(Theme.monoFont(11)).foregroundStyle(Theme.inkMuted).fixedSize(horizontal: false, vertical: true)
                    }
                }
                .frame(maxWidth: .infinity, minHeight: 64, alignment: .topLeading)
                .padding(.horizontal, 10).padding(.vertical, 8)
                .background(Theme.sidebar, in: RoundedRectangle(cornerRadius: 6))
                .accessibilityElement(children: .combine)
                .accessibilityLabel("Installer log: \(chat.installLog.last ?? "starting")")
            }
        case .signedOut:
            card {
                Text("Sign in to Claude").font(Theme.nameFont(13)).foregroundStyle(Theme.ink)
                Text("Your browser opens claude.ai to sign in. Pixel Lab never sees your password, and the Agent’s use counts toward your plan’s limits, the same as Claude Code in a terminal.")
                    .font(.system(size: 12)).foregroundStyle(Theme.inkMuted).fixedSize(horizontal: false, vertical: true)
                HStack(spacing: 8) {
                    Button("Sign in with browser") { chat.signIn() }.buttonStyle(AccentButtonStyle(height: 32))
                    Button("Check again") { Task { await chat.checkAgain() } }.controlSize(.large)
                }
            }
        case .waiting:
            card {
                HStack(spacing: 8) {
                    PulseDot()
                    Text("Finish signing in in your browser").font(Theme.nameFont(13)).foregroundStyle(Theme.ink)
                }
                Text("Waiting for claude.ai. Come back here when it says you’re signed in.")
                    .font(.system(size: 12)).foregroundStyle(Theme.inkMuted).fixedSize(horizontal: false, vertical: true)
                HStack(spacing: 8) {
                    Button("Open the page again") { chat.openLoginPage() }.controlSize(.large).disabled(chat.loginURL == nil)
                    Button("Cancel") { chat.cancelSignIn() }.controlSize(.large)
                }
            }
            .accessibilityAddTraits(.updatesFrequently)
        case .ready:
            EmptyView()
        }
    }

    private func card(@ViewBuilder _ content: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: 10) { content() }
            .padding(14)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Theme.card, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
}

/// An accent dot that pulses (still with Reduce Motion).
struct PulseDot: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    var body: some View {
        TimelineView(.animation(minimumInterval: 0.05, paused: reduceMotion)) { ctx in
            Circle().fill(Theme.accent).frame(width: 8, height: 8)
                .opacity(reduceMotion ? 1 : 0.3 + 0.7 * (0.5 + 0.5 * sin(ctx.date.timeIntervalSinceReferenceDate / 1.1 * 2 * .pi)))
        }
        .accessibilityHidden(true)
    }
}

// MARK: The Chats list (D12)

struct ChatListView: View {
    @Environment(AppModel.self) private var model
    var chat: ChatModel

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Chats").font(Theme.nameFont(16)).foregroundStyle(Theme.ink).accessibilityAddTraits(.isHeader)
                Spacer()
                NewChatButton(chat: chat)
            }
            .padding(.leading, 16).padding(.trailing, 10).padding(.vertical, 8)
            Divider().overlay(Theme.border)
            ScrollView {
                LazyVStack(spacing: 2) {
                    ForEach(chat.documentChats) { c in row(c) }
                }
                .padding(.horizontal, 8).padding(.vertical, 6)
            }
            Divider().overlay(Theme.border)
            VStack(alignment: .leading, spacing: 8) {
                Button("All proposals and activity in the Agent window") { model.agent.showWindow() }
                    .buttonStyle(AccentLinkStyle())
                ClaudeStatusLine(chat: chat)
            }
            .padding(.horizontal, 16).padding(.vertical, 10)
        }
    }

    private func row(_ c: Chat) -> some View {
        let current = c.id == chat.currentID
        let waiting = c.waiting(in: Set(model.agent.proposals.map(\.id)))
        let pins = c.pins.count
        let meta = [chat.docName(of: c), pins > 0 ? "\(pins) pinned" : nil, waiting > 0 ? "\(waiting) waiting" : nil].compactMap { $0 }.joined(separator: " · ")
        return Button { chat.open(c.id) } label: {
            HStack(alignment: .top, spacing: 10) {
                ClaudeAvatar(size: 36)
                VStack(alignment: .leading, spacing: 2) {
                    HStack(alignment: .firstTextBaseline) {
                        Text(c.title).font(.system(size: 13, weight: .semibold)).foregroundStyle(Theme.ink).lineLimit(1)
                        Spacer(minLength: 8)
                        Text(ChatRow.when(c.updatedAt)).font(.system(size: 11)).foregroundStyle(Theme.inkMuted)
                    }
                    Text(chat.preview(c)).font(.system(size: 12)).foregroundStyle(Theme.inkMuted).lineLimit(1)
                    Text(meta).font(Theme.monoFont(10.5)).foregroundStyle(Theme.inkMuted).lineLimit(1)
                }
            }
            .padding(10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(current ? Theme.selection : Color.clear, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel("\(c.title), \(ChatRow.when(c.updatedAt)), \(chat.preview(c))\(meta.isEmpty ? "" : ", \(meta)")")
        .accessibilityAddTraits(current ? .isSelected : [])
    }
}

/// "Claude Code 2.1.290 · signed in with your plan" and Sign out.
struct ClaudeStatusLine: View {
    var chat: ChatModel
    var body: some View {
        HStack(spacing: 8) {
            Circle().fill(chat.setup == .ready ? Theme.accent : Theme.gold).frame(width: 7, height: 7).accessibilityHidden(true)
            Text(chat.statusLine).font(.system(size: 12)).foregroundStyle(Theme.inkMuted).lineLimit(2)
            Spacer(minLength: 6)
            if chat.setup == .ready {
                Button("Sign out") { Task { await chat.signOut() } }.controlSize(.small)
            }
        }
        .accessibilityElement(children: .contain)
    }
}

struct NewChatButton: View {
    var chat: ChatModel
    var body: some View {
        Button { chat.newChat() } label: {
            Image(systemName: "square.and.pencil").font(.system(size: 15)).foregroundStyle(Theme.accent).frame(width: 32, height: 32).contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help("New chat")
        .accessibilityLabel("New chat")
    }
}

// MARK: A chat (D12)

struct ChatView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    var chat: ChatModel

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider().overlay(Theme.border)
            conversation
            Divider().overlay(Theme.border)
            VStack(alignment: .leading, spacing: 8) {
                ChatContextArea(chat: chat)
                ChatInputBar(chat: chat)
            }
            .padding(.horizontal, 12).padding(.top, 10).padding(.bottom, 14)
        }
    }

    private var header: some View {
        HStack(spacing: 6) {
            Button { chat.showList() } label: {
                HStack(spacing: 2) {
                    Image(systemName: "chevron.left").font(.system(size: 13, weight: .semibold))
                    Text("Chats").font(.system(size: 13, weight: .semibold))
                    Text("\(chat.documentChats.filter { !$0.messages.isEmpty }.count)")
                        .font(.system(size: 11, weight: .semibold)).foregroundStyle(Theme.ink)
                        .padding(.horizontal, 5).frame(minWidth: 18).background(Theme.selection, in: Capsule()).padding(.leading, 4)
                }
                .foregroundStyle(Theme.accent).frame(minHeight: 32).contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("All chats")
            VStack(spacing: 1) {
                Text(chat.current?.title ?? Chat.untitled).font(Theme.nameFont(13)).foregroundStyle(Theme.ink).lineLimit(1).truncationMode(.tail)
                Text("Claude · \(model.document?.ref.name ?? model.documentName ?? "")").font(.system(size: 11)).foregroundStyle(Theme.inkMuted).lineLimit(1)
            }
            .frame(maxWidth: .infinity)
            .accessibilityElement(children: .combine)
            Toggle("Details", isOn: Binding(get: { chat.details }, set: { chat.details = $0 }))
                .toggleStyle(.button).controlSize(.small)
                .help("Show the Pixel Lab command behind each step")
            NewChatButton(chat: chat)
        }
        .padding(.horizontal, 10).padding(.vertical, 8)
    }

    private var conversation: some View {
        let messages = chat.current?.messages ?? []
        let rows = ChatRow.rows(messages, typing: chat.running?.chatID == chat.currentID && chat.isRunning)
        return ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 2) {
                    let header = ChatRow.dateHeader(chat.current?.createdAt ?? Date())
                    (Text(header.day).fontWeight(.semibold) + Text(" \(header.time)"))
                        .font(.system(size: 11)).foregroundStyle(Theme.inkMuted)
                        .frame(maxWidth: .infinity).padding(.bottom, 8)
                    if messages.isEmpty { greeting }
                    ForEach(rows) { row in
                        rowView(row).padding(.top, row.first && row.side != .note ? 10 : 0).id(row.id)
                    }
                    Color.clear.frame(height: 1).id("bottom")
                }
                .padding(.horizontal, 12).padding(.top, 12).padding(.bottom, 16)
            }
            .accessibilityLabel("Conversation")
            .onChange(of: scrollKey(messages)) {
                if reduceMotion { proxy.scrollTo("bottom", anchor: .bottom) } else { withAnimation(.easeOut(duration: 0.2)) { proxy.scrollTo("bottom", anchor: .bottom) } }
            }
            .onAppear { proxy.scrollTo("bottom", anchor: .bottom) }
        }
    }

    private func scrollKey(_ messages: [ChatMessage]) -> String {
        "\(chat.currentID ?? "")|\(messages.count)|\(messages.last?.text?.count ?? 0)|\(chat.isRunning)"
    }

    private var greeting: some View {
        HStack(alignment: .bottom, spacing: 8) {
            ClaudeAvatar()
            Text("Hi! Select anything on the stage or in Layers, and it comes with your message, so “these” means what you picked. Pin a chip to keep it while you select something else.")
                .font(.system(size: 13)).foregroundStyle(Theme.ink).fixedSize(horizontal: false, vertical: true)
                .padding(.horizontal, 13).padding(.vertical, 8)
                .background(Theme.card, in: BubbleShape(mine: false))
            Spacer(minLength: 40)
        }
        .padding(.top, 6)
    }

    @ViewBuilder
    private func rowView(_ row: ChatRow) -> some View {
        switch row.kind {
        case .user(let m):
            HStack {
                Spacer(minLength: 48)
                UserBubble(message: m)
            }
        case .note(let m):
            NoteRow(message: m)
        default:
            HStack(alignment: .bottom, spacing: 8) {
                Group {
                    if row.last { ClaudeAvatar() } else { Color.clear }
                }
                .frame(width: 28, height: 28)
                VStack(alignment: .leading, spacing: 3) {
                    if row.first {
                        Text("Claude").font(.system(size: 11)).foregroundStyle(Theme.inkMuted).padding(.leading, 12).accessibilityHidden(true)
                    }
                    claudeRow(row)
                }
                Spacer(minLength: 40)
            }
        }
    }

    @ViewBuilder
    private func claudeRow(_ row: ChatRow) -> some View {
        switch row.kind {
        case .claude(let m): ClaudeBubble(message: m, chat: chat)
        case .steps(let steps): StepsRow(rowID: row.id, steps: steps, chat: chat)
        case .question(let m): QuestionCard(message: m, chat: chat)
        case .proposal(let m): ProposalCard(message: m, chat: chat)
        case .typing: TypingBubble()
        default: EmptyView()
        }
    }
}
