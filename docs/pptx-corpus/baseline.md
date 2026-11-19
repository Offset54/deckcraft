# `.pptx` round-trip corpus: baseline

What SlideCraft loses on the ten decks in [`manifest.md`](manifest.md), measured on 2026-11-19 at
code commit `efe1de7` (branch `claude/new-session-lm99ab`; this branch adds only documentation).
Run on Linux x86-64 with rustc 1.97.0 and `CRAFT_FONTS_DIR` pointing at a `craft-fonts`
checkout. I have not run it on Windows.

```bash
CARGO_INCREMENTAL=0 cargo build --release -p deckcraft-cli
DECKCRAFT_PPTX_CORPUS=<decks> CARGO_INCREMENTAL=0 cargo test -p deckcraft-engine --test it external_corpus -- --nocapture
target/release/deckcraft-cli info <deck>     # for each deck
```

## Summary

- **No crash and no hang.** All ten decks opened in `deckcraft-cli info` (0.04–0.23 s each),
  and the round-trip test finished in 35 s.
- **The test fails: 5 of 10 decks lost something** in the save-and-reopen cycle. One deck lost
  animations; four decks render differently after the round trip.
- **On first open,** the files hold more than the test compares. Two decks lose animations
  before any save, which the test cannot see (see [Test design](#test-design-described-not-fixed)).
- No deck reported a damaged part on open.

## Round trip (the test as it stands)

The test opens each deck, saves it as `.pptx`, opens that again, and compares the two
SlideCraft models slide by slide: text, shape count, notes text, animation count, transition
kind, and pixels within `Tolerance::SLIDE`. Its output, verbatim:

| Deck | Result | Losses reported |
|---|---|---|
| 01-premis-tutorial-137-slides | 137 slides, 0 damaged, **1 loss** | slide 89: 23 animations became 21 |
| 02-morph-smartart-nl | 14 slides, 0 damaged, **10 losses** | pixels differ on slides 1 (2.471 %), 2 (1.221 %), 3 (2.370 %), 4 (1.544 %), 6 (0.959 %), 8 (1.215 %), 10 (1.705 %), 12 (1.671 %), 13 (2.674 %), 14 (2.576 %) |
| 03-japanese-chart-4x3-ppt2010 | 40 slides, 0 damaged, 0 losses | |
| 04-arabic-morph-motion-paths | 19 slides, 0 damaged, 0 losses | |
| 05-video-smartart | 22 slides, 0 damaged, **2 losses** | pixels differ on slides 3 (2.644 %), 10 (3.444 %) |
| 06-sections-masters-epfl | 62 slides, 0 damaged, **8 losses** | pixels differ on slides 5 (0.427 %), 6 (0.348 %), 14 (0.968 %), 44 (1.655 %), 45 (1.547 %), 47 (0.231 %), 49 (0.693 %), 55 (1.065 %) |
| 07-wps-chinese-equations | 22 slides, 0 damaged, 0 losses | |
| 08-google-slides-arabic | 45 slides, 0 damaged, 0 losses | |
| 09-keynote-workshop | 10 slides, 0 damaged, 0 losses | |
| 10-libreoffice-workshop | 24 slides, 0 damaged, **23 losses** | pixels differ on 23 of 24 slides: 1 (1.189 %), 2 (4.767 %), 3 (0.910 %), 4 (2.229 %), 5 (2.299 %), 6 (1.527 %), 7 (3.873 %), 8 (2.233 %), 9 (3.268 %), 10 (3.900 %), 11 (3.588 %), 12 (3.225 %), 13 (1.375 %), 14 (0.899 %), 15 (1.803 %), 16 (1.459 %), 17 (1.286 %), 18 (1.085 %), 19 (1.296 %), 20 (1.245 %), 21 (3.828 %), 23 (5.573 %), 24 (0.611 %) |

The test ends with `assertion left == right failed: 5 of 10 decks lost something` (exit 101).
I did not look into why the pixels differ. That is the next task, not this one.

Slide 89 of deck 01 has 23 effects: 22 entrances and one emphasis. Six of them are aimed at single
paragraphs (`p:txEl`). The first open keeps all 23; the re-save loses two. Deck 02 slide 10 and deck 10 slide 7 have the same
pattern on first open (next section).

## First open (not measured by the test)

Here the package contents (counted from the XML) are compared with what `deckcraft-cli info`
reports SlideCraft loaded:

| Deck | Slides | Animation effects (package → loaded) | Transitions with an effect | Morph slides | Non-empty notes | Sections | Video/audio parts |
|---|---|---|---|---|---|---|---|
| 01 | 137 → 137 | 807 → 807 | 2 → 2 | 0 | 22 → 22 | 0 | 0 |
| 02 | 14 → 14 | **40 → 37** (slide 10: 5 → 2) | 7 → 7 | 7 → 7 | 0 | 0 | 0 |
| 03 | 40 → 40 | 3 → 3 | 0 | 0 | 37 → 37 | 0 | 0 |
| 04 | 19 → 19 | 572 → 572 | 19 → 19 | 16 → 16 | 0 | 0 | 0 |
| 05 | 22 → 22 | 14 → 14 | 0 | 0 | 0 | 0 | 1 → 1 |
| 06 | 62 → 62 | 112 → 112 | 0 | 0 | 28 → 28 | 7 → 7 | 0 |
| 07 | 22 → 22 | 0 | 0 | 0 | 7 → 7 | 0 | 0 |
| 08 | 45 → 45 | 15 → 15 | 0 | 0 | 44 → 44 | 0 | 0 |
| 09 | 10 → 10 | 12 → 12 | 0 | 0 | 0 | 0 | 0 |
| 10 | 24 → 24 | **144 → 136** (slide 7: 9 → 1) | 0 | 0 | 0 | 0 | 0 |

The two drops have the same pattern. Every effect on the slide is an Appear (`presetID="1"`)
aimed at one paragraph (`p:txEl`) of a shape, and the loaded slide has fewer animations than
paragraph effects. SlideCraft may fold paragraph effects into one "by paragraph" build, in which
case the count is not a loss of behaviour. Or it may drop them. I have not established which.
Whoever picks this up should compare the click sequence in the slide show.

Notes on how this was measured, so the numbers are not misread:

- Decks 05, 09 and 10 have a `<p:transition>` on every slide with no effect child (`<p:transition/>`,
  or only `spd`/`advClick`). SlideCraft loads these as "no transition", which is correct, so they
  count as 0 above.
- `deckcraft-cli info` lists only the **first** master's layouts (`crates/engine/src/cmd/inspect.rs`,
  `"layouts": m.map(…)` where `m` is the first master). Its layout count therefore looks low for
  decks with several masters (01: 22 in the package vs 11 listed; 02: 77 vs 19; 06: 34 vs 2). The
  reader does load every master (`crates/pptx/src/read.rs`, `read_masters`). This is a limit of
  `info`, not a loss.
- `info` reports no charts, SmartArt, tables, equations, embedded fonts or hyperlinks, so this
  table cannot say whether those survive the first open. Only the pixel comparison in the test
  covers them, and only across the re-save.

## Test design (described, not fixed)

These are gaps in `an_external_corpus_survives_a_pptx_round_trip` as it stands. The brief says to
describe them here and not change code.

1. **It compares SlideCraft with itself.** `losses(&doc, &reopened, …)` compares the deck after
   SlideCraft's first import with the deck after export and reimport. Anything dropped or changed
   while reading the original file is invisible unless the reader reports it as a damaged part,
   and damaged parts are printed but do not fail the test (`lossy` counts only `losses`). The
   first-open table above shows two decks (02 and 10) that lose animations this way. For both, the
   test reports only pixel losses and no animation loss.
2. **Several features are not compared at all:** sections, masters and layouts, slide size,
   embedded fonts, media parts, hyperlinks, chart data, SmartArt data and equation content. Text,
   shape count and pixels catch some of these indirectly. Sections, embedded fonts and hyperlink
   targets cannot be caught by any of the current checks.
3. **Animations and transitions are compared coarsely:** animation *count* per slide and
   transition *kind*. Changed effect types, order, timing or targets with the same count pass.
4. **No per-deck time limit, and the name prints after the work.** `println!` for a deck comes
   after import, export and reimport, so a deck that hangs stops the run without saying which
   deck it was. None of these ten hangs.
5. **Pixel results depend on the fonts the machine has.** Both renders happen on the same machine,
   so a run is self-consistent. The same deck can still report different pixel losses on Windows
   and on Linux, or with and without `CRAFT_FONTS_DIR`. The sample-deck test gates its pixel
   checks on the craft fonts being present; the corpus test does not.

## Reproducing on Windows

```powershell
.\docs\pptx-corpus\fetch.ps1 -Destination C:\path
$env:DECKCRAFT_PPTX_CORPUS = "C:\path"
cargo test -p deckcraft-engine --test it external_corpus -- --nocapture
```

`fetch.ps1` refuses a destination inside the repository. Expect the same slide counts and the same
animation result. Pixel percentages may differ (item 5 above).
