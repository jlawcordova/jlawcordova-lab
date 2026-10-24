# Plan: Object states and the animation pane (from intent.md 2026-10-24)

This plan builds [`spec.md`](spec.md), R1–R40 and D1–D8, as the owner approved it on 2026-10-24 with every Area of concern settled. Status: approved (the owner, 2026-10-24), with the boat's states made from an ambient loop the examples script gives it first (D7). [`animation-pane.md`](animation-pane.md) and the unmerged branch `feat/animation-pane` (`jlawcordova/jlawcordova-lab#17`) are read for how the scene pane worked. Their code isn't reused: it was written against the Node CLI and the window from before the scene canvas.

## Files that change

**Slice 1: the program** (`feat/object-states-program`)
- `crates/engine/src/states.rs` (new), D2:
  - `States` (parsed once per sprite), `run`, `at`, `changes` and `validate`.
  - The CSS for an item whose object has states, called from `motion::scene_css`.
- `crates/engine/src/lib.rs`: `pub mod states`.
- `crates/engine/src/engine.rs`:
  - `start` and `states` join `OBJECT_PROPS`. `validate_object_shape` calls `states::validate` (R7). R2's lockstep check, R8's warning.
  - `Resolved` gains `states: Option<Rc<States>>`, inherited through `extends` (R5). A sprite that `extends` and has its own `states` is a problem.
  - `draw` draws a stated sprite's frame columns as frame groups without the loop class (see "Where the spec leaves a choice", 1).
  - `Group` gains `states: Option<Rc<ItemStates>>`: the parsed states, the item's start state and, for a keyed track, its state changes.
  - `composite` asks `states::at` for the frame and the move when a group has `states` and a time is given. Otherwise it takes the `loop_frame` path, unchanged (R3).
  - `validate_items` checks an item's `state` (R7).
- `crates/engine/src/motion.rs`:
  - `KEY_PROPS` gains `state`. `validate_motion` checks a key's `state` against the object shown at that key (R7, R15).
  - `Filled` gains `state` and the time the state was entered.
  - `scene_css` takes the stated elements and appends their rules and their reduced-motion rules (R10–R14, R16).
- `crates/engine/src/ambient.rs`: `frames()` and `runs()` become `pub(crate)`, so `states.rs` steps moves exactly as actions do (R1).
- `crates/engine/src/serialize.rs`: `SPRITE` gets `start` and `states` after `layers`. `ITEM` gets `state` after `depth`. `KEY` gets `state`. There are new orders for a state (`frames`, `ms`, `loop`, `then`, `move`) and a move key.
- `crates/engine/src/format.rs`: the canonical shapes for `states` (one line per state, as `actions` is) and the key's `state`.
- `crates/engine/src/compile.rs`: `art sizes` gets a `states` line (D2, "Byte size"). The budgets are unchanged.
- `crates/engine/tests/states.rs` (new): `run`, `at` and `changes` for a loop, a hold, a `then` chain, a cycle and moves (R6). Validation for each R7 case.
- `crates/engine/tests/states_css.rs` (new): the CSS for each kind against fixtures, and R3's byte identity.
- `fixtures/states.pixellab/` (new): a small format-2 document with one sprite per kind:
  - a loop with a move, a hold, a `then` into a loop, and a cycle of two one-shots;
  - an item with its own `state`;
  - a scene whose track keys `state`;
  - a sprite without states beside them, for R3.
- `crates/pixellab/src/states_ops.rs` (new): the state edits as pure functions over a doc. That covers add, set (with rename), delete, duplicate, move, the frame edits, the move keys, `make` (R8), and remapping states when frames change (R30). Renames across scenes return the scene edits for the command to apply.
- `crates/pixellab/src/commands/object_states.rs` (new): D3's `object.state.*` commands, `object.states.make` and `object.state.run`.
- `crates/pixellab/src/commands/object_edit.rs`: on a sprite with states, `object.frame.add|duplicate|delete|move` act on the frame column of every frame-loop layer and remap the states (R2, R30). `object.animate` makes the new loop as long as the others. `object.still` follows R30's refusal. The results add `states`.
- `crates/pixellab/src/commands/object.rs`: `object.show` and `object.list` report states (D3). `object.render` gets `--state` and `--time`, and the items report `state` and `frame`.
- `crates/pixellab/src/commands/scene_motion.rs` and `crates/pixellab/src/motion_ops.rs`:
  - `scene.key.set` gets `--state <name>|clear`. With no values, it pins the item.
  - `scene.track.add` gives an item without an id one (R23, D3).
- `crates/pixellab/src/commands/scene_edit.rs`: `scene.set --state <name>|start`.
- `crates/pixellab/src/commands/scene.rs`: `scene.items` adds `state`.
- `crates/pixellab/src/render.rs`:
  - `render_object_image` takes `state` and the time since entering.
  - `render_scene_image` places states at `--time`, for keyed tracks and start states alike.
  - The items report `state` and `frame`.
