# Resizing a Map

Open the Resize dialog with `Map > Resize...`. The dialog is only available when a map is loaded.

## Setting new dimensions

The dialog shows the current map size and lets you enter a new width and height. Drag the value fields or type a number directly. Both dimensions must be at least 2 tiles; the maximum is determined by the Warzone 2100 engine's export limit.

## Choosing an anchor

The anchor controls where the existing terrain and objects end up inside the new bounds. Click any cell in the 3x3 grid preview to select it:

- **Middle Center** (default) — existing content is centered in the new map.
- **Top Left** — existing content stays at the top-left corner; new empty space is added to the right and bottom.
- **Bottom Right** — existing content moves to the bottom-right; new space is added to the top and left.
- The other six positions follow the same logic.

The preview visualizes the result: green bands show where new empty tiles will be added, and red bands show where existing tiles will be cropped.

## Object removal warning

If the new bounds are smaller than the current map in the chosen anchor position, any objects (structures, droids, features, labels, or gateways) that fall outside will be permanently removed. The dialog lists how many objects of each type will be deleted before you confirm.

When nothing will be removed, the dialog notes "Nothing will be removed."

## Applying the resize

Click `Apply` to perform the resize. The operation is added to the undo history, so you can undo it immediately with `Ctrl+Z` (or `Edit > Undo`) if the result is not what you expected.

## Related topics

- [Map Properties](maps-properties.md) — edit name, player count, and authorship.
- [Terrain](terrain.md) — fill or refine the newly added tile area.
