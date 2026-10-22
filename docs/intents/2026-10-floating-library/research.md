# Research: how similar tools lay out the canvas

Background for [`intent.md`](intent.md), gathered on 2026-10-21 from official docs, help pages, source code and forums. Each claim cites a numbered source at the end. "Unverified" means no official page confirmed it.

## Compared

| App | Zoom and pan | Tools | Library to canvas | Into a component and back | Saving | Hiding the interface |
|---|---|---|---|---|---|---|
| Figma | Pinch and ⌘-scroll zoom; a zoom menu; ⇧1 fit, ⇧2 selection [1] | A slim floating bar at the bottom (UI3) [3][5] | Not checked | "Go to main component", then a "Return to instance" prompt [6] | Autosave with checkpoints [7] | ⇧\ minimizes the UI [3] |
| Aseprite | The wheel zooms or scrolls, chosen in Preferences; keys 1–6 set the zoom; a zoom field [9][10]. H hand, Space as a temporary hand (forum) [11] | Docked toolbar | Pick a tile, then draw with it [12] | Unverified | Saves when asked; backups every 2 min for crash recovery [13] | Unverified |
| Tiled | ⌃-wheel zoom; ⌃+/⌃−/⌃0; Space or middle-drag pans [17] | Docked toolbars | Pick a tile in a docked Tilesets panel, then stamp | "Edit Tileset" opens it; the docs don't say how to return [18] | Unverified | Tab hides every dock [17] |
| LDtk | Space-drag pans; Space-right-drag zooms [19] | Unverified | A palette in the side panel that follows the layer [20] | A separate world view (⇧W) [19] | Unverified | Tab for a compact sidebar [20] |
| Sketch | Pinch or ⌘-scroll zoom; ⌘0, ⌘1 fit, ⌘2 selection; Space-drag pans [22][23] | A toolbar with a zoom menu [22] | Insert Symbol [23] | ⌘↩ goes into a symbol, Esc comes back out [23] | Unverified | ⌘. toggles the interface [23] |
| tldraw | ⇧1 fit, ⇧2 selection, ⇧0 100% [24][25] | V select, H hand [26] | Unverified | Not applicable | Unverified | ⌘. focus mode [25] |
| Excalidraw | Space-drag or wheel-drag pans; ⇧1 fit, ⇧2 selection [27] | V and H, at the top [27][28] | Drag from a library sidebar onto the canvas [29] | Not applicable | Unverified | ⌥Z zen mode [27] |
| Procreate | Pinch zooms; a quick pinch fits the canvas [21] | Unverified | Not applicable | Not applicable | Unverified | Unverified |

Also looked at: Pixelorama, Pixaki, Pixquare and Asset Forge. Little about them could be verified [15][16][30][31][32].

## Patterns nearly every tool shares
- **Zoom:** pinch and ⌘-scroll zoom [1][17][21][22].
- **Panning:** Space-drag works as a temporary hand [17][19][22][27].
- **Fit and selection shortcuts:** shared shortcuts for "fit everything" and "fit the selection" [1][22][24][27].
- **A visible zoom level:** most tools keep the zoom level on screen somewhere, as a field or a menu [1][9][22].
- **Hiding the interface:** a single key hides it, such as Tab or ⌘. [17][20][23][25][27].

## Patterns that vary
- **Where the toolbar sits:** floating at the bottom (Figma), at the top (Excalidraw), or docked (Tiled, Aseprite).
- **The library:**
  - Docked and picked, then stamped: Tiled, LDtk, Aseprite.
  - Dragged onto the canvas: Excalidraw.
- **Going into a component and back:**
  - A jump with a return prompt: Figma.
  - Keyboard in and out: Sketch.
  - A separate view: LDtk.
  - None of the tools checked uses a breadcrumb.

## Pitfalls
- **Figma's UI3 bottom bar** drew a feedback thread of about 426 replies. Many asked to hide it or move it back to the top, because it takes canvas space and draws the eye [8a]. Figma's stated reasons were a clearer top and one layout across its products [5].
- **Removed controls must stay reachable.** Apple's guidelines say a toolbar item can't be the only place for a command, and a hidden interface needs a reliable way back [34].
- **Gestures need a fallback.** Apple lists pinch among the standard gestures and says a gesture should never be the only way to do something [33].

## Sources
1. https://help.figma.com/hc/en-us/articles/360041065034-Zoom-and-view-options
3. https://help.figma.com/hc/articles/23954856027159
5. https://www.figma.com/blog/behind-our-redesign-ui3/
6. https://help.figma.com/hc/en-us/articles/360038665934
7. https://help.figma.com/hc/en-us/articles/360040328553
8a. https://forum.figma.com/t/ui3-feedback/87804
9. https://www.aseprite.org/docs/zoom/
10. https://aseprite.org/docs/mouse-wheel
11. https://community.aseprite.org/t/space-lmb-isnt-panning-the-screen-anymore/7990
12. https://www.aseprite.org/docs/tilemap/
13. https://aseprite.org/docs/data-recovery/
15. https://gamefromscratch.com/pixelorama-pixel-art-tool/
16. https://github.com/Orama-Interactive/Pixelorama
17. https://doc.mapeditor.org/en/latest/manual/keyboard-shortcuts/
18. https://doc.mapeditor.org/en/stable/manual/editing-tilesets/
19. https://deepnight.itch.io/ldtk/devlog/471310/ldtk-120-is-out-happy-new-year
20. https://ldtk.io/docs/general/editor-components/
21. https://help.procreate.com/procreate/handbook/interface-gestures/gestures
22. https://go.sketch.com/docs/interface-and-settings/the-mac-app-interface/the-canvas.md
23. https://go.sketch.com/docs/shortcuts/mac.md
24. https://github.com/tldraw/tldraw/blob/main/packages/tldraw/src/lib/ui/hooks/useKeyboardShortcuts.ts
25. https://github.com/tldraw/tldraw/blob/main/packages/tldraw/src/lib/ui/context/actions.tsx
26. https://github.com/tldraw/tldraw/blob/main/packages/tldraw/src/lib/ui/hooks/useTools.tsx
27. https://github.com/excalidraw/excalidraw/blob/master/packages/excalidraw/components/HelpDialog.tsx
28. https://gigazine.net/gsc_news/en/20211113-excalidraw
29. https://www.olares.com/docs/use-cases/excalidraw
30. https://apps.apple.com/app/id1447665380
31. https://apps.apple.com/app/id1659428179
32. https://itch.io/t/1638552/automating-right-mouse-click-and-drag-to-control-camera
33. https://developer.apple.com/design/human-interface-guidelines/gestures
34. https://developer.apple.com/design/human-interface-guidelines/toolbars
