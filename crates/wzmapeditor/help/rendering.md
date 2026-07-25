# Rendering & Overlays

The **View** menu controls what is drawn in the 3D viewport. Overlays help you inspect and navigate the map; visual effects control how the scene looks.

## Viewport Overlays

- **Show Grid** — tile grid lines over the terrain.
- **Show Border** — a highlight along the map boundary.
- **Show Labels** — script label regions as named boxes.
- **Show Gateways** — gateway segments as colored lines.
- **Show All Hitboxes** — collision boxes for every placed object — useful for spotting overlap issues.
- **Show FPS** — a readout centred against the top edge of the viewport, showing the frame rate, average/minimum/maximum frame times, and the GPU in use.

## Visual Effects

Toggle each effect from the View menu:

- **Sky** — renders the skybox above the horizon.
- **Fog** — distance fog that fades terrain to the sky color.
- **Shadows** — dynamic shadow casting from the sun.
- **Water** — animated water surface on water tiles.
- **Weather** — submenu to choose a weather condition (clear, rain, snow, etc.).

## Terrain Quality

The **Terrain Quality** selector in the View menu has three modes:

- **Classic** — the original low-resolution terrain renderer.
- **Normal** — not yet supported; the option is shown but disabled.
- **Remastered HQ** — high-resolution terrain textures. Requires the `high.wz` terrain overrides pack to be installed; the option is disabled and shows a tooltip if the pack is absent.

## Propulsion Heatmap

Press `H` to toggle the **propulsion-speed heatmap** overlay. Each tile is tinted by its traversal speed for the selected propulsion class, making slow or impassable terrain immediately visible. While the overlay is active, a row of buttons beside the toggle switches between **Wheeled** (the default), **Tracked**, **Half-Track**, **Hover**, and **Legs**; Hover is the only class treated as able to cross water. Press `H` again to return to the normal view.

## Deeper Graphics Settings

Sun direction, field of view, graphics backend, vsync, FPS cap, and the UI theme are configured in Settings. See [Graphics Settings](settings-graphics.md) for the full reference.

---

See also: [Graphics Settings](settings-graphics.md) · [Minimap](minimap.md) · [Balance Analysis](balance.md)
