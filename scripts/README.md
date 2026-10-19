# Scripts (`scripts/`)

Everything that builds, installs, checks or documents Pixel Lab, run from the repo root. The shell scripts are zsh and stop on the first error; the `.mjs` scripts use Node built-ins only. Each script's header comment is its full usage.

| Script | What it does |
| --- | --- |
| [`build-app.sh`](#build-appsh) | Builds `build/Pixel Lab.app` |
| [`install.sh`](#installsh) | Builds and installs the app into `~/Applications` |
| [`test-app.sh`](#test-appsh) | Runs the app's unit tests |
| [`snapshot.sh`](#snapshotsh) | Snapshots the app's main window to a PNG, read-only |
| [`acceptance.sh`](#acceptancesh) | The end-to-end acceptance pass, on a temp copy of the site |
| [`docs-cli.mjs`](#docs-climjs) | Regenerates `docs/cli.md` from the command registry |
| [`docs-site/`](#docs-site) | Builds the developer docs site |
| [`make-icon.mjs`](#make-iconmjs) | Renders the app icon with the engine |

## build-app.sh

```sh
scripts/build-app.sh
```

A release build of `app/` (`swift build -c release`), then the bundle from scratch: `Info.plist` (bundle id `com.jlawcordova.pixellab`, the version: `PIXELLAB_VERSION` (the release workflow passes the tag's), else `git describe --tags` (`0.4.0-3-gabc1234`, with `0.4.0` in `Info.plist`), else `package.json`'s `0.0.0-dev`; it's stamped into the bundle's `package.json` too, never the repo's), `AppIcon.icns` made from `AppIcon.png` with `sips` and `iconutil`, the CLI copied into `Contents/Resources/cli` (without its tests), SwiftPM's resource bundle, and an ad-hoc signature (`codesign -s -`). Works with the Command Line Tools.

## install.sh

```sh
scripts/install.sh                 # build, then install into ~/Applications
scripts/install.sh --no-build      # install the existing build
scripts/install.sh --dest <dir>    # install somewhere else (for testing)
```

Copies the app into `~/Applications` (created if missing; never `/Applications`, never sudo), then prints the `claude mcp add` command and a shell alias for the bundled CLI. It doesn't run them. Quit Pixel Lab first when updating.

## test-app.sh

```sh
scripts/test-app.sh
```

`swift test` for `app/`, with the Command Line Tools' Swift Testing macro plugin named explicitly (it isn't on the default plugin path).

## snapshot.sh

```sh
scripts/snapshot.sh <out.png> [doc] [dark|light] [zoom]
scripts/snapshot.sh /tmp/hero.png scene:hero-island dark
```

Starts the app with `PIXELLAB_SNAPSHOT`, which opens the doc, waits until it's rendered and checked and the visible thumbnails are in, draws the main window into a PNG and quits. It only opens and renders, so it's safe against the real checkout, and it needs no screen-recording permission. Uses `build/Pixel Lab.app` if it exists, else the debug build. `PIXELLAB_DOC` (or `PIXELLAB_SITE`) picks the document; `PIXELLAB_SNAPSHOT_SIZE` (default `1440x900`) the window size. A path that isn't a document snapshots the setup screen.

## acceptance.sh

```sh
scripts/acceptance.sh [out-dir]
```

The acceptance pass, end to end on a temp copy of the site (the real checkout is only copied): scene edits, save and publish (six SVGs into the temp site), then object painting and save (`acceptance/scene-object.txt`); an agent over MCP saving two proposals through the bundled CLI (`acceptance/mcp-agent.mjs`); the Agent window ghosting, approving and rejecting them (`acceptance/agent.txt`); a doc with unsaved edits changed on disk by the CLI and merged, reloaded and kept, then another agent's proposal (`acceptance/stale-agent.mjs`) approved into an unsaved copy (`acceptance/stale.txt`); and Save As to a copy, reopening it and publishing from it into a folder (`acceptance/document.txt`). It needs `build/Pixel Lab.app`. Exits 0 only if every step passed, and leaves logs, snapshots and the temp site in `out-dir`.

Its `expect` lines pin the site's art as it was on 2026-10-20 (15 scenes, 164 objects, the house at px -59,59). When the art changes, a failing `expect` means updating the scripts, not the app.

## docs-cli.mjs

```sh
npm run docs:cli                     # rewrite docs/cli.md
node scripts/docs-cli.mjs --check    # exit 1 if it's out of date (the tests and the docs workflow run this)
```

Writes [docs/cli.md](../docs/cli.md) from the registry: the command index, the command-line rules, the stdio protocol, MCP, proposals, and every command's usage, params and examples. Run it after changing a command; never edit `docs/cli.md` by hand.

## docs-site/

```sh
npm run docs:site                          # build/docs-site/
npm run docs:site -- --out /tmp/docs       # somewhere else
open build/docs-site/index.html
```

Builds the developer docs site from the README files and the docs under `docs/` (Node built-ins only): `build.mjs` holds the page list, the navigation and the link check; `markdown.mjs` the Markdown renderer; `style.css` and `site.js` the look and the theme toggle, search and copy buttons. The build fails on a broken link, anchor or image. How pages are added and how the site is deployed is in [Contributing](../docs/contributing.md#the-docs-site).

## make-icon.mjs

```sh
node scripts/make-icon.mjs [--document <path> | --site <path>] [--object house] [--out app/Sources/PixelLab/Resources/AppIcon.png]
```

Renders the app icon with the engine: an object (the house) at a whole pixel scale on a 1024×1024 warm squircle with a faint iso grid. It reads the document and never writes to it. `build-app.sh` turns the PNG into the `.icns`.
