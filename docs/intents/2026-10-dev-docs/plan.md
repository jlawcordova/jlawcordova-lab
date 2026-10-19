# Plan: developer docs, published as a site (from intent.md 2026-10-19)

## Files that change

- Docs: `docs/README.md` (new), `docs/architecture.md` (new), `docs/contributing.md` (new), `docs/testing.md` (new), `cli/README.md` (new), `app/README.md` (new), `scripts/README.md` (new), `README.md`, `CLAUDE.md`.
- Generator: `scripts/docs-site/build.mjs` (new), `scripts/docs-site/markdown.mjs` (new), `scripts/docs-site/style.css` (new), `scripts/docs-site/site.js` (new), `package.json` (the `docs:site` script).
- Tests: `cli/test/docs-site.test.mjs` (new).
- Deployment: `.github/workflows/docs.yml` (new).
- This change: `docs/intents/2026-10-dev-docs/{intent,spec,plan}.md` (new).

Nothing in `cli/lib/`, `app/` or `scripts/acceptance/` changes, and nothing touches the site checkout.

## Order of work

One slice, straight to a PR into `main`, on `feat/dev-docs`: the main session does it in one sitting, and the owner asked for one PR. One commit per step:

1. The change record: intent, spec and plan.
2. The developer docs (R1, R2): the seven new Markdown files, the README's Developer docs section.
3. The generator and its tests (R3–R7): `scripts/docs-site/`, `npm run docs:site`, `cli/test/docs-site.test.mjs`; fix any broken link the check finds in the docs.
4. The workflow (R8) and the docs about it (R9): `.github/workflows/docs.yml`, the deploy notes in `docs/contributing.md` and `scripts/README.md`, `CLAUDE.md`'s command list.

## Workflow and reviews

The main session runs every step on `feat/dev-docs` and opens the PR into `main`. Owner review: the PR itself, where the owner reads the docs, looks at the site's snapshots (night and day, desktop and phone) and decides the public-site question. Independent review: none; the tests, the link check and the PR's CI run of the build job are the verification.

## Risks

- **The renderer mis-renders a construct the docs use** (a list holding a code block, a table cell with `\|`). Each construct has a test, and the snapshots are checked by eye.
- **Anchors that differ from GitHub's,** which would break links that work on GitHub. The slug rules follow GitHub's, with tests on the anchors the docs already use, and the build's link check covers every anchor.
- **The deploy can't run** until the owner enables Pages (and the plan allows Pages on a private repo). The build job doesn't depend on it, so PRs are checked either way; the PR says what to enable.
- **Docs drift from the code.** The new docs describe structure and flows and link to the source rather than copying it; `docs/cli.md` stays generated and the workflow checks it.

## Proof

- `npm test` shows `# fail 0`, with the new R-numbered tests.
- `npm run docs:site` builds with no broken links; `npm run docs:cli` leaves `git status` clean.
- Snapshots of the built site in the browser: the home page and `docs/cli.html` in night and day, and one page at phone width, looked at before reporting.
- The PR's CI run of the Docs site workflow's build job passes; the deploy runs after merge once Pages is enabled.
- `app/`, `cli/lib/` and `scripts/acceptance/` are unchanged, so `swift build`, `scripts/test-app.sh` and `scripts/acceptance.sh` aren't affected; they are not re-run for this change.
