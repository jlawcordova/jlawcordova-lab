import Foundation
import Testing

@testable import PixelLab

/// The Agent chat (docs/intents/2026-10-agent-chat/spec.md): reading Claude
/// Code's stream, finding and starting it, the transcript's rows, steps as
/// sentences, links in replies, and chats on disk.
@Suite struct ChatTests {
    static func fixture(_ name: String) -> [String] {
        let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("Fixtures/\(name)")
        return ((try? String(contentsOf: url, encoding: .utf8)) ?? "").split(separator: "\n").map(String.init)
    }

    // MARK: The stream (D1)

    @Test func R1_theRealCLIsSignedOutTurnReadsAsSignedOut() {
        let events = Self.fixture("stream-real-signed-out.jsonl").flatMap { StreamParser.parse(line: $0) ?? [] }
        #expect(events.count == 3)
        #expect(events[0] == .started(session: "598c81ec-8c10-410a-bfa2-1aa5ba5fcd89", mcpStatus: "connected"))
        guard case .authFailed(let text) = events[1] else { Issue.record("no authFailed: \(events)"); return }
        #expect(text.contains("Failed to authenticate"))
        guard case .finished(let session, let isError, _) = events[2] else { Issue.record("no finished"); return }
        #expect(session == "598c81ec-8c10-410a-bfa2-1aa5ba5fcd89")
        #expect(isError)
    }

    @Test func R13_aTurnsToolCallsPairWithTheirResultsAndTheReplyComesLast() {
        let events = Self.fixture("stream-fake-move.jsonl").flatMap { StreamParser.parse(line: $0) ?? [] }
        let uses = events.compactMap { e -> String? in if case .toolUse(_, let name, _) = e { return StreamParser.toolName(name) }; return nil }
        #expect(uses == ["scene_items", "scene_move", "scene_move", "scene_render", "doc_save"])
        let results = events.compactMap { e -> Bool? in if case .toolResult(_, let ok, _) = e { return ok }; return nil }
        #expect(results == [true, true, true, true, true])
        guard case .text(let reply) = events[events.count - 2] else { Issue.record("no reply"); return }
        #expect(reply.hasPrefix("Moved pine 14.0 and pine 39.0 one tile down-right."))
        // The two moves changed the document: the turn ends with a change card (autosave spec, R18).
        let texts = events.compactMap { e -> String? in if case .toolResult(_, _, let t) = e { return t }; return nil }
        #expect(texts.filter(ChatModel.edited).count == 2)
    }

    @Test func R1_unknownLinesAreIgnoredAndNonJSONIsADiagnostic() {
        #expect(StreamParser.parse(line: #"{"type":"rate_limit_event","x":1}"#) == [])
        #expect(StreamParser.parse(line: #"{"type":"stream_event","event":{}}"#) == [])
        #expect(StreamParser.parse(line: "") == [])
        #expect(StreamParser.parse(line: "Warning: something") == nil)
        let mixed = #"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"…"},{"type":"text","text":"Hi"}]}}"#
        #expect(StreamParser.parse(line: mixed) == [.text("Hi")])
        let errorResult = #"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"no item 3","is_error":true}]}}"#
        #expect(StreamParser.parse(line: errorResult) == [.toolResult(id: "t1", ok: false, text: "no item 3")])
        #expect(StreamParser.parse(line: #"{"type":"result","subtype":"error_max_turns","session_id":"s"}"#) == [.finished(session: "s", isError: true, result: nil)])
    }

    // MARK: Finding and starting Claude Code (D1, D2)

    @Test func R4_theSearchOrderAndTheHooksTempHome() {
        let system = ClaudeLocator.Search(home: "/Users/me", useSystem: true, installer: nil)
        #expect(ClaudeLocator.candidates(chosen: "~/bin/claude", search: system).dropFirst() == ["/Users/me/.local/bin/claude", "/opt/homebrew/bin/claude", "/usr/local/bin/claude"])
        let temp = ClaudeLocator.Search(home: "/tmp/h", useSystem: false, installer: "/x/install.sh")
        #expect(ClaudeLocator.candidates(chosen: nil, search: temp) == ["/tmp/h/.local/bin/claude"])
        // Only the script hook may move the search.
        #expect(ClaudeLocator.Search.fromEnvironment(["PIXELLAB_CLAUDE_HOME": "/tmp/h"]).useSystem)
        let hooked = ClaudeLocator.Search.fromEnvironment(["PIXELLAB_SCRIPT": "/s.txt", "PIXELLAB_CLAUDE_HOME": "/tmp/h", "PIXELLAB_CLAUDE_INSTALLER": "/i.sh"])
        #expect(hooked == ClaudeLocator.Search(home: "/tmp/h", useSystem: false, installer: "/i.sh"))
    }

