# Unofficial custom build of DeckCraft

This fork is a personal, unofficial build of [DeckCraft](https://github.com/storytold/deckcraft) by the
ArtCraft team, with a few custom patches. It is **not made, endorsed or supported by the ArtCraft
team**. Its builds carry no ArtCraft logo, as the
[brand license](docs/brand/LICENSE-brand.txt) requires of modified versions.

- Branch `custom/patches`: the official release plus one commit per patch
  (`git log --oneline <release tag>..custom/patches`).
- [Releases](../../releases): unsigned Windows x64 builds made by
  [custom-windows.yml](.github/workflows/custom-windows.yml) on GitHub's servers.

## The patches

Each is one commit on `custom/patches`, with its tests in modules named `patch_tests` (the build
runs them before it packages anything).

| Patch | Where it shows | Commands |
|---|---|---|
| Crop mode | Picture Format › Crop; a picture's right-click menu | `picture.cropMode` |
| Number slides from | Design › Slide Size › Custom Slide Size | `design.slideSize {firstNumber}` |
| Kerning for fonts | Home › Font › Font… | `format.fontDialog {kern}` |
| Reverse Path Direction | Animations › Effect Options; the Animation Pane's row menu | `animation.reversePath` |
| Recent Colors | Every colour menu, after More Colors or the Eyedropper | `format.recentColor`, `format.recentColors` |
| Reading Order pane | Review › Reading Order | `review.readingOrder` (reorder with `arrange.reorder`) |
| Crop Fill, Fit, Aspect Ratio; crop fields | Picture Format › Crop menu; Format Picture pane › Picture › Crop | `picture.cropFill`, `picture.cropFit`, `picture.cropAspect`, `picture.cropPosition`, `picture.cropInfo` |
| Save as Picture; TIFF and BMP slides | An object's right-click menu; File › Export › Change File Type | `shape.saveAsPicture`, `file.export {format: tiff\|bmp}` |
| Ink Replay | Draw › Ink Replay | `draw.inkReplay` |

What these do differently from PowerPoint, as far as is known:

- **Reverse Path Direction** keeps the path where it is on the slide, so the object starts at the
  far end and travels back. This was written from the file format, not checked against PowerPoint.
- **Recent Colors** are kept with the presentation (ten, newest first; `p:clrMru` in a .pptx).
- **Reading Order** is the order of the objects back to front, so reordering here also restacks
  them, as dragging in the Selection pane does.
- **Ink Replay** draws at a steady pen speed: ink keeps the order it was drawn in, not the time
  each stroke took.
- **Save as Picture** draws at most 2048 pixels on the longer side. There is no SVG, EMF or WMF.

The upstream progress documents (`ROADMAP.md`, `docs/gaps.md`, `docs/parity-checklist.md`) are left
as the official release has them, so that moving the patches onto the next release does not
conflict in them.

For the official app, use the [official releases](https://github.com/storytold/deckcraft/releases).
