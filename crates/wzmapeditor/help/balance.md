# Balance Analysis

The **Balance** panel analyzes per-player starting resources and territory, helping you tune fairness.

## How It Works

Balance analysis requires at least one player structure on the map (an `A0CommandCentre` per player). The editor divides the map into zones — each tile is attributed to whichever player's start position is nearest — and credits every oil patch to the player whose zone it falls in. Structures and droids are counted by the player that owns them, not by the zone they stand in, so an object placed deep inside another player's territory still counts for its owner.

The result is cached. Click **Re-run** in the panel toolbar to refresh it after making changes — edits do not update the figures on their own.

## Reading the Summary Table

The table lists one row per detected player and shows:

- **Show** — marks that player's structures and droids with filled dots in the viewport and rings the oil patches credited to them. To draw the zone itself, use **Zone lines** or **Zone fill** below.
- **Player** — player slot number. A `*` suffix means no HQ was found for this slot. Click the cell to focus the camera on that player's start.
- **Start** — tile coordinates of the player's start position.
- **Oil** — count of oil resource patches credited to the player.
- **Struct** — total structures attributed to this player.
- **Droid** — starting droids assigned to this player.

Cells that deviate from the median value are highlighted in amber; categories where every player matches exactly are shown in green.

## Zone Visualization

Two checkboxes in the panel toolbar control the viewport overlay:

- **Zone lines** — draws the Voronoi partition boundaries as outlines on the terrain.
- **Zone fill** — tints each zone faintly with its player's color so areas of ownership are visible at a glance.

## Structure Breakdown

Expand the **Structure breakdown** section to see per-player counts for every structure type. Enable **Show only differences** to hide structure types where all players have equal counts, focusing attention on what is causing imbalance.

Click a structure entry to jump the camera to that structure; repeated clicks cycle through all copies of the same type for that player.

---

See also: [Validation & Problems](validation.md) · [Map Generator](generator.md)