- `crates/pixellab/src/commands/check.rs`: R7's problems and the R8 and R17 warnings.
- `crates/pixellab/src/commands/mod.rs`, `crates/pixellab/src/registry.rs`, `crates/pixellab/src/lib.rs`: the new modules.
- `crates/pixellab/src/server_mcp.rs`: the States paragraph in the instructions (D3).
- `crates/pixellab/tests/state_commands.rs` (new): every D3 command through the CLI and `serve --stdio`, and renames across scenes as one version. Also R30's remapping and refusals, R8's conversion and its three refusals (on a `character`-shaped copy), and the MCP tool list in `servers.rs`.
- `crates/pixellab/tests/state_render.rs` (new): R16's baked track, matched frame for frame against `scene render --time` at every 100 ms of the fixture's loop.
- `scripts/test/states-css.test.mjs` (new): the string check of the compiled CSS (D6).
- `scripts/test/states-browser.test.mjs` (new): the real-browser check in WebKit, Chromium and Firefox (D6).
- `package.json`: `playwright` as an exact-pinned devDependency. `package-lock.json` (new).
- `.github/workflows/test.yml` and `.github/workflows/release.yml`: `npm ci` and `npx playwright install` before `npm test`, with `--with-deps` on Linux.
- `scripts/make-examples.mjs`, `examples/harbor.pixellab/*`, `examples/town.pixellab/*` (D7):
  - The harbor's `lamp` gets a dim frame, then `idle` and `flicker`.
  - The harbor's `boat` object gets a one-pixel rock (`object ambient`), made into `idle` by `object states make`.
  - The town gets a track with a `state` key that switches a lamp to `flicker` once a loop.
- `docs/cli.md`: generated.

**Slice 2: poses become states** (`feat/object-states-poses`)
- `crates/pixellab/src/poses.rs` (new), D8:
  - The family, the merge (flatten, anchor, union box, color keys, renames) and its refusals.
  - The key rewrite and `--fold`.
  - The pixel check at every 100 ms.
- `crates/pixellab/src/commands/object_poses.rs` (new): `object.states.from-poses`.
- `crates/pixellab/src/commands/mod.rs`, `crates/pixellab/src/registry.rs`, `crates/pixellab/src/lib.rs`: the module.
- `crates/pixellab/src/server_mcp.rs`: one sentence on folding poses.
- `crates/pixellab/tests/poses.rs` (new): R37–R40 on a temp copy of a pose fixture. That covers the merge, the refusals, the key rewrite, `--fold` on an alternating signal, `--delete-unused`, one version, and the identical renders.
- `fixtures/poses.pixellab/` (new): a crane-hook-shaped family (three poses, one with two map layers) and a crossing-signal-shaped one (18 alternating keys).
- `docs/cli.md`: generated.

**Slice 3: the pane in scene mode** (`feat/object-states-scene-pane`)
- `app/Sources/PixelLab/Server/Protocol.swift`:
  - `MotionKey { t, move?, show?, pose?, state? }`, and `MotionTrack.keys`.
  - The `state` on items, `StateRun` (`segments`, `settle`, `changes`), and the state summaries in `object.show`.
- `app/Sources/PixelLab/Model/TimelineModel.swift`: D4's key model and `stateSections(for:)`, with runs cached by object and version.
- `app/Sources/PixelLab/Model/AnimationPaneLogic.swift` (new): pure functions for hit-testing keys and sections, snapping (100 ms, 10 ms, frame boundaries), inherited values and sections. The tests use it.
- `app/Sources/PixelLab/Model/SceneEditor.swift`: the item's State picker (R25), and the stage's key selected after a move (R24).
- `app/Sources/PixelLab/Model/Settings.swift`: `paneHeight`, `showTracks`.
- `app/Sources/PixelLab/Model/AppModel.swift`: the pane's focus, and opening the pane when an item is animated or a key is picked.
- `app/Sources/PixelLab/Views/TimelineView.swift` → `AnimationPane`: the header row, the resizable body, track rows, state lanes and the focus ring.
- `app/Sources/PixelLab/Views/PaneKeyCatcher.swift` (new): the `NSViewRepresentable` key catcher (D5).
- `app/Sources/PixelLab/Views/KeyInspector.swift` (new): KEY with State, and ANIMATION with Animate and Remove Track.
- `app/Sources/PixelLab/Views/SelectionInspector.swift`, `app/Sources/PixelLab/Views/InspectorView.swift`: they host KEY, ANIMATION and the State picker.
- `app/Sources/PixelLab/Views/MainWindow.swift`: the pane in place of `TimelineStrip`.
- `app/Sources/PixelLab/PixelLabApp.swift`: the Edit menu's key items (R31).
- `app/Sources/PixelLab/Support/ScriptRunner.swift`: D6's scene actions and expects.
- `app/Tests/PixelLabTests/AnimationPaneTests.swift` (new).
- `scripts/acceptance/animation.txt` (new), `scripts/acceptance/scenestates.txt` (new), `scripts/acceptance.sh`.

