# `.pptx` round-trip corpus: handoff

Status on 2026-11-23. For whoever picks this job up next.

## Done

- **Ten decks chosen and checked.** All are CC-BY-4.0 from Zenodo, listed with authors, DOIs,
  features and SHA-256 in [`manifest.json`](manifest.json) and [`manifest.md`](manifest.md).
- **Corpus repo filled.**
  [Offset54/slidecraft-pptx-corpus](https://github.com/Offset54/slidecraft-pptx-corpus) `main` is
  at `eb77646a292cb78b88075c1a2121b92a13979a5e` and holds:
  - `decks/` with all ten files, unmodified
  - `README.md`
  - `ATTRIBUTION.md`, with authors, source records and versions
  - `LICENSES/CC-BY-4.0.txt`
  - `manifest.json`

  The finder session pushed it on 2026-11-23. A separate clone of the repo found that all ten
  decks are present and every SHA-256 matches the manifest.
- **Fetch script points at the corpus repo.** [`fetch.ps1`](fetch.ps1) first downloads from the
  pinned commit (`corpus_repo` in `manifest.json`), then falls back to the Zenodo `direct_url`. It
  deletes any file whose SHA-256 does not match. All ten pinned raw URLs were downloaded with curl,
  and every hash matched. **The PowerShell script itself has not been run since this change,
  because no PowerShell was available.** Run it once on Windows.
- **Baseline.** [`baseline.md`](baseline.md), measured at code commit `efe1de7`:
  - No crash and no hang. All ten decks open in 0.04–0.23 s, and the round-trip test runs in 35 s.
  - The test fails: 5 of 10 decks lose something on save and reopen.
    - Deck 01: slide 89 has 23 animations before and 21 after.
    - Decks 02, 05, 06 and 10: pixel differences.
  - Decks 02 and 10 also lose animations on first open, which the test cannot see.

## Where things are

| What | Where |
|---|---|
| Docs, manifest and fetch script | `Offset54/deckcraft`, branch `claude/pptx-corpus-manifest`. Commits `3eea3ef` (fetch script), `85ec2f7` (manifest and baseline), then the handoff and repoint commits. No PR |
| Code the baseline ran on | `efe1de7`, branch `claude/new-session-lm99ab` |
| Test | `crates/engine/tests/it/pptx_roundtrip.rs`, `an_external_corpus_survives_a_pptx_round_trip`. Set `DECKCRAFT_PPTX_CORPUS` to the folder of decks |
| Decks | `Offset54/slidecraft-pptx-corpus` @ `eb77646`. Zenodo is the fallback. The decks must never be committed to `Offset54/deckcraft` (`AGENTS.md` §1.1) |
| Sessions | Parent `session_01XQuQYqt5JBD63TQpw7M7Qz`, finder `session_01XQuQYqDRbPx1rBT9SA8AVS`. The owner archives them after review |

## Open items

1. **PR or not.** `claude/pptx-corpus-manifest` changes docs only. The owner decides whether to
   open a PR. Do not change CI.
2. **Run `fetch.ps1` once on Windows**, to confirm the fallback logic after this change.
3. **Fix the test-design gaps** in [`baseline.md`](baseline.md#test-design-described-not-fixed).
   The test compares SlideCraft with itself after the first open, so it cannot see what that first
   open loses (decks 02 and 10). This is separate work, best done as its own PR.
4. **Fix the losses.**
   - Deck 01 slide 89 drops two `p:txEl` animations on re-save.
   - Decks 02, 05, 06 and 10 have pixel differences.
5. **Fill the coverage gaps** listed under `coverage_gaps` in the manifest:
   - No deck has embedded audio.
   - There is only one native chart.
   - There are few native PowerPoint equations.
   - PowerPoint for Mac and PowerPoint 2007 appear only among the alternates.

   Alternates A1–A11 cover some of these. A new deck goes into the corpus repo as a new commit.
   Then update `corpus_repo.commit` and `raw_base` in `manifest.json`.
6. **The parent session is still marked blocked** waiting on the corpus repo. That is resolved; the
   owner can close it.

## What a fresh session should do next

1. Read this file, `manifest.md` and `baseline.md`.
2. Ask the owner about item 1 (PR).
3. Pick up item 3 or 4 on a new branch from `main`:
   - Fetch the decks: `fetch.ps1`, or curl `raw_base + file` and check `sha256sum`.
   - Run the test with `DECKCRAFT_PPTX_CORPUS` set to that folder.
