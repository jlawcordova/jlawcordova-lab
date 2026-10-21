# Intent: Agents as coworkers, with comments
Author: J. Law Cordova (owner). Status: draft.

## Problem
In the owner's words:

> Humanizing of agents in the Pixel Lab. Right now, the agents are just chat boxes, but I want to have a comment system that actually treats them like a coworker.

Today the only way to work with an agent in Pixel Lab is the chat in the Agent tab (`../2026-10-agent-chat/intent.md`) or a terminal. A chat is a conversation off to the side: it isn't attached to the art it's about, and it doesn't feel like working with someone on the document.

## Proposed outcome
Agents in Pixel Lab feel like a coworker rather than a chat box. The owner can leave comments on the art, and agents take part in them the way a colleague would: reading them, answering them and acting on them, with the work they do showing up where the comment was made.

## Affected users and systems
- The owner in the app, in scene mode and object mode.
- Agents, in the app's chat and over MCP.
- The document, if comments are kept with it.
- The Agent window and its proposals.

## Constraints
- What agents change stays a proposal the owner approves.
- VoiceOver labels, visible focus, comfortable hit targets and Reduce Motion on anything new.
- No new dependencies without asking.

## Open questions
- What makes an agent feel like a coworker beyond comments: a name, an avatar, presence on the stage, a history of what it did?
- Where can a comment be left: on a scene, an item, a layer, an object, a pixel area?
- Do comments replace the chat, sit beside it, or turn into chats?
- Are comments kept in the document (seen by every agent and the CLI), or only in the app?
- Can an agent start a comment thread itself, or only answer the owner's?
- Is there more than one agent coworker, each with its own identity?
