# Spec: Agent chat (from intent.md 2026-10-21)
Status: approved (the owner accepted the intent, spec and plan up front and reviews everything in the final PR).

> **Superseded in part (2026-10-23):** R2's "no tool that writes files outside a proposal", R16, R17, R21 and R24's proposals are replaced by [`../2026-10-autosave/spec.md`](../2026-10-autosave/spec.md) (R16–R18): a chat's edits apply as Claude makes them, and a turn that changed something ends with a change card (Undo These Changes, Show in Versions). The rest of this spec still holds.

The prototype canvas named in the intent (`AgentChat.dc.html`, `Agent.dc.html`, `Groups.dc.html`, `GroupsDay.dc.html`, `Main.dc.html`) is the design for everything the owner sees. This spec settles what it leaves open: how Claude Code runs, how a message carries the selection, how questions and proposals come back, the setup flow, storage, and the tests. Where the app departs from the prototype, it says so where it happens.

Everything about the `claude` CLI below was checked against `claude --help` and `claude auth … --help` of Claude Code 2.1.290 on the owner's Mac on 2026-10-21, and a headless run against a temp copy of `examples/harbor.pixellab` (D1, "What a real run showed").

## Requirements

**Running Claude Code**

- **R1. The owner's own Claude Code.** The chat runs the `claude` CLI installed on this Mac, headless, signed in the way it already is (the owner's Claude plan). No API key, no new npm or Swift dependency, nothing of Claude Code bundled in the app.
- **R2. Only Pixel Lab's tools.** In the chat, Claude can use the pixellab MCP tools of the document the app has open, and nothing else: no shell, no file reads or edits, no web, no other MCP servers, no settings or hooks from the owner's own Claude Code configuration, and no tool that writes files outside a proposal.
- **R3. A conversation carries on.** Each chat keeps its Claude session, so a follow-up message (and a message after the app relaunches) continues the same conversation.
- **R4. Setup in the Agent tab.** The Agent tab finds `claude` in `~/.local/bin`, the usual Homebrew folders and the login shell's PATH (the app may be opened from the Finder, which gives it no shell PATH), or a command the owner chose. It shows, in turn: not installed (Install Claude Code with the official installer's command shown, Copy command, Check again, Choose the claude command…), installing (a live log ending with the version), signed out (Sign in with browser, through the CLI's own login), waiting for the browser (Open the page again, Cancel), and ready (the chat). Sign out goes back to signed out. This is separate from the app's first-launch Setup window.

**Where it lives**

- **R5. The right column's tabs.** The inspector column has a tab bar, Inspector | ✦ Agent. The toolbar's sparkle button (with its badge of waiting proposals) and Window › Agent (⌥⌘A) show the Agent tab. Window › Agent Window (⇧⌥⌘A) opens the Agent window.
- **R6. Ask Claude about this.** In scene mode, the Inspector shows an "Ask Claude about this" button ("…about these 3" for several) whenever something is selected; it shows the Agent tab and puts the focus in its input.

**The selection as context**

- **R7. What can be selected.** The chat's context is the stage selection, which scene layers already decide (docs/intents/2026-10-scene-layers/spec.md, D6): a click picks the outermost layer under the pointer, a double-click goes inside it, ⇧-click adds, hidden and locked layers can't be picked; inside a layer, or in a scene with no layers, every item can be picked (objects, blocks, ground, roads, shadows). A click picks what is drawn under the pointer, per pixel in paint order. The arrow keys move the selection and Esc clears it, as now.
- **R8. Every use of an object.** ⌥-click on the stage (a click, not a drag) selects every item that uses the object drawn under the pointer, among the items that can be picked where you are; ⇧⌥-click adds them. Edit › Select Every Use does the same for the selected item, from the keyboard.
- **R9. Chips.** Above the input, one chip per selected or pinned layer (its name and item count) or item (object and path). A chip can be removed (it leaves the selection, or is unpinned) and pinned (it stays in this chat's context while the selection changes). Pins belong to a chat. With nothing selected or pinned, a dashed chip says the whole scene is in view.
- **R10. The selection block.** Each message carries a `<selection document scene version>` block: each selected group (id, name, its items' paths) and each item (path, object, position, its group), as the saved document has them; with nothing selected it says the whole scene is in view. A disclosure under the chips, "What Claude receives", shows the block exactly as it will be sent.
- **R11. Unsaved things are left out.** A selected item or layer that isn't in the saved document as it is in the app is left out of the block (a line says how many were left out). Its chip is dashed gold with "not saved", and a warning above the input names them, with Save now.

**The conversation**

- **R12. A messages look.** Bubbles without tails and with one square corner: bottom-right for the owner's (accent), bottom-left for Claude's (card). Messages group by sender: "Claude" above the first bubble of a group, the avatar beside the last. The avatar is a plain asterisk on terracotta. A typing bubble shows while Claude works, and a date header tops each chat. The owner's messages show the chips they were sent with.
- **R13. Steps read as sentences.** Each tool call shows as a plain sentence ("Moved pine 134.0 one tile down-right"). A run of steps folds into one line, the latest step and "· N steps", which expands. A Details toggle in the chat header shows the raw call under each step.
- **R14. Links.** Item names ("pine 134.0") and layer names in Claude's replies are links that select them on the stage.
- **R15. Questions.** Claude can ask a question with two to four answers, shown as a card with a button per answer; choosing one sends it as the owner's next message, and the card keeps the choice.
- **R16. Proposals in the chat.** An edit Claude saves is a proposal, never a write (`--direct` stays off). In the chat it shows as a card: PROPOSAL #n, its summary and edits, its status (Waiting for you, Applied, Rejected, Settled elsewhere), Approve, Reject, Show/Hide on stage, and "Diff in the Agent window". Approving or rejecting asks what the Agent window asks. The outcome is noted in the chat.
- **R17. Proposals everywhere else.** A chat's proposal ghosts on the stage like any other (shown when the turn that made it ends in the open chat), with the banner "Previewing proposal #n from the chat “<title>”", counts in the toolbar badge and the Dock badge, and is listed in the Agent window. Ghosts show only for the chat you're in: switching chats hides a ghost from another chat.
- **R18. The input.** The placeholder holds a suggestion that fits the selection ("Raise them a level" for blocks, "Make these taller" for trees, "What are these?" otherwise; singular forms for one thing; the next idea once one was asked), with a small "tab" hint. Tab takes it only while the input is empty; otherwise Tab moves the focus. No suggestion buttons. Return sends, ⇧Return adds a line, and Stop interrupts while Claude works.
- **R19. Several chats.** A Chats list (each chat's avatar, title, time or day, last message, and "harbor · 2 pinned · 1 waiting"), New chat, and back to Chats from a chat. A new chat is titled by its first message. Each chat keeps its session, pins and history in the state dir, across launches. The stage selection is shared by every chat. Chats belong to the document they were made in; the list shows the open document's.

**The Agent window**

- **R20. Claude Code row.** Above CONNECT, a CLAUDE CODE row: a status dot and "2.1.290 · signed in with your plan · 3 chats" (or not installed, or signed out), Open chats (the Agent tab) and Sign out (or Set up).
- **R21. Where a proposal came from.** Each proposal row and the detail say its source: "Chat · <title>" with the asterisk avatar, or "<agent> · terminal". A chat's proposal's detail adds "From the chat “<title>”, asked about <what was selected>. Open the chat". Proposals show their number, #n.
- **R22. A Chats filter.** The activity log's filter is All | Chats | Errors, and each row says who made the call (the chat's title, or the agent).

**Every action, three doors; proof and standards**

- **R23. Registry commands.** The new actions are registry commands, so the CLI, the app and MCP agents have them: `chat.ask` (a question with answers) and `chat.context` (the selection block). `docs/cli.md` is regenerated.
- **R24. Proposals and activity know their source.** A proposal saved from a chat records the chat (id and title); every proposal gets a number (#n) that stays with it; each activity line records the agent and, from a chat, the chat. Existing proposals and activity lines without these keep working.
- **R25. Tests never use the plan or the network.** A fake `claude` (a script in this repo) answers `--version`, `auth status|login|logout` and headless turns in the real stream-json format, doing its tool calls through the real pixellab MCP server, so proposals, the activity log and the app are exercised end to end. Unit tests parse the real CLI's lines captured on 2026-10-21. The fake installer, used in tests in place of the real one, writes the fake `claude`.
- **R26. The script hook and the acceptance pass.** ScriptRunner gains actions and `expect` lines for the setup states, the tabs, sending, Stop, the steps, questions, links, chips, pins, Save now, the disclosure, chats, the cards (approve, reject, ghost) and the Agent window's filter and Claude Code row. `scripts/acceptance/` runs the chat end to end on a temp copy of the site and of `examples/harbor.pixellab`, and snapshots each new view in dark and light.
- **R27. Docs.** The README (the Agent tab, its setup, chats, context, the shortcuts ⌥⌘A, ⇧⌥⌘A, ⌥-click), `docs/architecture.md`, `docs/testing.md`, `app/README.md`, `cli/README.md`, `CLAUDE.md`'s hook list, the MCP server's instructions and `docs/cli.md` describe it.
- **R28. Accessibility and the app's rules.** VoiceOver labels on every message, step, chip, card, button, the input and the setup states; visible focus; hit targets of 24 pt or more (28 pt for chips and card buttons); a keyboard path through the tab bar, the Chats list, the conversation's buttons and the input; Reduce Motion stops the typing dots and the chevrons' animation. Colors from board 03 tokens (two new ones, D14), no `@State`, no new dependencies.

## Design

### D1. How Claude Code runs (R1–R3)

**One process per turn.** Sending a message starts

```
claude -p --input-format stream-json --output-format stream-json --verbose
       --mcp-config '<json>' --strict-mcp-config
       --tools "" --allowedTools "mcp__pixellab__*" --disallowedTools "mcp__pixellab__document_new"
       --permission-prompts none --setting-sources ""
       --append-system-prompt '<the chat prompt>'
       [--resume <session id>]
```

writes the one user message to its stdin as a stream-json line, closes stdin, and reads stream-json lines from stdout until it exits. Why each flag:

- `-p`, `--input-format stream-json`, `--output-format stream-json`, `--verbose`: headless, structured both ways; `--verbose` is required for stream-json output in `-p` mode.
- `--mcp-config` with `--strict-mcp-config`: the one MCP server, `pixellab`, and none of the owner's others. The config is `{"mcpServers":{"pixellab":{"type":"stdio","command":<node>,"args":[<bundled cli>,"serve","--mcp",--document|--site,<path>],"env":{"PIXELLAB_STATE":<state dir>,"PIXELLAB_CHAT":<chat id>,"PIXELLAB_CHAT_TITLE":<title>}}}}`: the same node, CLI, document and state dir as the app's own server, plus the chat (D7). No `--direct`, so `doc_save` proposes.
- `--tools ""`: no built-in tools (Bash, Read, Edit, Write, WebFetch, …). `--allowedTools "mcp__pixellab__*"`: the pixellab tools run without asking. `--disallowedTools mcp__pixellab__document_new`: the one exposed tool that writes files outside a proposal (`document.new` makes a new document folder); the others that write (`art.build`, `document.copy`, `proposal.apply`, `proposal.reject`) aren't exposed over MCP at all.
- `--permission-prompts none`: anything that would ask is denied, so a turn can never hang on a prompt nobody sees.
- `--setting-sources ""`: none of the owner's user, project or local settings (hooks, plugins, permissions, MCP servers) apply to the chat.
- `--append-system-prompt`: the chat prompt (D11) on top of Claude Code's own.
- `--resume <id>`: the chat's Claude session (R3). The first turn has none; the `session_id` of its `system`/`init` line is stored with the chat. A resume that fails ("No conversation found", a result error before any message) starts a new session once, and the chat notes "Claude started a new session; it doesn't remember the earlier messages".

**Why a process per turn** rather than one long-lived process per chat: a turn is the unit of work the owner sees (and the fake plays), Stop is simply ending the process, nothing outlives a quit, and `--resume` keeps the conversation. The cost is a second or two of start-up per message and that pixellab edits Claude didn't save die with the turn's MCP server; the prompt tells Claude to save (propose) before ending a turn that changed something.

**Working directory.** `<state dir>/chats/`, the same for every turn, so Claude Code stores the sessions under one project folder and `--resume` finds them.

**Environment.** The child gets the app's environment without every `CLAUDE*` and `ANTHROPIC_*` variable: a parent Claude Code session's variables (`CLAUDECODE`, `CLAUDE_CODE_*`, `ANTHROPIC_BASE_URL`) redirect its auth and make it fail (found in the run below), and an `ANTHROPIC_API_KEY` would bill an API account instead of the owner's plan (R1). `PATH` is the folders of `claude` and `node`, then `/usr/bin:/bin:/usr/sbin:/sbin`.

**What a real run showed (2026-10-21).** Run from a terminal inside a Claude Code session, the first line was `{"type":"system","subtype":"init","session_id":…,"tools":["mcp__pixellab__activity_list",…],"mcp_servers":[{"name":"pixellab","status":"connected"}],"permissionMode":"default",…}`: the flags above load the pixellab server and only its tools. The turn then failed auth: `{"type":"assistant","message":{"content":[{"type":"text","text":"Failed to authenticate: …"}],…},"error":"authentication_failed",…}` and `{"type":"result","is_error":true,"result":"Failed to authenticate: …",…}`. With the parent's variables removed, `claude auth status --json` reported `"loggedIn": false` (the owner's CLI login had expired), so no signed-in turn could be recorded without signing the owner in (Areas of concern). The app reads `error: "authentication_failed"` as signed out (D2).

**Reading the stream** (`StreamParser`, pure, unit-tested on those captured lines and on the documented shapes the fake writes):

| line | becomes |
| --- | --- |
| `system` / `init` | the session id; `mcp_servers[pixellab].status` other than `connected` ends the turn with "Pixel Lab's tools didn't start: <status>" |
| `assistant`, content `text` | a Claude bubble (consecutive text blocks join) |
| `assistant`, content `tool_use` | a step, pending (tool, input, id); `mcp__pixellab__chat_ask` is a question (D8) |
| `user`, content `tool_result` | completes its step (ok or error, the result's text); a `doc_save` result with `proposal` adds a proposal card (D7) |
| `assistant` with `error: "authentication_failed"` | the turn ends, the setup goes to signed out, the chat notes it |
| `result` | the turn ends; `is_error` adds a note with `result`; `session_id` is kept |
| anything else (`stream_event`, `rate_limit_event`, unknown types) | ignored |

Lines that aren't JSON go to the turn's diagnostics (shown with a failed turn), as does stderr.

**Stop** ends the process (SIGINT, then SIGTERM after 2 s) and notes "Stopped" ("· nothing was proposed" when no card came in the turn). **Quitting** the app stops a running turn the same way.

**One turn at a time.** One turn runs at a time, app-wide, counting a send that is still fetching its block (so Return twice, or Return and an answer, can't start two). While it runs, the other chats' inputs say "Claude is working in “<title>”" and can't send.

### D2. Finding Claude Code and the setup flow (R4)

`ClaudeLocator` checks, in order: the command chosen with "Choose the claude command…" (UserDefaults `claudePath`), `~/.local/bin/claude`, `/opt/homebrew/bin/claude`, `/usr/local/bin/claude`, then `zsh -lc 'command -v claude'` (5 s limit, as `Locator.findNode`). Each is run as `claude --version` (5 s limit); a first word like `2.1.290` is the version.

The setup state (`ClaudeSetup`): `checking` → `missing` | `signedOut` | `ready`, plus `installing` and `waiting`.

- **Signed in?** `claude auth status --json` (10 s limit), its `loggedIn`. Checked when the Agent tab first shows, after install, during sign-in, on Check again, and when a turn reports `authentication_failed`.
- **Install Claude Code** runs `/bin/bash -c 'curl -fsSL https://claude.ai/install.sh | bash'`, the command shown on the card, streaming its output into the log (the last 200 lines); then locates `claude` again and adds `claude --version → 2.1.290 (Claude Code)`. A failed install shows its last lines and the card again. **Copy command** puts that command on the clipboard ("Copied"). **I installed it, check again** locates again, else says "Still not found in ~/.local/bin or on your PATH. If you installed it somewhere else, choose the command." **Choose the claude command…** is an open panel (an executable file), checked with `--version`.
- **Sign in with browser** runs `claude auth login --claudeai` as a child (stdin a pipe, the cleaned environment). It opens the browser itself; the app reads its output for the first `https://` URL and keeps it for **Open the page again** (`NSWorkspace.open`). The app polls `claude auth status --json` every 2 s; `loggedIn: true` ends the child and lands in the chat, with the note "Signed in · Claude Code is ready". **Cancel** ends the child (back to signed out). After 10 minutes the wait ends with "Sign-in timed out", back to signed out. If the child exits without signing in, its last output lines show under the button, with "Or run claude auth login in Terminal, then Check again".
- **Sign out** (the chats list footer, the Agent window's row) asks first: "Sign out of Claude Code? This signs Claude Code out on this Mac, in Terminal too." Then `claude auth logout`, back to signed out.
- The card copy is the prototype's: "Set up the Agent" with the three numbered steps (Install Claude Code, Sign in with your Claude plan, Chat about your art), ticked as they're done.

**For tests** (only under the script hook, `PIXELLAB_SCRIPT`): `PIXELLAB_CLAUDE_HOME` replaces `~` in the search and skips the Homebrew folders and the login shell; `PIXELLAB_CLAUDE_INSTALLER` is run instead of the curl command. The owner's real install is never touched by a script.

### D3. Chats and their storage (R3, R19)

- `<state dir>/chats/<id>.json`, one per chat, written atomically by the app (as `layer-view.json`): `{ "version": 1, "id": "20261021-094112-a1b2c3", "title": "Move these one tile east", "document": "<document root>", "createdAt", "updatedAt", "session": "<Claude session id>"?, "pins": [{ "doc": "scene:harbor", "target": "layer:boats" | "12.0", "object": "pine"? }], "messages": [...] }`.
- Messages: `{ id, time, kind: "user" | "claude" | "step" | "question" | "proposal" | "note", … }`: a user message's `text` and `chips` (labels, with "(not sent)" on left-out ones); Claude's `text`; a step's `tool`, `input`, `ok`, `human`, `raw`, `result` (its first 400 characters); a question's `text`, `options`, `chosen`; a proposal's `proposal` (id), `number`, `doc`, `summary`, `edits`, `outcome` (`applied` | `rejected` | `elsewhere`, once known); a note's `text`.
- **Titles:** "New chat" until the first message, then that message (30 characters, "…" past that). The title goes to the chat's MCP server at each turn (D7).
- **The list:** the open document's chats (by `document`), newest `updatedAt` first. The Agent tab opens on the most recent one; with none, a new chat. New chat (the pencil-square button, also in the chat header) starts one that's saved only once it has a message.
- **Pins:** a pin is stored with its doc and, for an item, its object. Pins for another doc aren't shown. After an edit in the app reorders items (`reordered`, as the selection follows), pins follow. A pinned item whose path now holds another object, or no item, is dropped with the note "Unpinned pine 12.0: it moved or was removed".

### D4. A turn (R10–R18)

1. **Send** (Return, the send button, a question's answer, Tab-then-Return): the draft's text, trimmed, must be non-empty and the setup `ready` and no turn running.
2. The app asks the server for the block: `chat.context` with the open scene's selected and pinned targets (D5). In object mode the block names the object: `<selection document="…" object="house" version="…">(the whole object is in view)</selection>`.
3. The user message goes into the transcript (text, chips), the draft clears, the typing bubble shows.
4. The process starts (D1) with stdin `{"type":"user","message":{"role":"user","content":"<block>\n\n<text>"}}`.
5. Stream lines fill the transcript as they come (D1's table). The view keeps to the bottom while new rows arrive, unless the owner scrolled up.
6. At `result` or exit: the typing bubble goes, the session id and the chat are saved. A turn that made a proposal in the open chat selects it in the Agent model with its ghost on (R17).

### D5. Context: chips, the block and what's unsaved (R9–R11)

**Chips** (`ChatContext`, pure, unit-tested): the chat's pins for the open doc, then the selection's targets not already pinned, each as `{ target, label, sub, pinned, unsaved }`: a layer as its name and "12 items", an item as its object and path. With neither, the "<scene> whole scene" chip. Removing a chip unpins it and takes it out of the selection. The CONTEXT label's right side says "The whole scene", or "3 items · 1 pinned to this chat".

**`chat.context`** (registry, `cli/lib/commands/chat.mjs`; logic in `cli/lib/chat-context.mjs`):

| param | |
| --- | --- |
| `name` | the scene |
| `layers` | layer ids, as the app has them |
| `items` | item paths, as the app has them |
| `pinned` | the targets among those that are pinned (`layer:<id>` or a path), marked in the block |

It reads the scene twice: the working copy (`ctx.doc`, the app's session copy with unsaved edits) and the saved file (`readSource` + `importSource`). It matches each working item to a saved one by its content (object, at, id, class, layer, depth and its wrapper's attributes): the k-th working item with a given content is the k-th saved item with it. So an item that only moved down the list because another was placed before it keeps its saved path, and an item placed, moved or changed since the save has none. A layer is saved when the file has a layer with that id whose items are the same items. Returns:

```js
{
  document: 'harbor', scene: 'harbor', version: '3f2a9c1b',   // the saved file's hash, first 8 hex digits
  text: '<selection …>…</selection>',
  layers: [{ id, name, items: ['12.0', …], saved: true }],
  items: [{ path, savedPath: '14.0' | null, object, at, layer? }],
  unsaved: ['layer:boats', '216.0'],                            // the targets left out
}
```

The block:

```
<selection document="harbor" scene="harbor" version="3f2a9c1b">
  group boats "Boats" items=12.0,13.0,14.0 pinned
  item 45.0 object=pine at=tile 3,-1,1 group=trees
  item 46.0 object=rock at=px 31,6 level 0
  (1 selected item is left out: it isn't saved yet)
</selection>
```

With nothing: `  (nothing selected: the whole scene is in view)`. A group lists at most 60 item paths, then `…and 140 more (scene_items lists them)`. Positions read `tile c,r,l` or `px x,y` (and `level n`), as `scene.items` gives them. The document is its manifest name. On the CLI: `pixellab chat context harbor --items 45.0 --layers boats`.

**In the app:** the block is fetched again (coalesced) when the chips, the doc's dirty state or the items change; the disclosure shows its `text`; a chip whose target is in `unsaved` is dashed gold with "not saved"; the warning reads "pine 216.0 isn't saved yet, so Claude can't see it." with **Save now** (`AppModel.save()`), and the next message's chips carry "(not sent)" for them.

### D6. Picking on the stage (R7, R8)

- Picking is scene layers' (`LayersModel.pick`, `SceneEditor.hitTest`, `HitTester`): render bounds in paint order pick the candidates, topmost first, and each candidate's own art decides, so the click goes to what's drawn under the pointer. No engine change is needed; the app's hit-test uses each object's frame-0 art (a known follow-up of the mac-app record), the same as the prototype's still at time 0.
- **⌥-click** (R8): `stageMouseDown` with ⌥ picks and starts the item drag as now (⌥-drag still moves one item by pixels). On a mouse-up that didn't move, with ⌥ held, the selection becomes every item whose object is the object drawn under the pointer, among those pickable where you are (inside the layer you're in, if any; not hidden; not in a locked layer below where you are), selected as items even where the click would pick their layer: "every use" is about items. ⇧⌥ adds them. VoiceOver hears "Selected 8 pines". **Why it doesn't clash:** ⌥ means "by pixel" only once the pointer moves, and the selection change waits for the mouse-up.
- **Edit › Select Every Use** (no shortcut), enabled with one item selected, does the same for it.
- Arrows, ⇧-arrows, Page Up/Down and Esc are unchanged (the prototype's arrows stepped through items; the app's move the selection, as the intent says).

### D7. Proposals from a chat (R16, R17, R21, R24)

- **The chat on the MCP side.** `serve --mcp` reads `PIXELLAB_CHAT` and `PIXELLAB_CHAT_TITLE` from its environment into the session. A proposal it writes gains `chat: { id, title }`. Each activity line gains `agent` (the MCP client's name and version) and, from a chat, `chat: { id, title }`.
- **Numbers.** `state.writeProposal` gives a proposal a `number` the first time it's written: it appends the id to `proposal-numbers.log` in the state dir (one line per proposal, append-only, so two servers writing at once each get their own line) and the number is that line's position. A rewritten proposal keeps it. `proposal.list` and `proposal.show` return `number` and `chat`; older proposals have neither and show without a number.
- **Cards.** A `doc_save` tool result with `proposal` adds a proposal message `{ proposal, number, doc, edits }`, titled with its one edit or "2 edits to harbor". It goes after Claude's next words (or at the turn's end), so the card follows the reply about it, as in the prototype. Its status: waiting while `proposal.list` has it; `applied` or `rejected` when the app approved or rejected it (from the card, the banner or the Agent window, which tell the chat model); `elsewhere` when it left the list otherwise ("Settled elsewhere"): listed earlier in this launch and gone now, or, for a card from an earlier launch, not in the list at all (settled while Pixel Lab was closed). Approve or Reject on such a card marks it and notes "#3 isn't waiting any more: it was approved or rejected outside Pixel Lab." rather than doing nothing. Card buttons: **Approve** and **Reject** select it in `AgentModel` and run its `approve()` / `reject()` (the same questions and alerts); **Show on stage / Hide on stage** selects it with the ghost on, or turns the ghost off; **Diff in the Agent window** selects it and opens the window. The chat notes "You approved #12 · wrote scenes/harbor.mjs" or "You rejected #12 · nothing changed".
- **Ghosts per chat.** When a turn in the open chat makes a proposal, it's selected with its ghost on. Opening another chat, or the Chats list, turns the ghost off if the selected proposal is from a different chat. The Agent window's own selection works as before.
- **The banner** (`ProposalBanner`): for a chat proposal, "Previewing proposal #12 from the chat “Forest by the house”", the second line "2 edits · <summary>". Otherwise "Previewing proposal #11 from claude-code 2.1.0".
- **Badges:** the toolbar's sparkle and the Dock count every waiting proposal, chats' included, as now.

### D8. Questions (R15, R23): a pixellab tool

**Decision:** a registry command `chat.ask` (MCP tool `chat_ask`), not a text convention. In headless mode the app sees every `tool_use` with its input as structured JSON, so a tool call is an unambiguous, typed question; parsing replies for a question format would break the first time Claude phrases one differently.

| param | |
| --- | --- |
| `question` | one or two sentences |
| `options` | two to four short answers, as button labels |

It returns at once, without waiting for the answer (a turn can't block on the owner): with `PIXELLAB_CHAT` set, `{ asked: true, note: "Pixel Lab shows your question with these answers as buttons. Their choice arrives as their next message. End your turn now without doing more." }`; without (a terminal agent), `{ asked: false, note: "No Pixel Lab chat is attached to this connection: ask in your own conversation." }`. It doesn't mutate. On the CLI it prints the question and its answers. The activity line reads "asked: <question>". The chat prompt (D11) says to use it whenever Claude wants the owner to choose.

The app turns the `tool_use` into a question card; choosing an answer marks it chosen (the buttons go) and sends the answer's text as the next message, with the current context.

### D9. Steps as sentences (R13)

`StepText` (pure, unit-tested) makes each step's sentence from the tool, its input and its result's JSON:

| tool | sentence |
| --- | --- |
| `scene_items` | "Looked through the 89 items in harbor" |
| `scene_render` | "Rendered harbor to check how it looks" |
| `scene_show`, `scene_list`, `object_list`, `document_info`, `palette_get` | "Looked at harbor" / "Listed the 15 scenes" / "Listed the 164 objects" / "Read the document's details" / "Read the palette" |
| `object_show`, `object_render` | "Looked at the pine sprite (17×27)" |
| `scene_move` | "Moved pine 134.0 one tile down-right" (by `1,0` → down-right, `-1,0` up-left, `0,1` down-left, `0,-1` up-right, level `+1` "up a level", otherwise "by 2,0,1"); with `nudge`, "Nudged pine 134.0 by 2 px right" |
| `scene_set` | "Switched pine 153.0 to tree-small", "Set pine 134.0's level to 2", "Gave pine 134.0 the id big-pine" |
| `scene_place` | "Placed a pine at 3,-1,1" |
| `scene_remove` | "Removed pine 134.0" |
| `scene_layer_*` | "Grouped 3 items as Boats", "Moved Boats one tile down-right", … |
| `object_paint`, `object_fill`, `object_resize`, `object_new` | "Painted 3 pixels water-1 in house", "Made pine-tall 17×33", "Made a new object, pine-tall" |
| `doc_save` | "Saved it as proposal #12 for you to review" |
| `check` | "Checked harbor: no problems" / "…: 2 problems" |
| `chat_ask` | (a question card, not a step) |
| anything else | the result's `summary`, else "Used <tool>" |

Item names come from the input's `item` and the result (`object`, `summary`). A failed call reads "Couldn't <verb>: <first line of the error>" and shows a muted warning mark. **Raw** (the Details toggle): `scene_move name=harbor item=134.0 by=1,0,0`: the tool without `mcp__pixellab__`, then `key=value` for each input field (arrays and objects as compact JSON), and `→ proposal #12` after a `doc_save`.

**Folding:** consecutive steps (no message between) are one row: the latest step's sentence, "· 4 steps" when more than one, and a chevron that expands the list. While a turn runs, the row shows the latest step as it comes.

### D10. Links in Claude's replies (R14)

`ChatLinks` (pure, unit-tested) splits a reply into text and links: `<object> <path>` where the open scene has an item at that path using that object (`pine 134.0`), and a layer's exact name where it's at least 3 characters and stands as a word (`Boats`). A link selects that target (`LayersModel.select`, revealing its row) and focuses the stage. Item names in replies follow the prompt's convention (D11).

### D11. The chat prompt

Appended to Claude Code's system prompt (a Swift string in `ChatPrompt.swift`):

- You're the assistant inside Pixel Lab, a Mac editor for isometric pixel art. You can only use the pixellab tools. The owner sees your replies in a small chat column: keep them short, plain and friendly; no headings, tables or code blocks.
- Each message starts with a `<selection …>` block: what the owner selected, from the saved document. "These", "this" and "them" mean it. Paths and versions are from the saved file.
- Name an item as its object and path ("pine 134.0") and a group by its name, so the app can link them.
- Edits stay in memory until `doc_save`, which makes a proposal for the owner to approve; nothing you do changes the document. Edits you don't save are lost when your turn ends, so save before you finish a turn that changed something, and say what you changed.
- When the owner should choose (one or every use, which of two ways), call `chat_ask` with two to four short answers and end your turn.
- Unsaved work in the app is invisible to you; if the block says something was left out, say so.

### D12. The Agent tab (R5, R12, R18, R19, R28)

- **The column.** The inspector column is `RightColumn`: the tab bar (a segmented control: "Inspector", "✦ Agent") pinned at the top, then the Inspector as now or the Agent tab. `AppModel.rightTab` (`inspector` | `agent`), remembered in UserDefaults (not under the script hook). The column is 260–520 pt wide (ideal 340). Showing the Agent tab shows the column.
- **Setup** (not ready): the prototype's card: the 44 pt avatar, "Set up the Agent", "Claude Code on this Mac, signed in with your Claude plan. Its edits come back as proposals.", the three steps, and the state's card (D2).
- **Chats list:** "Chats" (Sora 16) and the new-chat button; rows (36 pt avatar, title, time, preview "You: …" or Claude's last words, the meta line); footer: "All proposals and activity in the Agent window" (opens it) and the status line "Claude Code 2.1.290 · signed in with your plan" with Sign out.
- **Chat:** header: "‹ Chats" with the count, the title and "Claude · harbor", Details (pressed when on), the new-chat button. The conversation (a `ScrollView` with a bottom anchor): the date header ("Today 9:41 AM", "Yesterday", the weekday, else the date), the empty chat's greeting bubble (the prototype's text), the rows, the typing bubble. Then the context bar and the input.
- **Bubbles:** padding 8×13, corner radius 18 with the one square corner (`UnevenRoundedRectangle`), max 82% of the width; mine: accent with `onAccent` text, my chips above in small mono capsules; Claude's: card, `ink` text with links (D10, ink with a 2 pt accent underline); 2 pt between a sender's bubbles, 10 pt between groups. Notes centered, 11 pt muted. Avatars 28 pt.
- **Typing bubble:** three 7 pt dots pulsing in turn (still, at full opacity, with Reduce Motion), VoiceOver "Claude is working".
- **Question card:** the card bubble with the question and its answers (the first an accent capsule, the rest bordered), 30 pt tall; once chosen, the choice in muted text.
- **Proposal card:** bordered card bubble: PROPOSAL #12, status capsule, summary (Sora), edits in mono (8 at most, then "… and 3 more"), buttons (Approve accent, Reject, Show on stage), "Diff in the Agent window".
- **Context bar:** CONTEXT and its count; the chips (28 pt capsules, a pin button and a remove button of 24 pt each), wrapping, at most 96 pt tall then scrolling; the unsaved warning; "What Claude receives" (a `DisclosureGroup` over a mono, selectable text box of at most 140 pt).
- **Input** (`ChatInputView`, an `NSTextView` in a scroll view, so the keys are exact): a rounded 20 pt capsule on the card; 1–5 lines; the placeholder (the suggestion, else "Select something on the stage, then ask Claude", else while running "Claude is working…") drawn muted; a "tab" key cap while a suggestion shows and the input is empty; send (30 pt accent circle with an up arrow, dimmed when empty) or Stop (ink circle with a square) while running. Keys: Return sends, ⇧Return and ⌥Return add a line, Tab takes the suggestion when empty (else moves focus), ⇧Tab moves back, Esc while running stops. VoiceOver: "Message Claude", with the hint "Press Tab to use the suggestion: Raise them a level".
- **Suggestions** (`ChatContext.suggestion`): over the visible, saved selection: all blocks (`block…` objects) → "Raise it a level", "Swap it for a road block", "Move it one tile down-right" (plural forms "Raise them a level", …); all trees (objects named like `pine`, `tree…`, `palm…`) → "Make this taller", "Swap it for another tree", "Move it one tile up-left"; else "What is this?", "Move it one tile down-right", "Make this taller" (plurals "What are these?", "Make these taller"). The first idea not already sent in this chat. A layer counts by its items.

### D13. Menus, toolbar, the Inspector and the Agent window (R5, R6, R20–R22)

- **Window menu:** "Agent" ⌥⌘A shows the Agent tab (and the main window); "Agent Window" ⇧⌥⌘A opens the window. The View menu's "Show Agent" item moves to these two. **Decision on ⌥⌘A:** the tab, as the owner said; the window keeps its own shortcut and is reachable from the Chats list footer, each card's diff link, and the Claude Code row's neighbours.
- **Toolbar:** the sparkle button shows the Agent tab; its badge and help are as now.
- **Inspector:** "Ask Claude about this" (bordered accent button, sparkle icon, 32 pt) under the selection's inspector, in scene mode, when the selection isn't empty.
- **Edit menu:** "Select Every Use" after Select None (D6).
- **Agent window:** the CLAUDE CODE row (D2's status; buttons Open chats → the Agent tab and the main window, Sign out, or Set up → the Agent tab); proposal rows "#12 · harbor" with the source line (the 14 pt asterisk avatar and "Chat · Forest by the house", or "claude-code 2.1.0 · terminal"); the detail's source line and, for a chat, "From the chat “…”, asked about pine 188.0 and pine 189.0. Open the chat" (the chips of the user message before the card; Open the chat opens that chat in the Agent tab); the activity filter All | Chats | Errors (Chats: lines with `chat`), with a "who" column (the chat's title or the agent).

### D14. Look (R12, R28)

- Two new tokens in `Theme.swift`: `claude` `#D97757` (both appearances: the avatar's terracotta, as in the prototype) and `onClaude` `#FFF7F0` (the asterisk). Everything else uses the board 03 tokens: `accent`/`onAccent` for my bubbles, `card`/`ink` for Claude's, `selection` for pressed toggles, `gold` for "not saved" and the banner icon, `border` for outlines.
- **The avatar** is a plain eight-armed asterisk drawn as four strokes (a SwiftUI `Path`), not Anthropic's logo or mascot: their brand guidelines don't grant third-party apps use of the Claude marks, so the intent's fallback holds (Areas of concern).
- Fonts: Sora for titles and the proposal summary, Silkscreen labels (CONTEXT, PROPOSAL #12, CHATS), SF Pro for bubbles and controls, SF Mono for paths, chips' sub-labels, raw steps and the block.

### D15. The script hook (R26)

New actions (in `ScriptRunner.swift` and `CLAUDE.md`):

```
righttab inspector|agent          agentwindow open           askclaude   (the Inspector's button)
setup check|install|signin|open|cancel|signout|copy|choose <path>|wait <state>
chat new|list|open <n>|send [<text…>]|type <text…>|stop|wait|remember   (expect session same|new|none: R3)
chatkey tab|shift+tab|return|shift+return|esc   (a real key event into the input)
details on|off   steps expand <n>   choose <answer n> [<question n>]   link <text…>  (clicks the link with that text in the last reply)
chip pin|unpin|remove <target>    savenow
card approve|reject|ghost|diff [<n>]   (the nth proposal card in the chat, default the last)
altclick <x> <y> [shift]          selectuses
activity all|chats|errors
expect setup checking|missing|installing|signedout|waiting|ready
expect righttab|chats|chattitle|messages|steps|steplabel|chatquestion|chosen|cards|cardstatus|chips|pinned|unsaved|placeholder|draft|draftlines|running|claudeinfo|lastnote|loginurl|details|ghostchat|activityrows|proposalsource|banner <value>
expect lastreply|context|setupmessage|installlog <text>   (contains)
```

(`choose` and `expect chatquestion`, not `answer` and `expect question`: the Agent window's alerts already have those.)

`chat wait` waits for the running turn to end; `setup wait <state>` for a setup state.

Under the script hook, the app finds `claude` with `PIXELLAB_CLAUDE_HOME` (D2): the acceptance pass points it at a temp home holding the fake (or nothing, for the install flow).

### D16. The fake Claude Code (R25)

`scripts/fake-claude/claude` (a Node script, no dependencies) and its turns in `scripts/fake-claude/turns.json`:

- `--version` → `2.1.290 (Claude Code)`.
- `auth status --json` → `{ "loggedIn": … , "authMethod": … }` from `$FAKE_CLAUDE_STATE/auth.json` (default: signed out, so the setup flow is exercised from the start of a fresh home).
- `auth login --claudeai` → prints "Opening your browser to sign in…" and `https://claude.ai/oauth/authorize?fake=1`, waits `FAKE_CLAUDE_LOGIN_MS` (default 800), writes `loggedIn: true`, prints "Login successful.". `auth logout` → writes `loggedIn: false`.
- `-p … --output-format stream-json`: reads the one user message, checks its flags (exits 2 with a message if `--tools ""`, `--strict-mcp-config` or `--permission-prompts none` is missing, so a regression in D1's flags fails the acceptance pass); signed out, writes the captured `authentication_failed` lines. Signed in: writes `system/init` (the session id: the resumed one, or a new UUID), then plays the first turn in `turns.json` whose `match` (a regex) matches the text after the block. Steps: `say` (an assistant text line), `tool` (an assistant `tool_use` line, then the real call through the pixellab MCP server it starts from `--mcp-config`, then a `user`/`tool_result` line with the server's real content), `ask` (a `chat_ask` call), and templates: `{{scene}}`, `{{items}}` (each selected item's saved path, for a step repeated per item), `{{first}}`. `FAKE_CLAUDE_STEP_MS` (default 60) paces the steps, so Stop can be tested. Ends with a `result` line. Turns cover: describe ("what"), move ("move|nudge"), raise ("raise"), taller with a question ("taller"), answer ("only|every"), and a default reply.
- `scripts/fake-claude/install.sh` (the fake installer): prints four lines like the real one and copies the fake to `$HOME/.local/bin/claude` (HOME being the temp home).
- `app/Tests/PixelLabTests/Fixtures/stream-*.jsonl`: the lines captured from the real CLI (D1) and lines from the fake, for the parser tests.

### D17. Docs (R27)

README: an "Agent chat" section (setup, the tab, context and chips, ⌥-click, questions, proposals, chats, Stop) and the shortcuts (⌥⌘A, ⇧⌥⌘A, ⌥-click, Tab, Return, ⇧Return); its Agent window section mentions sources and the Chats filter; the terms note (Areas of concern, first point). `docs/architecture.md`: the chat's process, the state dir's `chats/` and `proposal-numbers.log`, the proposal and activity fields. `docs/testing.md`: the fake, its env vars, the hook's chat actions, `chat.txt`. `app/README.md`, `cli/README.md` (the new files). `CLAUDE.md`'s hook list. The MCP instructions mention `chat_ask`. `npm run docs:cli`.

## Areas of concern

- **Terms of use (policy).** This uses the owner's own Claude Code install, signed in with the owner's own plan, for the owner's own use: the app starts the CLI the way a terminal would and adds no credentials of its own. If Pixel Lab is ever shipped to others, it must require each user's own Claude Code install and login, and Anthropic's current terms (Consumer Terms, Usage Policy, and the Claude Code and Agent SDK guidance on using subscription logins in other products) must be checked again first. The README says so. No release tag is pushed by this change.
- **The sign-in isn't verified live.** The owner's CLI login had expired on 2026-10-21 (outside the desktop app's session), and signing the owner in or out from a script would act on their account, so no signed-in turn of the real CLI was run; the fake plays the documented stream-json shapes and the captured init and auth-failure lines. `claude auth login --claudeai` normally opens the browser and completes through a local callback; if the CLI instead asks for a code pasted into the terminal, the app can't answer it, and the card offers "run claude auth login in Terminal, then Check again" (D2). **The owner should send one real message after merging** (README, "Verify the chat"), and the PR says so.
- **The installer runs a remote script.** "Install Claude Code" runs Anthropic's official `curl … | bash`, shown on the card before the owner clicks, as the intent asks. The app never runs it on its own, and tests never run it (D2).
- **Sign out acts on the whole Mac.** `claude auth logout` signs out the CLI everywhere, so it asks first (D2).
- **Plan usage.** Every message is a Claude Code turn on the owner's plan, and each turn starts its own MCP server; the tests never spend usage (R25).
- **The question transport (the intent's open question)** is a tool, `chat_ask` (D8). It's visible to terminal agents over MCP too, where it answers that no chat is attached.
- **Edits not saved within a turn are lost** (one process per turn, D1). The prompt says to save before ending a turn; a turn that stops early proposes nothing.
- **Chat transcripts on disk.** Chats are kept in the state dir (`chats/`), and Claude Code keeps its own session files under `~/.claude/projects/` for the chats' folder. Neither goes into the repo or the document. Deleting a chat isn't part of this change (a follow-up).
- **A new state-dir field, and a new file.** Proposals gain `number` and `chat`, activity lines `agent` and `chat`; `proposal-numbers.log` is new. Older proposals and lines still list and show; an older Pixel Lab ignores the new fields.
- **The avatar** stays the plain asterisk (D14).
- **The prototype's departures:** arrows move the selection rather than stepping through items (the intent); chips are one per group or item rather than one per item (the intent); the Library note keeps scene layers' wording; the steps' directions name screen directions (down-right) rather than compass points, to match the app's nudge buttons.
- **The canvas.** The prototype canvas's sticky asks for its boards to be updated once the feature ships. The canvas is shared, so that waits for the owner's say-so (asked in the final PR).

## Open questions

- From the intent, how a question reaches the app: a pixellab tool, `chat_ask` (D8).
- From the intent, checking and starting the sign-in without a terminal: `claude auth status --json` and `claude auth login --claudeai`, with a polled status (D2); flagged above.
- From the intent, ⌥⌘A: it shows the Agent tab; ⇧⌥⌘A opens the Agent window (D13).
- From the verification (for the owner): R2 keeps the owner's Claude Code settings, hooks, plugins and other MCP servers out of the chat (`--setting-sources ""`, `--strict-mcp-config`). The owner's own memory files (`~/.claude/CLAUDE.md`, auto-memory) may still load into a chat turn: `--bare` would skip them but also drops the CLI's own login, so it can't be used. As built they're allowed (they're the owner's own notes); say if the chat should be free of them too.
- From the verification: when Claude Code has lost a chat's session (its files under `~/.claude/projects/` cleared), the next turn starts a new session once and says so (D1); `chat-relaunch-3.txt` tests it. Nothing more is planned.
- From the intent's out-of-scope list, carried forward as follow-ups: object-mode context (a pixel region inside a sprite), and Claude seeing unsaved edits. Also new: deleting and renaming chats.
