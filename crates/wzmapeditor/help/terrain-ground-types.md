# Ground Types

The **Ground types** tool (key `6`) paints terrain by sampling randomly from a weighted pool of tiles. Each tile stamped within a stroke receives a fresh random tile from the pool plus an independent random rotation and flip. This produces natural-looking variation without requiring any manual tile-by-tile work.

## How pools work

A pool is a named collection of tiles with associated weights. Tiles with a higher weight appear more often; tiles with a weight of zero are excluded. Pools are defined per tileset and correspond to game-engine ground type identifiers.

To view and edit pools, open the **Tileset** panel. Select a pool from the list, then add or remove tiles and adjust their weights. Changes take effect immediately in the brush.

## Controls

- **Radius** (0–20 tiles) — brush size.
- **Pool info** — a read-only `Pool: <name> (<n> tiles)` line naming the pool being sampled. There is no pool selector on the brush itself: change the active pool from the Tileset panel, or with the `Ctrl+click` eyedropper. A warning replaces the count when the pool is empty, because an empty pool paints nothing.

## Eyedropper

`Ctrl+click` samples the ground type of the clicked tile and sets the active pool to match, so you can continue painting the same type as what is already on the map.

## Line draw

`Shift+click` arms line-draw. The next click paints a straight line between the two points using tiles sampled from the active pool. Press `Esc` or right-click to cancel.

## Relationship to Texture paint

Ground types and [Texture paint](terrain-texture-paint.md) share a toolbar button controlled by the **Single** / **Pool** toggle. Use Texture paint (Single) when you need a specific tile; use Ground types (Pool) when you want variety drawn from a curated set.

See also: [Texture Painting](terrain-texture-paint.md), [Terrain Tools](terrain.md).
