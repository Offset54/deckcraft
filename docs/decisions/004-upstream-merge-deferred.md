# 004: Upstream DeckCraft merge deferred, with a deadline

> **Decided:** 2026-11-23 by the owner · **Status:** deferred · **Deadline:** 2026-12-14 (three
> weeks). If `storytold/craft-libs` hasn't added the extension slots by then, SlideCraft forks
> craft-libs ([fallback plan](../upstream/craft-libs-fork-plan.md)).

## What happened

The Phase 2 report recommended merging upstream DeckCraft `main` before Phase 3. A trial merge
of upstream `14ff093` (125 commits past the fork's base) showed more than the report assumed.
Upstream (`05d9907`, "delegate PPTX/POTX/PPSX reader and writer to craft-pptx") deleted the
`.pptx` reader and writer from `crates/pptx`. `crates/pptx` now converts between DeckCraft's
model and the model of `craft-pptx` in `storytold/craft-libs` (pinned at `8048f8e`), and
craft-libs does the reading and writing.

`craft-pptx` keeps unknown extension XML (`raw_ext`) only for slides, layouts, masters and the
presentation. Paragraph properties (`a:pPr`), run properties (`a:rPr`) and table properties
(`a:tblPr`) have no such slot, and `Shape::raw_ext` exists but is never read from a file (the
`p:cNvPr` extensions are read only for the decorative flag). Taking the merge as it stands would
drop from `.pptx`:

- the paragraph composer, hyphenation, optical margin and grid-snap settings (P2.1, P2.3);
- paragraph and character style ids, and the marks of what was flattened from them (P2.2);
- threaded text links and text wrap (P2.4, P2.5);
- table column formats and the header-row style (P2.6);
- the corpus branch's fixes in the deleted writer files.

All of these would still be saved in `.deckcraft`.

The trial merge also had content conflicts in `Cargo.toml`, `README.md`,
`apps/deckcraft/src/main.rs`, `crates/model/src/{lib,table}.rs`, `crates/pptx/src/lib.rs`,
`crates/pptx/tests/it/roundtrip.rs` and two UI files. It was aborted and nothing was committed.

## Options considered

| Option | Result |
|---|---|
| **Defer, with a deadline (chosen)** | No regression now. Phase 3 continues on the current base. The fork stays behind upstream until the slots exist upstream, or until the deadline, whichever comes first. |
| Merge, keep the fork's own reader and writer | Nothing lost now, but upstream's future `.pptx` fixes would land in craft-libs and never reach the fork, which would diverge for good. |
| Merge, adopt `craft-pptx` as it is | Follows upstream and AGENTS.md's shared-library rule, but Phase 2's `.pptx` round trip regresses until craft-libs changes. |

## Plan

1. **Now:** the owner posts the issue in
   [`../upstream/craft-pptx-extension-slots.md`](../upstream/craft-pptx-extension-slots.md) to
   `storytold/craft-libs` (searched 2026-11-23: nothing like it exists; related tracking issue
   #778), offering the patch as a pull request.
2. **Until 2026-12-14:** keep new `.pptx` work small and in Phase 2's extension pattern (our
   namespace and URIs in `crates/pptx/src/opc.rs`), so either path stays mechanical.
3. **If the slots land by 2026-12-14:** merge upstream DeckCraft and map SlideCraft's settings
   into the slots in `crates/pptx/src/convert.rs`. SlideCraft writes the same XML, so files saved
   earlier still open with everything.
4. **If not:** fork `storytold/craft-libs` to `Offset54/craft-libs`, add the slots there, point
   the five PPTX-family dependencies at the fork, and merge upstream DeckCraft. The steps, the
   checks, and how the fork is dropped once upstream catches up are in
   [`../upstream/craft-libs-fork-plan.md`](../upstream/craft-libs-fork-plan.md). Creating the
   fork repository is the owner's step.
5. In either case, offer the corpus branch's general `.pptx` writer fixes to craft-libs, since
   they help every Craft app.
