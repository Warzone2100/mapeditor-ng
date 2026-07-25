# Testing Your Map

> **Desktop only.** Test-launching is not available in the web build.

Press `F5` or click the `Test Map` toolbar button to launch your map in
Warzone 2100 as a skirmish game. The editor writes a temporary `.wz` archive and
a skirmish configuration to your WZ2100 user directory, spawns the game, and
cleans up the temp files when the game exits.

## Setup

Before you can test, the **Warzone 2100 executable** must be configured: open
Settings (`Settings` button in the toolbar), go to the `Game` page, and set the
path to your `warzone2100` executable. Use `Browse...` to pick the file, or type
the path directly. If you have already set a WZ Data Directory, the executable
may be auto-detected from that location.

You do not need to save first. Testing packages the map as it currently stands
in the editor, so unsaved edits are included and a map that has never been saved
can still be test-launched.

## Limitations

- **Campaign maps cannot be test-launched.** Only skirmish maps are supported.
  The `Test Map` button is disabled for campaign maps.
- If no executable is configured, pressing `F5` opens Settings > `Game`
  automatically so you can set one.
- Only one test game can run at a time. The button is disabled while a test
  game is in progress.

---

See also: [Graphics & Theme](settings-graphics.md) · [Publishing a Map](maps-publish.md)
