# Fork patches

> **Last updated:** 2026-11-18 · **Base:** `storytold/deckcraft` `main` v0.19.0 (`5c69849`), merged in `86be392` · **Patches written on:** v0.18.1 (`7cc73ab`), up to `3c1d282` (`custom/patches`, tag `v0.18.2-custom.3`)

Everything this fork carries on top of the official release. No later change may regress one of
these. `CUSTOM-BUILD.md` is the short, user-facing list; this file is the engineering inventory:
what each patch does, the files it touches and the tests that hold it.

How it was produced: `git log 7cc73ab..3c1d282` (14 commits, 76 files, +4,337 / −27). The
patches were written on the v0.18.1 release. On 2026-11-18 upstream `main` (`5c69849`, v0.19.0,
132 commits further on) was merged into the fork (`86be392`) with no conflicts; every patch test
and `cargo xtask ci` pass on the result.

## Running the patch tests

Every patch keeps its tests in modules named `patch_tests`, in whichever crate it touches. The
three older patches use the filters `crop`, `custom_build` and `first_slide_number`. This is the
command the fork's Windows build runs before it packages anything
(`.github/workflows/custom-windows.yml`):

```bash
cargo test --locked --lib -p deckcraft-model -p deckcraft-anim -p deckcraft-pptx -p deckcraft-pdf -p deckcraft-engine -p deckcraft-ui-egui -- crop custom_build patch_tests first_slide_number
```

## Feature patches

| # | Commit | Patch | What it does | Commands | Tests |
|---|---|---|---|---|---|
| 1 | `843f45d` | Crop mode | Picture Format › Crop and the picture's right-click menu show crop handles: dragging a handle moves that edge while the picture stays put, dragging inside pans the picture under the frame; Enter/Escape ends it; one undo step per crop; works on rotated pictures. Ported from upstream PR 1907 | `picture.cropMode` | `engine/src/tools/crop.rs` (tool tests, filter `crop`), `ui-egui/src/tests.rs::a_pictures_context_menu_starts_crop_mode` |
| 2 | `6f25a15` | Number slides from | Slide Size dialog gets a "Number slides from" box; `design.slideSize` takes `firstNumber` (0–9999) alone or with a size | `design.slideSize {firstNumber}` | `engine/src/cmd/design/first_number.rs`, `ui-egui/src/dialogs/slide_size.rs`, `ui-egui/src/patch_tests/number_from.rs` (2), `pdf/src/tests.rs` (filter `first_slide_number`) |
| 3 | `28f30a3` | Kerning for fonts | Font dialog checkbox and size set the size from which letter pairs are kerned | `format.fontDialog {kern}` | `engine/src/cmd/format/kerning.rs`, `ui-egui/src/patch_tests/kerning.rs` (3) |
| 4 | `d0f035f` | Reverse Path Direction | Effect Options and the Animation Pane row menu reverse a motion path in place (the object starts at the far end and travels back; curves stay curves). Adds `deckcraft-anim` as a normal dependency of the engine | `animation.reversePath` | `anim/src/reverse.rs`, `engine/src/cmd/animation/reverse.rs`, `ui-egui/src/patch_tests/reverse_path.rs` (2) |
| 5 | `ae4c208` | Recent Colors | A colour from More Colors or the Eyedropper is kept with the presentation (ten, newest first) and offered as a row in every colour menu. Saved in `.deckcraft` and as `p:clrMru` in `.pptx` | `format.recentColor`, `format.recentColors` | `model/src/recent.rs`, `pptx/src/recent_colors.rs`, `engine/src/cmd/format/recent_colors.rs`, `ui-egui/src/patch_tests/recent_colors.rs` (2) |
| 6 | `0d2b18a` | Reading Order pane | Review › Reading Order lists the slide's objects in screen-reader order, with decorative tick boxes, Move Up/Down and drag reorder | `review.readingOrder` (reorders through `arrange.reorder`) | `engine/src/cmd/review/reading_order.rs`, `ui-egui/src/panels/reading_order.rs`, `ui-egui/src/patch_tests/reading_order.rs` (3) |
| 7 | `0df0268` | Crop Fill, Fit, Aspect Ratio; crop fields | Crop menu gains Aspect Ratio, Fill and Fit; the Format Picture pane gets a Crop section (picture size/offset, frame size/place) | `picture.cropFill`, `picture.cropFit`, `picture.cropAspect`, `picture.cropPosition`, `picture.cropInfo` | `engine/src/cmd/shape/crop_fit.rs`, `ui-egui/src/patch_tests/crop_fit.rs` (3) |
| 8 | `837299e` | Save as Picture; TIFF and BMP slides | Right-click › Save as Picture… draws the selected objects together as PNG, JPEG, GIF, TIFF or BMP (2048 px on the longer side at most); `file.export` writes slides as TIFF and BMP. Writes go through the automation write roots | `shape.saveAsPicture`, `file.export {format: tiff\|bmp}` | `engine/src/cmd/shape/save_picture.rs`, `engine/src/picture_export.rs`, `ui-egui/src/patch_tests/save_picture.rs` (3) |
| 9 | `fddee85` | Ink Replay | Draw › Ink Replay redraws the slide's ink stroke by stroke at a steady pen speed (30 s at most); the presentation is never changed | `draw.inkReplay` | `engine/src/ink_replay.rs`, `ui-egui/src/patch_tests/ink_replay.rs` (4) |