    @Test func R1_theEnvironmentDropsAParentSessionsVariablesAndAnyAPIKey() {
        let base = ["HOME": "/Users/me", "PATH": "/x", "CLAUDECODE": "1", "CLAUDE_CODE_SIMPLE": "1", "ANTHROPIC_BASE_URL": "http://x", "ANTHROPIC_API_KEY": "k", "PIXELLAB_STATE": "/s", "LANG": "en"]
        let env = ClaudeLocator.environment(base: base, claude: URL(fileURLWithPath: "/u/.local/bin/claude"),
                                            search: ClaudeLocator.Search(home: "/Users/me", useSystem: true, installer: nil), loginPath: [])
        #expect(env.keys.sorted() == ["HOME", "LANG", "PATH", "PIXELLAB_STATE"])
        #expect(env["PATH"] == "/u/.local/bin:/usr/bin:/bin:/usr/sbin:/sbin")
        let temp = ClaudeLocator.environment(base: base, claude: nil, search: ClaudeLocator.Search(home: "/tmp/h", useSystem: false, installer: nil))
        #expect(temp["HOME"] == "/tmp/h")
    }

    @Test("R21: a turn's PATH is claude's folder, the login shell's PATH, then the system's, without repeats")
    func R21_theChatsPathComesFromTheLoginShell() {
        let search = ClaudeLocator.Search(home: "/Users/me", useSystem: true, installer: nil)
        let login = ClaudeLocator.parsePath("/opt/homebrew/bin:/Users/me/.nvm/bin::/usr/bin:/u/.local/bin\n")
        #expect(login == ["/opt/homebrew/bin", "/Users/me/.nvm/bin", "/usr/bin", "/u/.local/bin"])
        let env = ClaudeLocator.environment(base: ["PATH": "/ignored"], claude: URL(fileURLWithPath: "/u/.local/bin/claude"), search: search, loginPath: login)
        #expect(env["PATH"] == "/u/.local/bin:/opt/homebrew/bin:/Users/me/.nvm/bin:/usr/bin:/bin:/usr/sbin:/sbin")
        #expect(ClaudeLocator.parsePath(nil).isEmpty)
        // The script hook's search never asks the owner's shell.
        let hooked = ClaudeLocator.environment(base: [:], claude: nil, search: ClaudeLocator.Search(home: "/tmp/h", useSystem: false, installer: nil))
        #expect(hooked["PATH"] == "/usr/bin:/bin:/usr/sbin:/sbin")
    }

    @Test func R4_versionAndSignInAreRead() {
        #expect(ClaudeLocator.parseVersion("2.1.290 (Claude Code)\n") == "2.1.290")
        #expect(ClaudeLocator.parseVersion("command not found") == nil)
        #expect(ClaudeLocator.parseSignedIn(#"{ "loggedIn": true, "authMethod": "claude.ai" }"#) == true)
        #expect(ClaudeLocator.parseSignedIn(#"{ "loggedIn": false }"#) == false)
        #expect(ClaudeLocator.parseSignedIn("Not logged in") == nil)
    }

