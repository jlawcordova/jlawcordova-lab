---
name: write-plan
description: Write plan.md, the implementation plan for an approved Pixel Lab spec, before any code is written, in the AI-native SDLC (intent.md → spec.md → plan.md → code and tests → PR). Use when a change under docs/intents/ has an approved spec.md and someone asks to plan, start building or implement it. Also use to update plan.md when implementation departs from it.
---
# Write a plan

`plan.md` is the written plan for making the change: the files that change, the branches, the order of the work, the workflow, the risks, and the proof that it works. It is written and approved before any code. Review later checks the diff against it.

Template and steps follow Stage 3 (Build) of Anthropic's [AI-native SDLC playbook](https://claude.com/resources/articles/the-ai-native-sdlc-playbook). They match the site repo's `write-plan` skill, adapted to this repo. Who runs the work follows Anthropic's article on [when to fold subagents into the main agent loop](https://claude.com/resources/articles/building-agents-when-to-fold-subagents-into-your-main-agent-loop): one session, in one context, runs the work from plan to PR (see "The main session runs the work").

## Steps

1. **Work read-only until the plan is approved.** Use Claude Code's plan mode where it's available. Otherwise read the codebase without editing anything.
2. **Read `intent.md` and `spec.md`** in the change's directory, plus `CLAUDE.md`. The spec must be approved. If it isn't, say so and stop.
3. **Draft the plan with the template below.** Name the files that change, the order of the work, and the tests and checks that prove it. Write each step in Order of work as one commit, and group the steps into as few slices as the change allows (see "How many slices"). Lay out the branches and the workflow too (see "Branches" and "Workflow and reviews"): which branch each slice runs on, who runs it (the main session, unless a step has a reason for a subagent), where the owner reviews, and whether any independent review runs.
4. **Interrogate the plan.** What could the change break: the other two doors of the registry, the session's undo, proposals and their `baseHash`, the script hook and acceptance scripts, the real site checkout? Which step is most risky? What other options were considered and not chosen, and why? Does any step hand work to a subagent, and would the main session do it as well in its own context?
5. **Iterate until someone who has never seen the conversation could implement the change from the plan alone.**
6. **Commit the approved plan as `plan.md`** next to `spec.md`, once the owner has approved it. On a main-session branch, it's the branch's first commit.
7. **Implement against the plan,** one commit per step in Order of work, in order, following its Workflow. The session that wrote the plan implements it, so what it learned reading the code carries into the build. Run the steps back to back, and hand over to the owner only at a review the plan names (see "Workflow and reviews"). With a solid plan, implementation is often a single pass.
8. **When the implementation departs from the plan, update `plan.md` in the same commit.** The PR's diff and the committed plan should always match.

## Branches

By default, planned work runs on a **main-session branch**: one branch that every slice merges into, and one final PR from it to `main`.

