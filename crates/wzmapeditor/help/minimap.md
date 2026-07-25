# Minimap

The **Minimap** panel provides a real-time top-down overview of the entire map alongside the 3D viewport.

## What the Minimap Shows

The minimap renders every tile shaded by its terrain type and height, using a color scheme that matches the active tileset (Arizona, Urban, or Rockies). On top of the terrain image, small dots mark every placed object:

- **Structures and droids** — colored by player slot.
- **Oil resources** — yellow dots.
- **Other features** — gray dots.

A white arrow shows the current camera position and facing direction in the 3D viewport.

## Navigating with the Minimap

Click anywhere inside the minimap image to jump the 3D viewport camera to that map location. The camera position arrow updates immediately as you pan or rotate the viewport, so you always know where you are on the map.

The minimap fills the available panel space and maintains the correct aspect ratio for the loaded map. If no map is open, the panel shows a placeholder message.

## Keeping the Minimap Current

The minimap regenerates automatically whenever terrain or objects change. There is no manual refresh step.

---

See also: [Interface Overview](interface-overview.md) · [Rendering & Overlays](rendering.md)