    @Test func R2_aTurnRunsWithOnlyPixelLabsTools() throws {
        let args = ClaudeTurn.arguments(mcpConfig: "{}", prompt: "P", resume: "abc")
        let pairs = Dictionary(stride(from: 0, to: args.count - 1, by: 1).map { (args[$0], args[$0 + 1]) }, uniquingKeysWith: { a, _ in a })
        #expect(args.first == "-p")
        #expect(pairs["--tools"] == "")
        #expect(pairs["--allowedTools"] == "mcp__pixellab__*")
        #expect(pairs["--disallowedTools"] == "mcp__pixellab__document_new")
        #expect(pairs["--permission-prompts"] == "none")
        #expect(pairs["--setting-sources"] == "")
        #expect(pairs["--input-format"] == "stream-json")
        #expect(pairs["--output-format"] == "stream-json")
        #expect(pairs["--resume"] == "abc")
        #expect(args.contains("--strict-mcp-config") && args.contains("--verbose"))
        #expect(!ClaudeTurn.arguments(mcpConfig: "{}", prompt: "P", resume: nil).contains("--resume"))

        let config = ClaudeTurn.mcpConfig(program: URL(fileURLWithPath: "/c/pixellab"), location: .document("/d/h.pixellab"),
                                          stateDir: "/s", chatID: "c1", title: "Pines")
        let v = try JSONDecoder().decode(JSONValue.self, from: Data(config.utf8))
        let server = v["mcpServers"]?["pixellab"]
        #expect(server?["command"]?.stringValue == "/c/pixellab")
        #expect(server?["args"]?.arrayValue?.compactMap(\.stringValue) == ["serve", "--mcp", "--document", "/d/h.pixellab"])
        #expect(server?["env"]?["PIXELLAB_CHAT"]?.stringValue == "c1")
        #expect(server?["env"]?["PIXELLAB_CHAT_TITLE"]?.stringValue == "Pines")
        #expect(server?["env"]?["PIXELLAB_STATE"]?.stringValue == "/s")

        let line = try JSONDecoder().decode(JSONValue.self, from: ClaudeTurn.input(block: "<selection/>", text: "Hi"))
        #expect(line["message"]?["content"]?.stringValue == "<selection/>\n\nHi")
        #expect(ClaudeTurn.resumeFailed(stderr: "No conversation found with session ID: x", started: false))
        #expect(!ClaudeTurn.resumeFailed(stderr: "No conversation found", started: true))
    }

    // MARK: Chats on disk (D3)

    @Test func R19_chatsRoundTripAndTitlesComeFromTheFirstMessage() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("chat-tests-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: dir) }
        let store = ChatStore(stateDir: dir.path)
        var chat = Chat(id: ChatStore.newID(), title: Chat.title(from: "Move these one tile east, please, and then some"), document: "/d", createdAt: Date(), updatedAt: Date(), session: "s1")
        chat.pins = [ChatPin(doc: "scene:harbor", target: "layer:boats"), ChatPin(doc: "scene:harbor", target: "14.0", object: "pine")]
        var step = ChatMessage(kind: .step)
        step.tool = "scene_move"
        step.input = ["item": "14.0", "by": [1, 0]]
        chat.messages = [ChatMessage(kind: .user, text: "Move these"), step]
        try store.save(chat)
        let older = Chat(id: ChatStore.newID(), title: "Old", document: "/d", createdAt: .distantPast, updatedAt: .distantPast)
        try store.save(older)
        try Data("not json".utf8).write(to: store.dir.appendingPathComponent("broken.json"))
        let loaded = store.load()
        #expect(loaded.map(\.id) == [chat.id, older.id])
        #expect(loaded[0].title == "Move these one tile east, ple…")
        #expect(loaded[0].pins == chat.pins)
        #expect(loaded[0].messages[1].input?["by"] == [1, 0])
        #expect(loaded[0].session == "s1")
        #expect(Chat.title(from: "  \n") == Chat.untitled)
        #expect(ChatStore.newID().wholeMatch(of: /\d{8}-\d{6}-[0-9a-f]{6}/) != nil)
    }

    // MARK: Rows (D9, D12)

