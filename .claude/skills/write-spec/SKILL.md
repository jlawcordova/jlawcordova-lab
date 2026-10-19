---
name: write-spec
description: Turn an accepted intent.md into spec.md, the requirements and design spec for a Pixel Lab change, in the AI-native SDLC (intent.md → spec.md → plan.md → code and tests → PR). Use when an intent under docs/intents/ has been accepted (merged) and someone asks for its spec or design, or wants to move an accepted intent forward.
---
# Write a spec

`spec.md` turns an accepted `intent.md` into a requirements and design spec that the build can be planned against, with areas of concern flagged. It records what was decided, next to the intent that records what was asked for.

The steps and the prompt follow Stage 2 (Design) of Anthropic's [AI-native SDLC playbook](https://claude.com/resources/articles/the-ai-native-sdlc-playbook). The playbook gives the prompt but no spec template, so the structure below is derived from what that prompt asks for. It matches the site repo's `write-spec` skill, adapted to this repo.

## Steps

1. **Read the intent.** It must be accepted, meaning merged with `Status: accepted`. If it's still a draft, say so and stop.
2. **Load the standards that apply.** These are this repo's equivalent of the playbook's brand, security, compliance and UX skills:
   - `CLAUDE.md`, including its rules and "Things Claude gets wrong";
   - the architecture and design record, `docs/intents/2026-10-mac-app/plan.md`: the site as source of truth, one command registry with three doors (CLI, `serve --stdio`, `serve --mcp`), agents propose and people approve, and the board 03 "Field notebook" look;
   - the theme tokens in `app/Sources/PixelLab/Theme/Theme.swift`, which every color in the app comes from;
   - the command reference `docs/cli.md` and the user-facing behavior in `README.md` (tour, shortcuts, MCP setup);
   - Mac conventions the record sets: menus and shortcuts, VoiceOver labels, visible focus, Reduce Motion, nearest-neighbor pixel art at integer zoom;
   - for anything touching art rules, the site's standards, read-only: its `CLAUDE.md`, the pixel-art engine spec (`~/Projects/jlawcordova.github.io/docs/intents/2026-10-pixel-art-engine/spec.md`) and its `pixel-art` skill;
   - the house standards set by earlier specs under `docs/intents/`;
   - any skill in `.claude/skills/` that matches the work.
3. **Run the prompt below** against the intent and the codebase.
4. **Write `spec.md` next to `intent.md`** with the structure below. Number requirements `R1`, `R2`, … so tests can name them (`R3: …`) and `verify-change` can trace them.
5. **Review the spec against the idea.** Does it solve the stated Problem? Is every Open question from the intent answered or carried forward? Does every requirement reach all three doors where it should (an app action an agent can't take over MCP is a gap)?
6. **Work through the Areas of concern first.** These are the points an analyst would have escalated. The owner resolves each one before the plan is written.
7. **Commit `spec.md` alongside `intent.md`** and open a PR into `main`. The owner approving and merging the spec is what starts the plan stage (`write-plan`).

The spec must not contradict the intent. If it has to, change the intent first, with the owner, in its own commit.

**Changes the site has to make.** The engine and the art live in the site and are never copied or ported here. If the design needs the site's engine, art or `npm run art` to change, the spec says exactly what, flags it under Areas of concern as an external dependency, and leaves that work to a change in the site repo with its own intent.

## The prompt (from the playbook)

> Read the attached intent.md and produce a requirements and design spec for integrating it into our existing codebase. Apply the skills available to you so the plan conforms to our brand guidelines, security policies and UX standards. Document the spec fully as spec.md, ready to hand to the engineering team. Describe clearly any areas of concern, especially where you cannot satisfy contradicting policies.

## Structure

```markdown
# Spec: <name of the change> (from intent.md <YYYY-MM-DD>)
Status: draft.

## Requirements
<What the change must do, as numbered, checkable statements (R1, R2, …) traced to the intent's Problem and Proposed outcome.>

## Design
<How it fits the existing codebase: commands and their params, the stdio and MCP doors, the session and proposals, the app's model and views, theme, keyboard and menus, copy. Enough detail to plan against, in the standards loaded in step 2.>

## Areas of concern
<Each concern, the standard or policy involved, and what the owner needs to decide. Call out any place where two standards contradict each other, and any change the site has to make first.>

## Open questions
<Each open question from the intent, answered or carried forward, plus any new ones.>
```

Set `Status: approved` in the PR before it merges.
