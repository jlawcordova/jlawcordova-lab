# Developer docs

For working on Pixel Lab: the `pixellab` program (the engine, the command line and the two servers, in Rust), and the Mac app. To use it, start with the [README](../README.md) (install, a tour, shortcuts, MCP setup, troubleshooting).

These docs are Markdown in the repo, next to what they describe, and are also published as a site, built by `npm run docs:site` ([Contributing](contributing.md#the-docs-site)).

## Start here

1. [Architecture](architecture.md): the three doors onto one command registry, the document as the source of truth and the engine's home, and how an edit and an agent's proposal travel.
2. [Contributing](contributing.md): intent → spec → plan, branches, the rules, and what a PR pastes.
3. [Testing](testing.md): the Rust suites, the repo tooling's tests, the app's unit tests, the script hook, snapshots and the acceptance pass.

For the two file kinds documents share objects and colors with, see [Libraries and palette files](libraries.md).

## The code

- [The program](../crates/pixellab/README.md) (`crates/pixellab/`, with the engine in `crates/engine/`): building it, the module map, a command's anatomy, adding a command, sessions and undo, errors.
- [The Mac app](../app/README.md) (`app/`): the model, the server connection, the views, the rules the code keeps, adding a feature.
- [Scripts](../scripts/README.md) (`scripts/`): building, installing, packaging the program, snapshots, the acceptance pass, the docs.
- [Example documents](../examples/README.md) (`examples/`): two documents with scene layers to try the app on and to test with.

## Reference

- [Document format](document-format.md): the `.pixellab` folder, format 2: manifest, palette, objects, scenes, ambient loops and actions, canonical form.
- [The site contract](site-contract.md): what a site may rely on in a compiled SVG.
- [Commands, the stdio protocol and MCP](cli.md): generated from the program's registry by `npm run docs:cli`.
- [CLAUDE.md](../CLAUDE.md): the instructions agents work by in this repo.

## The design record

Each change lives in `docs/intents/YYYY-MM-<slug>/`, with its intent, spec and plan:

- [2026-10-mac-app](intents/2026-10-mac-app/plan.md): the first build, Pixel Lab for Mac. Its plan is the architecture and design record, including the look (mood board 03, "Field notebook") and the follow-ups list.
- [2026-10-dev-docs](intents/2026-10-dev-docs/intent.md): these developer docs and their site ([spec](intents/2026-10-dev-docs/spec.md), [plan](intents/2026-10-dev-docs/plan.md)).
- [2026-10-depth-order](intents/2026-10-depth-order/intent.md): scenes draw their items by where they stand ([spec](intents/2026-10-depth-order/spec.md), [plan](intents/2026-10-depth-order/plan.md), [the site's migration notes](intents/2026-10-depth-order/site-notes.md)).
- [2026-10-rust-engine](intents/2026-10-rust-engine/intent.md): the engine and the `pixellab` program in Rust, format 2 (JSON sources), self-contained SVGs and the app with no Node ([spec](intents/2026-10-rust-engine/spec.md), [plan](intents/2026-10-rust-engine/plan.md)).

Screenshots from each build are in `docs/screenshots/`.