- **The main-session branch.** Before any code, the main session creates `feat/<slug>` from `main` (`<slug>` is the change's folder name without its date). The main session owns it: it commits the plan there and merges every slice into it.
- **Slices.** A slice is a step or a group of steps that one runner does: the main session by default, or a subagent where "The main session runs the work" allows one. Each slice runs on its own branch, `feat/<slug>-<slice>`, cut from `feat/<slug>`, and becomes a PR into `feat/<slug>`. A subagent that runs in parallel works in its own worktree. Small integration commits by the main session, such as a test that spans every slice, can go straight onto `feat/<slug>`.
- **Merging slices.** The main session merges a slice PR into `feat/<slug>` (`gh pr merge --squash --delete-branch`) once its checks pass, after any review the plan names for that slice. A slice only has to build and pass its checks, not be worth installing on its own.
- **When `main` moves,** merge it into `feat/<slug>` before the next slice starts. A slice merges `feat/<slug>` into its branch before it opens its PR.
- **The final PR.** One PR takes `feat/<slug>` to `main`. Only the owner merges it: `main` is what the owner installs from (`scripts/install.sh`). Agents never merge into `main` (`.claude/hooks/block-main-merge.sh`). A large change usually plans a verifier run against the whole spec on this PR.
- **The exception:** a change that is one slice, run by the main session in one sitting, may skip the main-session branch and go straight to a PR into `main`. The plan says so.

In the plan, Order of work starts with a branch diagram: the main-session branch, each slice's branch and PR, which slices run in parallel, and the final PR. The steps are then grouped under each slice's PR, for example `PR 2: steps 4–6`.

## How many slices

Default to **one slice**, with each step in Order of work as a commit inside it. Commits keep the steps reviewable one at a time, without a PR and a merge for each step.

Split into more than one slice only when the work can't, or shouldn't, be done by one runner in one pass, and say why in the plan. Valid reasons:

- **A foundation the rest builds on:** an early part that later slices need merged before they start.
- **A risky or hard-to-reverse step** (a change to the source format written to the site, the proposals or state-dir format, anything that writes to the site checkout) that should be merged and checked before anything builds on it.
- **An external dependency:** a step waits on something outside the change, such as a change in the site repo, a signing identity or an approval. Put the work that depends on it in its own slice, so the rest isn't blocked, and name the dependency in the plan.
- **Too large to review in one sitting,** where the parts are separable.

Parallel work is not a reason on its own: the main session runs independent steps one after another, in one context (see "The main session runs the work"). More slices cost a PR and a merge each. Keep the count as low as the reasons allow: a handful of slices, each holding several commits, not one per step. Number the PRs in Order of work.

## The main session runs the work

By default, the main session that wrote the plan runs every slice itself, in one context, from the first commit to the final PR. Context is lost at every hand-off: a subagent starts cold, works from a brief, and returns a summary, so why the plan chose one option over another, the quirk found in step 2 and the check that was flaky stay behind. In one context, each step builds on the last step's raw results, and the session that revises the plan is the one running it.

Instead of a subagent:

- **Batch independent tool calls in one turn:** reads and searches across `crates/` and `app/`, or `cargo test` alongside `swift build`, rather than fanning the work out to agents.
- **Load a skill** when a step needs a procedure (`write-spec`, `verify-change`), rather than a specialist agent for it.
- **Keep hard limits in tools and hooks,** not in a separate agent: the hook that blocks merges into `main`, the script hook that refuses the real checkout, the tests.

Plan a subagent only for one of these reasons, and name the reason in Workflow:

- **Context bloat:** a read-only sweep whose results the rest of the work doesn't need, only its conclusion, such as a search across many files or reading a long log or another repo's history. The subagent reads and reports; the main session decides and edits.
- **Fresh eyes on purpose:** an adversarial review or a `verify-change` run, where not sharing the implementer's context is the point.
- **Truly independent work in parallel:** parts that share no files, no command, params or protocol, and no output (neither reads what the other makes), each large enough that running them one after the other costs a long wait. This is rare here: the program and the app usually share a command, so one runner builds both. Don't split by area (`crates/` for one agent, `app/` for another) because the code lives in different places. Parts that are tightly coupled, where one uses what the other makes, they touch the same files or they're tested together, stay with one runner.

When a subagent runs:

- **Call it like a tool.** The main session keeps the conversation with the owner, the branch and the merges. It gives the subagent a self-contained brief (the files, the plan's section by path, what done looks like) and checks what comes back itself: the diff and the test output, not only the summary.
- **Model:** subagents that build (code, tests, PRs) run on Sonnet; subagents that define or review (brainstorming, specs, plans, an adversarial review) run on Opus.
- **If one context turns out to be enough,** do the rest in the main session and note it under the plan's departures.

## Workflow and reviews

The plan decides how the work runs, so the implementer doesn't stop to ask. Its Workflow section says:

- **Who runs each slice, and on which branch:** the main session, by default all of it. Any subagent is named with its reason from "The main session runs the work", its model, what it's given and what it returns. A table of the run, step by step, with who, where and what happens next, is the clearest form.
- **The merge rules:** who merges each slice into `feat/<slug>` and when, what happens when `main` moves, and who resolves a conflict between slices.
- **File ownership** when a subagent builds in parallel with other work: which files each slice may edit. A slice that needs a file another slice owns, or a shared file from an earlier slice, stops and reports instead of editing it. Shared files to watch: `crates/pixellab/src/registry.rs`, `docs/cli.md` (generated), `app/Sources/PixelLab/Support/ScriptRunner.swift`, `app/Sources/PixelLab/Model/AppModel.swift`.
- **Owner reviews:** the steps that pause for the owner, what they look at (snapshots, an installed build to try, a proposal in the Agent window), and what happens with their feedback. Plan one only where the owner's judgment is the point: a first design that sets the look for the rest, a choice between options, or a feel that can't be checked by a script. Everything else runs without a pause.
- **Independent review:** none by default. The implementer's own tests and checks, pasted into the PR, are the verification. Plan a `verify-change` run, or another adversarial subagent review, only where a second pair of eyes earns its cost: a change to what's written to the site, the proposals and approval path, the MCP server's surface, a change other work builds on, or the final PR of a large change. Name the PR or step it runs on.

When implementing, follow it as written:

- Don't pause between steps, between slices or before opening a PR to ask whether to go on. Merging a PR the owner must merge (anything into `main`) is still theirs, and so is anything the plan leaves to them.
- Stop early only when you're blocked: the plan doesn't cover what you found, a slice needs a file it doesn't own, a risk the plan names comes true and it says to stop (for example a test that should pass doesn't), a write that would touch the real site checkout, or an action outside the plan that's hard to reverse or outward-facing. Say what blocked you and what you need.
- Run `verify-change` or an adversarial review only where the plan names one, or when the owner asks.

## Template

```markdown
# Plan: <name of the change> (from intent.md <YYYY-MM-DD>)

## Files that change
<Each file, marked (new) or (deleted) where it applies, grouped by slice when there is more than one.>

## Order of work
<Branch diagram: the main-session branch feat/<slug>, each slice's branch and PR into it, which slices run in parallel, and the final PR to main. Or "One slice, straight to main", with the reason.>
<One commit per step, grouped under each slice's PR.>
1. <First step.>
2. <Next step.>

## Workflow and reviews
<Who runs each slice and on which branch (the main session by default; any subagent with its reason, model, brief and what it returns); the merge rules; file ownership if a subagent builds in parallel. The steps that pause for the owner's review, what they review and how feedback comes back; "none" if nothing does. Independent review: "none", or which PR or step gets verify-change or an adversarial subagent, and why.>

## Risks
<What could break, the riskiest step, and how each risk is handled.>

## Proof
<The checks that show it works, written so they can be run: npm test, swift build, scripts/test-app.sh, scripts/build-app.sh, scripts/acceptance.sh, script-hook runs with their expect lines, npm run docs:cli for command changes, snapshots in dark and light against the spec.>
```

## Example

```markdown
# Plan: place on top of what's under the pointer (from intent.md 2026-10-20)

## Files that change
app/Sources/PixelLab/Model/HitTesting.swift, app/Sources/PixelLab/Model/SceneEditor.swift,
app/Sources/PixelLab/Support/ScriptRunner.swift, app/Tests/PixelLabTests/StageTests.swift,
crates/pixellab/src/scene_ops.rs, crates/pixellab/src/commands/scene_edit.rs, crates/pixellab/tests/scene_commands.rs,
docs/cli.md (generated)

## Order of work
One slice, straight to a PR into main: four steps the main session does in one sitting, so
there's no main-session branch.
1. Add `scene.place --on-top` in scene-ops and its command, with R1–R3 tests; npm run docs:cli.
2. Hit-test the topmost block under the pointer and pass its top level when placing.
3. Script hook: `expect at` after placing on a block; add it to StageTests.
4. Snapshots in dark and light of a tree placed on a hill.

## Workflow and reviews
The main session runs all four steps on feat/place-on-top. Owner review: none; the script hook
and tests check the levels. Independent review: none.

## Risks
Items with art in frames other than frame 0 can miss the hit test (a known follow-up); the
pointer then falls back to the stepper's level, as today.

## Proof
npm test shows # fail 0, scripts/test-app.sh passes, scripts/build-app.sh builds, the script
hook run ends "all steps passed", and scripts/acceptance.sh passes unchanged.
```

A change with several slices starts its Order of work with a diagram like this:

```
main ── feat/<slug> ──────────────────────────────── PR 3 (owner merges) ──▶ main
          ├─ PR 1  command, CLI + MCP  ◀─ feat/<slug>-program
          └─ PR 2  app                 ◀─ feat/<slug>-app      (from PR 1's merge)
```

Both slices are the main session's, one after the other: PR 1 is the foundation the app builds on, and the split keeps each review to one sitting.
