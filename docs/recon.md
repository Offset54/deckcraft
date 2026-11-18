# Phase 0 reconnaissance

> **Written:** 2026-11-17 · **Status:** Phase 0 complete; base moved to upstream v0.19.0 on 2026-11-18 (decision 2), Phase 1 under way · **No feature code changed** (one test added for a fork patch)

What the three source apps look like today, how their models and command systems compare, how the
host renders against the alternatives, what this fork carries, and what is most likely to go
wrong. Everything here was read from the code at the commits below or measured in this session;
§10 says what ran and what did not.

| Repository | Commit | Version | Role |
|---|---|---|---|
| this fork (`custom/patches`) | `3c1d282` | 0.18.1 + 14 commits | Host |
| `storytold/deckcraft` `main` | `5c69849` | 0.19.0 | Upstream of the host |
| `storytold/figcraft` `main` | `954687f` | 0.18.1 | Source: canvas, layout, design system |
| `storytold/designcraft` `main` | `47ef1ec` | 0.21.0 | Source: typography, print PDF |
| `storytold/craft-libs` `main` | `de97c35` | — | Shared libraries all three depend on |
| `storytold/craft-fonts` `main` | `cc9269c` | — | Fonts, cloned next to the repo |

## 1. Findings that change the brief

The brief was written from the apps' descriptions. Six of its assumptions do not match the code.

1. **The host already renders with `vello_cpu`.** `crates/render` draws slides with
   `vello_cpu` 0.2, the same crate and version DesignCraft uses. `wgpu` only presents egui's
   textures. There is nothing to adopt; §5 has the numbers.
2. **The host already has enforced layering, copy-on-write undo, a slide sorter and name-based
   Morph.** `cargo xtask layers` exists (`xtask/src/layers.rs`). Undo is a stack of
   `Arc<Presentation>` snapshots whose slides and masters are themselves `Arc`. The Slide Sorter
   is a grid view. Morph matches by `!!name`, then name, then text, then geometry
   (`crates/anim/src/morph.rs`); only matching by a stable id is missing.
3. **Every host command is already reachable from MCP.** The MCP server has a generic
   `run_command` tool, so all 342 engine commands are exposed by construction. The only gap is
   the UI-only commands, which need a running app (`call_app`).
4. **None of the FigCraft crates named as "lift" candidates is self-contained.** `figcraft-layout`,
   `figcraft-pathops`, `figcraft-text` and `figcraft-render` all depend on `figcraft-doc`, and the
   layout solver mutates a `figcraft_doc::Document` directly. The same holds for DesignCraft's
   `compose`, whose entry point is `compose(doc: &Document, story: &Story, …)`.
5. **The self-contained pieces already exist as shared crates in `craft-libs`.** `craft-text` has a
   font-independent Knuth–Plass and greedy line breaker on a box/glue/penalty model, hyphenation
   with the bundled US English dictionary, bidi and segmentation. `craft-fig` reads the `.fig`
   container, `craft-idml` reads IDML packages, `craft-pdf` (which the host already uses) is the
   PDF writer. Upstream DeckCraft added a rule on 2026-11-16 (`f964d06`) to prefer these over local
   copies.
6. **`figcraft-doc` cannot be lifted for licence reasons.** Its node store is `imbl`
   (MPL-2.0-or-later), and `figcraft-text` uses `icu_segmenter` (Unicode-3.0). Neither identifier
   is in this repository's licence allowlist (`AGENTS.md`).

The net effect: Phase 1 is much smaller than the brief assumes, and Phases 2–4 are
re-implementation against the host model with the sibling code as the behavioural reference, not
crate lifts.

## 2. Workspace maps

Line counts are `wc -l` over every `.rs` file, tests included.

### 2.1 Host: DeckCraft fork (235k lines, 20 crates, 3 apps)