Files each feature patch touches (paths under `crates/`):

| # | Files |
|---|---|
| 1 | `engine/src/cmd/shape.rs`, `engine/src/cmd/shape/picture.rs`, `engine/src/keys.rs`, `engine/src/tools.rs`, `engine/src/tools/{click,crop,drag,pointer}.rs`, `ui-egui/src/canvas/{context_menu,input,overlays}.rs`, `ui-egui/src/ribbon/picture_format.rs`, `ui-egui/src/tests.rs` |
| 2 | `engine/src/cmd/design.rs`, `engine/src/cmd/design/{first_number,slide_setup}.rs`, `pdf/src/tests.rs`, `ui-egui/src/dialogs/{body,slide_size}.rs`, `ui-egui/src/lib.rs`, `ui-egui/src/patch_tests.rs`, `ui-egui/src/patch_tests/number_from.rs` |
| 3 | `engine/src/cmd/format.rs`, `engine/src/cmd/format/{font,kerning}.rs`, `ui-egui/src/dialogs/font.rs`, `ui-egui/src/patch_tests/kerning.rs` |
| 4 | `anim/src/lib.rs`, `anim/src/reverse.rs`, `engine/Cargo.toml`, `engine/src/cmd/animation.rs`, `engine/src/cmd/animation/reverse.rs`, `ui-egui/src/dialogs/about.rs`, `ui-egui/src/panels/animation.rs`, `ui-egui/src/ribbon/animations.rs`, `ui-egui/src/patch_tests/reverse_path.rs` |
| 5 | `engine/src/cmd/format.rs`, `engine/src/cmd/format/recent_colors.rs`, `model/src/{defaults,lib,recent}.rs`, `pptx/src/{lib,read,write,recent_colors}.rs`, `ui-egui/src/{app,lib,recent_colors,widgets}.rs`, `ui-egui/src/patch_tests/recent_colors.rs` |
| 6 | `engine/src/cmd/review.rs`, `engine/src/cmd/review/reading_order.rs`, `ui-egui/src/panels.rs`, `ui-egui/src/panels/reading_order.rs`, `ui-egui/src/ribbon/review.rs`, `ui-egui/src/patch_tests/reading_order.rs` |
| 7 | `engine/src/cmd/shape.rs`, `engine/src/cmd/shape/crop_fit.rs`, `engine/src/tools.rs`, `engine/src/tools/crop.rs`, `ui-egui/src/panels.rs`, `ui-egui/src/panels/{crop_section,format_shape}.rs`, `ui-egui/src/ribbon/picture_format.rs`, `ui-egui/src/patch_tests/crop_fit.rs` |
| 8 | `engine/src/cmd/file.rs`, `engine/src/cmd/shape.rs`, `engine/src/cmd/shape/save_picture.rs`, `engine/src/lib.rs`, `engine/src/picture_export.rs`, `ui-egui/src/app/files.rs`, `ui-egui/src/backstage/info.rs`, `ui-egui/src/canvas/context_menu.rs`, `ui-egui/src/{lib,save_picture}.rs`, `ui-egui/src/patch_tests/save_picture.rs`, `docs/mcp.md` |
| 9 | `engine/src/ink_replay.rs`, `engine/src/lib.rs`, `ui-egui/src/{app,canvas,ink_replay,lib}.rs`, `ui-egui/src/app/commands.rs`, `ui-egui/src/ribbon/draw.rs`, `ui-egui/src/patch_tests/ink_replay.rs` |

