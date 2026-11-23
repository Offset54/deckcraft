# `.pptx` round-trip corpus: handoff

Status on 2026-11-23. For whoever picks this job up next.

## Done

- **Ten decks chosen and checked.** All are CC-BY-4.0 from Zenodo, listed with authors, DOIs,
  features and SHA-256 in [`manifest.json`](manifest.json) and [`manifest.md`](manifest.md).
- **Fetch script.** [`fetch.ps1`](fetch.ps1) downloads every deck from Zenodo into a folder outside
  the repository and deletes any file whose SHA-256 does not match.
- **Baseline.** [`baseline.md`](baseline.md), measured at code commit `efe1de7`:
  - No crash and no hang. All ten decks open in 0.04–0.23 s, and the round-trip test runs in 35 s.
  - The test fails: 5 of 10 decks lose something on save and reopen.
    - Deck 01: slide 89 has 23 animations before and 21 after.
    - Decks 02, 05, 06 and 10: pixel differences.
  - Decks 02 and 10 also lose animations on first open, which the test cannot see.
- **Hashes rechecked on 2026-11-23.** All ten decks downloaded again from their Zenodo direct URLs
  match the manifest.

## Where things are

| What | Where |
|---|---|
| Docs and fetch script | `Offset54/deckcraft`, branch `claude/pptx-corpus-manifest`. Commits `3eea3ef` (fetch script), `85ec2f7` (manifest and baseline), then this file. No PR yet |
| Code the baseline ran on | `efe1de7`, branch `claude/new-session-lm99ab` |
| Test | `crates/engine/tests/it/pptx_roundtrip.rs`, `an_external_corpus_survives_a_pptx_round_trip`. Set `DECKCRAFT_PPTX_CORPUS` to the folder of decks |
| Corpus repo | `Offset54/slidecraft-pptx-corpus`: public, **still empty** |
| Decks | Zenodo (URLs in the manifest). The decks must never be committed to `Offset54/deckcraft` (`AGENTS.md` §1.1) |

## Not done, and why

1. **The corpus repo is still empty.** The wrap-up session could not get push access: attaching
   the repo was refused by the session's permission mode. It did the following instead:
   - Asked the finder session (`session_01XQuQYqDRbPx1rBT9SA8AVS`, disconnected) to push its local
     corpus commit. Read that session's latest events to see whether it replied.
   - Built a ready-to-push commit in its own scratchpad. That commit has `decks/` with the ten
     files, `README.md`, `ATTRIBUTION.md` (title, authors, date, Zenodo record and DOI, licence and
     SHA-256 for each deck), `SHA256SUMS` and `manifest.json`. The scratchpad does not outlive that
     session, so expect to rebuild the commit.
2. **`fetch.ps1` and the docs still point at Zenodo.** Making the corpus repo the main source would
   break the script while that repo is empty, so this waits for the decks to be pushed.

## Next steps for a fresh session

1. Attach `Offset54/slidecraft-pptx-corpus` with push access.
2. Download each deck from its `direct_url` in `manifest.json` into a folder outside this
   repository, and check every SHA-256. Never commit a deck whose hash does not match.
3. Push to `main` of the corpus repo:
   - `decks/<file>` for each deck
   - `README.md`
   - `ATTRIBUTION.md`, with the CC-BY-4.0 credit for each deck
   - `SHA256SUMS`
   - a copy of `manifest.json`
4. Confirm that all ten files are on `main` there and that their hashes match the manifest.
5. On this branch, make the corpus repo the main source and keep Zenodo as the fallback:
   - In `fetch.ps1`, try `https://raw.githubusercontent.com/Offset54/slidecraft-pptx-corpus/<commit>/decks/<file>`
     first, then `direct_url`. Pin the commit so the hash check stays meaningful.
   - Update `manifest.md` to match.
6. Decide with the owner whether to open a PR from `claude/pptx-corpus-manifest`. It changes docs
   only. Do not change CI.
7. Fix the test-design gaps in [`baseline.md`](baseline.md#test-design-described-not-fixed). The
   test compares SlideCraft with itself after the first open, so it cannot see what that first
   open loses (decks 02 and 10). This is separate work, best done as its own PR.
8. Then work through the losses: deck 01 slide 89 drops two `p:txEl` animations on re-save, and
   decks 02, 05, 06 and 10 have pixel differences.

## Coverage gaps in the corpus

These come from `coverage_gaps` in the manifest:

- No deck has embedded audio.
- There is only one native chart.
- There are few native PowerPoint equations.
- PowerPoint for Mac and PowerPoint 2007 appear only among the alternates.

`manifest.json` lists alternates A1–A11 that cover some of these.
