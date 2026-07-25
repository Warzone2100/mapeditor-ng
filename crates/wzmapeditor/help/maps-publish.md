# Publishing Maps

Publishing gets your map into the Warzone 2100 community maps database so other players can find and download it. The editor does not upload anything itself: it packages the map and opens a prefilled submission form on GitHub, where you attach the file and submit it yourself. This feature is available on desktop (Windows, macOS, Linux) only — it is not available in the web build.

## Before you publish

1. **Set correct metadata.** Open `Map > Properties…` and verify the map name, player count, author, and license. The name must be unique for that player count; the editor checks this and warns you in the Output panel if there is a conflict. See [Map Properties](maps-properties.md).
2. **Test your map.** Run through the map in-game using the `Test Map` toolbar button to make sure it plays as intended. See [Test Map](test-map.md).
3. **Save to a `.wz` file.** Use `File > Save` or `File > Save As... > .wz Archive...`. The `Publish to Maps Database…` menu item is disabled until the map has been saved to a `.wz` file.

## Submitting

With the map saved, choose `File > Publish to Maps Database…`. The editor:

1. Packages the saved `.wz` file as a `.wz.zip` archive (GitHub's web form requires the `.zip` extension to accept binary attachments).
2. Opens the `Publish to Maps Database` dialog, which shows the path to the packaged file and a button to open the prefilled GitHub submission form in your browser.

In the browser:

1. Click `Open submission form` in the dialog — this opens a prefilled GitHub issue on the `Warzone2100/map-submission` repository.
2. Drag the `.wz.zip` file shown in the dialog into the `Upload Map` field on the GitHub issue form.
3. Confirm the `Authorship` option is correct, then click `Submit new issue`.

The submission bot validates the map automatically and posts a comment with the result — approved maps are merged into the maps database.

## Notes

- The packaged `.wz.zip` file is placed next to your saved `.wz` file. If that location is not writable, it falls back to your system's temporary folder.
- Use `Reveal in Finder` / `Show in Explorer` / `Open Folder` in the dialog to locate the `.wz.zip` quickly.
- The `Publish to Maps Database…` menu item is grayed out with the tooltip "Save the map to a .wz file first" until a save path exists.

## Related topics

- [Map Properties](maps-properties.md) — set the name, player count, authorship, and license before submitting.
- [Test Map](test-map.md) — verify your map plays correctly before publishing.
