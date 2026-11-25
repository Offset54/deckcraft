# Fallback: fork craft-libs for the extension slots

> **Status:** prepared 2026-11-23, not started. Start it only if the deadline in
> [`decision 004`](../decisions/004-upstream-merge-deferred.md) (2026-12-14) passes without the
> slots in `storytold/craft-libs`. Creating the fork repository is a step for the owner (§1).

## 0. The trigger, checked on 2026-12-14

The slots count as landed when `storytold/craft-libs` `main` has all of the following:

- `raw_ext` on `ParaProps` and `RunProps` (`crates/drawingml/src/text.rs`) and on `Table`
  (`crates/drawingml/src/table.rs`);
- `Shape::raw_ext` filled from `p:cNvPr` in `crates/pptx/src/read/shapes.rs`;
- all four read and written (the issue in
  [`craft-pptx-extension-slots.md`](craft-pptx-extension-slots.md)).

A quick check from a craft-libs clone: `git grep -n raw_ext origin/main -- crates/drawingml/src/text.rs crates/drawingml/src/table.rs`.

If they have landed, skip the fork: do the upstream merge (§3) against `storytold/craft-libs` at
or after the merge commit. If an upstream pull request is open and under active review on that
day, the owner decides whether to wait a little longer or fork.

## 1. Create the fork (owner)

- On GitHub, fork `storytold/craft-libs` to **`Offset54/craft-libs`** (public, same MIT OR
  Apache-2.0 licence, `NOTICE` and `ATTRIBUTION.md` unchanged). The owner does this, or approves
  it in a session, because it creates a new remote.
- Add the fork to this environment's GitHub access, so sessions can push to it.
- Branch **`slidecraft/ext-slots`** from the revision upstream DeckCraft `main` pins for its
  PPTX crates on that day (on 2026-11-23: `8048f8ef92da80fb3ce5a056d59ca591ee66faf1`). Forking
  from upstream's pin, not craft-libs `main`, keeps the rest of craft-libs exactly as upstream
  DeckCraft tests it.

## 2. The patch (in the fork, one commit plus tests)

Follow craft-libs' own `AGENTS.md` and house rules; this is the same change as the issue, so it
can be offered upstream as a pull request unchanged.

| File | Change |
|---|---|
| `crates/drawingml/src/text.rs` | `raw_ext: Option<String>` on `ParaProps` and `RunProps`, skipped when absent, not inherited |
| `crates/drawingml/src/table.rs` | `raw_ext: Option<String>` on `Table` |
| `crates/drawingml/src/read.rs` | `ppr` (`a:pPr`, `a:lvlNpPr`) and `rpr` (`a:rPr`, `a:defRPr`, `a:endParaRPr`) keep their `a:extLst` children, bounded like the existing slots, dropping any with relationship references |
| `crates/drawingml/src/write.rs` | `ppr` and `rpr` write the kept children as the last child (count them as children, so an element with only kept extensions is written open) |
| `crates/pptx/src/read/shapes.rs` | `tblPr` keeps its `a:extLst` children in `Table::raw_ext`; `base` keeps `p:cNvPr`'s other than the decorative extension in `Shape::raw_ext` |
| `crates/pptx/src/write/shapes.rs` | `a:tblPr` and `p:cNvPr` write them back, after the extensions the writer emits itself |
| tests | each element keeps an unknown extension through open and save; relationship-bearing ones are dropped; oversized ones are bounded; documents without extensions write byte-identically |

Estimated size: about 150 lines plus tests. Run craft-libs' own CI for the touched crates.

## 3. Point SlideCraft at the fork (in `Offset54/deckcraft`, during the upstream merge)

1. Merge upstream DeckCraft `main`; resolve the conflicts listed in decision 004.
2. In the workspace `Cargo.toml`, change **five** dependency lines, the ones upstream pins
   together for PPTX (`craft-opc`, `craft-cfb`, `craft-xml`, `craft-drawingml`, `craft-pptx`),
   to `git = "https://github.com/Offset54/craft-libs", rev = "<fork commit>"`. Keep `version`.
   - Why all five: `craft-pptx` uses the other four from its own workspace. If any of them came
     from a different source, the app would see two incompatible `craft-drawingml` types and
     `crates/pptx/src/convert.rs` would not compile.
   - Why edit the lines rather than add a `[patch]` section: the change is explicit, sits in one
     place, leaves every other craft-libs crate on its own pin, and reverting it is the same five
     lines.
   - Check: `cargo tree -i craft-drawingml` names one package (with two copies it reports the
     name as ambiguous), and `Cargo.lock` lists the five from the fork only.
