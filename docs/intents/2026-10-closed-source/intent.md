# Intent: Pixel Lab as a closed-source project
Author: J. Law Cordova (owner). Status: draft.

## Problem
In the owner's words:

> Moving the GitHub project, this project, into a closed source project. So you might want to remove the GitHub pages and transfer the help section in-app instead.

Pixel Lab's repository is public today, and its developer docs are published to anyone as a GitHub Pages site (`../2026-10-dev-docs/intent.md`). The repo was made public only so that site could be hosted (the Pages plan didn't support a private repo). The owner wants the project closed source.

## Proposed outcome
- The project is closed source: the repository is private.
- The GitHub Pages docs site is removed.
- Help moves into the app: what a user needs to know is readable inside Pixel Lab instead of on a public site.

## Affected users and systems
- The repository and its visibility.
- The developer docs site and its publishing.
- The owner in the app, who gets a help section.
- The site repo, which depends on Pixel Lab today (and, once the Rust engine lands, runs its program in CI: `../2026-10-rust-engine/intent.md`).
- Releases and how the owner installs Pixel Lab.

## Constraints
- No secrets or personal data anywhere, as now.
- VoiceOver labels, visible focus and Reduce Motion on the help in the app.
- No new dependencies without asking.

## Open questions
- What happens to the developer docs (architecture, contributing, testing): kept as files in the private repo, dropped, or moved into the app's help too?
- Which help belongs in the app: the README's tour and shortcuts, the command reference, MCP setup?
- How does the site repo get Pixel Lab once it's private, for its build and its `pixellab` pin?
- Are the GitHub Releases (the DMG and zip) also private, or published somewhere else?
- Does the history stay as it is, or does the project start fresh?