## Build and identity patches

| # | Commit | Patch | What it does | Files | Tests |
|---|---|---|---|---|---|
| 10 | `38aef04` | Custom builds leave updates to the launcher | Built with `CRAFT_CUSTOM_BUILD` set, the app's own update check reads a version choice that pins it to the launcher's track and the check at start stays off, so it never installs an unpatched official release | `ui-egui/src/dialogs/updates_ui.rs` | `a_custom_build_leaves_updates_to_its_launcher` (filter `custom_build`) |
| 11 | `b2d72e7` | Custom builds carry no ArtCraft marks | Built with `CRAFT_CUSTOM_BUILD` set, the About window says the build is unofficial and based on DeckCraft by the ArtCraft team | `ui-egui/src/dialogs/about.rs` | `dialogs::about::patch_tests::a_custom_build_says_it_is_unofficial_and_an_official_one_says_nothing` (added 2026-11-17) |
| 12 | `787bea1`, `fa4973f`, `3c1d282` | Fork-only build | `custom-windows.yml` builds unsigned Windows x64 packages of `custom/patches` and runs the patch tests first; `CUSTOM-BUILD.md` lists the patches | `.github/workflows/custom-windows.yml`, `CUSTOM-BUILD.md` | The workflow is its own check |

## Known behaviour differences (from `CUSTOM-BUILD.md`)

- Reverse Path Direction was written from the file format, not checked against PowerPoint.
- Recent Colors are kept with the presentation, not per user.
- Reading Order is the stacking order, so reordering there also restacks.
- Ink Replay keeps stroke order but not stroke timing.
- Save as Picture has no SVG, EMF or WMF.

## Gaps

1. **Patch 11 had no test; it has one now.** The notice was a compile-time `option_env!` branch
   inside the About window's draw function. It moved into `custom_build_notice(label)`, the same
   shape as patch 10's `custom_build_updater`, and a test covers both arms. The About window draws
   the same text as before.
2. **Patch 11 covers the About window only.** The product is still called DeckCraft everywhere and
   links to `getartcraft.com` (`engine/src/links.rs`, the Windows installer's `ArtCraft` program
   folder in `packaging/windows/deckcraft.wxs`, the Discord link in the backstage). No ArtCraft
   logo is compiled into the app: the marks live only in `docs/brand/` and `README.md`. A renamed
   product needs all of these replaced (`docs/recon.md` §8).
3. **The fork's convention about upstream progress documents.** `ROADMAP.md`, `docs/gaps.md` and
   `docs/parity-checklist.md` are deliberately left as the official release has them, so that
   moving the patches onto the next release does not conflict. New work in this fork records its
   progress in `docs/reports/` and `docs/porting-log.md` instead.

## Verification log

| Date | What ran | Result |
|---|---|---|
| 2026-11-17 | The patch-test command above at `3c1d282`, on Linux x64, with craft-fonts at `cc9269c` | 93 passed, 0 failed (model 9, anim 49, pptx 2, pdf 1, engine 2, ui-egui 30) |
| 2026-11-17 | After adding patch 11's test: `cargo xtask ci` | All 8 steps passed (fmt, pdf-deps, clippy, test, doctest, assets, layers, wasm); the new test is among them |
| 2026-11-18 | `cargo xtask ci` after merging upstream `main` v0.19.0 (`86be392`) | All 8 steps passed; 2,179 tests, 0 failed |
