# The port queue (PR 2)

The queue `plan.md` step 7 runs from (Workflow, "The queue"). A unit is **ready** when everything in its Needs is merged into `feat/rust-engine-port`. The main session updates this file in each unit's merge commit, so the branch shows where the port stands.

Status: ready, running, merged, escalated (an S unit taken over by the main session). Tier: S is a Sonnet subagent, O the main session (or an Opus subagent when two O units are ready at once).

A unit's checks: `cargo fmt --check`, `cargo test` for its crate, its ported suite with every test name kept, and for the units that produce outputs (`compile`, `preview`, the commands) `PIXELLAB_GOLDENS=<dir> cargo test --test corpus` with no differences. The goldens come from `node scripts/goldens.mjs --out <dir>`.

| Unit | Node → Rust | Tests ported | Needs | Tier | Status |
|---|---|---|---|---|---|
| U1 | `iso.mjs` → `iso.rs` | iso cases of `engine.test.mjs` | PR 1 | S | merged |
| U2 | `svg.mjs`, `starter.mjs`, `starter-palette.mjs` → `svg.rs`, `starter.rs`, `starter-palette.json` | svg and starter cases | PR 1 | S | merged |
| U3 | `engine.mjs` → `engine.rs` (validate, resolve, `sceneAt`) | validation cases | PR 1 | O | merged |
| U4 | `depth.mjs` → `depth.rs` | `depth.test.mjs` | U1, U3 | O | merged |
| U5 | `motion.mjs` → `motion.rs` | motion cases of `engine.test.mjs` and `depth-motion.test.mjs` | U3 | O | merged |
| U6 | `edit.mjs` → `edit.rs` | edit cases of `engine.test.mjs` | U3 | S | merged |
| U7 | `compile.mjs` → `compile.rs` (budgets, lossless, `git` subprocess) | `compile.test.mjs`, corpus SVGs | U2–U5 | O | merged |
| U8 | `preview.mjs` → `preview.rs` | preview cases of `render.test.mjs`, corpus PNGs | U1, U3 | S | merged |
| U9 | the rest of `engine.test.mjs` | → `tests/engine.rs`, in two units by `describe` block | U3–U6 | S | merged |
| U10 | `errors.mjs`, `argv.mjs`, `params.mjs` | `params.test.mjs` | PR 1 | S | merged |
| U11 | `jsonrpc.mjs` | its cases in `servers.test.mjs` | PR 1 | S | merged |
| U12 | `state.mjs`, `activity-summary.mjs` | `activity-and-proposals.test.mjs` | U10 | S | merged |
| U13 | `document.mjs` → `document.rs` (resolution, manifest, R9's problem) | `document.test.mjs` | U10 | S | merged |
| U14 | `session.mjs`, `open-docs.mjs`, `context.mjs` | `sync.test.mjs` | U3, U6, U12, U13 | O | waiting |
| U15 | `merge.mjs`, `diff.mjs` | `merge.test.mjs` | U3 | O | merged |
| U16 | `watch.mjs` → `watch.rs` (polling, D2) | its cases in `sync.test.mjs` | U13 | S | merged |
| U17 | `scene-ops.mjs` | `scene-ops.test.mjs`, `groups.test.mjs` | U3, U4 | S | waiting |
| U18 | `layer-ops.mjs` | `layers.test.mjs` | U17 | S | waiting |
| U19 | `motion-ops.mjs` | `motion-commands.test.mjs` | U5 | S | merged |
| U20 | `render.mjs` | `render.test.mjs` | U7, U8 | S | waiting |
| U21 | `registry.mjs`, `cli.mjs`, `help.mjs`, `bin/pixellab.mjs` → `registry.rs`, `cli.rs`, `help.rs`, `main.rs` | `help.test.mjs`, `commands.test.mjs` | U10 | S | merged |
| U22 | `commands/scene.mjs`, `scene-edit.mjs` | scene parts of `edit-commands.test.mjs` | U14, U17, U21 | S | waiting |
| U23 | `commands/scene-layers.mjs`, `scene-motion.mjs` |  | U18, U19, U21 | S | waiting |
| U24 | `commands/object.mjs`, `object-edit.mjs` | `map-set.test.mjs`, object parts of `edit-commands.test.mjs` | U14, U21 | S | waiting |
| U25 | `commands/doc.mjs`, `document.mjs`, `proposal.mjs`, `check.mjs`, `art.mjs`, `common.mjs` | `art-commands.test.mjs` | U7, U14, U15, U21 | S | waiting |
| U26 | `commands/chat.mjs`, `chat-context.mjs` | `chat.test.mjs` | U14, U21 | S | waiting |
| U27 | `depth-edit.test.mjs`, the program parts of `depth-motion.test.mjs` | → program tests | U22–U24 | S | waiting |
| U28 | `serve.mjs`, `server-stdio.mjs`, `server-mcp.mjs` | `servers.test.mjs`, transcripts replayed | U11, U14, U16, U21 | O | waiting |
| U29 | `migrate.rs`, `document migrate` (D3, R8) | new tests: every refusal, notes, `colorNotes`, the legacy `source/` folder, the SVG check refusing, `--to`, in place; its output equals the bootstrap's for the corpus apart from `notes` | U7, U13, U25 | O | waiting |

## Signatures settled by the main session

- U3 settles `engine.rs`'s render internals before U4, U5 and U8 start on them.
- U14 settles `context.rs` (`Ctx`) and `session.rs` (`Session`) before the command units (U22–U26) start; their briefs give the final methods.
- U21 settles `registry.rs`, `argv.rs`, `params.rs` and `help.rs`; U22–U26 add their commands to the files in `commands/` only.
