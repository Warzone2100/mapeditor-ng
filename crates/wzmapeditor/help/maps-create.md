# Creating & Opening Maps

## Creating a new map

Choose `File > New Map...` or click `New` on the toolbar. The New Map dialog has the following fields:

- **Name** — the map's display name used in-game and in the maps database.
- **Width / Height** — tile dimensions. Available sizes: 32, 64, 128, 192, 250.
- **Tileset** — visual theme: Arizona (desert), Urban (city ruins), or Rockies (snow and rock).
- **Initial Height** — starting elevation applied uniformly to every tile (0–510).

Click `Create` to open the blank map in the editor. You can change the name and player count later via [Map Properties](maps-properties.md), and the map can be resized at any time via [Resizing a Map](maps-resize.md).

## Opening an existing map

- **Toolbar:** Click `Open…` and select a `.wz` archive from your filesystem.
- **File menu:** `File > Open .wz...` does the same.
- **Drag and drop:** Drop a `.wz` file directly onto the editor window.

## Importing legacy formats

Use `File > Import...` to bring in maps that predate the `.wz` format:

- **`.wz` Archive** — identical to `File > Open .wz...`: the current map is replaced and the imported file becomes your save target.
- **Legacy > Map Folder** — import a map from an unpacked folder (the classic layout with `game.map` and companion files in a directory).
- **Legacy > Binary game.map** — import a raw binary `game.map` file.

Note: Script maps (maps driven by a `game.js` file) cannot be loaded (yet), as their terrain is generated at runtime by the game engine.

## Browsing your local map collection (desktop only)

On desktop, click `Map Browser` on the toolbar or `File > Browse Maps...` to open a panel that lists your locally installed Warzone 2100 maps. This option is disabled in the web build.

## Related topics

- [Resizing a Map](maps-resize.md) — change map dimensions after creation.
- [Map Properties](maps-properties.md) — edit name, player count, and authorship.
- [Publishing Maps](maps-publish.md) — submit your map to the community database.
- [Generator](generator.md) — automatically generate terrain as a starting point.
