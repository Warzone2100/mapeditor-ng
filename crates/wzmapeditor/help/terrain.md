# Terrain Tools

The **Terrain** tab holds every map-editing brush along with the mirror controls. Select a tool from the palette or use its default key to activate it.

## Tools at a glance

- [Height brush](terrain-height-brush.md) (`1`–`4`) — sculpts terrain height with a soft falloff. Four modes: Raise, Lower, Smooth, Set.
- [Vertex sculpt](terrain-vertex-sculpt.md) (`V`) — selects individual vertices and drags them to precise heights.
- [Texture paint](terrain-texture-paint.md) (`5`) — paints one chosen tile per stroke, with rotation and flip control.
- [Ground types](terrain-ground-types.md) (`6`) — paints a random tile drawn from a weighted pool.
- [Stamp](terrain-stamp.md) (`7`) — captures a rectangular patch of map and replays it elsewhere.
- [Wall](terrain-walls.md) (`8`) — drags to place a connected run of walls with automatic corner selection.

The **Texture paint** and **Ground types** tools share a single toolbar button. Use the **Single** / **Pool** toggle to switch between them.

## Shared brush behavior

All terrain brushes follow the same conventions:

- **`Ctrl+click`** — eyedropper. Samples the tile index, orientation, height, or ground type under the cursor into the active brush.
- **`Shift+click`** — arms line-draw mode for the Texture, Ground type, and Height brush Set tools. A preview line appears; click again to stamp a straight line at the current brush size. Cancel with `Esc`, right-click, or by switching tools.
- **Undo** treats one complete stroke — a drag, a line, or a stamp click — as a single step. [Mirroring](terrain-mirror.md) is included in that step, so a single undo reverts all reflected copies.

Height is measured on a 0–510 scale. The [Height brush](terrain-height-brush.md) and [Vertex sculpt](terrain-vertex-sculpt.md) tools each offer different workflows for reshaping it.

## Mirror controls

The mirror selector at the bottom of the Terrain tab reflects every brush stroke across one or more map axes. See [Mirroring Edits](terrain-mirror.md) for details.

## Keyboard shortcuts

Default tool keys are listed above. For mouse gestures and how to rebind keys, see [Mouse & Gestures](mouse-gestures.md).
