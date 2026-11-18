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

## Attribution carried over

None in Phase 1: no code or asset was copied from FigCraft, DesignCraft or `craft-libs`. The new
sample deck, its picture and the golden pictures are contributor-original (`ATTRIBUTION.md`).
