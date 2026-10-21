# The program (`crates/pixellab/`)

`pixellab` is one native program, written in Rust with three crates (`serde`, `serde_json` and `miniz_oxide`), and it needs nothing installed to run: no Node, no runtime. It is the command line and the two servers built on the same commands: `serve --stdio` for the Mac app and `serve --mcp` for agents. The pixel-art engine it runs (validate, render, depth order, motion, compile, previews, migrate) is the library next door, `crates/engine/`. How it fits with the app and the site is in [Architecture](../../docs/architecture.md); every command, flag, method and error code is in the generated [command reference](../../docs/cli.md).

## Building

The Rust toolchain is pinned in [`rust-toolchain.toml`](../../rust-toolchain.toml); with [rustup](https://rustup.rs) installed, the first `cargo` run in the repo installs it.

```sh
cargo build --release                       # target/release/pixellab
target/release/pixellab --version           # pixellab 0.0.0-dev (a release says its tag)
target/release/pixellab help                # every command, from the registry
target/release/pixellab scene --help        # one group
target/release/pixellab document info --json   # a command, as JSON
cargo test                                  # the Rust suites (engine and program)
cargo fmt --check                           # formatting, which CI checks (rustfmt.toml: 140 columns)
```

`PIXELLAB_VERSION=1.2.3 cargo build --release` builds that version in, which `--version` and the MCP server's server info report; without it the program says `0.0.0-dev`. Releases do this from the tag, and [`scripts/package-program.sh`](../../scripts/package-program.sh) builds a release target's `pixellab-<os>-<arch>.tar.gz` and its `SHA256SUMS` line ([Scripts](../../scripts/README.md#package-programsh)). The Mac app carries a copy at `Contents/Helpers/pixellab`.

## The three doors

| Door | Starts as | For |
| --- | --- | --- |
| The command line | `pixellab <group> <verb> [args] [--json]` | People and scripts. One-shot: load, edit, validate, serialize, write atomically (or print the diff with `--dry-run`). |
| `serve --stdio` | `pixellab serve --stdio [--document <path>]` | The Mac app: newline-delimited JSON-RPC 2.0 on stdin and stdout, one session, edits kept in memory with undo until `doc.save`, notifications when files change. |
| `serve --mcp` | `claude mcp add pixellab -- <path to pixellab> serve --mcp` | Agents (Claude Code and others): the commands as MCP tools (`scene.place` is `scene_place`). `doc_save` writes a proposal for a person to approve in Pixel Lab; `--direct` writes the document instead (off by default). |

The document comes from `--document`, else `PIXELLAB_DOC`, else `~/.config/pixellab/config.json` (`{"document": "…"}`), else `--site <checkout>` / `PIXELLAB_SITE` (the `*.pixellab` under the site's `src/assets/pixel-art/`), else `~/Projects/jlawcordova.github.io`. Commands that need no document, like `document new`, don't look for one.

## The commands

The command list, with each one's params, examples and exit codes, is [docs/cli.md](../../docs/cli.md), generated from the registry: `npm run docs:cli` rewrites it from `pixellab help --markdown`, and nobody edits it by hand. `pixellab help` prints the same index in the terminal, and `pixellab <group> --help` or `pixellab <group> <verb> --help` one group or one command. The groups are `document` (info, new, copy, migrate), `palette`, `scene` (reading, editing, layers, motion, ambient loops and actions, sort), `object` (reading, painting, layers, frames, ambient loops and actions, footprint), `check`, `art` (build, preview, sizes), `doc` (the session's open docs, undo, save, merge), `proposal`, `activity` and `chat`.

## The state dir

Proposals, the activity log and the app's list of open docs live in the state dir: `$PIXELLAB_STATE`, else `~/Library/Application Support/Pixel Lab/`.

```text
proposals/<id>.json    an agent's saved edits, waiting to be approved or rejected
proposal-numbers.log   one proposal id per line, so a number is never reused
activity.jsonl         one line per MCP tool call, for the Agent window
open/<pid>.json        the docs the app has open, for doc.status's unsavedInApp
retired-proposals/     proposals from before format 2, moved here when the dir is opened
```

The app keeps its Agent chats (`chats/`) there too.

## Format 2 and `document migrate`

A document is a folder of JSON: `manifest.json` with `"format": 2`, `palette.json`, `objects/*.json` and `scenes/*.json`, written in a canonical form so a hand edit and a save diff small. The fields are in [Document format](../../docs/document-format.md); what a site may rely on in the compiled SVGs is in [The site contract](../../docs/site-contract.md).

A format-1 document (`.mjs` sources, or a site's legacy `source/` folder) is not edited: every command that needs it says it uses the old format. `pixellab document migrate` converts it in place (`--to <path>.pixellab` writes a copy and leaves the original), reading the `.mjs` files as data (comments become notes; anything that isn't plain data is refused with its file, line and column), and checking that every scene builds the same SVG from the JSON sources before it writes anything. It refuses while Pixel Lab has the document open with unsaved edits, and agents don't get it over MCP. The app offers it as **Migrate…** on its setup screen. `serve --stdio` opens a format-1 document just far enough for this.

## Module map

`crates/pixellab/src/`:

| File | What it does |
| --- | --- |
| `main.rs` | The entry point: calls `cli::main` with the process's streams and environment, and exits with its code. |
| `cli.rs` | The command-line door: argv → command → human text or `--json`, exit codes 0, 1, 2, `--version`. One-shot edits: runs the command in a fresh session, then saves (or prints the diff with `--dry-run`). `main()` takes its streams and env in an `Io`, so tests call it directly. |
| `argv.rs` | Parses the command line against the registry: the longest run of words that names a command, positionals into the command's `args`, every param as a `--kebab-case` flag, the global flags. |
| `params.rs` | `check_params()`: the JSON Schema subset every door checks params with (type, enum, pattern, minimum, maximum, minItems, maxItems, items, required, default). Refuses unknown params; with coercion, turns command-line strings into the declared types. |
| `registry.rs` | The registry: `Registry::new()` checks each definition (`check_definition`), sorts by name, and `call(ctx, name, params)` checks params and runs. `registry()` is the built-in list. |
| `commands/*.rs` | The commands, one file per area: `document` (document.info, document.new, document.copy, palette.get), `migrate` (document.migrate), `scene` (read, and `scene.sort`), `scene_edit` (with groups, depth overrides and levels), `scene_layers`, `scene_motion` (tracks, keys, paths), `ambient` (ambient loops and actions, for scenes and objects), `object` (read), `object_edit` (with `object.map.set` and `object.footprint`), `check`, `art` (art.build, art.preview, art.sizes), `doc` (session commands), `proposal` (proposals, activity.list), `chat` (chat.context, chat.ask). `common.rs` holds shared params and helpers. |
| `context.rs` | `Ctx`: what a command runs against. `ctx.sources()`, `ctx.names(kind)`, `ctx.find(query)`, `ctx.edit(…)`; read once per call, invalidated after an edit. Commands never read the disk themselves. |
| `session.rs` | `Session`: open docs as in-memory working copies with undo and redo, plugged into the `Ctx`, plus `edit()`. Saving: validate, serialize, conflict check by hash, atomic write; in propose mode (MCP) a save writes a proposal. `sync()` settles applied or rejected proposals and reloads clean docs whose files changed; `reload()` and `merge()` are `doc.reload` and `doc.merge`. |
| `merge.rs` | `merge3()`: the three-way line merge `doc.merge` and `proposal.apply` use. |
| `diff.rs` | A unified line diff (Myers), for `--dry-run`, `doc.save` and proposals. |
| `open_docs.rs` | `open/<pid>.json` in the state dir: the app's open docs, for `doc.status`'s `unsavedInApp`. |
| `layer_ops.rs` | Scene layers: a scene's `layers` list and each item's `layer`, finding layers by id or name, chains, the members' common parent, ids from names, and the edits behind `scene.layer.*`. Never reorders items for a layer's sake, so layers never change what's published. |
| `scene_ops.rs`, `sorting.rs` | The scene edits (each names the lab function it follows), and the depth sort every scene edit ends with, which says which paths moved. |
| `motion_ops.rs` | Motion edits: tracks, keys, the loop's length, `off_grid_moves()` (check's warnings) and `motion_path()`. |
| `chat_context.rs` | The Agent chat's selection block: matches the working copy's items to the saved file's by content, so a target is named by its saved path and an unsaved one is left out; the chat an MCP server works for comes from `PIXELLAB_CHAT` and `PIXELLAB_CHAT_TITLE`. |
| `document.rs` | Finds the `.pixellab` document (or the one inside a site checkout), reads and checks its manifest, and loads the art sources. A format-1 document gives the old-format problem. |
| `render.rs` | Scenes and objects to transparent PNGs at an integer scale, with each item's bounds for hit-testing; poses moving items at a time; leaves hidden layers out and renders one layer alone (`hide`, `focus`); the night theme's shadow color. |
| `errors.rs` | `CommandError` (a stable `code`, an exit code, optional `details`; `usage()`, `not_found()` and `document()` make the common ones) and `Error` (a command error, or an internal one). |
| `serve.rs` | `pixellab serve --stdio \| --mcp [--direct]`. |
| `jsonrpc.rs` | Newline-delimited JSON-RPC 2.0 over two streams, shared by both servers; maps errors to JSON-RPC codes. |
| `server_stdio.rs` | The app's server: one session, the commands as methods plus `session.info` and `shutdown`, and the notifications. Its `version()` is the program's: `0.0.0-dev` in a checkout, the release's from a tag. |
| `server_mcp.rs` | The MCP server: `initialize`, `ping`, `tools/list`, `tools/call`; tool names, `LEFT_OUT` (commands agents don't get), the annotations, the activity log (with the client and, from the Agent chat, the chat), and `sync()` before each call. |
| `state.rs` | The state dir: proposals (numbered once, through `proposal-numbers.log`) and `activity.jsonl`; atomic writes. |
| `watch.rs` | Watching for the stdio server: it polls the art sources and the state dir, only statting files. |
| `activity_summary.rs` | A few words per MCP tool call for the activity feed ("listed 15 scenes"). |
| `help.rs` | Help text and usage lines, and the generated reference (`help --markdown` is exactly `docs/cli.md`), from the registry. |

`crates/engine/src/` is the engine, with no I/O beyond reading a document's folder, writing a build's outputs and asking git for the sources at HEAD: `engine` (validate, resolve, render, `scene_at`), `iso`, `depth` (the depth order: boxes, which item is behind, the sort, motion's places), `motion`, `ambient` (ambient loops and actions), `edit`, `starter` and `starter-palette.json`, `svg`, `compile` (rect SVG → paths, the lossless check, budgets, `compile_scene`, the self-contained styles), `preview` (the review PNGs), `format` and `serialize` (reading a format-2 folder and the canonical text), `migrate` (format 1 → 2), and the small parts it carries so the program has no other crates: `jsnum` (JavaScript's number printing and rounding, which keeps output byte for byte what it always was), `png`, `deflate`, `sha256`, `omap`.

## A command

```rust
pub fn commands() -> Vec<Command> {
    let mut info = Command::new(
        "document.info",                       // dotted lowercase words; the MCP tool is document_info
        "Show the document in use and what it holds",
        json!({ "type": "object", "properties": {} }),   // a JSON Schema object
        false,                                 // mutates: true means it edits docs through ctx.edit
        info_run,
    );
    info.args = vec![];                        // params the CLI fills from positionals, in order
    info.format = Some(info_format);           // optional: the command line's human output
    vec![info]
}

fn info_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    // params are already checked and defaulted
    let loaded = ctx.sources()?;
    Ok(json!({ "scenes": loaded.sources.scenes.len() }))   // plain JSON, the same through every door
}

fn info_format(result: &Value, _: &Params) -> String {
    format!("scenes {}", result["scenes"])
}
```

Optional fields on `Command`: `description`, `examples` (command lines for the help and the reference), `writes_png` (the result has a base64 `png`, so the command line takes `--out`), `session` (only makes sense in `pixellab serve`, like `doc.undo`), `writes_direct` (writes files itself, like `art.build`, so `--dry-run` doesn't apply), `needs_document = false` (needs no open document, like `document.new`, so the command line doesn't look for one). `Command` in `registry.rs` is the source.

### Mutating commands

A command that edits a doc sets `mutates` and changes the doc only through `ctx.edit()`:

```rust
ctx.edit(DocKind::Scene, &name, None, |doc, loaded| {
    // change `doc` (a serde_json Value) in place; return an Err(CommandError) to refuse (the doc is restored)
    Ok(Edit::new(result, "pine placed at column 14, row 6, level 1"))
})
```

The `None` is a key that coalesces rapid repeats into one undo step. `ctx.edit` opens the doc if it isn't open, snapshots it for undo, runs the function and returns the result with the edit's summary. An edit that changes nothing isn't an undo step. The door decides what happens next: the command line saves at once (or diffs with `--dry-run`), the stdio server keeps the change in the session until `doc.save`, the MCP server turns `doc_save` into a proposal. So a command never writes the document itself (unless it's `writes_direct`).

## Adding a command

1. Add the definition to the right file in `src/commands/` and to that file's `commands()`. A new area gets a new file, declared in `commands/mod.rs` and listed in `registry()` in `registry.rs`.
2. Use the engine (`crates/engine/`) for anything the art's format decides (validate, serialize, edit, starters, compile, previews): never reimplement it in a command. Scene edits go in `scene_ops.rs`.
3. Return a `CommandError` with a stable `code` for failures a person or agent can act on (`not_found`, `invalid`, `conflict`, …), `CommandError::usage` for bad params beyond the schema.
4. Test it in `crates/pixellab/tests/`, named after the requirement it proves (`R3: …`), against the sample document (`make_document()` in `tests/common/mod.rs`, a temp copy of `fixtures/sample.pixellab`) or, for the owner's art, a copy of the site (`make_site()`). See [Testing](../../docs/testing.md).
5. Run `cargo test`, then `npm run docs:cli` to regenerate [docs/cli.md](../../docs/cli.md); `git status` should show only your edits.
6. It's now on the command line, in the stdio server and in MCP. If agents shouldn't have it, add it to `LEFT_OUT` in `server_mcp.rs` with the reason. If the app needs it, model its result in `app/Sources/PixelLab/Server/Protocol.swift` (or decode it where it's used) and call it through `ServerConnection` (see [the app's README](../../app/README.md#adding-a-feature)).

## Sessions, undo and saving

- A **session** (`Session` in `session.rs`) holds open docs, each with an undo and a redo stack. Its scene edit sorts the scene into depth order before it returns, so an edit and its sort are one undo step; `doc.merge` and `doc.reload` sort what they adopt. A step is a snapshot of the doc before the edit; at most `UNDO_LIMIT` (200) are kept. Edits with the same key within `COALESCE_MS` (1 s) merge into one step: a held arrow key, one paint stroke sent as many `object.paint` calls.
- **Saving** validates with the engine and refuses a doc with problems (`invalid`), serializes with the engine, and refuses (`conflict`) if the file's hash on disk changed since the doc was opened; `force` overrides both. The write is atomic. In propose mode it writes `proposals/<id>.json` to the state dir instead, listing the edits since the doc last matched its file (each undo step has a sequence number; the entry's `baseSeq` marks the last one on disk).
- The stdio server watches the sources by polling: a clean open doc reloads when its file changes; one with unsaved changes stays and is reported `stale`.

## Errors and exit codes

| | CLI exit | JSON-RPC |
| --- | --- | --- |
| ok | 0 | result |
| `CommandError` (ran and failed; `check` found problems) | 1 | -32000, `data.code` = the error's code |
| usage error (bad params, unknown command or flag) | 2 | -32602 (`usage`), or -32601 for an unknown method |
| a bug (an internal error; a panic inside a command is one for that call, and the servers keep their open docs) | 1 | -32603 (`internal`) |

With `--json`, errors go to stderr as `{"error": {"code", "message", "details?"}}`. MCP reports a failed command as a tool result with `isError: true`. An art build over budget is a warning, not an error: the SVGs are written, the result is `ok` and says which scene was over.

## Logs and output

Both servers write nothing but JSON-RPC to stdout; logs go to stderr. The command line writes results to stdout and errors to stderr, and stops quietly when stdout closes early (`| head`).
