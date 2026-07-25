# Walls

The **Wall** tool (key `8`) places connected runs of defensive walls. Drag across tiles to paint a wall segment; the tool automatically selects the correct straight, corner, T-junction, or cross piece based on each tile's neighbors.

## Placing walls

Click and drag in any direction. As you paint, each tile in the run is evaluated against its four cardinal neighbors and assigned the appropriate wall shape:

- **One direction** — straight.
- **Two perpendicular directions** — corner.
- **Three directions** — T-junction.
- **All four directions** — cross.

Releasing the mouse button commits the stroke as a single undo step.

With a [mirror mode](terrain-mirror.md) active the run is reflected as you paint, and each reflected tile picks its own shape from its own neighbours.

## Family

The **Family** selector determines which wall stat is used:

- Hardcrete Mk1
- Collective
- NEXUS
- BaBa
- Tank Trap

All pieces within a stroke share the same family. Change the family before painting to mix different wall types in separate strokes.

## Cross-shape corners

When the selected family includes a dedicated cross-corner variant (`CWall`), enabling **Cross-shape corners** causes L-shaped corners to use that variant instead of the base corner piece. This option is disabled automatically for families that have no cross piece.

## Tips

- Walls are exempt from the slope and cliff-face checks that other [objects](objects.md) must pass, so a run will follow uneven ground that would reject a normal structure. The tool only refuses tiles that fall outside the map or already hold another structure.
- To erase walls, switch to the object select tool and delete them. Painting terrain over a wall does not remove it — the terrain brushes never touch placed objects.

See also: [Terrain Tools](terrain.md), [Objects](objects.md).
