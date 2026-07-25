# Interface Overview

The editor window is divided into several areas. Understanding them from the start will help you work efficiently.

## Menu bar

The menu bar at the top of the window contains five menus:

- **File** — create, open, import, save, and publish maps.
- **Edit** — undo, redo, and duplicate selected objects.
- **View** — toggle grid, labels, fog, shadows, weather, terrain quality, and reset the panel layout.
- **Map** — open Map Properties, resize the map, or run the terrain generator.
- **Help** — open help topics or view version information.

## Toolbar

Below the menu bar is a row of quick-access buttons: `New`, `Open…`, `Save`, `Undo`, `Redo`, `Map Browser`, `Settings`, and `Test Map`.

`Map Browser` is disabled in the web build. `Test Map` is a desktop-only feature that launches Warzone 2100 with the current map loaded.

## 3D viewport

The central viewport renders your map in real time. Use the mouse to navigate:

- **Right-drag** to look around and fly through the scene using `W` `A` `S` `D`.
- **Shift + scroll**, or scroll while holding the right button, to change how fast the camera moves.
- **Left-click** to apply terrain tools or select objects.

## Terrain tools and tileset column

The left column holds all terrain editing tools — brushes for height, texture painting, ground types, vertex sculpting, stamps, walls, and mirroring. Below the tools is a tileset picker for selecting the texture tile to paint with. See [Terrain](terrain.md) for a full guide to these tools.

## Dockable panels

Panels surround the viewport and can be dragged, resized, and rearranged to suit your workflow. Use `View > Reset Layout` to return to the default arrangement.

- **Assets** — browse and place structures, droids, and features from the game's asset library.
- **Selection** — shows properties of the currently selected object so you can edit them in place.
- **Minimap** — a top-down overview of the full map. See [Minimap](minimap.md).
- **Hierarchy** — lists all placed objects in the scene.
- **Problems** — highlights map validation issues. See [Validation](validation.md).
- **Output** — displays log messages and operation results.
- **Balance** — compares per-player starting resources and territory. See [Balance](balance.md).

## Related topics

- [Terrain](terrain.md) — terrain tools in depth.
- [Objects](objects.md) — placing and configuring structures and features.
- [Minimap](minimap.md) — navigating and using the minimap panel.
- [Validation](validation.md) — checking your map for errors before publishing.
- [Balance](balance.md) — reviewing player-position fairness.
