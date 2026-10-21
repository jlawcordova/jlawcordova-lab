# Intent: Agent chat
Author: J. Law. Cordova (owner). Status: accepted (asked for directly on 2026-10-21, with the design agreed through a prototype; the owner accepted the intent, spec and plan up front and reviews everything in the final PR).

## Problem
Agents can already edit my art over MCP, but only from a terminal: I start Claude Code somewhere else, describe in words which trees or blocks I mean, and then come back to Pixel Lab to approve what it proposed. Pointing at things is the hard part. In my words:

- "Is it possible to run claude code on the app using my subscription? Then maybe there could just be a chat box in the app where I can talk directly to the agent, and the agent making MCP tools from there. It would make the tool more agent interactive."
- "I could be able to select objects or scenes, and have that be in the chat context so its easier to point at which items to edit."

## Proposed outcome
A chat with Claude inside the Mac app, run on my own Claude subscription through my own Claude Code install, where what I select in the scene goes along with my message. The design is the consolidated prototype canvas "Pixel Lab App Prototype" (https://claude.ai/artifact/MQspJo7hemS5691bzs2Rbz): the `AgentChat.dc.html` board (the main window with the Agent tab beside the Inspector, and its no-cli, signed-out and ready states), `Agent.dc.html` (the Agent window with proposals from chats and from connected agents), `Groups.dc.html` and `GroupsDay.dc.html` (how scene layers pick, go inside, hide and lock; the chat's selection behaves the same way) and `Main.dc.html` (the app map, menus and shortcuts). The prototype fakes Claude's replies with a script; the app matches its look and behaviour, not its code. The older "Agent Chat Prototype" canvas is superseded.

What I asked for, in my words:

- "These tool calls can be more humanized."
- "Both objects and ground and blocks, basically everything in the scene should be selectable."
- "Have this be like an imessage chat. Claude can have the asterisk avatar, or if allowed the Claude mascot."
- "Have a way to handle multiple chats."
- "Include … the flow from having no claude CLI to signing in."
- "Chat bubble can be minimal. No tail. Just the one corner having 0 radius."
- "Remove these badge suggestions. If possible, suggestions can be made in the input placeholder which can apply by pressing tab."
- "The session should not stop 3 times. I only need to check the final PR."

What I decided while prototyping:

**How it runs.** The app runs my installed Claude Code, signed in the way it already is, with only Pixel Lab's tools: no shell and no file edits, no API key, and nothing new to install into the app. A conversation carries on across messages and across app launches.

**Where it lives.** The right column gets two tabs, Inspector and ✦ Agent. The toolbar's sparkle button and Window › Agent (⌥⌘A) open the Agent tab. The Inspector gets an "Ask Claude about this" button that switches to the Agent tab. The Agent window stays as the place to review: proposals, their diff, and the activity. It gains a Claude Code status row (version, signed in, Open chats, Sign out) above Connect, each proposal says where it came from (a chat by its title, or the terminal agent), and the activity log gets a Chats filter. There is still a way to open the Agent window.

**Proposals.** What Claude changes from the chat is a proposal, exactly as an agent's is today: nothing changes the document until I approve. Chat proposals get the same stage banner ("Previewing proposal #n from the chat …"), the same ghosts, the same toolbar badge, and a place in the Agent window's list. In the chat, a proposal card has Approve, Reject, Show/Hide on stage, and a link to its diff in the Agent window.

**Selection as context.** What I can select is what scene layers let me pick, on the stage or in the Layers tree: a click picks a layer first, a double-click goes inside it, Shift-click adds, and hidden or locked layers can't be picked. Inside a group, or with no groups, everything in a scene can be picked: objects, blocks, ground, roads, shadows. A click picks what is drawn under the cursor, not the first box that contains it. The arrow keys and Escape keep working as they do now (arrows move the selection). Option-click selects every use of that object, unless that clashes with the layers' modifiers. What's selected shows as chips above the input, one per group or item; a group's chip shows its name and how many items it holds. A chip can be removed or pinned, and pins belong to a chat. Each message tells Claude exactly what's selected: the document, the scene and its saved version, each group with its items, and each item with its object, position and group; with nothing selected, it says the whole scene is in view. I can open a disclosure under the chips and see exactly what's sent. Things I haven't saved yet are left out, their chip says "not saved", and a warning offers "Save now".

**Chat look.** Bubbles have no tails and one square corner: bottom-right on mine (the accent colour), bottom-left on Claude's (the card colour). Messages group by sender, with "Claude" above the first bubble in a group and the avatar beside the last. The avatar is a plain asterisk on terracotta, not Anthropic's logo or mascot unless their brand guidelines allow it. A typing bubble shows while Claude works, and a date sits at the top of each chat. Item and group names in Claude's replies are links that select them on the stage.

**Tool steps.** Each tool call reads as a plain sentence ("Moved pine 134.0 one tile east"). A run of steps folds into one line showing the latest step and how many there were, which I can expand. A Details toggle shows the raw command under each step.

**Questions.** Claude can ask me a question with answer buttons, for example "pine is used 8 times. Make every pine taller, or only these 2?".

**Multiple chats.** A Chats list shows each chat's title, last message, time, pins and waiting proposals, with New chat. A chat's title comes from its first message. Each chat keeps its own conversation, pins and history. The stage selection is shared across chats; ghosts show only for the chat I'm in.

**Input.** The suggestion lives in the placeholder and fits what's selected ("Raise them a level" for blocks, "Make these taller" for trees, "What are these?" for the rest), with a small tab hint beside it. Tab takes the suggestion only while the input is empty; otherwise Tab moves focus as usual. No suggestion buttons. Enter sends, Shift+Enter adds a line, and Stop interrupts while Claude is working.

**Setup, in the Agent tab** (separate from the app's first-launch setup for the document and Node.js). The app finds Claude Code where it's usually installed and on my shell's PATH, even when opened from the Finder. If it isn't there: "Install Claude Code" (Anthropic's official installer, with the command shown), Copy command, Check again, and Choose the claude command…. Installing shows a live log that ends with the version. Signed out: "Sign in with browser", through Claude Code's own sign-in. While waiting: "Open the page again" and Cancel. Ready lands in the chat, and Sign out goes back to sign-in.

## Affected users and systems
- Me in the main window (the right column, the stage's selection, the Inspector, the toolbar and the Window menu) and in the Agent window.
- My Claude Code install and its sign-in, and my plan's usage.
- Agents connected over MCP from a terminal: their proposals and activity sit next to the chat's, marked by source.
- Proposals and the activity log in the state dir, and the documents the chat proposes changes to.

## Constraints
- This is for my own use of my own Claude Code install. If the app is ever shipped, it requires the user's own install and login, and Anthropic's current terms are checked again before any release.
- Chat edits go through the existing proposal path; direct writes stay off.
- No new npm or Swift dependencies. The app's rules hold: board 03 tokens, no `@State`, VoiceOver labels, visible focus, comfortable hit targets and Reduce Motion support.
- Tests and the acceptance pass never use my plan or the network: they run against a stand-in for Claude Code that plays back recorded replies. They run on temp copies of my document and of the example documents, never on the files themselves.
- The script hook can drive the chat (send, select, pin, switch tabs, approve from a card, and the setup states), there's an acceptance script for it, and snapshots of each new view in dark and light.
- Never merge into main and never push a release tag.
- When the feature ships, the prototype canvas's boards should be updated to match, but only after I say so: the canvas is shared across features.
- Out of scope, unless the spec makes a case for it: object-mode context (a pixel region inside a sprite), and Claude seeing unsaved edits. Both are follow-ups.

## Open questions
- How a question with answer buttons reaches the app while Claude runs headless: as plain text, or as a Pixel Lab tool. The spec decides and flags it.
- How to check whether Claude Code is signed in, and start its sign-in, without an interactive terminal. The spec finds a way and flags it.
- Whether ⌥⌘A still opens the Agent window, and how the window is opened otherwise. The spec decides.
