# Intent: developer docs, published as a site
Author: J. Law Cordova · 2026-10-19 · Status: proposed (the owner asked for the whole change in one PR, so this intent, its spec, its plan and the code are reviewed and accepted together by merging it)

## Problem

Pixel Lab's documentation is a user-facing README, a generated command reference and the design record of the first build. There's nothing written for someone working on the code: how the pieces fit, where things live in the CLI and the app, how to test a change. And what exists can only be read file by file in the repo.

## Proposed outcome

In the owner's words:

> Generate documentation as .md files. Then generate a static site based on the README files that would serve as developer docs. Have it deployed to a GitHub Page.

- Developer documentation, written as Markdown files in the repo.
- A static site generated from the README files (and the other docs), to serve as Pixel Lab's developer docs.
- The site deployed to GitHub Pages.

## Constraints

- One PR, ready for the owner to review, with the task complete.
- The repo's rules hold: no new dependencies without asking, nothing secret or personal published.

## Open questions

- The repo is private, and a GitHub Pages site is public unless the account is on GitHub Enterprise Cloud. Is it fine for the developer docs to be readable by anyone?
