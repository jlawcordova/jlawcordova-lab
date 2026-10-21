import AppKit
import Observation

/// The Agent tab (docs/intents/2026-10-agent-chat/spec.md): Claude Code's
/// setup, the chats, and the turn that's running. A turn is one `claude -p`
/// process (D1) whose stream fills the open chat's transcript (D4).
@MainActor @Observable
final class ChatModel {
    @ObservationIgnored weak var app: AppModel?

    // MARK: Setup (D2)

    enum Setup: String, Sendable {
        case checking, missing, installing, signedOut = "signedout", waiting, ready
    }

    var setup: Setup = .checking
    var install: ClaudeLocator.Install?
    /// The installer's output (the last 200 lines).
    var installLog: [String] = []
    /// A line under the setup card: Check again's answer, a failed install or sign-in.
    var setupMessage: String?
    var copied = false
    /// The page `claude auth login` opened, for Open the page again.
    var loginURL: URL?
    /// Under the script hook, Open the page again counts here instead of opening a browser.
    var openedLoginPage = 0
    @ObservationIgnored let search = ClaudeLocator.Search.fromEnvironment()
    @ObservationIgnored private var loginProcess: LineProcess?
    @ObservationIgnored private var installProcess: LineProcess?
    @ObservationIgnored private var pollTask: Task<Void, Never>?
    @ObservationIgnored private var started = false

    // MARK: Chats (D3)

    /// Every chat on disk, of every document.
    var chats: [Chat] = []
    /// A new chat, kept only in memory until it has a message.
    var draftChat: Chat?
    var currentID: String?
    var showingList = false
    var draft = ""
    /// Details: the raw call under each step.
    var details = false
    /// Folded step rows the owner opened, by row id.
    var expanded: Set<String> = []
    /// The views focus the input when this changes.
    var focusRequest = 0
    /// The input's height: one to five lines.
    var inputHeight: CGFloat = 30
    @ObservationIgnored weak var inputView: NSTextView?

    // MARK: The turn (D1, D4)

    struct Running: Equatable {
        var chatID: String
        var proposals = 0
        var started = false
        var stopped = false
        var authFailed = false
        var finished = false
        var retried = false
        var stderr: [String] = []
        var text: String
        var block: String
        /// Proposal cards wait for Claude's next words, so the card follows the reply about it (as in the prototype).
        var cards: [ChatMessage] = []
    }

    var running: Running?
    /// A send is fetching its block (before its turn starts).
    @ObservationIgnored private var sending = false
    @ObservationIgnored private var turn: LineProcess?

    var isRunning: Bool { running != nil }

    /// The app's node (its server's): on every claude's PATH.
    var node: URL? { app?.connection?.launch.node }

    // MARK: Paths

    var stateDir: String { app?.sessionInfo?.stateDir ?? AgentModel.defaultStateDir.path }
    var store: ChatStore { ChatStore(stateDir: stateDir) }
    /// The open document's root: chats belong to it.
    var documentRoot: String { app?.sessionInfo?.document?.root ?? app?.sessionInfo?.site ?? "" }

    /// The open document's chats, newest first (a new, empty one first of all).
    var documentChats: [Chat] {
        let saved = chats.filter { $0.document == documentRoot }.sorted { $0.updatedAt > $1.updatedAt }
        return (draftChat.map { [$0] } ?? []) + saved
    }

    var current: Chat? {
        guard let id = currentID else { return nil }
        if draftChat?.id == id { return draftChat }
        return chats.first { $0.id == id }
    }

    // MARK: Start

    /// The first time the Agent tab shows: read the chats and check Claude Code.
    func start() {
        guard !started else { return }
        started = true
        loadChats()
        Task { await check() }
    }

    /// A new document: its own chats.
    func documentChanged() {
        guard started else { return }
        loadChats()
    }

    func loadChats() {
        chats = store.load()
        defer { if let agent = app?.agent, agent.listed { proposalsListed(agent.proposals.map(\.id)) } }
        draftChat = nil
        if let first = documentChats.first { currentID = first.id } else { newChat() }
        showingList = false
    }

    // MARK: Setup actions

    /// Finds `claude`, then asks whether it's signed in.
    func check() async {
        let search = self.search
        let node = self.node
        let chosen = search.useSystem ? UserDefaults.standard.string(forKey: SettingsKey.claude) : nil
        if setup != .waiting { setup = .checking }
        let found = await Task.detached { ClaudeLocator.find(chosen: chosen, search: search, node: node) }.value
        install = found
        guard let found else {
            setup = .missing
            return
        }
        let on = await Task.detached { ClaudeLocator.signedIn(found.url, search: search, node: node) }.value
        if on == true {
            setup = .ready
            setupMessage = nil
        } else {
            setup = .signedOut
            if on == nil { setupMessage = "Couldn't tell whether Claude Code is signed in. Sign in, or run claude auth status in Terminal." }
        }
    }

