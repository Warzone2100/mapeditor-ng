# Stamp Tool

The **Stamp** tool (key `7`) captures a rectangular patch of the map — tiles, heights, and objects — and lets you replay that pattern elsewhere in one click or scattered across an area.

## Phase 1: Capture

Drag a rectangle over the area you want to record. The tool captures everything inside: tile textures and orientations, terrain heights, and placed objects (structures, features, droids).

Right-click to discard the current capture and draw a new rectangle.

## Phase 2: Place

Choose the placement mode **before** you capture: changing **Mode** afterwards discards the captured pattern and returns you to the Capture phase.

### Single

One click drops the full captured pattern centered on the cursor.

- The preview is **green** when the stamp fits entirely on the map.
- The preview is **red** when part of the stamp would fall off the edge — it still places, but the out-of-bounds portion is clipped.

**Toggles:**

- **Stamp tiles** — writes the captured tile textures and orientations.
- **Stamp terrain** — writes the captured tile heights.
- **Stamp objects** — places the captured structures, droids, and features.
- **Random rotation** — picks a random 90° rotation for the entire stamp per click.
- **Random flip** — picks a random X/Y flip for the entire stamp per click.

Right-click in Single mode returns to the Capture phase.

### Scatter

Drag inside a circular brush area to scatter randomly-sampled objects from the captured pattern across the surface.

- **Radius** (1–20 tiles) — the scatter brush size.
- **Density** (0.01–1.0 per tile²) — objects per square tile. The panel shows the expected number of objects per burst.
- **Stroke spacing** (1–10 tiles) — minimum cursor travel between scatter bursts while dragging, preventing clumping along slow strokes.
- **Min object spacing** (0–256 world units) — minimum gap between objects placed within a single burst.
- **Random rotation** and **Random flip** — as in Single mode, applied per scattered object.

Scatter only places objects from the captured pattern; tile and terrain toggles have no effect in this mode. Samples whose ground cannot carry them are dropped, so a burst over water or cliffs yields fewer objects than the density suggests.

## Tips

- Capture a section with interesting rock or ruin layout, then Scatter it to populate a wider area quickly.
- Use Single with **Stamp terrain** only (tiles and objects disabled) to copy a height profile from one part of the map to another.
- **Stamp objects** checks the ground under each structure before placing it. Anything landing on water, a cliff face, a slope that is too steep, or inside the 3-tile map-edge buffer is skipped, and the Output panel reports how many were dropped. Because a stamp writes its terrain before its objects, a pattern carrying its own heightfield is judged against that new terrain rather than whatever was there before. The overlap and spacing rules that govern [hand placement](objects.md) are deliberately not applied, so a captured cluster reproduces intact even where placing those buildings one by one would be refused.

See also: [Terrain Tools](terrain.md), [Objects](objects.md).
