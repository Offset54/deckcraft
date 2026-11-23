# Agent prompt: build "{{APP_NAME}}", a Figma Slides–class presentation app from DeckCraft + FigCraft + DesignCraft

> Fill in the `{{…}}` placeholders before handing this to the agent.

---

## 0. Your role and the goal

You are a senior Rust engineer working on a native presentation app. Your job is to turn **my patched fork of DeckCraft** into **{{APP_NAME}}**: one app that makes beautiful presentations easily. It should be as pleasant to use as Figma Slides, and it should have the strongest features of three open-source ArtCraft "Crafting Apps":

| Repo | What it is | What we want from it |
|---|---|---|
| `{{MY_DECKCRAFT_FORK_URL}}` (forked from `github.com/storytold/deckcraft`) | PowerPoint-class presentation app in pure Rust | **The host app.** Deck model, masters/layouts/themes, `.pptx` round-trip, transitions incl. Morph, animations, slide show, presenter view, charts, media, PDF/PNG/GIF export, AutoRecover, plus all of my patches |
| `github.com/storytold/figcraft` | Figma-class interface design tool in pure Rust | Canvas UX, auto layout, components/variants, variables + modes, vector networks, editable booleans, effects, collaboration, comments, Dev-Mode-style inspection, WASM plugins |
| `github.com/storytold/designcraft` | InDesign-class page layout in pure Rust | Pro typography: Knuth–Plass paragraph composer, hyphenation, optical margin alignment, baseline grid, paragraph/character styles, threaded text, text wrap, tables, print-grade PDF (PDF/X-4, PDF/A-2b), the fast `vello_cpu` renderer, copy-on-write O(1) undo |
| `github.com/storytold/craft-fonts` | Shared font repository | Clone next to the repo as `../craft-fonts` |

**Product thesis:** *Figma's canvas and design system, InDesign's typography, PowerPoint's presenting and compatibility, all in one fast native app that an AI agent can drive end to end.*

The end user is not a designer. Every feature has to answer this question: **does it make a good-looking deck easier to get?** Pick good defaults first and expose power second.

---

## 1. Ground rules (read before touching code)

1. **DeckCraft fork is the host.** Do not start a new workspace, and do not make FigCraft or DesignCraft the base. You *port* capabilities into the host. You do not merge three apps side by side.
2. **Preserve my patches.** Before anything else, diff my fork against `storytold/deckcraft` upstream (`git remote add upstream …; git fetch upstream; git log upstream/main..HEAD; git diff upstream/main...HEAD --stat`). Write `docs/fork-patches.md` that lists each patch, what it does, and which files it touches. No later change may silently regress one of them. Add a test for each patch that doesn't already have one.
3. **Port, don't vendor blindly.** The three apps share a philosophy (engine-first, a pure-data document model, a command registry, an egui UI on top, a JSON control channel, an MCP server). Their internals still differ: renderers (DeckCraft uses wgpu, FigCraft tiny-skia, DesignCraft vello_cpu), document models, and command registries. For each capability, first read the source crate and pick one approach:
   - **Lift as a crate** when it's self-contained (likely candidates: DesignCraft `compose`, FigCraft's auto-layout solver, FigCraft boolean ops via `linesweeper`).
   - **Re-implement against the host model** when it's tangled with the source app's document model.
   - Write the decision and its reasons in `docs/porting-log.md`.