**Slice 4: the pane in object mode** (`feat/object-states-object-pane`)
- `app/Sources/PixelLab/Model/StatesModel.swift` (new): D4's model, `graphLayout`, and play and try over a run's `changes`.
- `app/Sources/PixelLab/Model/ObjectEditor.swift`:
  - It owns `StatesModel`.
  - The canvas frame and offset come from a state's position, or from what's playing.
  - The frame edits move here from the inspector.
- `app/Sources/PixelLab/Views/StatesPane.swift` (new): the states list, the Sheet, Graph and Details tabs, the dope sheet's Frames and Move rows, and Add a frame.
- `app/Sources/PixelLab/Views/StateGraph.swift` (new): the graph `Canvas`, its hit-testing, and arrow drags.
- `app/Sources/PixelLab/Views/TimelineView.swift`: object mode hosts `StatesPane`.
- `app/Sources/PixelLab/Views/MainWindow.swift`: the pane above `ObjectStatusBar` in object mode.
- `app/Sources/PixelLab/Views/ObjectPanels.swift`:
  - `FramesSection` leaves the inspector.
  - `object animate` moves to the Layers row menu.
  - Checks are pinned to the bottom.
  - The KEY section serves a state's move keys.
- `app/Sources/PixelLab/Views/StageChrome.swift`: the Try chips.
- `app/Sources/PixelLab/PixelLabApp.swift`: New State (⌥⌘N), and ⌘G in object mode.
- `app/Sources/PixelLab/Support/ScriptRunner.swift`: D6's object-mode actions and expects.
- `app/Tests/PixelLabTests/StatesTests.swift` (new): `graphLayout`, playing a run, and the states list's VoiceOver strings.
- `scripts/acceptance/states.txt` (new), `scripts/acceptance.sh`.

**Integration** (the main session, on `feat/object-states`)
- `scripts/acceptance/poses.txt` (new), `scripts/acceptance.sh`: R38 on a temp site copy (D6).
- `docs/site-contract.md`: States (D7).
- `docs/document-format.md`: States, and `state` on items and keys.
- `README.md`: the tour's pane, States, and the shortcuts. Screenshots in `docs/screenshots/`.
- `CLAUDE.md`: the hook's new words.
- `docs/testing.md`: the browser check, and how to install its browsers.
- `app/README.md`, `docs/architecture.md`: `states.rs`, `StatesModel` and the pane.
- `docs/intents/2026-10-mac-app/plan.md`: the follow-ups (D7).
- `docs/intents/2026-10-object-states/plan.md`: departures, as they happen.

## Where the spec leaves a choice

The plan settles these, and records them under Departures if the build changes them:

1. **Classes and frame groups.**
   - Every item whose object has states gets an element class `<scene>--s<N>`.
   - Items that play the same share `N`, as `a<n>` does today. "The same" means the same object, start state and keyed track.
   - A stated sprite's frame `K` is one group, `<scene>--s<N>-f<K>`, holding that column of every frame-loop layer and no loop class. That keeps `frame_loop_rules` off it, and leaves R3's output for sprites without states untouched.
   - The map layers are drawn in each frame group, in layer order, so paint order is kept.
2. **Frame edits on a sprite with states act on the column.** Frame `k` is frame `k` of every frame-loop layer (R2), so add, duplicate, delete and move apply to each of them. A sprite without states keeps today's per-layer behavior.
3. **A pose change with no state key.** The item keeps its state's name and timing if the new pose's object has a state of that name. Otherwise it plays that object's start state, entered at the pose key's time. `check` doesn't warn, since the scene still plays the same every time.
4. **`object.state.run`'s default horizon** is the settle plus one period. That's `changes`'s own horizon (D2).
5. **The browser check fails, never skips,** when its browsers aren't installed. The message names `npx playwright install`. CI installs them, and `docs/testing.md` says so for local runs.
6. **R17's warning** is built as specified. If it fires on the examples or the fixtures without a real conflict, step 5 drops it and records that under Departures (spec, Areas of concern 4).

## Order of work

```
main ── (PR #58: spec and plan) ── feat/object-states ──────────────────────────── PR 5 (owner merges) ──▶ main
                                     ├─ PR 1  program      ◀─ feat/object-states-program
                                     ├─ PR 2  poses        ◀─ feat/object-states-poses         ┐ from PR 1's merge,
                                     ├─ PR 3  scene pane   ◀─ feat/object-states-scene-pane    ┘ in parallel
                                     ├─ PR 4  object pane  ◀─ feat/object-states-object-pane     (after PR 3 merges)
                                     └─ integration commits (poses acceptance, docs) ─ then PR 5
```

- The spec and this plan go to `main` in PR #58. The owner approved the plan on 2026-10-24 and asked for the work to start before PR #58 merges, so the main session cuts `feat/object-states` from `spec/object-states` (which holds `main`, the spec and this plan). Once PR #58 merges, the main session merges `main` into `feat/object-states`, as for any move of `main`.
- **Why four slices:**
  - PR 1 is the foundation: every other slice calls its commands. It also changes what Publish writes to the site, which is the change to check before anything builds on it.
  - PR 2 and PR 3 are independent once PR 1 lands. PR 2 owns only `crates/`, PR 3 only `app/` and `scripts/acceptance/`, so they run in parallel.
  - The app is split in two (PR 3, then PR 4) because it's too large to review in one sitting, and object mode builds on the pane PR 3 makes.