3. In `crates/pptx/src/convert.rs`, map SlideCraft's fields into and out of the slots, using the
   element names and URIs Phase 2 already writes (`crates/pptx/src/opc.rs` on this branch):
   - `ParaProps.raw_ext`: `sc:typography` (composer, hyphenate, opticalMargin, snapToGrid,
     style, colDerived);
   - `RunProps.raw_ext`: `sc:run` (style);
   - `Table.raw_ext`: `sc:columns`;
   - `Shape.raw_ext`: `sc:thread`, `sc:wrap`; `sc:canvas` (Phase 3: frames, auto layout,
     constraints, booleans, vector networks, P3 colours, blur, noise; JSON in `@data`). It is
     written on every kind of shape, **groups included** (frames are `p:grpSp`), so the slot
     must cover `p:nvGrpSpPr/p:cNvPr` as well as `p:sp`, `p:pic` and `p:cxnSp`. Two writer steps
     are app policy and become pre-export passes here: a frame's background rectangle (a first
     member, dropped on import) and an editable boolean written as one `p:sp` with its result
     outline (rebuilt as a group on import);
     Phase 4 adds two keys to the same `sc:canvas` JSON (`bindings`, `colorVars`: number,
     string and boolean bindings, and which colours name a variable), so it needs no new slot;
   - slide, layout and master `raw_ext`: `sc:baselineGrid`; slide `raw_ext` also `sc:modes`
     (Phase 4: the variable modes a slide uses instead of the deck's, JSON in `@data`);
   - presentation `raw_ext`: `sc:textStyles`; `sc:design` (Phase 4: the design system's
     variables, collections, modes, styles and components, as JSON text). Colour variables
     reach PowerPoint as plain `a:srgbClr` (their value on that slide, refreshed by the engine
     before export); variables mapped to a theme slot reach it through the theme's
     `a:clrScheme`/`a:fontScheme`, which craft-pptx already writes. No colour-level slot is
     needed.

   Keep the pre-export passes (`text_style::flatten_deck`, `Table::with_column_formats`, the
   engine's thread flow) and the post-import pass (`text_style::unflatten_deck`, thread merge) in
   this repository. They are app policy. Because the URIs and element names don't change,
   `.pptx` files SlideCraft saved before the move still open with everything.
4. Port the corpus branch's `.pptx` writer fixes (`11ec8ad`, `4d597f3`, `2c36fe9`) into the fork
   as separate commits, and offer them upstream too. They help every Craft app. Its animation fix
   (`f9e73f1`, `crates/anim`) and test changes (`3d08904`) stay in this repository.
5. Record the fork in `docs/fork-patches.md` and the porting log: what it changes, why, and the
   upstream issue or PR it waits on.
6. Gate: `cargo xtask ci` green. Phase 2's `.pptx` round-trip tests
   (`crates/pptx/tests/it/roundtrip.rs`) and the sample decks' round trip must pass unchanged.
   The corpus run must be no worse than the latest `docs/pptx-corpus/baseline.md`.

## 4. Keeping the fork small while it lives

- When upstream DeckCraft bumps its craft-libs pin, rebase `slidecraft/ext-slots` onto the new
  revision and bump the five lines. The patch is small, so this should be routine.
- Nothing else goes into the fork. Other craft-libs changes go upstream directly.
- Offer the fork's commits upstream as pull requests as soon as the fork exists. The owner sends
  them, or approves a session sending them.

## 5. Dropping the fork

1. When `storytold/craft-libs` merges the slots (the same or a compatible design), point the
   five lines back at `https://github.com/storytold/craft-libs` at a revision with the merge,
   ideally the one upstream DeckCraft pins.
2. If upstream named or shaped the slots differently, adjust only `convert.rs`. The file format
   SlideCraft writes doesn't change.
3. Check: `cargo tree --prefix none | grep -c Offset54/craft-libs` prints 0, and the gate in §3.6
   passes.
4. Remove the fork from `docs/fork-patches.md`, and archive `Offset54/craft-libs` (owner).

## Risks

- **Drift.** Every upstream pin bump means a rebase. It stays small only if nothing else goes
  into the fork (§4).
- **Lockfile churn** when the five lines move. Review `Cargo.lock` diffs for unrelated bumps, as
  Phase 2 did.
- **CI** must reach `github.com/Offset54/craft-libs`. It's the same host as today's craft-libs
  dependencies, so no network change is expected.
