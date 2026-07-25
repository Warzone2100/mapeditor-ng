# Graphics & Theme

Open Settings from the toolbar `Settings` button. The left sidebar lists all
pages; use it to jump between them.

## Viewport

The `Viewport` page controls appearance and overlay options.

- **Theme** — choose `System`, `Light`, or `Dark`. The change takes effect
  immediately with no restart required.
- **Show hitboxes on selected objects** — draws bounding boxes around selected
  objects to help with precise placement.
- **Show range on selected towers** — overlays the attack radius of any selected
  tower structure.

## Rendering

The `Rendering` page controls how the 3-D viewport looks. See
[Rendering](rendering.md) for a deeper explanation of each effect.

- **Sky** — toggles the sky dome.
- **Fog** — toggles atmospheric fog. When enabled, `Fog Start` and `Fog End`
  sliders control the falloff range.
- **Shadows** — toggles real-time shadow casting.
- **Water** — toggles animated water on water tiles.
- **Sun Direction** — three sliders (X, Y up, Z) adjust the sun angle used for
  lighting and shadows.
- **FOV** — field of view in degrees (20°–120°).

### Desktop-only options

The following controls appear only in the desktop build and are disabled in the
web build.

- **Graphics Backend** — selects the low-level rendering API (for example,
  Vulkan, Metal, or DX12). A restart is required for changes to take effect; the
  UI shows a warning and a `Restart now` button when the selected backend differs
  from the one currently in use.
- **Vsync** — when on, caps the frame rate to your monitor's refresh rate
  (reduces tearing). When off, renders as fast as possible at the cost of
  possible tearing. Takes effect after a restart.
- **Limit FPS** — independently caps the editor's frame rate without blocking
  the GPU swapchain. Useful for reducing CPU load on slower machines. The slider
  range is 15–240 fps.

## Maps

The `Maps` page has a single field: **Default author** — the name written into
`level.json` when you create a new map. Leave it blank to omit the author field.

## Problems

The `Problems` page lets you enable or disable individual validation warnings.
Errors cannot be disabled. Use `Enable All` or
`Disable All` to bulk-toggle warnings, then refine per-category. See
[Validation](validation.md) for a description of each rule.

---

See also: [Rendering](rendering.md) · [Mouse & Gestures](mouse-gestures.md) · [Validation](validation.md)