**PR 1: the program (steps 1–6)**
1. **The format and validation** (D1, R1, R2, R5, R7):
   - The key orders and canonical shapes.
   - `states::validate`, the item and key `state` checks, and R2's lockstep check.
   - `Resolved.states` through `extends`.
   - Tests "R1: …", "R2: …", "R5: a sprite that extends another has the base's states", and "R7: …" (one per case). A canonical round trip of `fixtures/states.pixellab`.
2. **The run** (D2, R6): `States`, `run`, `at` and `changes`, with moves stepped by `ambient`'s `runs`.
   - Tests "R6: …" for a loop, a hold, a `then` into a loop, a cycle of one-shots, and a move at 44 ms in a 480 ms state.
   - `at` must agree with `ambient::action_at` for a converted action.
3. **Renders** (D2, "Renders"):
   - `Group.states`, `draw`'s frame groups, and `composite` at a time.
   - `object render --state --time`, and `scene render` placing states.
   - Tests "R3: a sprite without states renders and compiles byte-identical", on the fixture and both examples as they are on `main` (goldens taken before this step's change).
4. **The CSS** (R9–R14, R16):
   - The per-state frame and move animations, the end animation, the selectors, `data-play` for one-shots, and the reduced-motion block.
   - The baked track for keyed items, with no `data-state` selectors.
   - `art sizes`' states line.
   - Tests in `states_css.rs` against fixture strings. "R16: a keyed track plays frame for frame as scene render --time" at every 100 ms.
   - `scripts/test/states-css.test.mjs`.
   - `scripts/test/states-browser.test.mjs` with Playwright, the lockfile and the CI steps. It runs R10, R11, R12 and R9's `data-play`, a `then` hand-over, R16 against `scene render --time`, and R14 under `reducedMotion: 'reduce'`.
5. **The commands** (D3, R4, R8, R15, R21–R25, R27, R30, R34):
   - `states_ops.rs` and `object_states.rs`.
   - The frame edits on columns, `scene.key.set --state` and pinning, `scene.set --state`, ids from `scene.track.add`, `object.show`, `object.list`, `scene.items`, and `check`'s problems and warnings.
   - The MCP instructions. `npm run docs:cli`.
   - Tests "R4: …", "R8: …" (with the `character` copy passing and the three refusals), "R30: …", and "R34: every new command is in all three doors" (an MCP tool listing and one `serve --stdio` call each).
   - A rename that touches two scenes is one version.
6. **The examples** (D7): `make-examples.mjs` as listed, then `npm run examples`. `npm test` checks the examples match. The harbor and town SVGs change only by the lamp's and the boat's states.

**PR 2: poses become states (steps 7–8)**

7. **The merge** (R37, D8): the family, flattening, the anchor and union box, merged keys with renames, the `pf` loop, the state names (`rest`, `drop`, …), and the refusals.
   - Tests "R37: …" on `fixtures/poses.pixellab`, including a pose with two map layers and each refusal.
8. **Keys, fold and the check** (R38–R40):
   - The key rewrite in `motion_ops.rs`, and `--fold` with `--name`.
   - The before-and-after pixel check at every 100 ms, which compares `composite` maps, not PNGs.
   - `--delete-unused`, and one version across every doc.
   - Tests "R38: …", "R39: the alternating signal becomes one flash key", "R40: …", and "R38: a fold that would change a frame is refused and changes nothing".
   - `npm run docs:cli`.

**PR 3: the pane in scene mode (steps 9–12)**

9. **The protocol and model:**
   - `MotionKey`, `TimelineModel`'s keys and selection, `AnimationPaneLogic`, `stateSections`, and the cached runs.
   - Swift tests: hitting a key, snapping, inherited values, and sections for a `then` hand-over.
10. **The pane** (R18–R21, R31–R33):
    - `AnimationPane` with track rows, state lanes and the resizable body.
    - The key catcher and focus ring, the Edit menu items, and the VoiceOver labels.
    - Script actions `key …`, `track …`, `loop`, `panekey` and `lane select`, and expects `key|keys|keymove|keyshow|keypose|loop|tracks|pane|lane`.
11. **The inspector and the stage** (R22–R25):
    - KEY with State, ANIMATION, and the item's State picker.
    - The stage's move selecting the key it made.
    - Script actions `keyset …` and `itemstate`, and expects `keystate` and `itemstate`.
12. **Acceptance and snapshots:**
    - `animation.txt` on the temp site copy (R35), and `scenestates.txt` on a harbor copy, matching `scene render` frames at three times.
    - Snapshots of the pane with a state lane, the KEY section, and the State picker, in dark and light.

**PR 4: the pane in object mode (steps 13–16)**

13. **`StatesModel`:** the states, selection, edits, `graphLayout`, play and try over `changes`, and Reduce Motion showing the settle frame (R29, R14). Swift tests for `graphLayout` and playing a run.
14. **The states pane** (R26–R28, R30):
    - The list, the header, the Frames row with Add a frame, the Move row, and Details.
    - `FramesSection` leaves the inspector. Animate moves to the Layers menu.
    - "No states yet" and "Blocks have no states".
    - Script actions `state …`, `stateframe …` and `statekey …`, and expects `states|start|state|stateframes|statekeys`.
15. **The graph and trying** (R28, R29, R31, R32):
    - `StateGraph` and its drags and menu, the Try chips, Play and Esc, ⌘G, ↑ and ↓, ⌥⌘N, and the VoiceOver labels.
    - Script actions `graph …`, `try`, `stateplay` and `statestop`, and expects `graph|trying|preview`.
16. **Acceptance and snapshots:**
    - `states.txt` on a harbor copy. It gives the `barrel` its first state with New State, adds a one-shot that hands over, links it in the graph, tries it, plays it, and checks the published SVG for `data-state` rules.
    - Snapshots of the Sheet, the Graph, Details, "No states yet" and the Try chips, in dark and light.

**Integration on `feat/object-states` (steps 17–19)**

17. **Poses end to end:** `poses.txt` on a temp site copy, read-only from the site checkout:
    - `cli object states from-poses crane-hook`, then `crossing-signal --fold`.
    - Expect the lanes and the key counts (the signal's 18 pose keys become 3 state keys).
    - Expect the scene to render the same at every 100 ms (R38).
18. **Docs** (R36): every doc under Integration, and `npm run docs:site`.
19. **The final run:**
    - Every check in Proof, on the merged branch.
    - Build the app and open it on a temp harbor copy (`scripts/try-example.sh harbor`) for the owner to try.
    - Open PR 5.

## Workflow and reviews

| Step | Who | Where | Then |
|---|---|---|---|
| Plan | main session | PR #58 (`spec/object-states`) | commit, push. The main session cuts `feat/object-states` from it, and the owner merges PR #58 when they're ready |
| PR 1, steps 1–6 | subagent (Sonnet), in its own worktree | `feat/object-states-program` | opens PR 1. An adversarial review (below) runs. The main session reads the diff, reruns the program checks, fixes or sends back blocking findings, and merges |
| PR 2, steps 7–8 | subagent (Sonnet), in its own worktree, from PR 1's merge | `feat/object-states-poses` | opens PR 2. The main session reruns the program checks and merges |
| PR 3, steps 9–12 | subagent (Sonnet), in its own worktree, from PR 1's merge, alongside PR 2 | `feat/object-states-scene-pane` | opens PR 3. The main session looks at the snapshots, reruns the app checks and `animation.txt`, and merges |
| PR 4, steps 13–16 | subagent (Sonnet), in its own worktree, after PR 3 merges | `feat/object-states-object-pane` | opens PR 4. The main session looks at the snapshots, reruns the app checks and `states.txt`, and merges |
| Steps 17–19 | main session | `feat/object-states` | PR 5 into `main`, and the app running for the owner |
| Review | subagent (Opus), with only "Use the `verify-change` skill on PR #<PR 5>." | read-only | PR comment. The main session fixes blocking findings on `feat/object-states` before handing over |

**Merging and ownership**
- The main session merges PRs 1–4 into `feat/object-states` with `gh pr merge --squash --delete-branch`, once their checks pass. Only the owner merges PR 5 into `main`.
- When `main` moves, the main session merges it into `feat/object-states` before the next slice starts. A slice merges `feat/object-states` into its branch before it opens its PR.
- **PR 2 and PR 3 run at the same time:**
  - PR 2 edits only `crates/`, `fixtures/poses.pixellab/` and `docs/cli.md`.
  - PR 3 edits only `app/` and `scripts/acceptance/` (`animation.txt`, `scenestates.txt`, `acceptance.sh`).
  - PR 2 merges first if both are ready. PR 3 then merges `feat/object-states` in, and only `docs/cli.md` can differ, which PR 3 never edits.
- PR 4 owns `app/` after PR 3 merges, and `states.txt` with its line in `acceptance.sh`.
- An app slice that needs a command change stops and reports. The main session makes the change on `feat/object-states` first, and the slice merges it in.
- Shared files: `registry.rs` (PR 1, then PR 2), `ScriptRunner.swift` and `PixelLabApp.swift` (PR 3, then PR 4), and `acceptance.sh` (PR 3, then PR 4, then integration). Each is edited by one slice at a time.

**Reviews**
- **Owner:** only at the end: PR 5, with the app running on a harbor copy to try. The look was set by the prototype boards the owner approved with the spec, so nothing pauses before then.
- **Adversarial review of PR 1:** a subagent (Opus) reads PR 1's diff against the spec's R3, R6 and R9–R16, and looks for these failures:
  - a sprite without states whose output changed;
  - a CSS state machine that breaks in one browser but passes the string check;
  - a run that disagrees between `at`, `changes` and the CSS;
  - a reduced-motion hole.
  It reports blocking findings to the main session, as a PR comment, before PR 1 merges. It runs because PR 1 changes what's published to the site and every other slice builds on it.
- **Independent:** one `verify-change` run on PR 5. The change touches the site contract, the format the site's document will carry, the MCP surface, and a command that rewrites the hero island's tracks.

## Risks

- **R3, byte-identical output for sprites without states.** This is the riskiest promise, because the acceptance pass pins the hero island's SVG and the site publishes it.
  - Stated sprites draw through their own path (choice 1), and `loop_frame` stays the path for everything else.
  - Step 3 takes goldens of the fixture and both examples before it changes `draw`, and compares bytes after.
  - `layers.txt` keeps pinning the hero island.
  - If any byte differs for a document without states, stop and fix before step 4.
- **The CSS state machine in real browsers** (spec, Areas of concern 3). The two-animation "then" and the custom-property end event can behave differently across engines.
  - The Playwright check runs all three engines in step 4, before any app work builds on the format.
  - If one engine can't do a part of it, stop and report with the failing case. The fallback (a `then` chain baked into one finite animation per state) changes the CSS shape, so the owner chooses.
- **Playwright in CI.**
  - Browsers download on every run, and the macOS release job can't use `--with-deps`.
  - Cache `~/.cache/ms-playwright` (Linux) and `~/Library/Caches/ms-playwright` (macOS), keyed on the lockfile.
  - The first CI run of PR 1 proves the steps. A failure there blocks PR 1, not the slices after it.
- **Frame edits on columns** (choice 2). These change what `object.frame.*` does on a sprite with states.
  - Sprites without states keep their per-layer behavior, which the existing frame tests cover.
  - R30's tests cover renumbering, a deleted frame leaving states, and the refusal.
- **`from-poses`' pixel check is slow on the hero island.** It renders 280 times at 28 s in 100 ms steps, twice.
  - It compares `composite` maps of only the changed scenes, with no PNG encoding.
  - The test fixture keeps its loop short.
  - `poses.txt` runs in the background with the acceptance pass.
  - If one run takes over a minute in release, render only the times where a family item's key or state changes (a departure, recorded).
- **The site's pin.**
  - v0.6.0 and v0.7.0 reject `states`, so nothing here writes to the site checkout or its document.
  - `poses.txt` and the R8 test use temp copies, made with `pixellab document copy` and `git archive`.
  - The site's conversion is a site change after it pins this release (spec, Areas of concern 1).
- **Undo for a rename across scenes,** and for `from-poses` across docs. Session undo is per doc, so these are one version through `history.rs` `put_files`, as library import is. They are taken back whole by `history undo` and Versions.
- **The examples change.** The harbor and town SVGs change, so `examples.test.mjs` and the example acceptance scripts must pass after `npm run examples`. Their `expect` lines pin layers and depth, not bytes. Any that break are updated in step 6, with the reason.
- **The pane's keyboard focus.** `.onKeyPress` was unreliable on the old branch, so the pane uses an AppKit key catcher from the start, and the script hook sends keys through `handleKey`.
- **Many tracks.** The hero island has 39. The pane scrolls, keeps the selected row in view, and caches runs per object version, so a scrub never waits on `object.state.run`.
- **Long checks.** `scripts/acceptance.sh` and the browser check take minutes. Run them in the background.

## Proof

- **Program:**
  - `cargo fmt --check`
  - `cargo test`, with no failures, including every new "R…" test
  - `npm test`, showing `# fail 0`. That includes `states-css.test.mjs`, `states-browser.test.mjs` in all three browsers, and the examples matching.
  - `npm run docs:cli`, leaving `git status` clean apart from the change's own files
- **App:**
  - `swift build --package-path app`, with no errors
  - `scripts/test-app.sh`, with every test passing, including `AnimationPaneTests` and `StatesTests`
  - `scripts/build-app.sh`
- **End to end:**
  - `scripts/acceptance.sh`, exiting 0, with `animation`, `scenestates`, `states` and `poses` passing
  - `layers.txt` still finds the hero island's SVG byte-identical (R3)
- **UI:** snapshots in dark and light of:
  - the scene pane with a state lane, and a selected key with the KEY section
  - the item's State picker
  - the states list with the Sheet, the Graph and Details
  - "No states yet"
  - the Try chips over the canvas
  Each is looked at against the boards before it's reported, and they're attached to PR 5.
- **Docs:** `npm run docs:site`, building with no broken links.
- **For the owner:** the built app open on a temp harbor copy, ready to try.

## Departures

What PR 1 built differently from the plan above (steps 1 to 6):

- **`object.state.run` came in step 4,** not step 5: the browser check reads the run through it. Its result is `{ object, state, segments: [{ state, from, to }], settle, changes: [{ t, frame, move: [x, y] }] }`, where `settle` is `{ kind: "loop", from, period }` or `{ kind: "hold", from, frame, move }`. A hold's `from` is when its last state has played all its frames (its length), with its move held at its last key.
- **Frames.** Every frame of the sprite is drawn as a group, not only the ones a state plays, so a frame no state uses yet (one just added, before a state plays it) still renders for `--frame`. A sprite with one frame draws no frame groups and puts its layers straight in the element.
- **The CSS** differs from D2's text in how it is written, not in what it plays:
  - A state's move is written as one stop for each pixel step, in a keyframes with `step-end`, from `changes`. It isn't built with `ambient`'s `frames()` (its `steps(n,end)` stops can't hold a hand-over's several states in one keyframes). `ambient::runs` and `at_percent` are `pub(crate)` so `states.rs` steps moves as an action does, and the run compares in ms (a step at `length × percent ÷ 100`, so 44 ms of 480 is 44.0016 ms, as the CSS rounds it).
  - Each frame has two keyframes for a state, one for the finite part and one (`…-l`) for the settle loop, not one shared.
  - A one-shot state's end animation lasts the state's length for a `then`, and until its last frame starts for a hold, and at least 1 ms.
  - Percent stops are written never later than the time they stand for, and less than 1 ms earlier: floored to 3 decimals up to a 100 s loop or finite part, and one more decimal for each ten times longer. So a whole-ms render agrees with the CSS.
  - A scene validation was added (R9): a one-shot state and an action of the same name in one scene is a problem, since both are `<scene>--<name>`. So is a one-shot state named as a frame loop the scene plays (of a sprite without states or a block, placed or a track's pose), whose keyframes are `<scene>--<loop>` too.
  - With no attribute, an item plays its start state under animation names of its own, `<scene>--s<N>--<state>…` (a state's name has no double hyphen, so none can make them). Naming that state with `data-state` or `data-play`, or taking the name off, changes the names, so it restarts the state as R11 says for any other; the first time a page names the state an item is already in, it restarts too. That play has no end animation: `<scene>--<state>` fires for a state a page names (R12), not for an item's start state playing at page load.
- **A keyed item's reduced-motion frame** is the first frame of its baked track (its state at time 0). R14 says the settle frame, which a scene's track has no single one of.
- **`Filled` is unchanged.** The state timeline (`states::timeline`) is built from the track's keys and the objects' states, and the pose-change rule of choice 3 lives there.
- **Choice 3 holds only for a track with a `state` key.** Only such a track is baked (R16). A track that only changes pose isn't, so a page's `data-state` still reaches its item (R10, R17), and each object it poses the item as plays its own start state from time 0, in the CSS and in a render, not from the pose key. `states::timeline` says so (`entered` 0). Baking those tracks too would have kept choice 3 but taken `data-state` away from items the spec doesn't give to the scene.
- **Render API.** `CompositeOptions` and `ObjectImageOptions` didn't change shape. A sprite is rendered in a state with `render_object_in` and `render_object_image_in`; a scene render marks a keyed item with the `state` it is in and `since` (private to the render), and `composite` reads them.
- **R16's frame-for-frame test** is in `crates/engine/tests/states_css.rs`, which plays the compiled CSS with a small player and compares every pixel with `scene render` at every 50 or 100 ms (including a hand-over between poses). `state_render.rs` holds the render tests (object at a state and time, scene placing states). R3's goldens are `crates/engine/tests/data/states-r3/` (the sample, the examples as they were, and a frames document with loops, ambient, actions and poses).
- **R17's warning** (`scene_owns_state`) fires only for a group that carries `data-state` or `data-play` itself and holds a keyed item. The spec's guess (any group with a `data-*` attribute) would fire on every item inside the site's `data-enter` wrappers.
- **`scene.key.set --state clear` and `scene.set --state start`** are words, so `clear` and `start` can't be state names (`object.state.add` and `--rename` refuse them).
- **Names the compiler makes.** A state's name can't start with `mv` or `am` as its first word, or with `a` or `s` and a number (`mv-boat`, `am-1`, `a2`, `s1-glow`): those are the generated `<scene>--…` names of scene motion, ambient loops and stated items, beside which a one-shot state's end would clash. `check` reports one (R7), and `object.state.add`, `--rename` and `object states make` refuse one.
- **A state written twice** in an object's text is reported when the document is read (R7), since JSON keeps only the last: `objects/<name>.json: states.<state>: written twice; …`, beside the file's load errors (the object still loads). An edit's working copy has only the last, so the problem goes once the object is edited.
- **`scene render`'s item boxes** come from the state each stated item is in and when it entered it (its `state` and `since`), not from its object's start state from time 0.
- **`art sizes`' states line** is the bytes of the rules that play the states, as a `states` entry (raw and gzip, with HEAD's) and a table of its own. The objects' costs still include them.
- **`object.frame.*` on a sprite with states** ignores `--layer` and acts on the column. `object.animate` on a layer of one makes a loop of the same length as the others, and `object.still` on its last frame loop keeps the frame the states renumber to 0.
- **Files.** `crates/pixellab/src/commands/object_states.rs` has `object.state.run` from step 4 and the rest from step 5. `scripts/test/states-fixture.mjs` is shared by the two Node tests. `crates/pixellab/src/scene_ops.rs` keeps an item's `state` when its id or class changes. The fixture's scene has a `{ group }` with a `data-zone`, for the browser check of a switch on a group. `crates/engine/tests/data/frame-goldens.txt` has the harbor's and town's new SVG hashes (their previews are unchanged). `examples/README.md` and `docs/contributing.md` (Node's one package) say what changed.
- **The browser check on this Mac.** WebKit and Chromium pass. Playwright's Firefox (1543, Firefox 155) won't start here: it exits with "Could not find profile folder", even for `-CreateProfile` run by hand, so it isn't verified locally. `PIXELLAB_BROWSERS=chromium,webkit` limits a run (CI never sets it); otherwise a browser that can't start fails with a message naming `npx playwright install`.
- **Firefox was verified in the adversarial review,** which ran on Linux: both Firefox tests passed beside WebKit and Chromium, and PR 1's CI is green.
- **The end-event checks read the event's own time.** `states-browser.test.mjs` checks `animationend`'s `elapsedTime` (480 ms for `hop`, 100 ms for `dead`), and only bounds the wall clock from below. Its first version bounded the wall clock from above too, and it failed once in a full `npm test` run while the machine was busy with `cargo test`.

How the work runs, changed by the owner on 2026-10-24:

- **The adversarial review of PR 1 ran in a cloud session,** not as a local subagent. It posted its findings on PR #60 ("Adversarial review (plan: PR 1)"), and the review fixes were pushed to the PR before it merged.
- **The main session builds slices 2, 3 and 4 itself,** one after the other, not with subagents. Slice 2 (poses) was started before PR 1 merged, on `feat/object-states-poses` cut from PR 1's head, and merges `feat/object-states` in once PR 1 lands. Slices 3 and 4 still run in order, so PR 2 and PR 3 no longer run at the same time, and the file ownership rules for parallel slices don't come into play. Each slice still opens its PR into `feat/object-states`, and the main session merges it once its checks pass.

What PR 2 built differently from the plan above (steps 7 and 8):

- **`--as` names the folded state,** not `--name`: every command's first argument is its `name` param (here the base object), so `--name` would name the object. `--as` is the word `object state duplicate` uses for a new state's name. Without it, the folded state is the run's first state with `-loop` (`left-loop`), since the run's first state (`left`) is already a state of its own (R37 gives every pose one).
- **A command word may have hyphens.** The registry's names were dotted lowercase words; `object.states.from-poses` needs `from-poses`, so a word is now lowercase words joined by hyphens. The MCP tool name turns hyphens into underscores too (`object_states_from_poses`).
- **Objects that extended the base are written out whole** (`detached` in the result): a sprite that extends another has its states (R5), and its `rows` would be read against the merged frame loop. Written with their resolved layers and keys and no `extends`, they draw exactly as before and have no states. The site's `crossing-signal-left` and `-right` extend `crossing-signal`.
- **More refusals than D8 lists:** a base that already has states; a pose or base layer with an ambient loop or actions (`object states make` first); a merge whose differing layers carry a class, which a frame loop can't keep; and a base nothing poses.
- **Layers.** When every member has the same layers and the ones that differ sit together, the layers they all draw the same stay map layers with their class, and the differing ones become the frame loop in their place. Otherwise every member is flattened into the one frame loop.
- **The check's times** are every 100 ms of the loop and each key time of the base's tracks (which can fall between, like the site's 13575 ms). Comparing either side of every key of every track, as first written, took 1,670 renders and 63 s on the hero island. The check now reuses the drawn tree while nothing in the scene is placed, posed or ordered differently, and composites again for what plays inside it (loops, states). That's 22 s and 70 MB for the hero island's `crane-hook` in a release build. It compares pixel maps, not PNGs (`render::ScenePixels`, `poses::differences`).
- **It writes its files itself** (`writes_direct`), as one version through `put_files`, and has its own `dryRun`, so the CLI's global `--dry-run` checks the renders without writing.
- **On a temp copy of the site's document,** `from-poses crane-hook` rewrote 11 keys, and `from-poses crossing-signal --fold --as flash --delete-unused` folded both signals' 16 alternating keys into one `flash` key each (3 keys a track), deleted `crossing-signal-left`, and left `crossing-signal-right`, which another scene places. `check` gave the same 27 warnings as before, and the hero island's SVG went from 540,282 to 538,219 bytes.
- **Files.** `crates/pixellab/src/poses.rs` holds the plan and the check, `commands/object_poses.rs` the command, `render.rs` `ScenePixels`, `tests/poses.rs` the tests, and `tests/params.rs` pins the new command. Steps 7 and 8 are one commit, since the merge and the key rewrite share the module, after a work-in-progress commit pushed before PR 1 merged.
