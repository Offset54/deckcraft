# SlideCraft: handoff

Status on 2026-11-25: Phase 4 done on `claude/phase-4-design-system` (waiting for the owner's Phase 5 go-ahead); upstream merge deferred until 2026-12-14. Start here if you know nothing about this project.

## Goal

This job turns this fork of DeckCraft into **SlideCraft**: one native Rust presentation app with
Figma Slides–style ease of use. It takes canvas and design-system features from FigCraft and
typography from DesignCraft, and keeps DeckCraft's presenting and `.pptx` compatibility.

The source of truth is the owner's brief,
[`brief/slidecraft-agent-prompt.md`](brief/slidecraft-agent-prompt.md). It is kept verbatim with
its `{{…}}` placeholders; the values that fill them are in decision 001. It sets out the phases,
the acceptance checks and the rule to **stop after each phase and wait for the owner's
go-ahead**.

Repository rules are in [`AGENTS.md`](../AGENTS.md). Two of them matter here:
- No PowerPoint-produced files in this repository.
- No CI changes in feature work.

## Decisions

Made:
- **Product identity** ([`decisions/001-product-identity.md`](decisions/001-product-identity.md)):
  - name SlideCraft
  - application identifier `io.github.offset54.slidecraft`
  - native extension `.slidecraft`
  - phases continue on `claude/new-session-lm99ab`
- **Base moved onto upstream DeckCraft v0.19.0** before Phase 1 (merge `86be392`, no conflicts;
  [`recon.md`](recon.md) §11 item 2).
- **Upstream merge deferred until 2026-12-14** ([`decisions/004-upstream-merge-deferred.md`](decisions/004-upstream-merge-deferred.md)):
  upstream now delegates `.pptx` to `craft-pptx`, which can't yet keep Phase 2's paragraph,
  run, table and shape-level extensions.
  - The owner posts [`upstream/craft-pptx-extension-slots.md`](upstream/craft-pptx-extension-slots.md)
    to `storytold/craft-libs`.
  - If the slots haven't landed by **2026-12-14**, follow
    [`upstream/craft-libs-fork-plan.md`](upstream/craft-libs-fork-plan.md): fork to
    `Offset54/craft-libs` (the owner creates the repository), patch, point the five PPTX-family
    crates at the fork, merge upstream.
  - Either way, new `.pptx` work follows Phase 2's extension pattern until then.
- **Real-world test decks** live in the separate public repo
  [Offset54/slidecraft-pptx-corpus](https://github.com/Offset54/slidecraft-pptx-corpus), not here
  ([`pptx-corpus/`](pptx-corpus/manifest.md)).

Open (for the owner; context in [`recon.md`](recon.md) §11 and
[`reports/phase-1.md`](reports/phase-1.md) §5):
1. **Shared crates.** Use `craft-text`, `craft-fig`, `craft-idml` and `craft-pdf` from
   `craft-libs`, or copy the code from DesignCraft and FigCraft. Recommended: use the shared
   crates. *Phase 2 went ahead on 2026-11-23 with the recommendation (`craft-text`); the owner did
   not answer this question explicitly.*
2. **Document tree.** *Decided 2026-11-23: keep the nested tree
   ([`decisions/002-document-tree.md`](decisions/002-document-tree.md)).*
3. **Collaboration.** Keep it as a stretch goal or drop it.
4. **Branches and merging.** Where each phase's branch merges, and whether this fork's workflow
   may change.
5. **Image-generation bridge.** Keep it or delete it. This is the brief's `{{OPTIONAL: …}}` item.
6. **Installer identity step.** The installers and `APP_ID` still use DeckCraft's identity. This
   is one packaging change and can't be built or tested in the cloud (decision 001, "Pending").

## Progress

| Phase | State | Report |
|---|---|---|
| 0: reconnaissance | Done | [`reports/phase-0.md`](reports/phase-0.md), [`recon.md`](recon.md), [`fork-patches.md`](fork-patches.md) |
| 1: foundations | Done: golden images, `.pptx` round trip by pixels, command audit, undo bench | [`reports/phase-1.md`](reports/phase-1.md), [`command-audit.md`](command-audit.md), [`porting-log.md`](porting-log.md) |
| 2: typography | Done: paragraph composer, hyphenation, optical margins, styles, baseline grid, threaded text, text wrap, table column formats | [`reports/phase-2.md`](reports/phase-2.md) |
| 3: canvas and layout | Done on `claude/phase-3-canvas` (not yet merged into the main work branch): Grid view switch, editing inside groups, measurement, frames, constraints, auto layout, vector networks, editable booleans, blur and noise, Display P3 | [`reports/phase-3.md`](reports/phase-3.md), [`decisions/003-view-mode.md`](decisions/003-view-mode.md) |
| 4: design system | Done on `claude/phase-4-design-system` (from `claude/phase-3-canvas`, with `claude/new-session-lm99ab` merged; not yet merged into the main work branch): variables with modes and aliases, paint/effect/grid styles, text styles on variables, components with variants, properties and overrides, the theme kit (every built-in theme in Light and Dark), a 34-layout library, Tidy up; a contact-sheet check of 544 slides for overflow, clipping and WCAG AA | [`reports/phase-4.md`](reports/phase-4.md) |
| 5–8 | Not started | The brief, §4 |

## `.pptx` corpus

- The ten decks are CC-BY-4.0 and live in `Offset54/slidecraft-pptx-corpus` at `eb77646`. Every
  SHA-256 matches [`pptx-corpus/manifest.json`](pptx-corpus/manifest.json).
- [`pptx-corpus/fetch.ps1`](pptx-corpus/fetch.ps1) downloads them, with Zenodo as a fallback.
- Baseline ([`pptx-corpus/baseline.md`](pptx-corpus/baseline.md)): no crash and no hang, but 5 of
  10 decks lose something on save and reopen. The test also can't see what is lost on first open.
- Open items, and details, are in [`pptx-corpus/HANDOFF.md`](pptx-corpus/HANDOFF.md).

## Where things are

| What | Where |
|---|---|
| Main branch for the work | `Offset54/deckcraft` `claude/new-session-lm99ab`. It includes the corpus branch merged in. No PR |
| Key commits | `87d4bbb` Phase 0 · `86be392` upstream v0.19.0 merge · `d16f329` Phase 1 report · `efe1de7` SlideCraft rebrand · `528c9d9` decision 001 corpus update |
| Corpus docs branch | `claude/pptx-corpus-manifest` at `e1504e7`, already merged here |
| Brief's first commit | `claude/pensive-gates-mw50t6` at `4e7ddd2`; the same file is in this branch |
| Test decks | `Offset54/slidecraft-pptx-corpus` `main` at `eb77646` |
| Upstream | `storytold/deckcraft`. The fork's patches are listed in [`fork-patches.md`](fork-patches.md) |

## First steps for a new session

1. Read these, in order:
   - the brief
   - this file
   - [`reports/phase-1.md`](reports/phase-1.md) and [`reports/phase-2.md`](reports/phase-2.md)
   - [`recon.md`](recon.md) §1 and §11
2. Set up the build:
   - Clone `storytold/craft-fonts` next to the repo as `../craft-fonts`.
   - On Linux, install `libasound2-dev` and `mesa-vulkan-drivers`, and add the Rust target
     `wasm32-unknown-unknown` ([`recon.md`](recon.md) §12).
   - Run `CARGO_INCREMENTAL=0 cargo xtask ci`. With incremental builds, `target/` reaches 28 GB.
3. Phase 4 is done; ask the owner for the Phase 5 go-ahead and for the decisions in
   [`reports/phase-4.md`](reports/phase-4.md) §5 (and those still open in
   [`reports/phase-3.md`](reports/phase-3.md) §5). Check the dates in decision 004 (2026-12-14).
4. Phase 4's design system: `crates/model/src/{variables,design_styles,component,design_kit}.rs`,
   the engine's design pass `crates/engine/src/design_system.rs`, the library
   `crates/engine/src/library.rs`, commands in `crates/engine/src/cmd/{variables,design_styles,components,library}.rs`.
   `SLIDECRAFT_CONTACT_SHEET=<file.png> cargo test -p deckcraft-engine --test it contact_sheet`
   writes the design-check contact sheet.
5. For corpus work, follow [`pptx-corpus/HANDOFF.md`](pptx-corpus/HANDOFF.md).