| Layer | Crate | Lines | Role |
|---|---|---|---|
| L0 | `geom`, `color`, `metafile`, `bitstream` | 6.0k, 0.8k, 7.5k, 0.1k | Units, transforms, ~180 preset shapes; DrawingML colour; EMF/WMF reader |
| L1 | `model`, `fonts`; `h264`, `hevc`, `vp9`, `av1` | 9.9k, 3.7k; 45.2k | Presentation model, placeholder inheritance; font database; video decoders |
| L2 | `text`, `anim`, `format`, `media` | 6.0k, 6.5k, 2.4k, 3.1k | Shaping and line layout; animation timeline, transitions, Morph; `.deckcraft`; media playback |
| L3 | `render`, `pptx` | 15.4k, 22.2k | `vello_cpu` raster; PPTX read/write |
| L4 | `pdf` | 1.5k | PDF export through `craft-pdf` |
| L5 | `engine` | 47.3k | Session, history, 342 commands, pointer tools, keys, recovery, panic guard |
| L6 | `ui-egui`, `mcp` | 48.8k, 1.9k | egui frontend and control channel; MCP server |
| apps | `deckcraft`, `deckcraft-cli`, `deckcraft-web` | 4.9k | Desktop, headless CLI, WASM |

### 2.2 FigCraft (374k lines, 17 crates, 4 apps)

| Layer | Crate | Lines | Role |
|---|---|---|---|
| L0 | `geom`, `color` | 1.8k, 2.4k | Affine and shape builders; colour spaces incl. Display P3, paints, blend modes |
| L1 | `doc` | 16.0k | Flat node map (`imbl`), properties, components, styles, variables, prototype data, comments |
| L2 | `layout`, `text`, `pathops`, `collab`, `plugin` | 3.5k, 9.3k, 3.2k, 10.2k, 2.9k | Auto layout and constraints; shaping and line layout; booleans (`linesweeper`) and stroke outline; collaboration core; WASM sandbox (`wasmi`) |
| L3 | `render`, `format` | 13.8k, 41.7k | `tiny-skia` raster with retained rendering; `.figcraft`, REST JSON, `.fig`, Sketch, Penpot |
| L4 | `svg`, `pdf`, `vectordrawable` | 6.6k, 2.8k, 2.3k | Exports |
| L5 | `engine` | 130.9k | Session, history, 498 commands, tools, component sync, prototype runtime, Dev Mode |
| L6 | `ui-egui`, `mcp`, `collab-net` | 94.2k, 4.7k, 9.7k | Frontend (AccessKit), MCP, WebSocket sessions |
| apps | `figcraft`, `-cli`, `-web`, `-server` | 18.0k | Desktop, CLI, WASM, collaboration server |

### 2.3 DesignCraft (342k lines, 17 crates, 3 apps)

| Layer | Crate | Lines | Role |
|---|---|---|---|
| L0 | `geom`, `color` | 3.3k, 3.4k | Units and measurement parsing; CMYK/RGB/spot, ICC |
| L1 | `doc`, `fonts` | 20.0k, 5.8k | Spreads, items, stories, five kinds of style, tables; font database and shaping |
| L2 | `compose`, `images` | 29.2k, 3.9k | The text engine (see §3); image decoding |
| L3 | `render`, `pdf`, `idml`, `epub`, `pptx`, `textimport`, `format` | 11.7k, 11.5k, 17.5k, 3.1k, 2.4k, 2.2k, 1.1k | `vello_cpu` raster with tiles and damage regions; PDF/X-4, PDF/A-2b; IDML; EPUB; a one-way PPTX export |
| L4 | `tools` | 10.8k | Pointer tools, snapping, smart guides |
| L5 | `engine` | 87.8k | Session, history, ~560 commands, compose cache, preflight |
| L6 | `ui-egui`, `mcp` | 117.3k, 2.8k | Frontend, MCP |

In both source apps the engine and UI crates are about 60% of the code. They are written against
their own `Session` and document and are reference material only.

### 2.4 Shared libraries relevant here (`craft-libs`)