4. **Keep the layering clean.** Copy DesignCraft's enforced layering (`cargo xtask layers`) into the host: geometry/color → document → text/images → render/import-export → tools → engine → UI/MCP → apps. New code goes in the lowest layer it can. The UI must stay swappable.
5. **Every feature is a command.** Every user action is a registered command with a stable id, undo/redo support, and exposure through the GUI, the CLI (`run --cmd`), the JSON control channel and the MCP server. If an agent can't do something, it isn't done.
6. **Licenses and attribution.** All three repos are MIT OR Apache-2.0. Keep the copyright headers and carry over each source repo's `NOTICE` and `ATTRIBUTION.md` entries for any code or assets you port. Fonts keep their OFL. **The ArtCraft name, wordmark and logos are trademarks excluded from the license.** Remove them from the product UI, icons, bundle ids and docs and replace them with `{{APP_NAME}}` branding, unless I confirm in writing that I have permission. "Compatible with ArtCraft" style wording in the docs is fine.
7. **Don't break the file format.** `.pptx` open → save → reopen must stay lossless for everything DeckCraft supports today. New features with no PowerPoint equivalent (variables, auto layout, components) get saved in a native format (see §4.6). When exporting to `.pptx`, flatten them to the closest faithful static result.
8. **Small, green, reviewable steps.** Work on a branch per phase. Run `cargo xtask ci` (or the host's equivalent: fmt, clippy `-D warnings`, tests, layering) before every commit. Never commit with red CI. Commit messages say what changed and why.
9. **When you're unsure, read the source.** Don't guess at an API in any of the three repos. Open the file. If two designs both look viable and the choice is expensive to undo, stop and write the trade-off into `docs/decisions/NNN-*.md` and ask me.

---

## 2. Phase 0: Reconnaissance (no feature code yet)

Deliver `docs/recon.md` containing:

- Workspace map of all three repos: crates, their layer, and the key types (document node, command trait/registry, history/undo, renderer entry point, text layout entry point, MCP tool registration).
- Side-by-side comparison of the three **document models** and three **command registries**, plus a proposal for how FigCraft and DesignCraft concepts map onto the host's slide model. Examples: FigCraft Frame ↔ slide or group, FigCraft Component ↔ layout/master placeholder, DesignCraft Story ↔ text body.
- Renderer comparison. Benchmark the host's current slide render against `vello_cpu` (DesignCraft) and `tiny-skia` (FigCraft) on the sample deck at 1×, 2× and thumbnail size. Recommend whether to adopt `vello_cpu` for export/thumbnails, the canvas, or neither.
- The fork-patch inventory from rule 2.
- A risk list with the top 5 things most likely to go wrong.

Stop after Phase 0 and wait for my go-ahead.

---

## 3. Target experience (what "done" feels like)

Model the UX on Figma Slides, adapted to a native app:

- **Two views of the same deck:** *Grid view* (all slides as a zoomable board that you can reorder, group into sections and select in multiples) and *Single-slide view* (focused editing with the filmstrip). Switch with one keystroke.
- **Infinite-canvas feel inside a slide.** FigCraft-grade selection, smart guides, distance measurement, nudge, multi-select edits, and editing inside groups without ungrouping.
- **Design panel on the right**, context-sensitive like Figma's: Layout (auto layout), Position, Fill, Stroke, Effects, Text, and Slide (background, layout, theme). Every numeric field accepts math (`120/2`) and scrubbing.
- **Themes as a real design system:** a theme is a set of FigCraft-style **variables with modes** (color, number, string, boolean). Examples are Light/Dark modes and brand palettes, type scale, spacing scale and corner radius. Changing a variable restyles every bound property in the deck instantly. Themes still map to and from PowerPoint theme colors/fonts when you save to `.pptx`.
- **Smart layouts that make it hard to make ugly slides:**
  - Auto layout frames (hug/fill/fixed, gap, padding, wrap, grid tracks with `fr`) for text + image cards, feature grids, agendas and timelines.
  - Components with variants and properties for reusable slide parts (title block, stat card, quote, logo row, footer). Instances take overrides, and editing the main component updates every instance.
  - Layout library: 30+ ready-made, auto-layout-based slide layouts (title, section, 2/3/4-up, big number, quote, image-left/right, full-bleed image, comparison, timeline, team, agenda, closing). All are built from the theme's variables so they reflow and restyle.
  - "Tidy up": align and evenly distribute a messy selection into a clean auto-layout grid.
- **Typography that looks professional by default** (from DesignCraft):
  - Knuth–Plass paragraph composer for body text, single-line for short labels.
  - Dictionary hyphenation (off by default for titles), optical margin alignment, and better justification.
  - Paragraph and character styles bound to theme variables (Title, Subtitle, Heading, Body, Caption, Quote, Big Number).
  - Baseline grid snapping as an option per slide or master.
  - Threaded text across text boxes, text wrap around images and shapes, and pro tables (row/column styles, banding, alignment on decimal).
  - OpenType features and variable font axes (from FigCraft), with bidi and IME.
  - Auto-fit that tries composer tweaks (tracking, hyphenation, a slight scale) before it shrinks the font size.
- **Rich visuals** (from FigCraft): vector networks, pen/pencil, editable boolean ops, variable-width strokes, shape builder, gradients, image fills with crop/fit modes, blend modes, shadows, background/layer blur, noise, glass, masks, and Display P3 colour.
- **Presenting stays best-in-class** (from DeckCraft): Morph and other transitions, entrance/emphasis/exit/motion-path animations, presenter view with notes, timer and next slide, rehearsed timings, and audio/video. Add *Smart Animate between auto-layout states*. Morph should match nodes across slides by stable id or layer name, the way Figma's Smart Animate does.
- **Interactive slides (stretch):** live polls, alignment scales and simple prototype links between slides (FigCraft prototyping connections), which run in present mode. Results get stored in the deck file.
- **Collaboration (stretch):** port FigCraft's live-session model (cursors, per-person undo, self-hostable server) and comments pinned to slide coordinates. Gate it behind a feature flag until it's stable.
- **Speed is a feature:** opening a 100-slide deck, switching views and scrubbing a variable must feel instant. Use copy-on-write documents with cheap undo snapshots (DesignCraft/FigCraft approach), relayout only the dirty nodes, and cache composed text.

---

## 4. Phased plan (each phase ends with green CI and a short demo)

For every phase, deliver: the code, tests, new commands registered in the MCP server, an updated `docs/porting-log.md`, and a sample deck (`samples/phase-N.*`) that exercises the new feature and renders headlessly to PNG with the CLI.

### Phase 1: Foundations
- Enforced layering (`xtask layers`) in the host.
- Unified command registry audit. Every existing DeckCraft action has a stable id, undo support and MCP exposure. List any gaps and fix them.
- Copy-on-write document + O(1) undo snapshots if the host doesn't already have them. Measure before and after.
- Golden-image test harness: render sample decks headlessly and diff against stored PNGs with a tolerance.

### Phase 2: Typography engine (DesignCraft)
- Bring in DesignCraft's `compose` (paragraph composer, hyphenation, justification, optical margins) as the text engine for slide text bodies. Keep the existing engine behind a flag until golden tests pass.
- Paragraph/character styles, baseline grid, threaded text, text wrap, pro tables.
- `.pptx` round-trip: compose decisions must not change saved text content. Line breaks may differ from PowerPoint, so document the known differences.

### Phase 3: Canvas and layout (FigCraft)
- Selection model, smart guides, measurement, editing inside groups, frames.
- Auto layout (flow, wrap, grid tracks, hug/fill/fixed, padding, gap) plus constraints.
- Vector networks, pen/pencil, editable booleans, effects, P3.
- Grid view + single-slide view.

### Phase 4: Design system (FigCraft)
- Variables with modes and aliases. Paint/text/effect/grid styles. Bind any property to a variable.
- Components, variants, properties, instance overrides, swap, Assets panel.
- Rebuild the built-in themes and the 30+ layout library on top of variables and components.
- Theme ↔ `.pptx` theme mapping.

### Phase 5: Presenting upgrades
- Smart Animate/Morph that matches by stable node id and layer name, including auto-layout state changes.
- Presenter view polish and rehearse mode, kept intact from DeckCraft.
- Interactive slides (polls, alignment scales, prototype links) behind a flag.

### Phase 6: Import/export
- Keep: `.pptx` open/save, PDF, PNG/JPEG, animated GIF.
- Add: print-grade PDF (PDF/X-4, PDF/A-2b) via the shared `craft-pdf` + DesignCraft's PDF path, best-effort `.fig` import of frames as slides (FigCraft), and IDML import of pages as slides (DesignCraft).
- Native `.{{EXT}}` format: a documented, versioned archive that stores everything (variables, components, auto layout, interactions). It must be readable by a headless CLI.

### Phase 7: Collaboration, plugins, AI hooks (stretch)
- FigCraft live sessions + comments, behind a flag.
- Sandboxed WASM plugins (FigCraft's metered interpreter) with a slide-oriented API.
- {{OPTIONAL: ArtCraft bridge. A command that sends a placeholder and a prompt to a locally installed ArtCraft app (or any image-generation backend I configure) and drops the result in as an image fill. Opt-in only, with no network calls by default.}}
- Agent-first "make me a deck" flow via MCP: given an outline (Markdown or JSON), pick layouts from the library, fill placeholders, apply the theme, run Tidy and auto-fit, and render previews. This is the main proof that the command surface is complete.

### Phase 8: Polish and release
- Accessibility (AccessKit, as in FigCraft), keyboard coverage, contrast checks on built-in themes.
- Native builds for macOS/Windows/Linux, plus the web/WASM build if the host supports it.
- Docs: user guide, `docs/mcp.md`, `docs/control-protocol.md`, file-format spec, porting log, NOTICE/ATTRIBUTION.

---

## 5. Quality bar and acceptance checks

A phase is done only when **all** of these pass:

- `cargo xtask ci` is green: fmt, clippy `-D warnings`, tests, layering.
- Golden-image tests pass for every sample deck, and the new sample deck renders headlessly.
- `.pptx` round-trip suite passes, covering DeckCraft's existing samples plus 10 real-world decks I'll put in `testdata/pptx/`. Report any loss explicitly.
- Every new feature is reachable from the MCP server. Prove it with a scripted test that builds the phase's sample deck **only through MCP commands**.
- Fork-patch tests still pass.
- Performance budget holds: open a 100-slide deck in under 1.5 s, keep 60 fps canvas pan/zoom on a mid-range laptop, apply a variable mode switch across 100 slides in under 100 ms. Measure these and record the results in the phase report.
- Design check: render the layout library in every built-in theme in both Light and Dark modes into one contact sheet PNG and include it in the phase report. Nothing should overflow, clip or have contrast below WCAG AA.

---

## 6. How to report back

At the end of each phase, write `docs/reports/phase-N.md` with:
1. What shipped (with the contact sheet / screenshots).
2. What was ported vs re-implemented, and why.
3. Test, benchmark and round-trip results.
4. Known gaps and the risks for the next phase.
5. Any decision that needs me.

Then stop and wait for my go-ahead before starting the next phase.

---

## 7. Placeholders to fill

- `{{APP_NAME}}`: product name (must not use ArtCraft trademarks unless permitted)
- `{{MY_DECKCRAFT_FORK_URL}}`: your patched fork
- `{{EXT}}`: native file extension
- `{{OPTIONAL: …}}`: keep or delete the ArtCraft/image-generation bridge
