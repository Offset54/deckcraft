# 001: Product identity — SlideCraft

> **Decided:** 2026-11-18 by the owner · **Status:** code and docs done; installers pending

## Decision

| Item | Value |
|---|---|
| Product name | **SlideCraft** |
| Application identifier | `io.github.offset54.slidecraft` |
| Native document extension | `.slidecraft` (`.deckcraft` still opens; both are the same package) |
| Real-world test decks | A separate public repo, `Offset54/slidecraft-pptx-corpus`, of openly licensed decks with a credit and licence per file, run through `DECKCRAFT_PPTX_CORPUS` (changed 2026-11-19; first decided: a private folder on the owner's computer). Filled 2026-11-23: ten CC-BY-4.0 decks at `eb77646` ([`docs/pptx-corpus/`](../pptx-corpus/manifest.md)) |
| Branch | Phases continue on `claude/new-session-lm99ab` |

## Why

- DeckCraft's brand licence requires a published modified version to drop the ArtCraft marks and
  not present itself as an ArtCraft product. "DeckCraft" itself is not one of the marks, so the
  new name is the owner's product choice, not a licence requirement.
- The identifier must differ from DeckCraft's `ai.storyteller.deckcraft`, so the two install side
  by side and keep separate settings. The owner has no domain of their own, so the identifier
  comes from the public GitHub owner.
- DeckCraft already opens `.slidecraft` files as native (it used that name before it was renamed),
  so the new extension needs no format change and files stay interchangeable both ways.

## What changed (2026-11-18)

- `deckcraft_model::brand`: the name, identifier, extension, the "based on DeckCraft" credit and
  DeckCraft's copyright line, in one place. User-visible strings in the UI, engine messages, CLI,
  MCP, PDF and PPTX metadata read from it.
- Settings, recovery files and dictionaries move to a `SlideCraft` folder (`slidecraft` on Linux),
  so they don't mix with an installed DeckCraft.
- Save writes `.slidecraft`; Open, the file dialogs and the engine accept both extensions.
- No ArtCraft logos, website or Discord links in the app or the README; `docs/brand/` keeps only
  the licence text. About credits DeckCraft and keeps its copyright. `tests_brand.rs` guards it.

## Test decks in their own repo

The real-world decks are other people's files, many saved by PowerPoint. `AGENTS.md` §1.1 keeps
files PowerPoint produced out of this repository, so they live in a separate public repo. Only
licences that allow public redistribution qualify there (CC0, CC-BY, CC-BY-SA, public domain, MIT,
Apache-2.0), each file unmodified, with its author, source and licence in that repo's
`ATTRIBUTION.md`. This repository keeps only the manifest and the baseline results
(`docs/pptx-corpus/`).

## Kept on purpose

- Crate and binary names (`deckcraft-*`, `deckcraft`, `deckcraft-cli`), so upstream changes merge
  cleanly. Renaming the binaries is a packaging change.
- The built-in theme called "DeckCraft", table-style names and the package's `mimetype`: they are
  data in files, shared with DeckCraft.
- `LICENSE-MIT`, `LICENSE-APACHE` and DeckCraft's copyright line in `NOTICE`: the licences require
  them.

## Pending: installers

The installers (`packaging/`) and the running app's platform identifier (`APP_ID` in
`apps/deckcraft/src/main.rs`, which must match the installed desktop file) still use DeckCraft's
identity, links and the Windows "ArtCraft" suite folder. They change together, in one packaging
step, because:

- the Windows installer's folder and upgrade code decide how this fork's builds install through
  the ArtCraft Launcher, which `custom-windows.yml` and the "leave updates to the launcher" patch
  rely on;
- macOS, Windows and Flatpak packaging can't be built or tested in this cloud environment.
