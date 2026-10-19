---
name: write-intent
description: Capture an idea, ticket or incident for Pixel Lab (the pixellab CLI, its MCP server and the Mac app) as intent.md, the first artifact of the AI-native SDLC (intent.md → spec.md → plan.md → code and tests → PR). Use whenever someone describes a change they want (a feature, a redesign, a fix too big for one PR, a follow-up from the plan's list, a review or an incident) and it has no intent.md yet, or asks to write, draft or capture an intent.
---
# Write an intent

`intent.md` is a proto-spec in the originator's own words: what is wanted, why, and under which constraints. It's human readable, version controlled, and the input the spec stage reads. It is not a design: no files, commands, types or code. The section below draws the line, because drifting into the spec is the most common way an intent goes wrong.

Template and steps follow Stage 1 (Plan) of Anthropic's [AI-native SDLC playbook](https://claude.com/resources/articles/the-ai-native-sdlc-playbook). They match the site repo's `write-intent` skill (`~/Projects/jlawcordova.github.io/.claude/skills/write-intent/`), adapted to this repo.

## Steps

1. **Let the originator describe the problem in their own words.** What they can't do today in the app, the CLI or over MCP, who it affects (the owner at the Mac, an agent), what better looks like, what's out of scope. No formal language is needed.
2. **Brainstorm until the idea is concrete.** Ask the questions an analyst would ask: scope, users, constraints, and what success looks like. Ask about what's wanted, not how to build it. Ask a few at a time, and don't fill gaps with guesses.
3. **Pick its home:** `docs/intents/YYYY-MM-<slug>/intent.md`, where `YYYY-MM` is the current month and `<slug>` is short kebab-case. Check that no existing directory under `docs/intents/` already covers the change.
4. **Write `intent.md` with the template below**, keeping the section names and order exactly. Put what's out of scope under Constraints.
5. **Check the draft for spec leakage** with the test in the next section, and take out anything that fails it.
6. **Show the draft to the originator** and correct anything Claude misunderstood. Don't commit before they've reviewed it.
7. **Check it's safe to commit** against `CLAUDE.md`: no secrets or personal data. The repo is public: the Author line names the owner only.
8. **Commit it on its own branch and open a PR into `main`.** The owner accepts the intent by merging the PR, which is what starts the spec stage (`write-spec`). Closing the PR rejects it. Set `Status: accepted` in the PR before it merges.

An intent can also come from a trigger rather than a person, such as a follow-up listed in a plan, a review finding, a failing check or a bug the owner hit in the app. Use the same template, and put the anomaly and its evidence under Problem.

If what's wanted is really a change to the site's art or engine (`~/Projects/jlawcordova.github.io`), it belongs in the site repo's own `docs/intents/`, written with its `write-intent` skill. An intent here can say it depends on one there.

## Intent, not spec

The intent says what the owner wants and why. The spec decides how. Claude keeps to that line when drafting an intent, when reviewing one, and when folding the owner's answers into one.

**The test:** could the owner have written this sentence without opening the code or designing the solution? If not, it belongs in the spec.

These belong in the spec, not the intent:

- Command names, params, JSON-RPC methods, MCP tool shapes and state-dir files (`scene.place`, "a `level` param").
- Files, Swift types, views, scripts and which repo something lives in (this one or the site).
- Ship order, migration steps and release sequencing.
- Fallbacks, error states and edge-case behavior Claude worked out, rather than the owner asked for.
- Exact numbers and lists the owner didn't give: shortcuts, zoom steps, the members of a set.
- Visual and interaction details beyond the outcome the owner described.

Two rules keep the line in place:

- **Only the owner's decisions go in.** When the owner states something specific, such as "⌘D duplicates" or "keep the last five proposals", record it as they said it, in outcome terms where possible ("placing on a block puts the item on top of it", not "infer `level` from the hit test"). Never add a decision of Claude's own, however sensible. A gap Claude spots in review goes back to the owner as a question, and a design gap is left for the spec.
- **Open questions are about what's wanted.** "Should an agent be able to approve its own proposals?" is an intent question. "Which command returns the proposal diff?" or "Where is the setting stored?" is a spec question: leave it out, because the spec answers design questions anyway. If the owner wants the spec to propose something, a single line saying so is enough.

## Template

```markdown
# Intent: <short name of the change>
Author: <name> (<role>). Status: draft.

## Problem
<What can't be done today, who it affects, and why it matters. In the originator's words.>

## Proposed outcome
<What better looks like, described by its result, not its implementation.>

## Affected users and systems
<Who and what this touches: the owner in the app, agents over the CLI or MCP, scenes and objects, the site checkout, the install.>

## Constraints
<Rules any solution must follow: stack, no new dependencies, the site as source of truth, accessibility, what must not change, and what's out of scope.>

## Open questions
<What isn't decided yet. The spec answers these or carries them forward.>
```

## Example

```markdown
# Intent: Place on top of what's under the pointer
Author: J. Law. Cordova (owner). Status: draft.

## Problem
Placing an object on top of a block means setting the level by hand
first, so building up a scene (a tree on a hill, a crate on a crate)
takes two steps and often lands the item inside the block instead.

## Proposed outcome
With the Place tool, clicking on top of a block puts the new item on
that block, the way it would sit there in the site's lab.

## Affected users and systems
The owner placing items in scene mode. Agents placing over MCP should
get the same result.

## Constraints
Undo stays one step per placement. Pixel placement (⌥-click) is
unchanged. No change to the site's engine or art.
Out of scope: moving existing items by dragging onto a block.

## Open questions
Should the level stepper still win when the owner has set it on purpose?
```
