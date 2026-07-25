# Texture Painting

The **Texture paint** tool (key `5`) paints a single chosen tile across the map surface. It operates in **Single** mode — one specific tile per stroke, with full control over orientation.

## Choosing a tile

Open the **Tileset** panel and click any tile to make it the active source; the active tile is outlined there. The tool's own properties show the choice as `Selected Texture: <index>`, with the current rotation and flip beneath the buttons (for example `90° FlipX`) while **Set Orientation** is enabled.

## Controls

- **Radius** (0–20 tiles) — brush size. All tiles within the radius receive the paint.
- **Set texture** — when enabled, writes the tile index. Disable to repaint only rotation and flip without changing which tile is used.
- **Set orientation** — when enabled, writes the rotation and flip. Disable to repaint only the tile index while leaving existing orientations in place.
- **Rotate ↺ / ↻** — steps the orientation through 90° increments.
- **Flip X** — mirrors the tile horizontally.
- **Randomize** — picks a fresh random rotation and flip for each tile stamped within the stroke, adding variety without requiring manual changes.

## Eyedropper

`Ctrl+click` samples the tile index and orientation from the clicked tile into the active brush, so you can match whatever is already on the map.

## Line draw

`Shift+click` arms line-draw. The next click paints a straight line of tiles between the two points at the current radius. Press `Esc` or right-click to cancel.

## Single vs. Pool

Texture paint works tile-by-tile with a fixed source. If you want the brush to pick randomly from a set of related tiles instead, switch to **Pool** mode — that is the [Ground types](terrain-ground-types.md) tool.

See also: [Ground Types](terrain-ground-types.md), [Terrain Tools](terrain.md), [Mouse & Gestures](mouse-gestures.md).
