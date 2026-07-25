# Vertex Sculpt

The **Vertex sculpt** tool (key `V`) lets you select individual terrain vertices and drag them to precise heights. Unlike the [Height brush](terrain-height-brush.md), which applies a broad soft-edged stroke, Vertex sculpt gives you direct control over each grid point.

## Selecting vertices

Click any vertex to select it. A plain click replaces whatever was selected before, so use `Shift+click` or a `Ctrl+drag` box to build up a multi-vertex selection — all selected vertices then move together when you drag. The panel displays the current selection count; use **Clear selection** to deselect all.

- **`Shift+click`** — adds an unselected vertex to the selection, or removes an already-selected vertex from it.
- **`Ctrl+drag`** — draws a box and selects all vertices inside it, replacing the current selection.
- **`Ctrl+Shift+drag`** — draws a box and adds all vertices inside it to the existing selection.

## Dragging

After selecting one or more vertices, drag any selected vertex up or down. All selected vertices move by the same delta. The **Soft radius** extends the drag influence to nearby unselected vertices with a falloff, so the terrain blends smoothly outward rather than producing a sharp spike.

- **Soft radius** (0–12 tiles) — the falloff zone around each selected vertex. At 0, only the selected vertices move. Higher values pull surrounding vertices along at decreasing strength.

## Tips

- Build a rough shape quickly with the [Height brush](terrain-height-brush.md), then switch to Vertex sculpt for detail work on ridges, ramps, and cliffs.
- `Ctrl+drag` is efficient for selecting a clean rectangular ridge line before pulling it to an exact height.

See also: [Height Brush](terrain-height-brush.md), [Terrain Tools](terrain.md).
