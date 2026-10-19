# Developer docs

For working on Pixel Lab: the `pixellab` CLI and its servers, and the Mac app. To use it, start with the [README](../README.md) (install, a tour, shortcuts, MCP setup, troubleshooting).

These docs are Markdown in the repo, next to what they describe, and are also published as a site, built by `npm run docs:site` ([Contributing](contributing.md#the-docs-site)).

## Start here

1. [Architecture](architecture.md): the three doors onto one command registry, the document as the source of truth and the engine's home, and how an edit and an agent's proposal travel.
2. [Contributing](contributing.md): intent → spec → plan, branches, the rules, and what a PR pastes.
3. [Testing](testing.md): the CLI suites, the app's unit tests, the script hook, snapshots and the acceptance pass.

## The code

- [The CLI and servers](../cli/README.md) (`cli/`): the module map, a command's anatomy, adding a command, sessions and undo, errors.
- [The Mac app](../app/README.md) (`app/`): the model, the server connection, the views, the rules the code keeps, adding a feature.
- [Scripts](../scripts/README.md) (`scripts/`): building, installing, snapshots, the acceptance pass, the docs.

## Reference

- [Commands, the stdio protocol and MCP](cli.md): generated from the registry by `npm run docs:cli`.
- [CLAUDE.md](../CLAUDE.md): the instructions agents work by in this repo.

## The design record

Each change lives in `docs/intents/YYYY-MM-<slug>/`, with its intent, spec and plan:

- [2026-10-mac-app](intents/2026-10-mac-app/plan.md): the first build, Pixel Lab for Mac. Its plan is the architecture and design record, including the look (mood board 03, "Field notebook") and the follow-ups list.
- [2026-10-dev-docs](intents/2026-10-dev-docs/intent.md): these developer docs and their site ([spec](intents/2026-10-dev-docs/spec.md), [plan](intents/2026-10-dev-docs/plan.md)).

Screenshots from each build are in `docs/screenshots/`.
