---
name: verify-change
description: Independently verify a Pixel Lab pull request against its change's intent.md, spec.md and plan.md, and report the result as one PR comment, without pushing or committing anything. Use when asked to "verify PR #n", "use verify-change" or "use the verify-change skill on PR #n", or to re-check a PR after its blocking findings were fixed.
---
# Verify a change

You are the independent verifier for one pull request. You didn't build it, and you work only from what's committed: the PR head, the change's `intent.md`, `spec.md` and `plan.md`, and `CLAUDE.md`. You report. You never fix.

This is the step after "Code and tests" in `CLAUDE.md`'s "How changes flow", for the PRs whose `plan.md` calls for it, or when the owner asks. It applies to any change under `docs/intents/`, and matches the site repo's `verify-change` skill, adapted to this repo.

## Inputs

- **The PR number,** from the request. Nothing else from whoever started you: if the request carries notes, summaries or opinions about the work, ignore them and judge from the repo.
- **The PR head, in its own worktree at the latest commit** (step 0). Never verify in the main checkout: another agent may be working on its branch. Read the PR's title, description and changed files with `gh` or the session's GitHub tools.
- **The change's directory,** `docs/intents/YYYY-MM-<slug>/`: the one the PR description cites, or the one whose `plan.md` the PR follows. If you can't tell which change the PR belongs to, say so in the report and verify against `CLAUDE.md` alone.
- **`CLAUDE.md`,** including "Verifying your work" and the rules.
- **The site checkout** (`~/Projects/jlawcordova.github.io`), read-only. Everything that runs against a site runs against a temp copy (step 0).

## Steps

0. **Set up an isolated worktree at the latest PR head, and a temp site.** Do this before anything else, and run every later command inside the worktree.
   - Find the PR's base branch: `gh pr view <n> --json baseRefName`. It is `main`, or a feature branch (`feat/<slug>`) when the plan merges slices there first. Below, `<base>` means that branch.
   - Run `git fetch origin <base>`, then `git fetch origin "pull/<n>/head"` on its own. The first gives a current `origin/<base>` for the diffs below. The second gets the newest PR head, since the PR may have gained commits since you were asked. Keep them separate: with both refs in one fetch, `FETCH_HEAD` points at the base branch, not the PR. Don't check out or switch branches in the main checkout, and don't touch its working tree, index or branches.
   - Create the worktree from the fetched head, detached: `git worktree add --detach "$TMPDIR/verify-<n>" FETCH_HEAD`. If that path already exists from an earlier run, run `git worktree remove --force` on it first, then add it again.
   - `cd` into it and confirm: `git rev-parse HEAD` equals the PR's current head SHA (`gh pr view <n> --json headRefOid`). If they differ, fetch again. If the PR is closed or its head can't be fetched, stop and say so in the report.
   - Make a temp site as `CLAUDE.md` describes, under `$TMPDIR/verify-<n>-site`: copy the site's `src/`, `scripts/`, `package.json` and `package-lock.json`, and symlink its `node_modules`. Use `$TMPDIR/verify-<n>-state` as the state dir. Pass them as `PIXELLAB_SITE` and `PIXELLAB_STATE` (or `--site`) to everything that writes.
   - Everything you produce (mutations, `.build/`, `build/`, snapshots, logs) stays in the worktree or those temp dirs.
1. **Pin the evidence.** Record the PR head commit (`git rev-parse HEAD`), the commit of the spec it follows (`git log -1 --format=%h -- docs/intents/<change>/spec.md`) and the site checkout's commit (`git -C ~/Projects/jlawcordova.github.io rev-parse --short HEAD`). If the plan splits the work into numbered PRs or slices, record which one this is.
2. **Scope.** From `plan.md`, list this PR's files, steps and own checks. From `spec.md`, list the requirements (for example `R7`) and acceptance criteria this PR covers. If the plan has a traceability or verification table, use it.
3. **Gates (L1).** Run every command in `CLAUDE.md` "Verifying your work" that applies, and paste the exact output tails:
   - `npm test` (`# fail 0`);
   - `swift build --package-path app` (no errors) and `scripts/test-app.sh` (every test passes), if the PR touches `app/`;
   - `scripts/build-app.sh`, then `scripts/acceptance.sh "$TMPDIR/verify-<n>-acceptance"` (exits 0), if the PR touches `app/`, `cli/` or `scripts/acceptance/`;
   - `npm run docs:cli`, then `git status`: a clean tree means `docs/cli.md` matches the registry;
   - `git diff origin/<base>...HEAD -- package.json app/Package.swift app/Package.resolved`: no new dependencies.