    /// I installed it, check again.
    func checkAgain() async {
        setupMessage = nil
        await check()
        if setup == .missing { setupMessage = "Still not found in ~/.local/bin or on your PATH. If you installed it somewhere else, choose the command." }
    }

    func copyInstallCommand() {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(ClaudeLocator.installCommand, forType: .string)
        copied = true
    }

    /// Install Claude Code: Anthropic's installer (or, under the script hook, the fake), its output in the log.
    func runInstaller() {
        guard installProcess == nil else { return }
        setup = .installing
        installLog = []
        setupMessage = nil
        let command = search.installer.map { "'\($0.replacingOccurrences(of: "'", with: "'\\''"))'" } ?? ClaudeLocator.installCommand
        var env = ProcessInfo.processInfo.environment
        if !search.useSystem { env["HOME"] = search.home }
        do {
            let p = try LineProcess(executable: URL(fileURLWithPath: "/bin/bash"), arguments: ["-c", command], environment: env)
            installProcess = p
            Task {
                var status: Int32 = 0
                for await o in p.output {
                    switch o {
                    case .line(let l), .stderr(let l): appendInstall(l)
                    case .exited(let s): status = s
                    }
                }
                installProcess = nil
                await installFinished(status: status)
            }
        } catch {
            setup = .missing
            setupMessage = "Couldn't run the installer: \(error.localizedDescription)"
        }
    }

    private func appendInstall(_ line: String) {
        let clean = line.replacingOccurrences(of: "\u{1B}\\[[0-9;]*[A-Za-z]", with: "", options: .regularExpression).trimmingCharacters(in: .whitespaces)
        guard !clean.isEmpty else { return }
        installLog.append(clean)
        if installLog.count > 200 { installLog.removeFirst(installLog.count - 200) }
    }

    private func installFinished(status: Int32) async {
        let search = self.search
        let node = self.node
        let found = await Task.detached { ClaudeLocator.find(chosen: nil, search: search, node: node) }.value
        guard status == 0, let found else {
            setup = .missing
            let expected = URL(fileURLWithPath: search.home).appendingPathComponent(".local/bin/claude")
            let why = await Task.detached { ClaudeLocator.diagnose(expected, search: search, node: node) }.value
            setupMessage = status == 0 ? "The installer finished, but claude in ~/.local/bin didn't answer (\(why)). Choose the command, or check again." : "The installer stopped (exit \(status)). Its last lines are above."
            return
        }
        install = found
        appendInstall("claude --version → \(found.version) (Claude Code)")
        try? await Task.sleep(for: .milliseconds(app?.script == nil ? 900 : 50))
        let on = await Task.detached { ClaudeLocator.signedIn(found.url, search: search, node: node) }.value
        setup = on == true ? .ready : .signedOut
    }

    /// Choose the claude command…: an open panel, then `--version`.
    func chooseClaude() {
        let panel = NSOpenPanel()
        panel.title = "Choose the claude command"
        panel.message = "Pick the claude command Claude Code installed (often ~/.local/bin/claude)."
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.showsHiddenFiles = true
        panel.directoryURL = URL(fileURLWithPath: search.home).appendingPathComponent(".local/bin")
        guard panel.runModal() == .OK, let url = panel.url else { return }
        Task { await choose(path: url.path) }
    }

    func choose(path: String) async {
        let search = self.search
        let node = self.node
        guard let v = await Task.detached(operation: { ClaudeLocator.version(of: URL(fileURLWithPath: path), search: search, node: node) }).value else {
            setupMessage = "\(path) didn't answer like Claude Code (claude --version)."
            return
        }
        if app?.script == nil { UserDefaults.standard.set(path, forKey: SettingsKey.claude) }
        install = ClaudeLocator.Install(url: URL(fileURLWithPath: path), version: v)
        let on = await Task.detached { ClaudeLocator.signedIn(URL(fileURLWithPath: path), search: search, node: node) }.value
        setup = on == true ? .ready : .signedOut
        setupMessage = nil
    }