    @Test func R12_R13_stepsFoldAndRowsGroupBySender() {
        func m(_ k: ChatMessage.Kind, _ t: String = "") -> ChatMessage {
            var x = ChatMessage(kind: k, text: t)
            if k == .step { x.human = t }
            return x
        }
        let msgs = [m(.user, "Move"), m(.step, "Looked"), m(.step, "Moved a"), m(.step, "Moved b"), m(.claude, "Done"), m(.proposal), m(.note, "You approved"), m(.user, "Thanks")]
        let rows = ChatRow.rows(msgs, typing: true)
        #expect(rows.count == 7)
        guard case .steps(let run) = rows[1].kind else { Issue.record("no steps row"); return }
        #expect(run.count == 3)
        #expect(ChatRow.stepsLabel(run) == "Moved b · 3 steps")
        #expect(ChatRow.stepsLabel([run[0]]) == "Looked")
        // Claude's group: steps, reply, card. "Claude" above the first, the avatar beside the last.
        #expect(rows[1].side == .claude && rows[1].first && !rows[1].last)
        #expect(rows[2].side == .claude && !rows[2].first && !rows[2].last)
        #expect(rows[3].side == .claude && rows[3].last)
        #expect(rows[4].side == .note)
        #expect(rows[5].side == .me && rows[5].first && rows[5].last)
        #expect(rows[6].kind == .typing && rows[6].first && rows[6].last)
    }

    @Test func R12_theDateHeader() {
        var cal = Calendar(identifier: .gregorian)
        cal.timeZone = TimeZone(identifier: "UTC")!
        let now = Date(timeIntervalSince1970: 1_792_540_800) // 2026-10-21 00:00 UTC, a Wednesday
        let later = now.addingTimeInterval(9 * 3600)
        #expect(ChatRow.dateHeader(later, now: later, calendar: cal).day == "Today")
        #expect(ChatRow.dateHeader(now.addingTimeInterval(-3600), now: later, calendar: cal).day == "Yesterday")
        #expect(ChatRow.dateHeader(now.addingTimeInterval(-3 * 86400), now: later, calendar: cal).day.isEmpty == false)
        #expect(ChatRow.dateHeader(now.addingTimeInterval(-400 * 86400), now: later, calendar: cal).day.contains("2025"))
    }

    // MARK: Steps as sentences (D9)

    @Test func R13_toolCallsReadAsSentences() {
        let objects = ["134.0": "pine", "51": "rock"]
        let at: (String) -> String? = { objects[$0] }
        #expect(StepText.human(tool: "scene_move", input: ["name": "harbor", "item": "134.0", "by": [1, 0]], result: nil, ok: true, objectAt: at) == "Moved pine 134.0 one tile down-right")
        #expect(StepText.human(tool: "scene_move", input: ["item": "134.0", "by": [0, -2]], result: nil, ok: true, objectAt: at) == "Moved pine 134.0 2 tiles up-right")
        #expect(StepText.human(tool: "scene_move", input: ["item": "51", "by": [0, 0, 1]], result: nil, ok: true, objectAt: at) == "Moved rock 51 up a level")
        #expect(StepText.human(tool: "scene_move", input: ["item": "51", "nudge": [2, 0]], result: nil, ok: true, objectAt: at) == "Nudged rock 51 by 2 px right")
        #expect(StepText.human(tool: "scene_items", input: ["name": "harbor"], result: #"{"scene":"harbor","items":[{},{},{}]}"#, ok: true) == "Looked through the 3 items in harbor")
        #expect(StepText.human(tool: "object_show", input: ["name": "pine"], result: #"{"name":"pine","kind":"sprite","size":[11,18]}"#, ok: true) == "Looked at the pine sprite (11×18)")
        #expect(StepText.human(tool: "doc_save", input: ["doc": "scene:harbor"], result: #"{"wrote":true}"#, ok: true) == "Saved harbor")
        #expect(StepText.human(tool: "history_undo", input: ["version": 4], result: nil, ok: true) == "Took back version 4")
        #expect(StepText.human(tool: "scene_place", input: ["object": "rock", "at": [0, 0, 1]], result: nil, ok: true) == "Placed a rock at 0,0,1")
        #expect(StepText.human(tool: "check", input: [:], result: #"{"target":{"name":"harbor"},"problems":[{}]}"#, ok: true) == "Checked harbor: 1 problem")
        #expect(StepText.human(tool: "scene_layer_new", input: ["items": ["1", "2"]], result: #"{"layer":{"id":"boats","name":"Boats"},"items":2}"#, ok: true) == "Grouped 2 items as Boats")
        #expect(StepText.human(tool: "scene_move", input: ["item": "3"], result: "no item 3 in harbor\nmore", ok: false) == "Couldn't move it: no item 3 in harbor")
        #expect(StepText.human(tool: "doc_undo", input: [:], result: #"{"summary":"undid pine moved"}"#, ok: true) == "Undid pine moved")
        #expect(StepText.human(tool: "frobnicate", input: [:], result: nil, ok: true) == "Used frobnicate")
    }