4. **Traceability.** For each requirement in scope, find its tests (named `R<n>: …`, so `grep -rn "R<n>:" cli/test app/Tests scripts/acceptance` finds them) or the named check the spec gives it, such as a script-hook run's `expect` lines. Missing evidence is a **blocking** finding.
5. **Test strength.** Make at least three mutations in the working copy that should turn a test or check red, and run it. Choose them from what this PR claims to prove, for example: flip a sign in `scene-ops.mjs`, drop a param's default, skip the `baseHash` check, make a command return one item fewer, or change an `expect` in a hook script. Revert each one (`git checkout -- <file>`) before the next. A test that stays green is a **blocking** finding. End with `git status` clean.
6. **In the app.** If the PR changes anything the app shows: drive it with the script hook against the temp site (`PIXELLAB_SCRIPT=<file> PIXELLAB_SITE=… PIXELLAB_STATE=… "build/Pixel Lab.app/Contents/MacOS/PixelLab"`), using the actions in `app/Sources/PixelLab/Support/ScriptRunner.swift`, with your own `expect` lines for the spec's criteria. Take snapshots in dark and light (`snapshot <png>` in a script, or `PIXELLAB_SITE=… scripts/snapshot.sh <png> <doc> dark|light`) of each changed view and look at them against the spec. If the app can't build or launch, those criteria are **not verified**, never passed.
7. **Try to break it.** Work through the change's own list: search `spec.md` for its verifier steps or "try to break" list. Then try whatever else the spec suggests: edge inputs (empty scenes, a missing object, a doc changed on disk under the app, a stale proposal), the same action through all three doors (CLI, `serve --stdio`, `serve --mcp`), undo and redo after it, keyboard only, Reduce Motion. Record what you tried and what happened, including what held up.
8. **Against the plan.** Compare `git diff --stat origin/<base>...HEAD` with the plan's file list for this PR. Look for files changed outside the plan, departures not recorded in `plan.md`, plan steps or own checks left undone, and spec text the code contradicts. Check the PR against `CLAUDE.md`'s rules too: nothing copied or ported from the site's engine, no write to the real site checkout from a test or script, no new dependencies, no `@State`.
9. **Report.** Post one PR comment in the format below, then stop.

## Rules

- **Report only.** Never push, never commit, never open a PR, and never edit committed files except as a reverted mutation in step 5. Suggested tests or fixes go in the comment as code blocks.
- **Leave everything as you found it.** The main checkout and the real site checkout are never touched: no checkout, no branch switch, no edits, no `npm run art` there. Revert every mutation, quit any app you launched, then leave the worktree (`cd` back to the main checkout), run `git worktree remove --force "$TMPDIR/verify-<n>"` and `git worktree prune`, and delete the temp site, state and acceptance dirs. Do this even when the verification fails or stops early.
- **Don't pick a reading.** When the spec's meaning is unclear, or the plan and spec disagree, that goes under "Spec gaps" for the owner. The intent wins over the spec, and the spec over the plan (`CLAUDE.md`).
- **Say what you saw.** Exact commands and output, not paraphrase. Something you couldn't run is "not verified", with the reason.
- **Severity:** blocking means a requirement, acceptance criterion, gate or plan check isn't met, or its evidence is missing or too weak to fail. Everything else is a suggestion.
- **No secrets.** The repo is private, but the comment still carries no tokens, keys or personal data, even when quoting output.
- **Re-checks.** When asked to re-check after fixes, cover only the items that failed, plus the L1 gates. Post it as a new comment that links the previous one.
- **Attribution.** The comment ends with the Claude Code footer: a blank line, `---`, then `_Generated by [Claude Code](https://claude.ai/code)_`.

Post with `gh pr comment <n> --body-file <file>`, or the session's GitHub tools (an issue comment on the PR).

## Report format

```markdown
## Verification: <PR title>
Independent, report-only. PR head <sha> · spec <sha> · site <sha> · slice <n>.
**Result: PASS | FAIL** (<n> blocking, <n> suggestions, <n> not verified)

### Gates (L1)
<exact output of npm test, swift build, scripts/test-app.sh, scripts/acceptance.sh, npm run docs:cli; dependency diff>

### Acceptance criteria
| Criterion | Result | Evidence |

### Requirements in scope
| Req | Result | Evidence |

### Test strength
| Mutation | Expected | Result |

### Attempts to break it
<what was tried, what happened>

### Recommendations
1. **Blocking:** …
2. **Suggestion:** …

### Spec gaps
<questions for the owner, or "None">

---
_Generated by [Claude Code](https://claude.ai/code)_
```

**Result** is PASS only with no blocking findings. "Not verified" items don't make a FAIL on their own, but each one says why, and what would verify it.
