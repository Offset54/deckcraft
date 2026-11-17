# Phase 0 report: reconnaissance

> **Date:** 2026-11-17 · **Branch:** `claude/new-session-lm99ab` · **Base:** `3c1d282`

## 1. What shipped

- [`docs/recon.md`](../recon.md): the workspace maps of DeckCraft, FigCraft and DesignCraft, the
  key types, the model and command-registry comparison with a mapping proposal, the renderer
  comparison with measurements, the risk list and the decisions needed.
- [`docs/fork-patches.md`](../fork-patches.md): every patch this fork carries, its files, its
  commands and its tests.
- One test, for the only fork patch that had none (the custom-build notice in the About window).
  The notice moved into a function so the test can call it; the window shows the same text.

No feature code changed. There are no screenshots because nothing visible changed.

## 2. Ported and re-implemented

Nothing yet. The preliminary approach for each capability is in `recon.md` §9. The main change
from the brief: the pieces it expected to lift as crates from DesignCraft and FigCraft depend on
those apps' document models, and the genuinely self-contained parts already exist as shared
crates in `craft-libs` (`craft-text`, `craft-fig`, `craft-idml`, `craft-pdf`).

## 3. Tests and measurements

| What | Result |
|---|---|
| Fork patch tests at the fork head | 93 passed, 0 failed |
| The new test | Passes |
| `cargo xtask ci` | All 8 steps passed (fmt, pdf-deps, clippy, test, doctest, assets, layers, wasm) |
| Open a 100-slide deck | 9 ms (`.deckcraft`), 16–22 ms (`.pptx`); budget 1,500 ms |
| Render 100 slides, single thread | 64 ms at thumbnail size, 251 ms at 1×, 445 ms at 2× |
| `vello_cpu` against `tiny-skia`, same scene | `tiny-skia` is 1.9× to 4.2× slower |

Measured on a 4-vCPU Linux container, not on the mid-range laptop the budget names. The 100-slide
deck is the sample's eight slides duplicated; it holds no pictures or video. `.pptx` round trip
was not run beyond the benchmark's save and reopen: the round-trip suite and the ten real-world
decks belong to Phase 1.

## 4. Known gaps and risks for the next phase

- Phase 1 is smaller than planned: layering enforcement, copy-on-write undo and MCP exposure of
  every command already exist in the host. What remains is the golden-image harness, the `.pptx`
  round-trip suite, a measured baseline for undo cost, and the audit of UI-only commands.
- The host has no stored-image comparison tests today, so nothing would catch rendering drift
  from a new text engine.
- The five top risks are in `recon.md` §10.

## 5. Decisions needed

The seven decisions are in `recon.md` §11. The two that block Phase 1:

1. **Base.** Stay on v0.18.1 or move the patches onto upstream `main` (v0.19.0) first.
   Recommended: move first.
2. **Branches.** Where phase branches merge, and whether the fork's own workflow may change.

The product name, the file extension and the image-generation bridge are still unfilled in the
brief; they are needed for the rebrand and by Phase 6 (native format).