    @Test func R13_theRawCallForDetails() {
        #expect(StepText.raw(tool: "scene_move", input: ["by": [1, 0], "item": "134.0", "name": "harbor"], result: nil) == "scene_move name=harbor item=134.0 by=1,0")
        #expect(StepText.raw(tool: "doc_save", input: ["doc": "scene:harbor"], result: #"{"wrote":true}"#) == "doc_save doc=scene:harbor")
    }

    // MARK: Links (D10)

    @Test func R14_itemAndLayerNamesInRepliesAreLinks() {
        let objects = ["14.0": "pine", "39.0": "pine", "51": "rock"]
        let segs = ChatLinks.segments("Moved pine 14.0 and rock 51 into Boats. pine 99.0 isn't there; boats is lowercase; Boatshed isn't a word match.",
                                      objectAt: { objects[$0] }, layers: [(id: "boats", name: "Boats"), (id: "ab", name: "Ab")])
        let links = segs.filter { $0.target != nil }
        #expect(links.map(\.text) == ["pine 14.0", "rock 51", "Boats"])
        #expect(links.map(\.target) == [.item("14.0"), .item("51"), .layer("boats")])
        #expect(segs.map(\.text).joined() == "Moved pine 14.0 and rock 51 into Boats. pine 99.0 isn't there; boats is lowercase; Boatshed isn't a word match.")
        // An item named by the wrong object isn't a link.
        #expect(ChatLinks.segments("rock 14.0", objectAt: { objects[$0] }, layers: []).allSatisfy { $0.target == nil })
    }
}

/// The chat's context (docs/intents/2026-10-agent-chat/spec.md, D5, D6): chips, the CONTEXT line,
/// the placeholder's suggestion, every use of an object, and where a proposal came from.
@Suite struct ChatContextTests {
    let items = [
        SceneItemInfo(path: "0", object: "pine", at: ItemAt(tile: [0, 0, 1])),
        SceneItemInfo(path: "1", object: "pine", at: ItemAt(tile: [1, 0, 0]), layer: "trees"),
        SceneItemInfo(path: "2", object: "pine", at: ItemAt(tile: [2, 0, 0]), layer: "grove"),
        SceneItemInfo(path: "3", object: "block-road", at: ItemAt(tile: [3, 0, 0]), layer: "ground"),
        SceneItemInfo(path: "4", object: "rock", at: ItemAt(tile: [4, 0, 0])),
    ]
    let layers = [
        SceneLayerInfo(id: "trees", name: "Trees", items: 2),
        SceneLayerInfo(id: "grove", name: "Grove", parent: "trees", items: 1),
        SceneLayerInfo(id: "ground", name: "Ground", items: 1),
    ]
    var tree: LayerTree { LayerTree(items: items, layers: layers) }
    var byPath: [String: SceneItemInfo] { Dictionary(uniqueKeysWithValues: items.map { ($0.path, $0) }) }

    @Test func R9_pinsComeFirstThenTheSelectionAndGroupsShowTheirItemCount() {
        let chips = ChatContext.chips(selection: [.layer("trees"), .item("4")], pins: [.item("4"), .item("0")], tree: tree, items: byPath, unsaved: ["0"])
        #expect(chips.map(\.id) == ["4", "0", "layer:trees"])
        #expect(chips.map(\.pinned) == [true, true, false])
        #expect(chips[2].label == "Trees" && chips[2].sub == "2 items")
        #expect(chips[1].sub == "0 · not saved" && chips[1].unsaved)
        #expect(ChatContext.countLine(chips) == "1 group, 2 items · 2 pinned to this chat")
        #expect(ChatContext.countLine([]) == "The whole scene")
        #expect(chips.map(ChatContext.messageLabel) == ["rock 4", "pine 0 (not sent)", "Trees · 2 items"])
        // A target the scene no longer has makes no chip.
        #expect(ChatContext.chips(selection: [.item("9"), .layer("gone")], pins: [], tree: tree, items: byPath, unsaved: []).isEmpty)
    }

