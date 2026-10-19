# Spec: developer docs, published as a site (from intent.md 2026-10-19)
Status: draft (reviewed with its intent, plan and code in one PR, at the owner's request).

## Requirements

**The docs (Markdown in the repo)**

- **R1.** Developer documentation is written as Markdown files in the repo, next to what they describe: a README in each code folder (`cli/`, `app/`, `scripts/`) for its internals, and guides under `docs/` (a developer-docs home, the architecture today, contributing, testing). They describe the code as it is, and link to the design record and the generated command reference rather than repeating them.
- **R2.** The root README points developers to the developer docs and the published site.

**The site**

- **R3.** One command, `npm run docs:site`, builds a static site from the README files and the other docs into `build/docs-site/`: the root README is the home page, each folder's README is that folder's page, and `docs/cli.md`, the guides, `CLAUDE.md` and every change under `docs/intents/` are pages too. It uses Node built-ins only (no new dependencies) and never writes to the docs it reads.
- **R4.** Links work on the site as they do on GitHub: a link to another page's `.md` becomes a link to that page; heading anchors use GitHub's ids, so `#setup-for-claude-code` and `#sceneplace` land where they do on GitHub; a link to a repo file that isn't a page (a source file, a workflow, a folder) goes to that file on GitHub; images are copied next to the pages that use them.
- **R5.** The build fails, naming the file and the link, when a link to a page, an anchor or an image is broken, so a broken doc can't be published.
- **R6.** Every page has the same frame: the site title, a sidebar of every page in sections (Start, Guides, Code, Reference, Design record), the page's own contents (its h2 and h3), and a footer naming its source file with a link to it on GitHub. A search box finds pages and headings by title.
- **R7.** The look is board 03, "Field notebook": the app's tokens (`app/Sources/PixelLab/Theme/Theme.swift`), night and day following the system, with a toggle that remembers the reader's choice. Sora for headings, Silkscreen only for short uppercase labels, a monospace face for code. It works at phone width (16 px gutters, no sideways page scroll; wide tables and code scroll in their own box), keyboard focus is visible, and nothing animates.

**The deployment**

- **R8.** A workflow builds the site on every pull request (without deploying, so a broken doc fails the PR) and on every push to `main`, and deploys the `main` build to GitHub Pages; it can also be run by hand. It checks first that `docs/cli.md` matches the command registry, so the published reference is never stale. It asks for no more permissions than each job needs.
- **R9.** The docs say how to build the site locally, how to add a page, and how the deployment works, including the one-time Pages setting.

## Design

- **Docs.** New: `docs/README.md` (home: reading order, what's where), `docs/architecture.md` (the three doors, an edit's path from the app to disk, an agent's proposal, the state dir, the site as source of truth), `docs/contributing.md` (intent → spec → plan, branches, the checks a PR pastes, conventions), `docs/testing.md` (the CLI suites and their site copy, Swift tests, the script hook, snapshots, the acceptance pass, CI), `cli/README.md` (module map, a command's anatomy, adding a command, sessions, servers, errors), `app/README.md` (the model, the server connection and its events, views, theme, build constraints, adding a feature), `scripts/README.md` (every script). Changed: `README.md` (a Developer docs section), `CLAUDE.md` (the new command).
- **Generator.** `scripts/docs-site/build.mjs` (the page list and nav, link rewriting, the link check, the HTML frame), `scripts/docs-site/markdown.mjs` (a small CommonMark-style renderer for what the docs use: ATX headings, paragraphs, nested ordered and unordered lists, fenced code inside lists, block quotes, GFM tables with `\|`, thematic breaks, HTML comments dropped, inline code, emphasis, links, images, bare-URL autolinks), `scripts/docs-site/style.css` and `scripts/docs-site/site.js` (theme toggle, search, copy buttons on code). Raw HTML in Markdown is escaped, not passed through. Pages land at their source path with `.md` → `.html`, a `README.md` → `index.html`, so every link is relative and the site works under any base path (`/jlawcordova-lab/` on Pages, `file://` locally). Search reads an index of page titles and headings from `assets/search-index.js`, a script rather than JSON, so it also works from `file://`.
- **Workflow.** `.github/workflows/docs.yml`: job `build` (Ubuntu, Node 22: `node scripts/docs-cli.mjs --check`, `node --test cli/test/docs-site.test.mjs`, `npm run docs:site`, `actions/upload-pages-artifact`), job `deploy` on `main` and manual runs only (`actions/deploy-pages`, `pages: write` and `id-token: write`, the `github-pages` environment). `contents: read` otherwise. Pages deploys don't cancel each other.
- **Tests.** `cli/test/docs-site.test.mjs`, run by `npm test`, named after the requirements: the renderer's output for each construct, GitHub's heading ids, link rewriting, a full build into a temp folder with every link checked, and the build refusing a broken link.

## Areas of concern

- **The site is public.** GitHub Pages sites are public on every plan but Enterprise Cloud, and Pages for a private repo needs a paid plan. The docs hold no secrets; `docs/cli.md` names the owner's home folder (`/Users/jlawcordova/…`), the same name as the GitHub account. Resolved 2026-10-19: the owner accepted public docs and made the repo public, since the account's plan doesn't allow Pages on a private repo.
- **It's served under the owner's domain.** The user site (`jlawcordova.github.io`) has the custom domain `jlawcordova.com`, so this project's Pages site is served at `https://jlawcordova.com/jlawcordova-lab/`. The site has no page at that path today; one added later would be shadowed by Pages' project site.
- **One-time setting.** Pages must be enabled for the repo with Source: GitHub Actions (Settings › Pages) before the first deploy; the workflow's `GITHUB_TOKEN` can't enable it. Until then the deploy job fails and the build job still checks every PR. Done 2026-10-19.
- **Fonts from Google Fonts.** Sora and Silkscreen load from `fonts.googleapis.com`, so readers' browsers ask Google for them; without them it falls back to system faces.

## Open questions

- Public docs (from the intent): answered yes; the repo is now public.
