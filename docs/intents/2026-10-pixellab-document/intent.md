# Intent: Pixel Lab owns the art: a `.pixellab` document, and a build that publishes to a site
Author: J. Law Cordova (owner) · 2026-10-19 · Status: proposed (the owner asked for PRs that complete the change up to the build, so this intent, its spec, its plan and the code are reviewed and accepted together by merging)

## Problem

In the owner's words:

> The current tooling, the MCP server, tends to apply changes when approved directly to the site. Is it possible to have this in a custom file instead, like our own `.pixellab` file, so that changes are applied to this file, and the agent applies changes to that file? We then have an option to build, which generates some sort of files that would be usable for the Astro site. And from there it can be immediately usable, or when the site is run it immediately takes effect.

Today an approved proposal is a one-way door into the site checkout's working tree: there is nothing between "I like this proposal" and "the site checkout is now dirty". Git is the only undo, which is fine for one edit and awkward for a session of twenty. Pixel Lab also can't work at all without a checkout of the site, because the engine, the compiler (`npm run art`) and the art all live there, and Pixel Lab imports them from it at run time.

## Proposed outcome

- **A Pixel Lab document.** The art lives in a file of Pixel Lab's own, a `.pixellab` document, that the app opens, edits and saves like any Mac document. An agent's approved changes land in that document, not in a site. The owner can make a new document, open one from anywhere, and **Save As** a copy, with no site checkout on the machine.
- **Agents still propose, the owner still reviews.** Every change an agent makes over MCP is still shown to the owner and approved or rejected by them, as today. Approving writes it into the open document.
- **A build that publishes.** From the document, one action produces the files the Astro site uses, into a folder the owner chooses, so the site picks them up at once (in `astro dev`, on its next reload). The owner can also build without a site at all, into any folder.
- **Pixel Lab is the home of the engine and the compiler.** What the site's `npm run art` does today (validate, compile, check the result is lossless and within budget, write the SVGs, previews, size reports, starters) is done by Pixel Lab, from the lab repo, through its CLI, its MCP server and the app. The site depends on Pixel Lab for its art; Pixel Lab stands alone and can serve another site.
- **Node stays, hidden.** The owner never types `npm`: the app's build button does the work through the CLI it bundles. Node remains a requirement of the app, as it is today for the MCP server.
- **The site's own art editor retires.** The web lab at `/lab/pixel-art/` is replaced by Pixel Lab; the site keeps its committed SVGs and the components that inline them, and the entrance lab, which doesn't use the engine, stays.

## Affected users and systems

- The owner in the app: opening, saving and publishing a document; the setup screen and Settings, which today ask for a site checkout.
- Agents over the CLI and MCP: the same commands, run against a document instead of a site; their proposals; the headless path the site's skills follow (`npm run art -- --check`, `--preview`, `--sizes`, `--new`).
- The engine, the compiler and the PNG previews, which move from the site repo to the lab repo.
- The site repo (`jlawcordova.github.io`): its art sources become a `.pixellab` document kept in the repo next to the SVGs it compiles to; its engine, compiler and web lab go; its tests, skills, agent and docs that name them change. That work is a change in the site repo, with its own intent, which depends on this one.
- The install and the release: what the app bundles.

## Constraints

- The owner reviews every agent change before it reaches the document (intent decision, 2026-10-19).
- Editing and saving a document must work with no site checkout, including Save As (owner, 2026-10-19).
- Hiding Node behind the app is enough; a no-Node app is not a goal now (owner, 2026-10-19).
- The web lab is retired, not kept on a dependency (owner, 2026-10-19).
- The existing art must come through unchanged: the SVGs the site commits today must be byte-identical when built from the document.
- The repo's rules hold: no new npm or Swift dependencies without asking; the app builds with the Command Line Tools; colors from the board 03 tokens; VoiceOver, focus, hit targets and Reduce Motion on anything interactive; nothing secret or personal in either public repo.
- Nothing writes to the real site checkout from tests or scripts.
- Out of scope: a Swift port of the engine; running the engine inside the app without Node; editing more than one document at a time; a format for documents other than this site's kind of art.

## Open questions

- Should the site's art tests, which today load the sources through the engine, keep running in the site repo against Pixel Lab as a development dependency, or move to the lab? (The owner said the site depends on the lab; the spec proposes the split.)
- How does a document say where its build goes, so publishing to the site is one action but a copy saved elsewhere doesn't write into the site? (For the spec.)