    @Test func R18_theSuggestionFitsTheSelectionAndMovesOnOnceAsked() {
        let block = { (o: String) in o.hasPrefix("block") }
        #expect(ChatContext.suggestion(objects: [], isBlock: block, asked: []) == nil)
        #expect(ChatContext.suggestion(objects: ["block-road", "block"], isBlock: block, asked: []) == "Raise them a level")
        #expect(ChatContext.suggestion(objects: ["block"], isBlock: block, asked: []) == "Raise it a level")
        #expect(ChatContext.suggestion(objects: ["pine", "tree-small"], isBlock: block, asked: []) == "Make these taller")
        #expect(ChatContext.suggestion(objects: ["pine"], isBlock: block, asked: ["make this taller"]) == "Swap it for another tree")
        #expect(ChatContext.suggestion(objects: ["rock", "pine"], isBlock: block, asked: []) == "What are these?")
        #expect(ChatContext.suggestion(objects: ["rock"], isBlock: block, asked: []) == "What is this?")
        #expect(ChatContext.isTree("pine") && ChatContext.isTree("tree-small") && !ChatContext.isTree("pinecone-sign") && !ChatContext.isTree("street"))
    }

    @Test func R8_everyUseKeepsToWhatAClickCanReach() {
        // At the top level, every pine, inside layers too: "every use" is about items.
        #expect(tree.everyUse(of: "pine", items: items, inside: nil, hidden: [], locked: []) == ["0", "1", "2"])
        // Inside Trees, only its pines; a hidden or locked layer below drops its items.
        #expect(tree.everyUse(of: "pine", items: items, inside: "trees", hidden: [], locked: []) == ["1", "2"])
        #expect(tree.everyUse(of: "pine", items: items, inside: nil, hidden: ["grove"], locked: []) == ["0", "1"])
        #expect(tree.everyUse(of: "pine", items: items, inside: nil, hidden: [], locked: ["trees"]) == ["0"])
        #expect(tree.everyUse(of: "pine", items: items, inside: "trees", hidden: [], locked: ["trees"]) == ["1", "2"], "a locked layer's contents are reached from inside it")
    }

    @Test func R21_aVersionSaysWhoMadeItAndActivityWhereItCameFrom() {
        let agent = VersionAuthor(kind: "agent", name: "claude-code 2.1.290", chat: ProposalChat(id: "c", title: "Pines"))
        #expect(agent.display == "Claude")
        #expect(VersionAuthor(kind: "cli").display == "The CLI")
        #expect(VersionAuthor(kind: "owner").display == "You")
        #expect(VersionAuthor(kind: "outside").display == "Outside Pixel Lab")
        let line = try? JSONDecoder().decode(ActivityEntry.self, from: Data(#"{"time":"t","tool":"scene_move","ok":true,"agent":"claude-code 2.1.290","chat":{"id":"c","title":"Pines"}}"#.utf8))
        #expect(line?.who == "chat · Pines")
        // A proposal card from an older chat still decodes, and shows as set aside.
        let old = try? JSONDecoder().decode(ChatMessage.self, from: Data(#"{"id":"m","time":0,"kind":"proposal","text":"Moved","proposal":"p","number":3}"#.utf8))
        #expect(old?.kind == .proposal && old?.number == 3)
    }

    @Test func R18_aToolResultThatChangedTheDocumentMakesAChangeCard() {
        #expect(ChatModel.edited(#"{"doc":"scene:harbor","changed":true,"summary":"pine moved"}"#))
        #expect(ChatModel.edited(#"{"version":3,"changed":true,"written":["scene:harbor"]}"#))
        #expect(!ChatModel.edited(#"{"doc":"scene:harbor","changed":false,"summary":"nothing changed"}"#))
        #expect(!ChatModel.edited(#"{"scene":"harbor","items":[]}"#))
        #expect(ChatModel.titled("pine moved to column 4") == "Pine moved to column 4")
    }
}
