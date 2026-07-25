# Map Generator

> The generator is experimental.

**Map > Generate** opens the procedural map generator dialog. It produces a complete starting map — terrain, oil, structures, and droids — that you can then edit freely.

## Main Options

### Layout

- **Name** — internal map name written to the output file.
- **Width / Height** — map dimensions in tiles (48, 64, 96, 128, 192, or 250).
- **Tileset** — visual theme: Arizona, Urban, or Rockies.
- **Players** — number of player start positions to generate. The dropdown offers 2 to 8 and 10.
- **Symmetry** — mirrors the layout: Vertical (left/right), Horizontal (top/bottom), Both (4-way), Central (180° rotation), or Diagonal (rotational, square maps only).

### Terrain

- **Height levels** — number of distinct terrain plateaus (3–5).
- **Level frequency** — how often height changes occur across the map.
- **Height variation** — amount of fine-grained noise added within each level.
- **Flatness** — how aggressively slopes are smoothed.
- **Water bodies** — number of water features placed (0–5).

### Resources

- **Oil per base** — unbuilt oil resource patches placed in a ring around each player start.
- **Extra oil** — additional oil scattered across the map.
- **Trucks per player** — construction droids assigned to each player at the start.
- **Oil drums** — collectible pickup features scattered across the map.
- **Scatter decorative features** — enables random debris and props with an adjustable density slider.
- **Place scavenger bases** — adds neutral scavenger camps with a count slider.

### Seed

The **Seed** field controls reproducibility. The same seed with the same settings always produces the same map. Click **Randomize** to pick a new random seed, or type any number to fix one. After generation the dialog shows the seed that was used so you can recreate the run.

## After Generation

The generated map replaces any open map and is immediately editable. Use the terrain tools to sculpt heights and textures, place additional objects, and check fairness in the [Balance](balance.md) panel before publishing.

---

See also: [Creating Maps](maps-create.md) · [Terrain](terrain.md) · [Balance Analysis](balance.md)
