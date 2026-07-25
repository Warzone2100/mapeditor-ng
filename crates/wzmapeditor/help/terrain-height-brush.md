# Height Brush

The Height brush sculpts terrain height across a circular area with a soft falloff. Activate it with keys `1`–`4`, each of which selects a different mode directly.

## Modes

- **Raise** (`1`) — increases height under the brush while the mouse is held.
- **Lower** (`2`) — decreases height under the brush while the mouse is held.
- **Smooth** (`3`) — blends height differences toward an average, flattening bumps.
- **Set** (`4`) — pushes all tiles inside the radius toward the **Target height**.

Raise, Lower, and Smooth re-fire continuously while the mouse button is held. Set commits once per tile touched — re-dragging over the same tile has no additional effect until you release and stroke again.

## Controls

- **Radius** (0–20 tiles) — brush size. The cursor highlight in the viewport is a square covering the affected tiles, although the falloff itself is circular.
- **Strength** (0.1–5.0) — the height delta applied per tick. Only shown in Raise and Lower modes; Smooth and Set do not use it.
- **Target height** (0–510) — the destination height used by Set mode.

## Line draw

In **Set** mode, `Shift+click` arms line-draw. The next click drops the brush in a straight line between the two points at the current radius. Press `Esc` or right-click to cancel before the second click.

## Eyedropper

`Ctrl+click` samples the height at the clicked tile into **Target height**, so you can match an existing elevation precisely.

## Tips

- For large flat plateaus, use **Set** with line-draw to paint clean edges before filling the interior.
- For fine vertex-level work, switch to [Vertex sculpt](terrain-vertex-sculpt.md) after roughing in shape with the Height brush.
- For mouse gestures and how to rebind keys, see [Mouse & Gestures](mouse-gestures.md).

See also: [Terrain Tools](terrain.md), [Vertex sculpt](terrain-vertex-sculpt.md).
