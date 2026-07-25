# Mirroring Edits

The mirror selector in the **Terrain** tab reflects every brush stroke across one or more map axes simultaneously. It is the fastest way to build a symmetrical map, because every height change, texture or ground-type stroke, vertex drag, wall run, and stamp placement is automatically duplicated on the mirrored side.

## Mirror modes

The selector is a row of compact glyph buttons under **Mirror**; hover any of them for a tooltip naming the mode.

- **Off** — no mirroring. Edits apply only where you paint.
- **Vertical** — reflected across the left/right midline, so left and right swap.
- **Horizontal** — reflected across the top/bottom midline, so top and bottom swap.
- **Both** — 4-way reflection across both midlines.
- **Central** — point reflection through the map centre (180° rotation).
- **Diagonal** — reflected across both diagonals. Square maps only; the button is disabled when width and height differ.

## Viewport feedback

The active mirror axes are drawn as lines over the map whenever a tool that mirrors is active — every tool in the list below. This makes it easy to see exactly where the midlines fall before you start painting. Tools that do not mirror hide both the axes and the selector.

## What respects mirroring

Mirroring is applied at the stroke level, not per-pixel. These tools respect the active mirror mode:

- [Height brush](terrain-height-brush.md) strokes in all four modes
- Vertex sculpt drags
- Texture paint and Ground type strokes
- Stamp placements (Single and Scatter)
- Object placement
- [Wall](terrain-walls.md) runs

Each reflected wall tile works out its own straight, corner, T-junction, or cross piece from its own neighbours, so mirrored runs join up correctly rather than repeating the original's rotations.

The eyedropper (`Ctrl+click`) and line-draw (`Shift+click`) preview also respect mirroring, so you can see where the reflected stroke will land before committing.

**Undo** reverts the full mirrored stroke in one step — you do not need to undo each reflected copy separately.

## Tips

- Set the mirror mode before starting work on a new map to keep both sides in sync from the beginning.
- **Both** mode is useful for 2v2 or 4-player maps where all quadrants should be identical.
- **Central** mode suits head-to-head maps where each player's half should be a 180° rotation of the other, and works on any map size.
- **Diagonal** mode suits maps with rotational symmetry rather than axis symmetry.

See also: [Terrain Tools](terrain.md), [Height Brush](terrain-height-brush.md).