| Crate | What it gives | Used today by |
|---|---|---|
| `craft-text` (5.1k) | Knuth–Plass and greedy line breaking on boxes/glue/penalties, balanced rag, hyphenation (US English bundled, 47 languages through `hypher`), bidi, UAX #14 with kinsoku | Not yet by any of the three |
| `craft-pdf` (31k) | PDF read, render and write over Krilla | Host (write only), DesignCraft |
| `craft-prepress` (13k), `craft-cms` (10k) | Ink model, plates, ICC | DesignCraft |
| `craft-fig` (7.5k) | `.fig` container, kiwi schema, vector blobs | FigCraft |
| `craft-idml` (8.4k) | IDML package, designmap, value codecs | DesignCraft |
| `craft-drawingml`, `craft-deckcraft-format`, `craft-opc`, `craft-metafile`, `craft-video` | DrawingML geometry and colour, the `.deckcraft` package, OPC names, EMF/WMF, video decoders | Upstream DeckCraft `main` only (not this fork's base) |
| `craft-mcp-server`, `craft-control-protocol` | MCP and control-channel machinery | — |

## 3. Key types

| | Host (DeckCraft) | FigCraft | DesignCraft |
|---|---|---|---|
| Document | `model::Presentation` — `slides: Vec<Arc<Slide>>`, `masters: Vec<Arc<Master>>` | `doc::Document` — `nodes: imbl::OrdMap<NodeId, Arc<Node>>`, `pages: Vec<NodeId>` | `doc::Document` — `spreads: Vec<Arc<Spread>>`, `stories: BTreeMap<StoryId, Arc<Story>>`, `styles: Arc<Styles>` |
| Node | `model::Shape` in a nested `Vec<Shape>`; `ShapeKind::{Shape, Picture, Group, Connector, Table, Chart, Media, Ink, Model3d, Opaque}` | `doc::Node` in a flat map with `parent` and `children: Vec<NodeId>`; `NodeKind::{Page, Frame, Group, Section, Rectangle, …, Vector, Text, BooleanOp, Component, ComponentSet, Instance, Slice}` | `doc::Item` under `Spread.items: Vec<Arc<Item>>`; text lives in `Story`, not in the item |
| Geometry | `Xfrm` (offset, extent, rotation, flips) in points; `Option` = inherited | `transform: Affine` + `width`, `height` | `xf: Affine` + path |
| Paint | One `Option<Fill>`, one `Option<Line>`, `Option<Effects>`; `None` = inherited | `fills: Vec<Paint>`, `strokes: Vec<Paint>`, `effects: Vec<Effect>` | One `Fill`, one `Stroke`, `Effects` |
| Text | `TextBody` on the shape: paragraphs of runs with DrawingML properties | `TextData` on the node: string + styled ranges | `Story`: one `String` + paragraph and character run lists, threaded over `frames: Vec<ItemId>` |
| Inheritance | slide → layout → master → theme, resolved at render time (`model::resolve`) | instance → main component with per-layer overrides; variables with modes and aliases | based-on style chains; parent spreads with per-page overrides |
| Command | `engine::cmd::CommandSpec {id, label, menu, shortcut, params, enabled, run, journal, undoable}`, `cmd!` | `engine::cmd::CommandSpec {id, label, params, history, run}`, `spec!`; menus and shortcuts in `catalog.rs` | Same struct as the host, `cmd!` |
| Run signature | `fn(&mut Session, &Value) -> Result<Value>` | same | same |
| Dispatch | `Session::execute(id, params)` inside a panic guard | same | same |
| History | `History {undo, redo: Vec<HistoryEntry>}`, entry = `Arc<Presentation>` + selection; byte budget | `Arc<Document>` + selection + page; grouped steps | `Arc<Document>` + selection |
| Renderer entry | `render::render_slide(&Presentation, index, &RenderOpts) -> Image` | `render::render_view(&Document, page, &View, &Options) -> Option<Pixmap>` | `render::Renderer` (tiles, damage regions) |
| Text layout entry | `text::layout(&Ctx, &Shape, &TextBody, &Opts) -> TextLayout` | `text::layout(&TextData, width, height) -> TextLayout`, cached | `compose::compose(&Document, &Story, &[FrameSpec], &ComposeOptions) -> ComposedStory` |
| MCP tools | `mcp::tools::tool_definitions()` / `call_tool`; generic `run_command`, `batch` | `mcp` with tools, prompts and resources | `mcp` |

## 4. Document models and command registries, side by side

### 4.1 Where the models differ

- **Tree shape.** The host nests shapes in vectors, as PresentationML does. FigCraft keeps a flat,
  persistent map of nodes keyed by a stable id. FigCraft's layout solver, component sync and
  collaboration core all rely on that map.
- **Inheritance direction.** The host inherits downward from masters (a placeholder with
  `xfrm: None` takes its box from the layout). FigCraft copies a component's subtree into each
  instance and re-syncs. These are different mechanisms and both have to exist.
- **Paint stacks.** FigCraft allows several fills and strokes per node; the host and PowerPoint
  allow one.
- **Text ownership.** A DesignCraft story belongs to the document and flows through frames. A
  host text body belongs to one shape.
- **Ids.** Host ids come from one deck-wide counter, so a duplicated slide's shapes get new ids
  and nothing links a shape to its counterpart on the next slide.

### 4.2 Proposed mapping onto the host

The proposal keeps the host's nested, PPTX-shaped tree and adds optional fields that default to
"absent", so existing files, commands and the PPTX writer are unaffected until a feature is used.

| Source concept | Host today | Proposed host form | On `.pptx` export |
|---|---|---|---|
| FigCraft top-level Frame | `Slide` | `Slide` (a `.fig` import turns each top-level frame into a slide) | Slide |
| FigCraft nested Frame | `ShapeKind::Group` | A frame is a group with its own size, optional fill, clip and optional `AutoLayout`; children carry `ChildLayout` (hug/fill/fixed, alignment, grid cell) | Group with resolved absolute boxes |
| FigCraft Group, Section | `ShapeKind::Group`; `Section` (of slides) | Unchanged | Unchanged |
| Constraints | — | `constraints` on a shape inside a frame, applied on frame resize | Resolved boxes |
| Component, ComponentSet | — | Deck-level component library of `Arc`'d shape subtrees with property definitions and variants | Not written (instances are flattened) |
| Instance | — | `ShapeKind::Instance {component, props, overrides}`; materialised children cached | Group of the materialised shapes; component link kept in an extension element for our own round trip |
| Layout / master placeholder | `Layout`, `Master`, `Placeholder` | Unchanged. The layout library is ordinary layouts whose placeholders sit inside auto-layout frames | Layouts with placeholders at their resolved boxes, so PowerPoint still treats them as placeholders |
| Variables, collections, modes, aliases | `Theme` (12 colour slots, major/minor fonts), `ColorRef::Scheme` | `Presentation.variables` as in FigCraft; `bound: BTreeMap<field, VariableId>` on shapes, runs and slides; a mode set per deck, slide or frame. Bindings resolve at render time, as scheme colours do today | The active mode is written into the theme's colour and font scheme where a variable maps to one of the 12 slots or the two fonts; every other binding is written as its resolved value |
| Paint, text, effect styles | Shape styles from the theme (`ShapeStyle`), list styles | Named styles in the deck, resolved before the placeholder chain | Direct formatting |
| DesignCraft Story | `TextBody` | `TextBody` stays the stored form. Threading is a link from a shape to the next shape in the chain; the first shape owns the body | Each box gets the text that fell in it |
| Paragraph and character styles | List styles per level | Named styles referenced from paragraphs and runs | Direct formatting; the style name kept in an extension element |
| Composer settings (Knuth–Plass, hyphenation, optical margins, baseline grid) | Greedy UAX #14 breaking, justification, autofit | Per-paragraph and per-body settings feeding `craft-text` | Not written; PowerPoint breaks lines itself |
| Text wrap | — | `wrap` on a shape; exclusions become the composer's per-line widths | Not written |
| DesignCraft table | `model::Table` with styles | Extend the existing table (row and column styles, decimal alignment) | Table, with direct formatting |
| Vector network, editable boolean | `Geom::Custom` paths, Edit Points, destructive `shape.merge` (`i_overlay`) | `Geom::Custom` stays; a boolean group keeps its operands | `custGeom` of the result |
| Effects (background blur, noise, glass, progressive blur) | DrawingML effects, 3-D, artistic effects | Additional effect kinds | A picture of the shape where DrawingML has no equivalent |
| Prototype connection | `Shape.click` / `hover` hyperlink actions, animation triggers | Slide links map onto the existing click actions | Native PowerPoint action |
| Comment pin | `Slide.comments` with x, y and shape | Unchanged | Unchanged |
| Stable identity for Smart Animate | Name matching | An `origin` id copied when a shape or slide is duplicated, matched before the name | Not written (name matching still works in PowerPoint) |

Two consequences follow from the table:

- A variable mode switch does not rewrite slides. It changes one value in the deck and the
  renderer resolves bindings as it draws, which is how theme colours work now. That is what makes
  the 100 ms budget for 100 slides reachable.
- Everything in the right-hand column that says "not written" or "flattened" has to survive our
  own `.pptx` round trip. The host already has the mechanism (`raw_ext` and `KeptPart`); each
  feature has to use it and be tested for it.

### 4.3 Command registries

| | Host | FigCraft | DesignCraft |
|---|---|---|---|
| Registered commands | 342 (`deckcraft-cli commands`) | 498 `spec!` registrations | ~560 `cmd!` registrations |
| Spec fields | id, label, menu, shortcut, params, enabled, run, journal, undoable | id, label, params, history, run | as host |
| Menu and shortcut | In the spec | Separate `catalog.rs` and `keymap.rs` | In the spec |
| `enabled` check | Yes, with a reason string | No | Yes |
| Variants | `cmd!`, `cmd!(query …)`, `cmd!(noundo …)` | `history: bool` | as host |
| Id style | PowerPoint ribbon (`slide.new`, `format.bold`, `arrange.group`) | Figma menus (`layout.add`, `component.create`, `variable.addMode`) | InDesign menus |
| Nested commands | One undo step for the outermost | Grouped steps | One undo step |
| Gestures | `Session::pointer`, `Session::key` | Tools in the engine | Separate `tools` crate, Begin/Preview/Commit |

The host and DesignCraft registries are the same design. A FigCraft command ports by rewriting its
body against the host model and adding an `enabled` function; nothing about the registry itself
needs to change. New commands take new namespaces in the host's style: `layout.*`, `component.*`,
`variable.*`, `style.*`, `vector.*`, and typographic settings under the existing `format.*`.

## 5. Renderers

### 5.1 What each app uses

| App | Rasterizer | Around it |
|---|---|---|
| Host | `vello_cpu` 0.2, single-threaded by default (`RenderOpts::threads`) | Whole-slide render to an RGBA image uploaded as an egui texture; show transitions composite cached slide textures |
| DesignCraft | `vello_cpu` 0.2 | Background worker, tiles, damage regions, mip-mapped images |
| FigCraft | `tiny-skia` 0.12 | Worker thread, banded parallel rendering, retained image re-rendering only what changed, levels of detail when zoomed out |

### 5.2 Measurements

Linux x64 cloud container with 4 virtual CPUs, release build with thin LTO, medians. This is not
the mid-range laptop the performance budget names, so read the numbers as relative.

Host renderer on the built-in sample deck (8 slides) and on a 100-slide deck made by duplicating
those 8 evenly, single-threaded:

| Size | Per slide, sample (min–max) | All 8 | All 100 |
|---|---|---|---|
| Thumbnail, 0.2× (192×108) | 0.29–1.42 ms | 5.5 ms | 64 ms |
| 1× (960×540) | 0.65–9.0 ms | 19.4 ms | 251 ms |
| 2× (1920×1080) | 1.7–14.8 ms | 34.4 ms | 445 ms |

With four worker threads the same runs took 115 ms, 291 ms and 458 ms for 100 slides: these
slides are too light for threading to help, and at thumbnail size it costs.

The same synthetic slide-like scene (1,581 paths: 40 rounded cards, 40 stroked ellipses, 1,500
glyph-sized curved outlines) through both rasterizers:

| Size | `vello_cpu`, 1 thread | `vello_cpu`, 4 threads | `tiny-skia` | `tiny-skia` vs `vello_cpu` 1 thread |
|---|---|---|---|---|
| Thumbnail, 0.2× | 1.37 ms | 1.06 ms | 2.63 ms | 1.9× slower |
| 1× | 3.23 ms | 1.91 ms | 11.35 ms | 3.5× slower |
| 2× | 6.39 ms | 3.25 ms | 26.55 ms | 4.2× slower |

The sample deck could not be drawn by FigCraft's or DesignCraft's own renderers: each draws only
its own document model. The synthetic scene is the like-for-like comparison of the rasterizers
themselves; it has no text shaping, images, gradients or effects.

Other baselines for the performance budget, on the 100-slide deck (`deckcraft-cli bench`):

| Step | `.deckcraft` | `.pptx` | Budget |
|---|---|---|---|
| Open | 9 ms | 16–22 ms | 1,500 ms |
| Save | 4 ms | 14–17 ms | — |
| Export PDF | 2.9 s | 3.1 s | — |
| Peak memory | 220 MiB | 220 MiB | — |

The sample slides hold no pictures or video, so a real deck will open more slowly than this.

### 5.3 Recommendation

**Change nothing in the rasterizer.** The host is already on `vello_cpu` for the canvas, export
and thumbnails, and `vello_cpu` is two to four times faster than `tiny-skia` on the same geometry.

What is worth taking from the siblings is the machinery around the rasterizer, and only when a
phase needs it:

- DesignCraft's worker, tiles and damage regions, for editing a slide at high zoom.
- FigCraft's retained image and levels of detail, for a grid view of a hundred or more slides.
- FigCraft's effect implementations (background blur, glass, noise) are written against
  `tiny-skia` pixmaps and have to be rewritten on the host's filter path.

`vello_cpu` 0.4 is on crates.io; the host and DesignCraft are on 0.2. An upgrade is a separate,
upstream-shaped task and is not proposed here.

## 6. Fork patches

Fourteen commits on top of v0.18.1: nine feature patches, two custom-build identity patches and
three fork-only build commits. The full inventory with files and tests is in
[`fork-patches.md`](fork-patches.md).

| Patch | Commands | Has tests |
|---|---|---|
| Crop mode | `picture.cropMode` | Yes |
| Number slides from | `design.slideSize {firstNumber}` | Yes |
| Kerning for fonts | `format.fontDialog {kern}` | Yes |
| Reverse Path Direction | `animation.reversePath` | Yes |
| Recent Colors | `format.recentColor`, `format.recentColors` | Yes |
| Reading Order pane | `review.readingOrder` | Yes |
| Crop Fill, Fit, Aspect Ratio; crop fields | `picture.cropFill`, `.cropFit`, `.cropAspect`, `.cropPosition`, `.cropInfo` | Yes |
| Save as Picture; TIFF and BMP slides | `shape.saveAsPicture`, `file.export {format}` | Yes |
| Ink Replay | `draw.inkReplay` | Yes |
| Custom builds leave updates to the launcher | — | Yes |
| Custom builds carry no ArtCraft marks | — | Yes (added in this phase) |
| Fork-only Windows build and patch list | — | The workflow itself |

One patch had no test. This phase added it: the About window's custom-build notice moved into a
function that a test can call, with no change to what the window shows.

## 7. The fork against upstream

- The fork is based on the v0.18.1 release, not on upstream `main`.
- Upstream `main` is 132 commits ahead at v0.19.0: 221 files changed, +3,462 / −59,978. Most of
  the deletions are code moving into `craft-libs`: the video codecs, geometry and colour
  (`craft-drawingml`), the `.deckcraft` package layer, OPC names, base64.
- A trial three-way merge of upstream `main` into the fork head (`git merge-tree`) reports no
  textual conflict. It was not compiled or tested.
- The three apps pin different `craft-libs` revisions (`craft-pdf`: host `6e03f65`, DesignCraft
  `a5ac3aa`; Krilla: FigCraft `e11f9d9`). Taking a shared crate at a sibling's revision into the
  host can force the host's other `craft-libs` pins to move with it.

## 8. Branding and licences

- **Marks.** `docs/brand/LICENSE-brand.txt` requires a published modified version to remove the
  ArtCraft marks and not present itself as an ArtCraft product; saying in plain text that it is
  based on DeckCraft by the ArtCraft team is allowed. No ArtCraft logo is compiled into the app.
  What remains for a renamed product: the product name itself in the UI, bundle ids and docs;
  `docs/brand/` and the logo block in `README.md`; the `getartcraft.com` and Discord links
  (`crates/engine/src/links.rs`, backstage); the `ArtCraft` program folder in the Windows
  installer (`packaging/windows/deckcraft.wxs`).
- **Code licences.** All three apps and `craft-libs` are `MIT OR Apache-2.0`. Code may be copied
  between Craft apps (`AGENTS.md` §2) with headers and `NOTICE` entries carried over.
- **Dependencies outside the host allowlist.** `imbl` (MPL-2.0-or-later) under FigCraft's
  document and collaboration; `icu_segmenter` (Unicode-3.0) in FigCraft's text. The proposal in
  §4.2 avoids both.
- **Hyphenation data.** US English is public-domain and CC0 (DesignCraft's own patterns, also in
  `craft-text`). Other languages come from `hypher` with per-language pattern licences listed in
  DesignCraft's and `craft-libs`' `NOTICE`; Slovak is GPL and is kept behind a separate feature.
  Each language added needs its licence checked against the allowlist.
- **Test decks.** `AGENTS.md` §1.1 forbids committing files produced by PowerPoint. The ten
  real-world decks the brief places in `testdata/pptx/` cannot be committed if PowerPoint wrote
  them; they need to live outside the repository.

## 9. Porting approach by capability (preliminary)

Each decision is recorded properly in `docs/porting-log.md` when its phase starts.

| Capability | Source | Approach | Why |
|---|---|---|---|
| Knuth–Plass, greedy and balanced breaking | DesignCraft `compose/breaker.rs`; `craft-text` | Depend on `craft-text` | Already extracted, font-independent, permissively licensed |
| Hyphenation | DesignCraft `compose/hyphen`; `craft-text` | Depend on `craft-text` | Same data, already shared |
| Optical margins, baseline grid, wrap, threading, styles | DesignCraft `compose` (29k) | Re-implement on the host's `text` crate | Bound to `designcraft_doc::Story`, `Styles` and `Item` |
| Auto layout, constraints | FigCraft `layout` (1.2k + 2.3k of tests) | Re-implement, porting the tests | The solver reads and writes `figcraft_doc::Document` |
| Booleans | FigCraft `pathops` (`linesweeper`) | Decide in Phase 3: keep the host's `i_overlay`, or wrap `linesweeper` directly | The FigCraft wrapper depends on `figcraft-doc` types; the path functions themselves take `BezPath` |
| Variables, components, styles | FigCraft `doc` and `engine` | Re-implement the data types; port behaviour from the engine tests | `imbl`; flat node map |
| Effects | FigCraft `render` | Re-implement on `vello_cpu` | Written against `tiny-skia` |
| Smart Animate | FigCraft `ui-egui/src/present` | Extend the host's `anim::morph` | FigCraft's lives in its UI crate; the host's is already in a lower layer |
| Collaboration | FigCraft `collab`, `collab-net` | Open question | The core syncs a flat object/property model the host does not have |
| Plugins | FigCraft `plugin` (`wasmi`) | Likely liftable; the host API is new | The crate depends only on `figcraft-color`, not on the document. To confirm in Phase 7 |
| Print PDF (PDF/X-4, PDF/A-2b) | DesignCraft `pdf` on `craft-pdf`, `craft-prepress` | Extend the host's `pdf` crate using the same shared crates | The standard handling is in `craft-pdf`/Krilla; the 11k lines in DesignCraft map its own document |
| `.fig` import | `craft-fig`; FigCraft `format/fig` | Depend on `craft-fig`; write the mapping to slides | Container is shared; mapping is per app |
| IDML import | `craft-idml`; DesignCraft `idml` | Depend on `craft-idml`; write the mapping to slides | Same |

## 10. Top five risks

1. **Model mismatch drives everything.** The FigCraft features the product wants most (auto
   layout, components, variables, collaboration) are written against a flat node map with affine
   transforms and paint stacks; the host is a nested, PPTX-shaped tree with single fills and
   inherited boxes. Keeping the host tree is the only way to keep the 342 commands and the PPTX
   writer, but it means every one of those features is new code, and collaboration may not fit at
   all. *Mitigation:* additive optional fields; port the siblings' behaviour tests as the
   specification; decide the collaboration question before promising it.
2. **PPTX round trip.** Every native-only concept needs a flattening on export and a way back on
   reopen, and the new composer changes line breaks, which changes auto-fit scales and the heights
   of auto-sizing boxes that get saved. The host has no golden-image harness today to catch
   drift. *Mitigation:* build the golden-image and round-trip suites first; keep the current text
   engine as the default until they pass; treat a changed saved geometry as a failure.
3. **Upstream is moving fast, and toward shared crates.** The fork sits on a release that is 132
   commits and one minor version behind, while upstream is cutting whole crates over to
   `craft-libs`. Work done here on `geom`, `color`, `format` or `pptx` lands on code upstream has
   already replaced, and every phase makes the patches harder to move. *Mitigation:* decide the
   base before Phase 1 (§11, decision 2).
4. **Size.** The two source apps are 374k and 342k lines against the host's 235k, and Phases 2–7
   draw on most of both. The portable parts are the small lower crates; the engines and UIs are
   not portable. Each phase is weeks of work, and a phase that stops halfway leaves a feature that
   exists in the model but not in the UI or the file format. *Mitigation:* the phase gates in the
   brief; one thin vertical slice (model, command, render, file, MCP test) before widening.
5. **Licences and brand.** Lifting FigCraft's document or collaboration core would bring in
   MPL-licensed `imbl`; hyphenation languages carry their own licences; and the product cannot
   ship under a new name while ArtCraft links, folders and marks remain. *Mitigation:* the
   re-implementation route above; a licence check per added language; a rebrand pass once the
   name is chosen.

## 11. Decisions needed

1. **Placeholders.** *Decided 2026-11-18: SlideCraft, `io.github.offset54.slidecraft`, `.slidecraft` ([`decisions/001-product-identity.md`](decisions/001-product-identity.md)).* The brief's `{{APP_NAME}}`, `{{EXT}}` and the optional image-generation
   bridge are unfilled. Phase 1 does not need them; the rebrand and the native format do. This
   document assumes `{{MY_DECKCRAFT_FORK_URL}}` is this repository.
2. **Base.** *Decided 2026-11-18: move first.* Stay on v0.18.1, or move the patches onto upstream `main` (v0.19.0) before Phase 1.
   Recommended: move first. The trial merge is clean, and Phases 2 and 6 want the `craft-libs`
   cut-overs upstream already made.
3. **Shared crates.** Use `craft-text`, `craft-fig`, `craft-idml` and `craft-pdf` from
   `craft-libs` rather than copying the equivalent code out of DesignCraft and FigCraft.
   Recommended: yes; it is also upstream's current rule.
4. **Document tree.** Keep the host's nested shape tree and add frames, layout, instances and
   bindings as optional fields (§4.2), or migrate the host to a flat node map. Recommended: keep
   the tree. This is the expensive-to-undo choice and gets a `docs/decisions/` record before
   Phase 3.
5. **Collaboration.** Keep it in scope as a stretch goal knowing it likely needs its own sync
   model, or drop it.
6. **Branches and merging.** The brief asks for a branch per phase. Where those merge
   (`custom/patches`, or a new long-lived product branch) and whether the fork's own workflow may
   change needs stating; `AGENTS.md` forbids CI changes in feature work.
7. **Real-world decks.** Where the ten test decks live, given that PowerPoint-produced files may
   not be committed.

## 12. What ran

| Check | Result |
|---|---|
| `cargo build --release -p deckcraft-cli` | Built in 3 min 40 s |
| `deckcraft-cli bench` on the sample and on the 100-slide deck (`.deckcraft` and `.pptx`) | Ran; numbers in §5.2 |
| Renderer harness (host at three sizes; `vello_cpu` against `tiny-skia`) | Ran; numbers in §5.2. The harness is a scratch crate and is not in the repository |
| Fork patch tests (command in `fork-patches.md`) at the fork head | 93 passed, 0 failed |
| `git merge-tree` of upstream `main` into the fork head | No conflicts; result not compiled |
| FigCraft, DesignCraft | Read, not built |
| `cargo xtask ci` with the added test | All 8 steps passed: fmt, pdf-deps, clippy, test, doctest, assets, layers, wasm |

Two things about the machine it ran on. The container needed three additions before CI could
pass, none of them a code problem: the ALSA development headers (`libasound2-dev`, for the desktop
app's audio), a Vulkan driver (`mesa-vulkan-drivers`: 13 UI tests draw through wgpu and found no
adapter without it) and the `wasm32-unknown-unknown` Rust target. `cargo-nextest` is not installed,
so the tests ran under `cargo test`.
