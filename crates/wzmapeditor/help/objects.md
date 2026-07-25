# Placing Objects

Objects are the structures, droids, and features that populate a map. The editor provides two dedicated tools for working with them, plus tools for gateways and script labels.

## Object Select and Object Place

Press `S` to activate **Object Select**. Click any placed object in the viewport to select it; hold `Shift` or drag a box to multi-select. With objects selected you can:

- Move them by dragging in the viewport.
- Rotate the selection with `R` (90° per press).
- Delete with `Delete` or `Backspace`.
- Duplicate in place with `Ctrl+D`.
- Click empty terrain or switch tools to clear the selection.

**Object Place** is armed from the Assets panel (see below). Once armed, click in the viewport to place the chosen asset. Press `R` before clicking to rotate the placement ghost. Switch tools to cancel.

## Arming Placement from the Assets Panel

The **Assets** panel lists every structure, feature, and droid template available in the loaded game data. Use the **Structures**, **Features**, and **Droids** tabs to browse, or type in the search box to filter across all categories.

Click any entry to arm it for placement — the cursor changes to a ghost preview in the viewport. If the game data includes a multiplayer overlay, campaign-only items are grouped under a "Campaign-only" divider and can be hidden by unchecking the **Campaign** checkbox. Switch between **Grid** and **List** view using the button in the panel toolbar.

## Setting the Player Owner

Before placing, set the owning player with the player selector in the Assets panel toolbar. You can also change the owner of already-placed objects in the **Selection** panel after selecting them.

## The Hierarchy and Selection Panels

The **Hierarchy** panel lists every object on the map. Click an entry to select it in the viewport.

The **Selection** panel (also called the Properties panel) names the selected object's stat in a read-only line, then offers editable **X**/**Y** position, **Rotation**, **Player**, and — for structures — **Modules**. Changes apply immediately and can be reverted with `Ctrl+Z`. Dragging a value counts as a single undo step no matter how far you drag it, and applying a rotation or player to a multi-object selection undoes as one step for the whole selection.

## Gateways and Script Labels

Press `G` to switch to the **Gateway** tool. Click and drag to draw a gateway segment; gateways guide AI pathing between zones. Toggle their visibility in the View menu with **Show Gateways**.

Press `L` to activate the **Script Label** tool for placing named regions used by campaign scripts. Toggle visibility with **Show Labels** in the View menu.

---

See also: [Interface Overview](interface-overview.md) · [Terrain Stamps](terrain-stamp.md) · [Validation](validation.md)
