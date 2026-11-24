# `.pptx` round-trip corpus: baseline

Status on 2026-11-24: four coverage decks (11–14) were added, and **13 of 14 decks pass**. Deck 11
(embedded audio) loses each narration's play length on save and reopen
([Decks 11–14](#decks-1114-added-2026-11-24)). On 2026-11-23 all ten original decks passed on
branch `claude/pptx-corpus-losses`, with the test made stricter
([Results after the fixes](#results-after-the-fixes-2026-11-23)). The original
baseline follows that section unchanged, except where a later finding is marked.

## Decks 11–14, added 2026-11-24

The owner approved four decks to fill the manifest's coverage gaps. They went into the corpus repo
at `b94da4b`. Measured on `claude/owner-decisions` (code identical to `claude/new-session-lm99ab`
at `e3371cd`), on Linux x86-64 with `CRAFT_FONTS_DIR` set, using all 14 decks:

```
DECKCRAFT_PPTX_CORPUS=<corpus>/decks CARGO_INCREMENTAL=0 cargo test -p deckcraft-engine --test it external_corpus -- --nocapture
```

| Deck | What it adds | Result |
|---|---|---|
| 01–10 | | 0 losses each (01: 7.7 s, the slowest) |
| 11 `11-mac-audio-narration.pptx` | Embedded audio: 9 `.m4a` narrations, one per slide; PowerPoint for Mac | **9 losses**, one per slide (4.2 s) |
| 12 `12-ppt-equations-tables.pptx` | 65 native PowerPoint equations, 16 tables | 0 losses (3.1 s) |
| 13 `13-mac-charts-notes.pptx` | PowerPoint for Mac, 2 native charts, notes on all 49 slides | 0 losses (3.6 s) |
| 14 `14-ppt2007-charts.pptx` | PowerPoint 2007, 2 native charts, 6 tables | 0 losses (3.3 s) |

The test fails: `1 of 14 decks lost something`, 48 s in total. No crash, no hang, and no part was
damaged on open.

**Deck 11's loss.** On every slide, the slide show's media-play effect keeps its target,
trigger, step, start and repeat count, but its duration doesn't survive the round trip:

```
slide 1: playback changed: effect 1 of 1: Played { target: (Some(11), None), trigger: None, step: 0,
start_ms: 0, span_ms: 16128, repeats: 1000, what: "Media play " } became Played { … span_ms: 0, … }
```

On first open the duration matches the clip (16.1 s to 44.9 s). After SlideCraft writes the deck
and reads it back, it is 0. The writer most likely drops the media timing node's duration (`dur`
on the `p:cMediaNode`'s `p:cTn`), or the reader doesn't take it from the written form. This has
not been checked in the code yet. It is recorded here and in [`HANDOFF.md`](HANDOFF.md) as the
next corpus fix. Equations (12) and charts (13, 14) survive. As before, the test compares those
only through pixels.

## Results after the fixes (2026-11-23)

Measured on branch `claude/pptx-corpus-losses`, on Linux x86-64 with rustc 1.97.0 and
`CRAFT_FONTS_DIR` set to a `craft-fonts` checkout. Not run on Windows. The baseline was reproduced
first, at `61fff99`, the tip of `claude/new-session-lm99ab` (code unchanged since `efe1de7`): the
same five decks failed with the same losses and the same pixel percentages.

### What the test checks now

`an_external_corpus_survives_a_pptx_round_trip` (`crates/engine/tests/it/pptx_roundtrip.rs`):

- It prints each deck's name **before** opening it, and the time it took after.
- **First open.** It counts animation effects in the package XML: every `p:cTn` with a
  `presetClass`, by shape id and `p:txEl` paragraph, slide by slide in `p:sldIdLst` order. It
  checks that count against what the slide-show timeline plays after the first open. A
  by-paragraph build counts once per paragraph it plays, including sub-paragraphs with text that
  play with their parent. A shortfall is a loss, and the missing targets are named.
- **Save and reopen.** For each slide it compares **what the slide show plays**, effect by
  effect, instead of the animation count. Each effect is compared by target shape (by its place
  in the shape tree) and paragraph, sequence (main or trigger), click step, start, span, repeats,
  and class, effect and option. It also compares slide size, section names and sizes, layouts per
  master and embedded font names. Text, shape count, notes, transition kind and pixels are
  compared as before.
- Losses from either stage fail the test.

Still not compared: hyperlink targets, chart data, SmartArt data, equation content and media
parts (only through pixels). Pixel results still depend on the fonts the machine has (gap 5
below).

### Result

| Deck | Baseline | Now |
|---|---|---|
| 01 | slide 89: 23 animations became 21 | 0 losses |
| 02 | pixels on 10 slides | 0 losses |
| 03 | 0 | 0 |
| 04 | 0 | 0 |
| 05 | pixels on slides 3, 10 | 0 losses |
| 06 | pixels on 8 slides | 0 losses |
| 07 | 0 | 0 |
| 08 | 0 | 0 |
| 09 | 0 | 0 |
| 10 | pixels on 23 slides | 0 losses |

The test passes (`1 passed`, 21 s for the ten decks). No deck reports a first-open loss.

### What the losses were

1. **`p:txEl` paragraph effects: the reader folds them; it does not drop them.**
   - **Decks 02 (slide 10) and 10 (slide 7):** the reader folds the paragraph effects into one
     by-paragraph build. The timeline plays that build exactly as the file does, so the first
     open loses nothing. Deck 02's slide 10 has 5 effects in the file: shape 10's paragraphs 0
     and 3 play on click, and paragraphs 1 and 2 are sub-paragraphs that play with paragraph 0.
     Deck 10's slide 7 has 9 effects, one click per non-empty paragraph. The baseline's "40 → 37"
     and "144 → 136" compared effects in the file with animations in the Animation Pane, and the
     pane lists a build once.
   - **Deck 01 slide 89 was a real timing loss.** Shape 4567 has three separate 200 ms Fade
     effects, one per paragraph, each After Previous. The first open keeps all three. The writer
     marks them `bldP build="p"`, and on reopening the reader folds them into one build (21
     animations). The timeline then played a build that is not on click with its paragraphs a
     fixed 0.5 s apart, so paragraph 2 started at 0.95 s instead of 0.65 s. But the file, and
     the writer, chain each paragraph after the one before it ends. **Fix** (`crates/anim`): later
     paragraphs of a With/After Previous build now start after the previous one ends. This is
     unchanged for 500 ms effects with no delay.
2. **SmartArt text moved to the top (decks 05 and 02).** In the model, text on a shape with no
   anchor sits in the middle. The `p:sp` writer spells that out as `anchor="ctr"`. The SmartArt
   drawing writer (`dsp:sp`) did not, so on reopening the text read as top-anchored. **Fix:** the
   drawing writer writes the anchor the same way.
3. **Text narrower after a save (decks 10 and 02).** A deck without a `p:defaultTextStyle`
   (LibreOffice writes none) reads back with an empty style. The writer filled in DeckCraft's
   defaults, whose kerning changed every line of text on reopening. **Fix:** the element is
   written only when the deck has one. It is optional in the schema, and new presentations
   always have one.
4. **Outlines appeared (deck 06).** Its themes have line styles with no fill
   (`<a:ln w="9525"><a:prstDash val="solid"/></a:ln>`), which SlideCraft draws as no line. The
   theme writer filled them with `phClr`, so shapes whose style points at them gained an outline.
   **Fix:** a line style without a fill is written without one.

Each fix has a regression test on a deck built in code. Each test fails without its fix:

- `crates/anim/src/tests.rs`: `an_after_previous_paragraph_build_chains_its_paragraphs`
- `crates/engine/tests/it/pptx_roundtrip.rs`:
  `separate_after_previous_paragraph_effects_play_the_same_after_a_pptx_round_trip`
- `crates/pptx/tests/it/smartart.rs`: `smartart_text_stays_centred_after_a_save`
- `crates/pptx/tests/it/roundtrip.rs`: `no_default_text_style_stays_none` and
  `a_theme_line_style_without_a_fill_keeps_none`

### Open questions these fixes leave

- **Fill-less theme line styles.** SlideCraft draws them as no line, and the fix keeps the file
  as it was. Whether PowerPoint draws them the same way has not been checked. If it draws a
  line, the renderer is wrong, not the round trip.
- **The chained build.** By-paragraph builds that are not on click now play like the `.pptx` they
  save to. Whether PowerPoint's slide show chains them the same way has not been observed here.
  Its files store them that way.

## Baseline (2026-11-19)

Measured on 2026-11-19 at code commit `efe1de7` (branch `claude/new-session-lm99ab`). The
documentation-only branch `claude/pptx-corpus-manifest` recorded it.
Run on Linux x86-64 with rustc 1.97.0 and `CRAFT_FONTS_DIR` pointing at a `craft-fonts`
checkout. I have not run it on Windows.

```bash
CARGO_INCREMENTAL=0 cargo build --release -p deckcraft-cli
DECKCRAFT_PPTX_CORPUS=<decks> CARGO_INCREMENTAL=0 cargo test -p deckcraft-engine --test it external_corpus -- --nocapture
target/release/deckcraft-cli info <deck>     # for each deck
```

### Summary

- **No crash and no hang.** All ten decks opened in `deckcraft-cli info` (0.04–0.23 s each),
  and the round-trip test finished in 35 s.
- **The test fails: 5 of 10 decks lost something** in the save-and-reopen cycle. One deck lost
  animations; four decks render differently after the round trip.
- **On first open,** the files hold more than the test compares. Two decks lose animations
  before any save, which the test cannot see (see [Test design](#test-design-described-not-fixed)).
- No deck reported a damaged part on open.

### Round trip (the test as it stood)

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

### First open (not measured by the test then)

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

*Later finding (2026-11-23):* it folds them, and the build plays what the file plays. See
[What the losses were](#what-the-losses-were), item 1.

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

### Test design (described, not fixed)

These were the gaps in `an_external_corpus_survives_a_pptx_round_trip` at `efe1de7`. On
2026-11-23, gaps 1, 3 and 4 were fixed, and gap 2 was fixed for sections, layouts, slide size and
embedded fonts. See [What the test checks now](#what-the-test-checks-now). Gap 5 is open.

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
