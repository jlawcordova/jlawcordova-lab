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

- **The document is the source of truth, and the engine lives here** (`crates/engine/`), in Rust. A site publishes its art with the `pixellab` program and the [site contract](site-contract.md). Never write to the real site checkout, or the real document inside it, from tests or scripts: use a temp copy (`make_document()`, `make_site()` in `crates/pixellab/tests/common/mod.rs`). A change to the site's art is a change in the site repo.
- **Every action is a command** in the registry, so the CLI, the app and agents can all take it. MCP edits become proposals the owner approves; `--direct` stays off by default.
- **No new crates, npm packages or Swift dependencies** without asking. The Rust crates are `serde`, `serde_json` (with `preserve_order`) and `miniz_oxide`, and nothing else; Node is repo tooling only, and its one package is `playwright` (a dev dependency, for the states' browser check; `npm ci` installs it and `npx playwright install` the browsers).
- **The app builds with the Command Line Tools:** no `@State`; never `xcode-select -s`.
- **Colors come from the board 03 tokens** in `Theme.swift`. Pixel art is nearest-neighbor at integer zoom.
- **Accessibility:** VoiceOver labels, visible focus, comfortable hit targets and Reduce Motion on anything interactive or animated.
- **No secrets, tokens or personal data** in commits, PRs, comments or docs. The repo is public, and so are the art it edits and the docs site.

## Before you open a PR

Run what applies, from [Testing](testing.md), and paste the output in the PR:

- [ ] after a `crates/` change: `cargo fmt --check` is clean and `cargo test` passes
- [ ] `npm test` shows `# fail 0`
- [ ] after a command change: `npm run docs:cli`, and `git status` shows only your edits
- [ ] after an `app/` change: `swift build --package-path app` with no errors, `scripts/test-app.sh` passes
- [ ] after an `app/`, `crates/` or `scripts/acceptance/` change: `scripts/build-app.sh`, then `scripts/acceptance.sh --only <stages>` exits 0 for the stages that cover it, named in the PR (the full pass is optional)
- [ ] UI: a script-hook run with `expect` lines, and snapshots of each changed view in dark and light, looked at
- [ ] docs: `npm run docs:site` builds (no broken links)

Commit messages are imperative, with a body when it helps. When an agent wrote the change, they end with its `Co-Authored-By:` line, and PR bodies with the Claude Code line.

## Building

The program and the engine are Rust. Install it with [rustup](https://rustup.rs); `rust-toolchain.toml` pins the toolchain (with `rustfmt` and `clippy`), and rustup installs it the first time you run `cargo` in the repo. Then, from the repo root:

```sh
cargo build --release     # target/release/pixellab
cargo test                # the program's tests (see Testing)
cargo fmt                 # 140 columns, per rustfmt.toml
node --version            # 22 or later, for npm test, the docs and the scripts
```

`scripts/build-app.sh` builds the program and bundles it into the app, so the app needs the same toolchain. The CI workflows use `rustup toolchain install` on the pinned file, so a change to the pin is a change to `rust-toolchain.toml` alone.

## Adding a command

Every action is a command, so one command works from the terminal, the app and agents:

1. Write it in a file under `crates/pixellab/src/commands/` (the one for its group, such as `scene_edit.rs`, or a new file): a `Command` with its dotted name, summary, params as a JSON Schema object, whether it mutates, and its `run`.
2. Register it: the file's `commands()` returns its list, and `registry()` in `crates/pixellab/src/registry.rs` extends the registry with each file's list, so a new file needs one line there (and `pub mod` in `commands/mod.rs`). The registry checks every definition when it is built, so a bad name or schema fails the tests.
3. Test it in `crates/pixellab/tests/`, named after the requirement it proves, on a temp copy ([Testing](testing.md#adding-a-test)).
4. Run `npm run docs:cli`. It builds the program and writes `docs/cli.md` from `pixellab help --markdown`; never edit that file by hand.
5. If agents shouldn't have it, add it to `LEFT_OUT` in `crates/pixellab/src/server_mcp.rs` with the reason. If the app needs it, see [the app's README](../app/README.md#adding-a-feature).

## CI

| Workflow | When | What |
| --- | --- | --- |
| Tests (`.github/workflows/test.yml`) | every PR, every push to `main`, by hand | on Linux: `cargo fmt --check`, `cargo test`, `npm test` |
| Docs site (`.github/workflows/docs.yml`) | every PR, every push to `main`, by hand | builds the program, checks `docs/cli.md` is current, tests and builds the docs site; deploys `main` to GitHub Pages |
| Release Pixel Lab (`.github/workflows/release.yml`) | a `v<version>` tag on `main` | tests, builds, packages and smoke-tests the app and the program (macOS and Linux), then publishes the release |

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