    /// Sign in with browser: `claude auth login --claudeai`, polling `auth status` until it's signed in.
    func signIn() {
        guard let install, loginProcess == nil else { return }
        setup = .waiting
        setupMessage = nil
        loginURL = nil
        let env = ClaudeLocator.environment(base: ProcessInfo.processInfo.environment, claude: install.url, node: node, search: search)
        do {
            let p = try LineProcess(executable: install.url, arguments: ["auth", "login", "--claudeai"], environment: env, closeInput: false)
            loginProcess = p
            var lastLines: [String] = []
            Task {
                for await o in p.output {
                    switch o {
                    case .line(let l), .stderr(let l):
                        if loginURL == nil, let r = l.range(of: #"https://\S+"#, options: .regularExpression) { loginURL = URL(string: String(l[r])) }
                        lastLines = Array((lastLines + [l]).suffix(4))
                    case .exited:
                        break
                    }
                }
                loginProcess = nil
                guard setup == .waiting else { return }
                await pollOnce()
                if setup == .waiting {
                    setup = .signedOut
                    pollTask?.cancel()
                    setupMessage = (lastLines.isEmpty ? "" : lastLines.joined(separator: "\n") + "\n") + "Or run claude auth login in Terminal, then Check again."
                }
            }
        } catch {
            setup = .signedOut
            setupMessage = "Couldn't start the sign-in: \(error.localizedDescription)"
            return
        }
        pollTask?.cancel()
        pollTask = Task {
            let deadline = Date().addingTimeInterval(600)
            while !Task.isCancelled, setup == .waiting, Date() < deadline {
                try? await Task.sleep(for: .seconds(app?.script == nil ? 2 : 0.3))
                await pollOnce()
            }
            if setup == .waiting, !Task.isCancelled {
                cancelSignIn()
                setupMessage = "Sign-in timed out."
            }
        }
    }

    private func pollOnce() async {
        guard setup == .waiting, let url = install?.url else { return }
        let search = self.search
        let node = self.node
        if await Task.detached(operation: { ClaudeLocator.signedIn(url, search: search, node: node) }).value == true {
            loginProcess?.stop()
            loginProcess = nil
            pollTask?.cancel()
            setup = .ready
            note("Signed in · Claude Code is ready")
        }
    }

    func openLoginPage() {
        guard let loginURL else { return }
        if app?.script != nil { openedLoginPage += 1 } else { NSWorkspace.shared.open(loginURL) }
    }

    func cancelSignIn() {
        pollTask?.cancel()
        loginProcess?.stop()
        loginProcess = nil
        setup = .signedOut
    }

    /// Sign out: asks first, since it signs Claude Code out everywhere on this Mac.
    func signOut() async {
        guard let install else { return }
        if app?.script == nil {
            let alert = NSAlert()
            alert.messageText = "Sign out of Claude Code?"
            alert.informativeText = "This signs Claude Code out on this Mac, in Terminal too. Your chats are kept."
            alert.addButton(withTitle: "Sign Out").hasDestructiveAction = true
            alert.addButton(withTitle: "Cancel")
            let window = app?.mainWindow
            let response = window != nil ? await alert.beginSheetModal(for: window!) : alert.runModal()
            guard response == .alertFirstButtonReturn else { return }
        }
        stop()
        let search = self.search
        let node = self.node
        _ = await Task.detached { ClaudeLocator.run(install.url, ["auth", "logout"], search: search, node: node, timeout: 15) }.value
        setup = .signedOut
    }

    /// Quitting: end whatever runs.
    func stopEverything() {
        turn?.stop()
        loginProcess?.stop()
        installProcess?.stop()
        pollTask?.cancel()
    }

    // MARK: Chats

    func newChat() {
        let now = Date()
        let chat = Chat(id: ChatStore.newID(now), title: Chat.untitled, document: documentRoot, createdAt: now, updatedAt: now)
        draftChat = chat
        currentID = chat.id
        showingList = false
        draft = ""
        chatSwitched()
        focusRequest += 1
    }

    func open(_ id: String) {
        currentID = id
        showingList = false
        chatSwitched()
        focusRequest += 1
    }

    func showList() {
        showingList = true
        chatSwitched()
    }

    /// Ghosts show only for the chat you're in (D7): another chat's ghost goes when you switch.
    func chatSwitched() {
        guard let agent = app?.agent, let p = agent.selected, let from = p.chat else { return }
        if showingList || from.id != currentID { agent.setGhostShown(false) }
    }

    /// Changes a chat (the draft or a saved one) and saves it; a draft with a message becomes a saved chat.
    func update(_ id: String, save: Bool = true, _ change: (inout Chat) -> Void) {
        if var d = draftChat, d.id == id {
            change(&d)
            // A new chat is kept once it has a message or a pin.
            if d.messages.isEmpty, d.pins.isEmpty {
                draftChat = d
                return
            }
            draftChat = nil
            chats.insert(d, at: 0)
        } else if let i = chats.firstIndex(where: { $0.id == id }) {
            change(&chats[i])
        } else {
            return
        }
        guard save, let chat = chats.first(where: { $0.id == id }) else { return }
        try? store.save(chat)
    }

    /// Adds a message to a chat (and moves it to the top of the list).
    func append(_ m: ChatMessage, to id: String) {
        update(id) { c in
            if c.title == Chat.untitled, m.kind == .user, let t = m.text { c.title = Chat.title(from: t) }
            c.messages.append(m)
            c.updatedAt = Date()
        }
    }

    /// A note in the open chat (or the given one).
    func note(_ text: String, in id: String? = nil) {
        guard let id = id ?? currentID else { return }
        append(ChatMessage(kind: .note, text: text), to: id)
    }

    // MARK: Sending (D4)

    /// What the chat can't send now, if anything.
    var cantSend: String? {
        if setup != .ready { return "Claude Code isn't ready" }
        if let r = running { return r.chatID == currentID ? "Claude is working…" : "Claude is working in “\(chats.first { $0.id == r.chatID }?.title ?? "another chat")”" }
        if app?.document == nil { return "Open a scene or an object first" }
        return nil
    }

    /// Return, the send button, an answer: sends the text with the context.
    func send(_ override: String? = nil) async {
        let text = (override ?? draft).trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty, cantSend == nil, !sending, let id = currentID else { return }
        // Two quick sends (Return twice, Return and an answer) can't both start a turn while the block is fetched.
        sending = true
        defer { sending = false }
        if override == nil { draft = "" }
        let context = await messageContext()
        var m = ChatMessage(kind: .user, text: text)
        m.chips = context.chips
        append(m, to: id)
        startTurn(chatID: id, text: text, block: context.block)
    }

    /// The selection block and the chips the message shows (D5): fetched fresh, so it's what's selected now.
    func messageContext() async -> (block: String, chips: [String]) {
        await refreshContext(force: true)
        return (context?.text ?? "", chips.map(ChatContext.messageLabel))
    }

    // MARK: Context: chips, pins and the block (D5)

    /// The last `chat.context` for what's selected and pinned, and what it was asked for.
    var context: ChatContextResult?
    @ObservationIgnored private var contextKey: String?
    @ObservationIgnored private var contextLoading = false
    @ObservationIgnored private var contextAgain = false

    /// The open chat's pins on the open scene.
    var pins: [LayerTarget] {
        guard let doc = app?.document, doc.ref.kind == .scene else { return [] }
        return (current?.pins ?? []).filter { $0.doc == doc.ref.id }.compactMap { LayerTarget(spec: $0.target) }
    }

    /// The chips: pins, then the selection.
    var chips: [ChatChip] {
        guard let doc = app?.document, doc.ref.kind == .scene else { return [] }
        return ChatContext.chips(selection: doc.selection, pins: pins, tree: doc.layerTree, items: doc.itemsByPath, unsaved: Set(context?.unsaved ?? []))
    }

    /// What `chat.context` should be asked for now; a change means fetching it again.
    var wantedContextKey: String {
        guard let doc = app?.document else { return "" }
        if doc.ref.kind == .object { return "object:\(doc.ref.name)|\(doc.isDirty)" }
        let targets = pins + doc.selection.filter { !pins.contains($0) }
        return "\(doc.ref.id)|\(targets.map(\.description).joined(separator: ","))|\(pins.map(\.description).joined(separator: ","))|\(doc.isDirty)|\(doc.items.hashValue)"
    }

    /// Fetches the block again when what it's for changed (coalesced, like the stage's render).
    func refreshContext(force: Bool = false) async {
        guard let app, let doc = app.document, let connection = app.connection else { context = nil; return }
        let key = wantedContextKey
        if !force, key == contextKey { return }
        if contextLoading, !force {
            contextAgain = true
            return
        }
        contextLoading = true
        defer { contextLoading = false }
        repeat {
            contextAgain = false
            let key = wantedContextKey
            var params: [String: JSONValue]
            if doc.ref.kind == .object {
                params = ["object": .string(doc.ref.name)]
            } else {
                let pinned = pins
                let targets = pinned + doc.selection.filter { !pinned.contains($0) }
                params = [
                    "name": .string(doc.ref.name),
                    "layers": .array(targets.compactMap(\.layerID).filter { doc.layerTree.byID[$0] != nil }.map { .string($0) }),
                    "items": .array(targets.compactMap(\.itemPath).filter { doc.itemsByPath[$0] != nil }.map { .string($0) }),
                    "pinned": .array(pinned.map { .string($0.description) }),
                ]
            }
            if let r: ChatContextResult = try? await connection.call("chat.context", params), app.document === doc {
                context = r
                contextKey = key
            }
        } while contextAgain && app.document === doc
    }

    /// The chips left out of the block, as "pine 216.0" and "Boats".
    var unsavedNames: [String] {
        chips.filter(\.unsaved).map { $0.target.layerID != nil ? $0.label : "\($0.label) \($0.target.description)" }
    }

    /// The pin button: pins a selected target to the open chat, or unpins it.
    func togglePin(_ t: LayerTarget) {
        guard let id = currentID, let doc = app?.document, doc.ref.kind == .scene else { return }
        let spec = t.description
        update(id) { c in
            if let i = c.pins.firstIndex(where: { $0.doc == doc.ref.id && $0.target == spec }) {
                c.pins.remove(at: i)
            } else {
                c.pins.append(ChatPin(doc: doc.ref.id, target: spec, object: t.itemPath.flatMap { doc.itemsByPath[$0]?.object }))
            }
        }
        app?.editor.announce(pins.contains(t) ? "Pinned to this chat" : "Unpinned")
    }

    /// The remove button: unpins it and takes it out of the selection.
    func removeChip(_ t: LayerTarget) {
        if pins.contains(t) { togglePin(t) }
        guard let app, let doc = app.document, doc.selection.contains(t) else { return }
        app.layers.select(doc.selection.filter { $0 != t }, reveal: false)
    }

    /// Save now (the unsaved warning): saves the open doc, so the block can name what was left out.
    func saveNow() async {
        guard let app else { return }
        if await app.save() { note(app.lastEdit ?? "Saved") }
        await refreshContext(force: true)
    }

    /// An edit put the items in depth order and moved some (old path → new): pins follow their items.
    func followSort(_ reordered: [String: String]) {
        guard let ref = app?.document?.ref.id, !reordered.isEmpty else { return }
        for c in chats where c.pins.contains(where: { $0.doc == ref && reordered[$0.target] != nil }) {
            update(c.id) { chat in
                for i in chat.pins.indices where chat.pins[i].doc == ref {
                    if let to = reordered[chat.pins[i].target] { chat.pins[i].target = to }
                }
            }
        }
    }

    /// The scene's items came back: a pinned item whose path now holds another object, or nothing, is unpinned.
    func itemsChanged() {
        guard let app, let doc = app.document, doc.ref.kind == .scene, doc.itemsLoaded, app.layers.itemsCurrent else { return }
        let ref = doc.ref.id
        for c in chats {
            let gone = c.pins.filter { p in
                guard p.doc == ref else { return false }
                if let t = LayerTarget(spec: p.target), case .layer(let id) = t { return doc.layerTree.byID[id] == nil }
                return doc.itemsByPath[p.target]?.object != p.object
            }
            guard !gone.isEmpty else { continue }
            update(c.id) { chat in chat.pins.removeAll { gone.contains($0) } }
            for p in gone { note("Unpinned \(p.object.map { "\($0) " } ?? "")\(p.target): it moved or was removed", in: c.id) }
        }
    }

    private func startTurn(chatID: String, text: String, block: String, retried: Bool = false) {
        guard let app, let install, let launch = app.connection?.launch else {
            note("Pixel Lab's server isn't running.", in: chatID)
            return
        }
        let title = chats.first { $0.id == chatID }?.title ?? Chat.untitled
        let config = ClaudeTurn.mcpConfig(node: launch.node, cli: launch.cli, location: launch.location, stateDir: stateDir, chatID: chatID, title: title)
        let session = chats.first { $0.id == chatID }?.session
        let args = ClaudeTurn.arguments(mcpConfig: config, prompt: ChatPrompt.text, resume: session)
        let env = ClaudeLocator.environment(base: ProcessInfo.processInfo.environment, claude: install.url, node: launch.node, search: search)
        let cwd = store.dir
        try? FileManager.default.createDirectory(at: cwd, withIntermediateDirectories: true)
        do {
            let p = try LineProcess(executable: install.url, arguments: args, environment: env, directory: cwd, input: ClaudeTurn.input(block: block, text: text))
            turn = p
            running = Running(chatID: chatID, retried: retried, text: text, block: block)
            Task {
                for await o in p.output { handle(o) }
            }
        } catch {
            running = nil
            note("Couldn't start Claude Code: \(error.localizedDescription)", in: chatID)
        }
    }

    /// Stop: ends the turn's process.
    func stop() {
        guard running != nil else { return }
        running?.stopped = true
        turn?.stop()
    }

    private func handle(_ o: LineProcess.Output) {
        guard var r = running else { return }
        switch o {
        case .line(let line):
            guard let events = StreamParser.parse(line: line) else {
                r.stderr = Array((r.stderr + [line]).suffix(20))
                running = r
                return
            }
            for e in events { apply(e) }
        case .stderr(let line):
            r.stderr = Array((r.stderr + [line]).suffix(20))
            running = r
        case .exited(let status):
            finish(status: status)
        }
    }

    private func apply(_ e: StreamEvent) {
        guard var r = running else { return }
        let id = r.chatID
        switch e {
        case .started(let session, let status):
            r.started = true
            running = r
            update(id) { $0.session = session }
            if let status, status != "connected" {
                note("Pixel Lab's tools didn't start: \(status)", in: id)
                stop()
            }
        case .text(let t):
            defer { flushCards() }
            update(id) { c in
                if let last = c.messages.last, last.kind == .claude, last.time.timeIntervalSinceNow > -600, c.messages.last?.id == lastClaudeID {
                    c.messages[c.messages.count - 1].text = (last.text ?? "") + "\n\n" + t
                } else {
                    let m = ChatMessage(kind: .claude, text: t)
                    lastClaudeID = m.id
                    c.messages.append(m)
                }
                c.updatedAt = Date()
            }
        case .toolUse(let toolID, let name, let input):
            lastClaudeID = nil
            let tool = StreamParser.toolName(name)
            if tool == "chat_ask" {
                var m = ChatMessage(kind: .question, text: input["question"]?.stringValue ?? "")
                m.toolID = toolID
                m.options = input["options"]?.arrayValue?.compactMap(\.stringValue) ?? []
                append(m, to: id)
                return
            }
            var m = ChatMessage(kind: .step)
            m.toolID = toolID
            m.tool = tool
            m.input = input
            m.done = false
            m.ok = true
            m.human = StepText.human(tool: tool, input: input, result: nil, ok: true, objectAt: objectAt)
            m.raw = StepText.raw(tool: tool, input: input, result: nil)
            append(m, to: id)
        case .toolResult(let toolID, let ok, let text):
            var proposal: ChatMessage?
            update(id) { c in
                guard let i = c.messages.lastIndex(where: { $0.toolID == toolID }) else { return }
                if c.messages[i].kind == .question {
                    if !ok { c.messages[i].chosen = "" ; c.messages[i].text = "Couldn't ask: \(text.split(separator: "\n").first.map(String.init) ?? "")" }
                    return
                }
                var m = c.messages[i]
                let tool = m.tool ?? ""
                m.ok = ok
                m.done = true
                m.result = String(text.prefix(400))
                m.human = StepText.human(tool: tool, input: m.input ?? [:], result: text, ok: ok, objectAt: objectAt)
                m.raw = StepText.raw(tool: tool, input: m.input ?? [:], result: text)
                c.messages[i] = m
                if ok, tool == "doc_save", let p = Self.proposal(from: text) { proposal = p }
            }
            if let p = proposal {
                r.cards.append(p)
                r.proposals += 1
                running = r
            }
        case .authFailed(let text):
            r.authFailed = true
            running = r
            setup = .signedOut
            note("Claude Code isn't signed in (\(text.isEmpty ? "signed out" : text)). Sign in to go on.", in: id)
        case .finished(let session, let isError, let result):
            r.finished = true
            running = r
            if let session { update(id) { $0.session = session } }
            if isError, !r.authFailed, !r.stopped { note(result.map { "Claude stopped: \($0)" } ?? "Claude stopped with an error.", in: id) }
        }
    }

    @ObservationIgnored private var lastClaudeID: String?

    /// The turn's proposal cards that wait, after the words that came.
    private func flushCards() {
        guard var r = running, !r.cards.isEmpty else { return }
        let cards = r.cards
        r.cards = []
        running = r
        lastClaudeID = nil
        for p in cards {
            append(p, to: r.chatID)
            proposalMade(p, chatID: r.chatID)
        }
    }

    /// A `doc_save` result that made a proposal, as a card.
    nonisolated static func proposal(from text: String) -> ChatMessage? {
        guard let data = text.data(using: .utf8), let r = try? JSONDecoder().decode(JSONValue.self, from: data), let id = r["proposal"]?.stringValue else { return nil }
        let edits = r["edits"]?.arrayValue?.compactMap(\.stringValue) ?? []
        let doc = r["doc"]?.stringValue
        let name = doc.flatMap { DocRef(id: $0)?.name } ?? "the document"
        // The one edit, else how many: "pine moved to column 4, row 0…", "2 edits to harbor".
        let title = edits.count == 1 ? edits[0].prefix(1).uppercased() + edits[0].dropFirst() : "\(edits.count) edits to \(name)"
        var m = ChatMessage(kind: .proposal, text: title)
        m.proposal = id
        m.number = r["number"]?.doubleValue.map { Int($0) }
        m.doc = doc
        m.edits = edits.count == 1 ? [] : edits
        return m
    }

    /// A turn made a proposal: in the open chat, it's selected with its ghost on (D7).
    func proposalMade(_ m: ChatMessage, chatID: String) {
        guard let id = m.proposal, let agent = app?.agent else { return }
        madeThisLaunch.insert(id)
        Task {
            await agent.refreshList()
            guard chatID == currentID, !showingList, agent.proposals.contains(where: { $0.id == id }) else { return }
            agent.select(id)
            agent.setGhostShown(true)
        }
    }

    /// The proposals listed now: a card whose proposal was listed and is gone was settled elsewhere.
    @ObservationIgnored private var seenProposals = Set<String>()

    func proposalsListed(_ ids: [String]) {
        let now = Set(ids)
        // Gone: listed earlier in this launch and not now, or a card from an earlier launch (or a turn's
        // card not made in this launch) whose proposal isn't listed: settled while Pixel Lab was closed.
        // A card made in this launch waits until its proposal has been listed once (the list can lag the save).
        let waitingCards = Set(chats.flatMap(\.messages).filter { $0.kind == .proposal && $0.outcome == nil }.compactMap(\.proposal))
        let gone = seenProposals.subtracting(now).union(waitingCards.subtracting(madeThisLaunch).subtracting(now))
        seenProposals.formUnion(now)
        guard !gone.isEmpty else { return }
        for c in chats where c.messages.contains(where: { $0.kind == .proposal && $0.outcome == nil && gone.contains($0.proposal ?? "") }) {
            update(c.id) { chat in
                for i in chat.messages.indices where chat.messages[i].kind == .proposal && chat.messages[i].outcome == nil && gone.contains(chat.messages[i].proposal ?? "") {
                    chat.messages[i].outcome = "elsewhere"
                }
            }
        }
        seenProposals.subtract(gone)
    }

    /// Proposals whose cards this launch's turns made.
    @ObservationIgnored private var madeThisLaunch = Set<String>()

    /// Approved or rejected here (a card, the banner or the Agent window): the card says so, with a note.
    func proposalSettled(_ id: String, applied: Bool, detail: String) {
        seenProposals.remove(id)
        for c in chats {
            guard let m = c.messages.first(where: { $0.kind == .proposal && $0.proposal == id }) else { continue }
            update(c.id) { chat in
                if let i = chat.messages.firstIndex(where: { $0.id == m.id }) { chat.messages[i].outcome = applied ? "applied" : "rejected" }
            }
            note("You \(applied ? "approved" : "rejected") \(m.number.map { "#\($0)" } ?? "the proposal") · \(detail)", in: c.id)
        }
    }

    // MARK: Card actions (R16)

    /// Whether a card's proposal is the one ghosted on the stage.
    func ghostOn(_ m: ChatMessage) -> Bool { app?.agent.visibleGhost?.proposalID == m.proposal && m.proposal != nil }

    /// Selects a card's proposal in the Agent model (as the Agent window would).
    private func selectProposal(_ m: ChatMessage) async -> Bool {
        guard let id = m.proposal, let agent = app?.agent else { return false }
        if !agent.proposals.contains(where: { $0.id == id }) { await agent.refreshList() }
        guard agent.proposals.contains(where: { $0.id == id }) else {
            // It isn't waiting any more: say so on the card, rather than a button that does nothing.
            for c in chats where c.messages.contains(where: { $0.id == m.id }) {
                update(c.id) { chat in
                    if let i = chat.messages.firstIndex(where: { $0.id == m.id }), chat.messages[i].outcome == nil { chat.messages[i].outcome = "elsewhere" }
                }
                note("\(m.number.map { "#\($0)" } ?? "That proposal") isn't waiting any more: it was approved or rejected outside Pixel Lab.", in: c.id)
            }
            return false
        }
        agent.select(id)
        return true
    }

    func approve(_ m: ChatMessage) async {
        guard await selectProposal(m) else { return }
        await app?.agent.approve()
    }

    func reject(_ m: ChatMessage) async {
        guard await selectProposal(m) else { return }
        await app?.agent.reject()
    }

    /// Show on stage / Hide on stage.
    func toggleGhost(_ m: ChatMessage) async {
        guard let agent = app?.agent else { return }
        if ghostOn(m) {
            agent.setGhostShown(false)
            return
        }
        guard await selectProposal(m) else { return }
        if let ref = agent.selected?.ref, app?.document?.ref != ref { await agent.openInEditor() } else { agent.setGhostShown(true) }
    }

    /// Diff in the Agent window.
    func showDiff(_ m: ChatMessage) async {
        _ = await selectProposal(m)
        app?.agent.showWindow()
    }

    /// The Agent window's Open the chat: the chat a proposal came from, in the Agent tab.
    func openChat(forProposal p: ProposalSummary) {
        guard let id = p.chat?.id, chats.contains(where: { $0.id == id }) else { return }
        app?.showAgentTab()
        open(id)
        app?.mainWindow?.makeKeyAndOrderFront(nil)
    }

    /// What the owner had selected when they asked for a proposal: the chips of the message before its card.
    func askedAbout(_ proposalID: String) -> String? {
        for c in chats {
            guard let i = c.messages.firstIndex(where: { $0.kind == .proposal && $0.proposal == proposalID }) else { continue }
            let chips = c.messages[..<i].last(where: { $0.kind == .user })?.chips ?? []
            guard !chips.isEmpty else { return nil }
            return chips.count <= 2 ? chips.joined(separator: " and ") : chips.prefix(2).joined(separator: ", ") + " and \(chips.count - 2) more"
        }
        return nil
    }

    private func finish(status: Int32) {
        flushCards()
        guard let r = running else { return }
        turn = nil
        running = nil
        lastClaudeID = nil
        let id = r.chatID
        // Steps the process never answered.
        update(id) { c in
            for i in c.messages.indices where c.messages[i].kind == .step && c.messages[i].done == false { c.messages[i].done = true }
        }
        if r.stopped {
            note(r.proposals == 0 ? "Stopped · nothing was proposed" : "Stopped", in: id)
            return
        }
        if ClaudeTurn.resumeFailed(stderr: r.stderr.joined(separator: "\n"), started: r.started), !r.retried {
            update(id) { $0.session = nil }
            note("Claude started a new session; it doesn't remember the earlier messages.", in: id)
            startTurn(chatID: id, text: r.text, block: r.block, retried: true)
            return
        }
        if !r.finished, !r.authFailed {
            let why = r.stderr.last { !$0.trimmingCharacters(in: .whitespaces).isEmpty } ?? ""
            note("Claude Code stopped (exit \(status))\(why.isEmpty ? "" : ": \(why)")", in: id)
        }
        if let app, app.document?.ref.kind == .scene { app.requestItems() }
    }

    /// Choosing an answer sends it as the next message, and the card keeps the choice.
    func answer(_ messageID: String, _ option: String) async {
        guard let id = currentID, cantSend == nil else { return }
        update(id) { c in
            if let i = c.messages.firstIndex(where: { $0.id == messageID }) { c.messages[i].chosen = option }
        }
        await send(option)
    }

    // MARK: What the views show

    /// "~/.local/bin/claude": a path under the search's home (the hook's temp home, else the owner's) with "~".
    func displayPath(_ url: URL) -> String {
        let home = search.home.hasSuffix("/") ? search.home : search.home + "/"
        return url.path.hasPrefix(home) ? "~/" + url.path.dropFirst(home.count) : url.path
    }

    /// The suggestion in the placeholder (R18): it fits the saved chips, and skips what this chat already asked.
    var suggestion: String? {
        guard let app, let doc = app.document, doc.ref.kind == .scene else { return nil }
        var objects: [String] = []
        for c in chips where !c.unsaved {
            objects += doc.layerTree.paths(of: c.target).compactMap { doc.itemsByPath[$0]?.object }
        }
        let blocks = Set(app.objects.filter { $0.kind == "block" }.map(\.name))
        let asked = (current?.messages ?? []).filter { $0.kind == .user }.compactMap(\.text)
        return ChatContext.suggestion(objects: objects, isBlock: { blocks.contains($0) }, asked: asked)
    }

    /// What the empty input shows: the suggestion, else why it can't send, else a hint.
    var placeholder: String {
        if let r = running { return r.chatID == currentID ? "Claude is working…" : (cantSend ?? "") }
        if let suggestion { return suggestion }
        return cantSend ?? "Select something on the stage, then ask Claude"
    }

    /// The CONTEXT line's right side.
    var contextSummary: String {
        guard let doc = app?.document else { return "" }
        if doc.ref.kind == .object { return "The whole object" }
        return ChatContext.countLine(chips)
    }

    /// The document a chat belongs to, by name (its folder's, without .pixellab).
    func docName(of c: Chat) -> String {
        c.document == documentRoot ? (app?.documentName ?? "") : URL(fileURLWithPath: c.document).deletingPathExtension().lastPathComponent
    }

    /// The Chats list's second line: "You: …", Claude's last words, a card or a step.
    func preview(_ c: Chat) -> String {
        guard let m = c.messages.last(where: { $0.kind != .note }) ?? c.messages.last else { return "No messages yet" }
        switch m.kind {
        case .user: return "You: \(m.text ?? "")"
        case .proposal: return "Proposal \(m.number.map { "#\($0)" } ?? ""): \(m.text ?? "")"
        case .step: return m.human ?? ""
        default: return m.text ?? ""
        }
    }

    /// "Claude Code 2.1.290 · signed in with your plan".
    var statusLine: String {
        let v = install.map { "Claude Code \($0.version)" } ?? "Claude Code"
        switch setup {
        case .ready: return "\(v) · signed in with your plan"
        case .missing: return "Claude Code isn't installed"
        case .signedOut, .waiting: return "\(v) · signed out"
        case .checking, .installing: return v
        }
    }

    /// A card's status: Waiting for you, Applied, Rejected, Settled elsewhere.
    func statusText(_ m: ChatMessage) -> String {
        switch m.outcome {
        case "applied": return "Applied"
        case "rejected": return "Rejected"
        case "elsewhere": return "Settled elsewhere"
        default: return "Waiting for you"
        }
    }

    // MARK: Links and names

    /// The object at a path of the open scene (the working copy).
    func objectAt(_ path: String) -> String? { app?.document?.itemsByPath[path]?.object }

    /// A link in a reply: select it on the stage.
    func select(_ target: LayerTarget) {
        guard let app, let doc = app.document, doc.ref.kind == .scene, doc.layerTree.exists(target) || doc.itemsByPath[target.itemPath ?? ""] != nil else { return }
        app.layers.select([target])
        app.editor.focusStage()
    }

    /// The links of a reply.
    func segments(_ text: String) -> [ChatLinks.Segment] {
        guard let doc = app?.document, doc.ref.kind == .scene else { return [ChatLinks.Segment(text: text)] }
        let layers = doc.layerTree.layers.map { (id: $0.id, name: $0.name) }
        return ChatLinks.segments(text, objectAt: { [weak doc] p in doc?.itemsByPath[p]?.object }, layers: layers)
    }
}

/// `chat.context`'s result (docs/intents/2026-10-agent-chat/spec.md, D5).
struct ChatContextResult: Codable, Sendable, Equatable {
    var text: String
    var version: String
    var unsaved: [String]
}
