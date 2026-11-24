# `.pptx` round-trip corpus: handoff

Status on 2026-11-24. For whoever picks this job up next.

**Latest (2026-11-24):** with the owner's yes, decks 11–14 were added to the corpus repo at
`b94da4b` to fill the coverage gaps. 13 of 14 decks pass. Deck 11 (embedded audio) loses each
narration's play length on save and reopen ([`baseline.md`](baseline.md#decks-1114-added-2026-11-24)).
That is now the next fix (item 7).

**Latest:** on branch `claude/pptx-corpus-losses` all ten decks pass the round-trip test, and the
test is stricter. Items 3 and 4 below are done there. See
[`baseline.md`](baseline.md#results-after-the-fixes-2026-11-23). There is no PR; the owner
decides.

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
| Docs, manifest and fetch script | `Offset54/deckcraft`, branch `claude/pptx-corpus-manifest` (commits `3eea3ef`, `85ec2f7`, `800eeef`, `e1504e7`), merged into `claude/new-session-lm99ab` on 2026-11-23. No PR |
| Code the baseline ran on | `efe1de7`, branch `claude/new-session-lm99ab` |
| Test | `crates/engine/tests/it/pptx_roundtrip.rs`, `an_external_corpus_survives_a_pptx_round_trip`. Set `DECKCRAFT_PPTX_CORPUS` to the folder of decks |
| Decks | `Offset54/slidecraft-pptx-corpus` @ `eb77646`. Zenodo is the fallback. The decks must never be committed to `Offset54/deckcraft` (`AGENTS.md` §1.1) |
| Sessions | Parent `session_01XQuQYqt5JBD63TQpw7M7Qz`, finder `session_01XQuQYqDRbPx1rBT9SA8AVS`. The owner archives them after review |

## Open items

1. **PR or not.** `claude/pptx-corpus-manifest` changes docs only. The owner decides whether to
   open a PR. Do not change CI.
2. **Run `fetch.ps1` once on Windows**, to confirm the fallback logic after this change.
3. **Done on `claude/pptx-corpus-losses`: the test-design gaps** in
   [`baseline.md`](baseline.md#test-design-described-not-fixed).
   - The test names each deck before it works on it.
   - It checks first-open animation effects against counts taken from the package XML.
   - It compares what the slide show plays, effect by effect, instead of a count.
   - It compares slide size, sections, layouts and embedded fonts.
   - Still open:
     - Hyperlinks, chart, SmartArt and equation data are compared only through pixels.
     - Pixel results depend on the fonts the machine has. The test does not gate on them.
     - There is no per-deck time limit, only the name printed first.
4. **Done on `claude/pptx-corpus-losses`: the losses.** Decks 02 and 10 lost nothing on first
   open: the reader folds `p:txEl` effects into a build that plays the same. Four fixes:
   - **Deck 01:** a by-paragraph build that isn't on click played with a fixed 0.5 s stagger.
     It now chains its paragraphs, as the file does (`crates/anim`).
   - **SmartArt (decks 02 and 05):** SmartArt drawing shapes now keep their middle anchor.
   - **Deck 10 and 02:** `p:defaultTextStyle` is no longer invented for decks without one.
   - **Deck 06:** theme line styles without a fill are no longer given one.

   Two questions are left open. They are listed under
   [Open questions](baseline.md#open-questions-these-fixes-leave): whether PowerPoint draws
   fill-less theme lines, and whether its slide show chains these builds. Both need PowerPoint
   observed on Windows or Mac.
   - **Commit `3d08904` is red when checked out alone.** It adds the deck 01 regression test, and
     the fix only lands in `f9e73f1`. CI passed at every pushed tip. *Decided 2026-11-24 by the
     owner: keep the history as it is.* The branch was already merged into
     `claude/new-session-lm99ab` (merge `9b75851`), which the Phase 3 session builds on, so a
     squash would have meant rewriting pushed, shared history for one red intermediate commit.
     Bisecting across it: treat `3d08904` as "skip".
5. **Done 2026-11-24: fill the coverage gaps.** Decks 11–14 are in the corpus repo at `b94da4b`, and
   `corpus_repo` in `manifest.json` points there. Only Hebrew is still out (alternate A11). The gaps were:
   - No deck has embedded audio.
   - There is only one native chart.
   - There are few native PowerPoint equations.
   - PowerPoint for Mac and PowerPoint 2007 appear only among the alternates.

   Alternates A1–A11 cover some of these. A new deck goes into the corpus repo as a new commit.
   Then update `corpus_repo.commit` and `raw_base` in `manifest.json`.
6. **The parent session is still marked blocked** waiting on the corpus repo. That is resolved; the
   owner can close it.
7. **Deck 11 loses media timing on save.** Every slide's media-play effect comes back with
   `span_ms: 0` instead of the clip length. Find out whether the writer drops the media node's
   duration or the reader doesn't recover it from what was written, fix it, and add a regression test
   built from a deck we generate (never deck 11 itself: `AGENTS.md` §1.1).

## What a fresh session should do next

1. Read this file, `manifest.md` and `baseline.md`.
2. Ask the owner about item 1 (PR).
3. Pick up item 3 or 4 on a new branch from `main`:
   - Fetch the decks: `fetch.ps1`, or curl `raw_base + file` and check `sha256sum`.
   - Run the test with `DECKCRAFT_PPTX_CORPUS` set to that folder.
