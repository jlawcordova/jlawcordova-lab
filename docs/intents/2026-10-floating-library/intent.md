# Intent: A canvas-first window, with a floating library
Author: J. Law Cordova (owner). Status: draft.

## Problem
This started as a floating library and grew into the whole window. In the owner's words:

> We need better library placing in the UX. Right now, the library is in the left sidebar, but we want to move it into something that floats in the middle, similar to what Figma does.

> I find the UI to be difficult to navigate. First, the handling of the scene or navigating in the scene. Every time I have to move, I have to click on the button, like the pan or the selection. Instead of that, I think we can have something similar to Figma.

Today:
- Moving around the stage means switching tools in the toolbar.
- Zoom is a toolbar menu.
- The stage shows full scroll bars.
- The Library is a tab in the left sidebar beside Layers. Placing from it means switching the sidebar away from Layers and picking from a narrow column.
- Scene and object editing are switched with a Scene/Object control in the toolbar.
- The toolbar also carries buttons for things that already live elsewhere: the grid, the Agent window, showing and hiding the inspector, and Save.

## Proposed outcome
The canvas becomes the window, the way Figma works. The owner's decisions:

- **Zooming** is done on the trackpad: two fingers to pinch in and out. There's no zoom button.
- **Scroll bars** are thinner and transparent, so they're barely noticeable.
- **A floating bar** sits at the bottom center of the canvas and holds the controls that remain:
  - **Navigation:** two controls only, the regular arrow that selects and the pan (hand) tool.
  - **The Library:** picking an object and dragging it straight onto the scene places it.
  - **Every other relevant button** that isn't removed below.
- **Removed toolbar buttons:** the grid and Agent buttons go, because they're already in the panels. The whole toolbar goes with them.
- **The right panel** stays on the right, always visible, with no button to show or hide it.
- **The Scene/Object switch goes.** To open an object, right-click it in the Library, as today. In the object view, a back button inside the canvas returns to the scene, or clicking any scene goes to that scene.
- **Save** moves into the right panel. Saving stays explicit for now; autosave is a separate intent (`../2026-10-autosave/intent.md`).

The design is the prototype canvas "Pixel Lab App Prototype" (https://claude.ai/artifact/MQspJo7hemS5691bzs2Rbz). The owner reviewed it and approved it on 2026-10-21. Its boards show the new window in each state:

| Board | What it shows |
|---|---|
| `Groups.dc.html` and `GroupsDay.dc.html` | The scene, the floating bar and Library, zooming and panning, and going into an object and back |
| `Object.dc.html` | The object view |
| `Animation.dc.html` | The scene with its animation pane |
| `Documents.dc.html` | Saving and publishing |
| `AgentChat.dc.html` | The Agent tab |
| `Main.dc.html` | The app map, menus and shortcuts |

In it:
- The Library opens from the floating bar as icons only. The tiles are small, with no names, no close button and no instructions. A large library scrolls inside it with a thin scroll bar that doesn't shift the layout.
- Publish sits beside Save in the right panel.
- The proposals badge that was on the Agent button moves to the Agent tab.

The prototype fakes the app; the app matches its look and behavior, not its code.

[`research.md`](research.md) compares how similar tools handle these patterns: Figma, Aseprite, Tiled, LDtk, Sketch, tldraw, Excalidraw and others.

## Affected users and systems
- The owner in the Mac app's main window, in scenes and objects: navigating, placing, saving and publishing.
- The Agent tab and the proposals badge.
- The app's test hook and acceptance pass, which drive the current toolbar and mode switch.
- Object libraries, if they land (`../2026-10-object-libraries/intent.md`).

## Constraints
- Board 03 tokens. Pixel art drawn nearest-neighbor at integer zoom, including while pinching.
- VoiceOver labels, visible focus, a keyboard path, comfortable hit targets and Reduce Motion. Anything that leaves the screen stays reachable from the menus and by keyboard.
- No new dependencies without asking.
- Out of scope:
  - Autosave (`../2026-10-autosave/intent.md`).
  - Any change to the document, the engine, the CLI or MCP.

## Open questions
- Should pinch zoom stop at the zoom levels there are today, or reach further?
- Does the owner want a way to hide all the panels for a clean look at the art?
