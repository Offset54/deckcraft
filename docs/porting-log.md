# Porting log

Every capability brought into this app from FigCraft, DesignCraft or `craft-libs`: what was
taken, how (lifted as a crate, depended on, or re-implemented against the host model), and why.
The capability-by-capability plan is in [`recon.md`](recon.md) §9; decisions that need the owner
go to `docs/decisions/`.

## Phase 1: Foundations (2026-11-18)

Nothing was ported. Every Phase 1 item was either already in the host or built here.

| Item | Source considered | Decision | Why |
|---|---|---|---|
| Base | storytold/deckcraft `main` | Merged upstream `main` (v0.19.0, `5c69849`) into the fork | The owner chose to move first. 132 commits, no conflicts; the patches' tests and `cargo xtask ci` pass on the result. Brings the `craft-libs` cut-overs (DrawingML geometry and colour, the `.deckcraft` package layer, video codecs) that later phases build on |
| Enforced layering | DesignCraft `xtask/src/layers.rs` | Keep the host's | The host already has `cargo xtask layers` with the same shape (layer table, UI-crate ban below L6, dev-dependency exceptions) and it runs in `cargo xtask ci`. 18 crates, no violations |
| Command registry | FigCraft `cmd.rs` (`spec!`), DesignCraft `cmd.rs` | Keep the host's; add audit tests | The host and DesignCraft registries are the same design; FigCraft's lacks `enabled` and keeps menus apart. What was missing was a check, not a mechanism ([`command-audit.md`](command-audit.md)) |
| Copy-on-write document, O(1) undo | FigCraft `imbl` node map; DesignCraft `Arc` spreads | Keep the host's; measure it | The host already shares every untouched slide, master and media item between undo steps (`Arc<Slide>`, `Arc<Master>`, `Arc<Vec<u8>>`), with a byte budget. `imbl` is MPL-2.0, outside the licence allowlist. Measured: an undo step keeps ~28 KiB on a 100-slide deck against ~21 KiB on an 8-slide one, so the cost follows the slide edited, not the deck |
| Golden-image harness | DesignCraft's damage-region pixel tests; FigCraft's retained-render random walk | Built in the host | Both siblings compare renders inside their own renderer's tests against their own document model. The host needed whole-deck comparison against stored pictures: `deckcraft_render::golden` (compare with a channel and area tolerance, a diff picture, PNG decode) and `crates/engine/tests/it/golden.rs` |
| `.pptx` round trip by pixels | — | Built in the host | `crates/engine/tests/it/pptx_roundtrip.rs`: each sample deck goes through `.pptx` and must still match its golden pictures; `DECKCRAFT_PPTX_CORPUS=<folder>` runs a loss report over decks kept outside the repository |

## Phase 2: Typography (started 2026-11-23)

The owner gave the go-ahead on 2026-11-23 without choosing between the shared crates and copies
(recon §11 item 3); this phase uses the recommended default, the shared `craft-text`, which is
also upstream's rule for `craft-libs`.

| Item | Source considered | Decision | Why |
|---|---|---|---|
| Paragraph composer (Knuth–Plass) | DesignCraft `compose::breaker`; `craft-text::linebreak` | Depend on `craft-text` (`linebreak`, `hyphen`) at the rev the workspace already pins (`63196e6`); re-implement the item builder against the host's shaped cells (`crates/text/src/compose.rs`) | `compose` takes DesignCraft's `Document`/`Story` and cannot be lifted (recon §1 item 4). `craft-text` is the same breaker extracted with no document types: box/glue/penalty items, three elasticity tiers, rag, hyphenation zone, hanging edges, and a greedy fallback with bounded work. The host keeps its shaping, bidi and placement; only the choice of breaks changes |
| Hyphenation | DesignCraft `compose::hyphen`; `craft-text::hyphen` | `craft-text`, US English only | Bundled public-domain dictionary plus CC0 patterns. The other languages' hypher patterns carry licences outside the allowlist, so no language feature is enabled; non-English runs (by `lang`) are not hyphenated |
| Optical margin alignment | DesignCraft `compose::breaker::hang` | Copied the protrusion table; applied through `craft-text`'s `Item::hang` | Sibling Craft code (MIT OR Apache-2.0) may be copied (AGENTS.md §2). The table is standard protrusion amounts; the placement change is the host's |
| Where the switch lives | — | Three optional `ParaProps` fields (`composer`, `hyphenate`, `opticalMargin`) that inherit through masters, layouts and placeholders like every other paragraph property | Unset means the single-line breaker, so every existing deck and every `.pptx` lays out exactly as before (tested). New decks can opt in per style level |
| `.pptx` storage | — | An `a:pPr/a:extLst/a:ext` with SlideCraft's own URI and `sc:typography` | PowerPoint has no markup for these. Extensions with unknown URIs are kept and ignored, so SlideCraft round-trips them and PowerPoint breaks lines its own way. Saved text never changes: an automatic hyphen is drawn, not inserted |
| Paragraph and character styles | DesignCraft `doc::styles` (paragraph/character styles with based-on chains) | Re-implemented on the host model (`crates/model/src/text_style.rs`, `style.*` commands) | DesignCraft's styles hang off its own story model. Here a style is a named `ParaProps` + `RunProps` pair; paragraphs and runs name one, and `resolve::para`/`resolve::run` put it between direct formatting and the placeholder chain. Built-ins (Title, Subtitle, Heading, Body, Caption, Quote, Big Number; Strong, Emphasis, Accent) use theme fonts and colours, so they restyle with the theme; Phase 4's variables will take over that binding |
| Styles in `.pptx` | — | Flattened into direct formatting per shape on export; ids in `a:pPr`/`a:rPr` extensions; the deck's definitions as JSON in a `p:presentation` extension; the reader strips what the styles supply | PowerPoint has no named text styles. Flattening shows the same text in PowerPoint; stripping on import makes the round trip exact, so editing a style later still reaches the paragraphs |
| Readable coloured styles | — | Built here | A style's text colour with under 3:1 contrast against the shape's solid fill gives way to the shape's own text colour, in rendering and in the `.pptx` flattening alike. Found when Big Number (Accent 1) vanished on the default Accent 1 shape |

## Attribution carried over

None in Phase 1: no code or asset was copied from FigCraft, DesignCraft or `craft-libs`. The new
sample deck, its picture and the golden pictures are contributor-original (`ATTRIBUTION.md`).

Phase 2: the optical margin protrusion table in `crates/text/src/compose.rs` (`hang`) is copied
from DesignCraft's `crates/compose/src/breaker.rs` at `d3190ef` (MIT OR Apache-2.0, Copyright (c)
2026 ArtCraft Team and the DesignCraft contributors); `NOTICE` carries that line. The US English hyphenation
dictionary and patterns come in through the `craft-text` dependency and are listed in
`ATTRIBUTION.md` under "Bundled through dependencies".
