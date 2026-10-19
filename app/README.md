# The Mac app (`app/`)

Pixel Lab.app is a SwiftPM package: one executable target, `PixelLab` (SwiftUI with AppKit where a Mac editor needs it, Swift 6 language mode, macOS 15+, no external packages), and a Swift Testing target, `PixelLabTests`. It doesn't read or write the art itself: it starts the bundled CLI's `serve --stdio` and does everything through it. How that fits together is in [Architecture](../docs/architecture.md); the look is board 03 in the [design record](../docs/intents/2026-10-mac-app/plan.md#board-03-look-field-notebook).

```sh
swift build --package-path app     # debug build, no errors
scripts/test-app.sh                # the unit tests
scripts/build-app.sh               # build/Pixel Lab.app: release build, icon, bundled CLI, ad-hoc signature
open "build/Pixel Lab.app"
```

It builds with the Xcode Command Line Tools; full Xcode isn't needed. If a tool ever does need it, set `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer` for that command; never `xcode-select -s`.

## Layout

| Folder | What's in it |
| --- | --- |
| `Sources/PixelLab/PixelLabApp.swift` | The `App`: the main window, the Agent window (a single `Window` scene), Settings, the menus (`PixelLabCommands`), and the `AppDelegate` (Save / Don't Save before quitting with unsaved changes, stopping the server). |
| `Sources/PixelLab/Model/` | The state, all `@Observable` classes: `AppModel` (one per app, `AppModel.shared`: the connection, the scene and object lists, the open `DocumentModel`, zoom, grid, notices, save and build), `SceneEditor` (scene tools, selection, keys, drags, place), `ObjectEditor` (paint tools, layers, frames, strokes), `TimelineModel` (play, scrub, the frame cache), `AgentModel` (proposals, activity, the ghost, the Dock badge), `ThumbnailStore` (library thumbnails), plus value types: `HitTesting`, `ObjectPaint` (pixel math, strokes), `ProposalGhost`, `NewDocDraft`, `Settings`. |
| `Sources/PixelLab/Server/` | Talking to `pixellab serve --stdio`: `ServerConnection` (an actor: spawns node, sends requests, matches responses, streams notifications as `ServerEvent`s, restarts a crashed server), `Protocol.swift` (`Codable` shapes of results and notifications), `JSONValue` (any JSON), `Locator` (finds node and the bundled CLI, checks a document or a site checkout; `DocumentLocation` says which the server gets). |
| `Sources/PixelLab/Views/` | The windows and their parts: `MainWindow` (setup or the document window, the toolbar), `SidebarView`, `ItemsTree`, `StageView` (with `StageGeometry`, `StageInput`, `StageOverlay`), `InspectorView`, `SelectionInspector`, `ObjectPanels`, `TimelineView`, `AgentWindow`, `StaleBanner` (a dirty doc changed on disk: Merge, Reload, Keep Mine), `BuildArtSheet`, `NewDocSheet`, `SetupView` (and `SettingsView`). |
| `Sources/PixelLab/Theme/` | `Theme.swift`: the board 03 tokens (night and day), fonts (Sora and Silkscreen if bundled, else SF Pro and SF Mono), `SectionLabel`. |
| `Sources/PixelLab/Support/` | `ScriptRunner` (the test hook), `Snapshotter` (self-snapshots), `PixelImage` (nearest-neighbor scaling), `WindowGuard` (Save / Don't Save on close). |
| `Sources/PixelLab/Resources/` | `AppIcon.png` (from `scripts/make-icon.mjs`), `Fonts/`. |
| `Tests/PixelLabTests/` | `StageTests` (geometry, hit-testing, keys), `ObjectPaintTests` (strokes, pixel colors), `AgentTests` (ghost matching, diffs), `StaleTests` (the changed-on-disk banner). |

## How the app talks to the server

1. `AppModel.bootstrap()` finds node (Settings, else `zsh -lc 'command -v node'`, else Homebrew's paths), the CLI (`Contents/Resources/cli` in the bundle, `PIXELLAB_CLI`, or a `cli/` above the executable for `swift run`) and the document (`PIXELLAB_DOC`, else `PIXELLAB_SITE`, else Settings, else the document in `~/Projects/jlawcordova.github.io`). Anything missing shows `SetupView` with what's wrong and how to fix it, with New… to make a document.
2. It starts `ServerConnection` (`node <cli> serve --stdio --document <path>`, or `--site <checkout>`, with node's folder first on `PATH`), asks for `session.info`, loads the palette, the scene and object lists and the Agent window's lists, and opens the last doc.
3. Commands are calls: `try await connection.call("scene.place", ["name": …, "object": …, "at": …])`, decoded into a `Codable` type from `Protocol.swift` or `JSONValue`. A failed command throws `RPCError`, whose `dataCode` is the server's stable `data.code` (`invalid`, `conflict`, `not_found`, …).
4. Notifications drive the views: `AppModel.handle(_:)` reacts to `doc.changed` (re-render, re-check, dirty state), `files.changed` (reload lists, drop thumbnails, reload the open doc), `doc.stale` (the open doc's file changed under unsaved edits: the banner), `proposals.changed` and `activity.appended` (the Agent window).

The stage renders at scale 1 (`scene.render`, `object.render`) and zooms on screen with `PixelImage`, nearest-neighbor at a whole number of points per art pixel. Item bounds in the render result are what clicks hit-test against.

## Rules the code keeps

- **No `@State`.** The Command Line Tools ship without the SwiftUI macro plugin, so `@State` doesn't build. Keep view state in `AppModel` (or one of its editors). `@Observable`, `@Environment`, `@AppStorage` and `@FocusState` are fine.
- **Colors come from `Theme`.** Never a literal color in a view; every token has a night and a day value.
- **Pixel art is nearest-neighbor at integer zoom,** everywhere, including snapshots.
- **Accessibility:** VoiceOver labels on every control, visible focus, comfortable hit targets on custom controls, and Reduce Motion respected (with it on, the timeline doesn't play; scrubbing still previews).
- **Everything is a command.** If the app needs to do something new to a doc, it's a new command in the registry first (see [the CLI's README](../cli/README.md#adding-a-command)), so agents and the command line get it too.

## Adding a feature

1. If it changes a doc or needs data the app doesn't have, add or extend the command in `cli/` first, with tests, and regenerate `docs/cli.md`.
2. Model the result in `Server/Protocol.swift` (or next to the model that uses it) and call it from the model: `AppModel`, `SceneEditor`, `ObjectEditor` or `AgentModel`. Views call model methods; they don't call the server.
3. Add the view, with colors from `Theme`, labels, focus and a keyboard path. Shortcuts live in `PixelLabCommands` or the stage's key handling (`SceneEditor.handleKey`, `ObjectEditor`).
4. Add script-hook actions or `expect`s to `Support/ScriptRunner.swift` if the feature needs them, and a unit test in `Tests/PixelLabTests/` for any pure logic (geometry, pixel math).
5. Prove it: `swift build`, `scripts/test-app.sh`, `scripts/build-app.sh`, a script-hook run with `expect` lines, snapshots in dark and light, and `scripts/acceptance.sh`. The commands are in [Testing](../docs/testing.md).

## Running it against a test document

Never point a development build at your real document while trying edits: save writes it. Copy it (the [testing guide](../docs/testing.md#the-script-hook) has the recipe) and run:

```sh
PIXELLAB_DOC=/tmp/copy.pixellab PIXELLAB_STATE=/tmp/state "build/Pixel Lab.app/Contents/MacOS/PixelLab"   # or PIXELLAB_SITE=/tmp/site
```

`PIXELLAB_STATE` keeps proposals and the activity log away from the installed app's. Settings (⌘,) change the document and node paths and the appearance for normal use.
