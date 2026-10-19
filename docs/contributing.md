# Contributing

How a change gets from an idea to `main`. Agents follow the same rules, written for them in [CLAUDE.md](../CLAUDE.md).

## How changes flow

Planned work follows Anthropic's [AI-native SDLC playbook](https://claude.com/resources/articles/the-ai-native-sdlc-playbook), as the site repo does. Each stage commits an artifact in the change's folder, `docs/intents/YYYY-MM-<slug>/` (the month the intent was written), and the next stage starts by reading it:

1. **`intent.md`:** what's wanted and why, in the owner's words. Accepted by merging its PR.
2. **`spec.md`:** numbered requirements (R1, R2, …) and the design, with areas of concern. Approved by merging.
3. **`plan.md`:** the files, the order of work (one commit per step), the workflow and reviews, the risks and the proof, written before any code. Implement only against an approved plan; when the work departs from it, update `plan.md` in the same commit.
4. **Code and tests,** then a PR that cites the plan and pastes the verification output. Tests are named after the requirement they prove.
5. **Independent verification,** when the plan calls for it: a fresh session runs the `verify-change` skill on the PR and reports as a PR comment.

If documents disagree, the intent wins over the spec, and the spec over the plan: fix the lower one. The skills that write each stage are in `.claude/skills/` (`write-intent`, `write-spec`, `write-plan`, `verify-change`). The first build, `2026-10-mac-app`, predates them and has only a plan; it stays as the design record.

A small, self-contained fix can go straight to a PR into `main`.

## Branches

- Planned work runs on `feat/<slug>`. Each slice is a branch `feat/<slug>-<slice>` and a PR into `feat/<slug>`; one final PR takes `feat/<slug>` to `main`. A one-slice change may go straight to `main`.
- **Only the owner merges into `main`.** It's what Pixel Lab is installed from. A hook (`.claude/hooks/block-main-merge.sh`) stops agents from doing it.
- A branch cut from `origin/main` tracks it, so push new branches with `git push -u origin <branch>`. Large pushes (screenshots) may need `git -c http.postBuffer=157286400 push`.

## Rules

- **The site checkout is the source of truth** for the art and the engine. Import the engine at runtime; never copy or port it. Never write to the real checkout from tests or scripts: use a temp copy. A change the engine or the art needs is a change in the site repo.
- **Every action is a command** in the registry, so the CLI, the app and agents can all take it. MCP edits become proposals the owner approves; `--direct` stays off by default.
- **No new npm or Swift dependencies** without asking.
- **The app builds with the Command Line Tools:** no `@State`; never `xcode-select -s`.
- **Colors come from the board 03 tokens** in `Theme.swift`. Pixel art is nearest-neighbor at integer zoom.
- **Accessibility:** VoiceOver labels, visible focus, comfortable hit targets and Reduce Motion on anything interactive or animated.
- **No secrets, tokens or personal data** in commits, PRs, comments or docs. The repo is public, and so are the art it edits and the docs site.

## Before you open a PR

Run what applies, from [Testing](testing.md), and paste the output in the PR:

- [ ] `npm test` shows `# fail 0`
- [ ] after a command change: `npm run docs:cli`, and `git status` shows only your edits
- [ ] after an `app/` change: `swift build --package-path app` with no errors, `scripts/test-app.sh` passes
- [ ] after an `app/`, `cli/` or `scripts/acceptance/` change: `scripts/build-app.sh`, then `scripts/acceptance.sh` exits 0
- [ ] UI: a script-hook run with `expect` lines, and snapshots of each changed view in dark and light, looked at
- [ ] docs: `npm run docs:site` builds (no broken links)

Commit messages are imperative, with a body when it helps. When an agent wrote the change, they end with its `Co-Authored-By:` line, and PR bodies with the Claude Code line.

## CI

| Workflow | When | What |
| --- | --- | --- |
| Docs site (`.github/workflows/docs.yml`) | every PR, every push to `main`, by hand | checks `docs/cli.md` is current, tests and builds the docs site; deploys `main` to GitHub Pages |
| Release Pixel Lab (`.github/workflows/release.yml`) | a `v<version>` tag on `main` | tests, builds, packages and smoke-tests the app, then publishes the release |

Releasing is in the README, [Releases](../README.md#development). Don't push a release tag unless the owner asks.

## The docs site

The developer docs site is built from the README files and the Markdown under `docs/`, and published to GitHub Pages from `main`.

```sh
npm run docs:site            # build/docs-site/
open build/docs-site/index.html
```

- **Pages** are the files listed in `PAGES` in `scripts/docs-site/build.mjs`, in their sidebar sections, plus every `.md` under `docs/intents/` (found on their own, newest change first). A file's page is its path with `.md` → `.html`, and a `README.md` is its folder's `index.html`.
- **To add a page,** write the Markdown, add it to `PAGES` with its section and title, link to it from a page that's already there, and run `npm run docs:site`.
- **Links** are written as they work on GitHub: relative `.md` links (with `#anchors`), image paths, and links to source files. The build turns `.md` links into page links, copies images, and points links to files that aren't pages at GitHub. It **fails on a broken link, anchor or image**, naming the file.
- **The Markdown** is the subset the docs use (headings, paragraphs, lists, code fences, block quotes, tables, links, images, emphasis, inline code). Raw HTML is shown as text, and HTML comments are dropped. If something renders wrong, fix the renderer in `markdown.mjs` with a test, not the doc.
- **Deploying:** every push to `main` runs the Docs site workflow, which builds the site and deploys it with `actions/deploy-pages`. The site is served at `https://jlawcordova.com/jlawcordova-lab/` (the user site's custom domain applies to project sites too). Pull requests build it without deploying.
- **Setup:** GitHub Pages is on for the repo with **Settings › Pages › Build and deployment › Source: GitHub Actions** (turned on 2026-10-19). If it's ever turned off, the deploy job fails and the build job still checks every PR.
