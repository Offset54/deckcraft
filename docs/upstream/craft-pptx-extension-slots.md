# Issue for storytold/craft-libs: extension slots on paragraph, run and table properties

> **For the owner, not part of the issue.** Post everything below the line as a new issue on
> `storytold/craft-libs`. Searched 2026-11-23: no open or closed issue or pull request asks for
> this; the nearest is the tracking issue #778. Craft rule 2 asks the poster to comment on the
> issue first and to claim it before opening a pull request; offer that only if you're willing to
> send the patch (it is the patch in [`craft-libs-fork-plan.md`](craft-libs-fork-plan.md) §2).
> Context: [`../decisions/004-upstream-merge-deferred.md`](../decisions/004-upstream-merge-deferred.md).

---

**Title:** craft-drawingml / craft-pptx: keep `a:extLst` on paragraph, run, table and shape properties

**Body:**

`craft-pptx` keeps unknown extension XML (`raw_ext`) on slides, layouts, masters and the
presentation, so an app can round-trip its own data in a `.pptx`. Paragraph properties, run
properties and table properties have no such slot, and `Shape::raw_ext` exists but is never
filled from a file: the `a:extLst` of `p:cNvPr` is read only for the decorative flag. Everything
else in these lists is dropped on read and can't be written. That leaves an app with paragraph-,
run-, table- or shape-level settings PowerPoint has no markup for (typography settings, named
styles, a table's column formats, linked text boxes) nowhere to keep them in a `.pptx`.

ECMA-376 allows `a:extLst` as the last child of `a:pPr`, `a:lvlNpPr`, `a:rPr`, `a:defRPr`,
`a:endParaRPr`, `a:tblPr` and `p:cNvPr`, and PowerPoint keeps extensions it doesn't understand.

Context: an app built on DeckCraft is moving onto craft-pptx (#778) and would lose its
paragraph-, table- and shape-level extensions on the move.

**Proposal**

1. `crates/drawingml/src/text.rs`: add `raw_ext: Option<String>` to `ParaProps` and `RunProps`
   (the `a:ext` children, verbatim). Like other per-element data, it is not inherited through
   list styles.
2. `crates/drawingml/src/table.rs`: the same on `Table`, for `a:tblPr`.
3. `crates/drawingml/src/read.rs` (`ppr`, `rpr`) and `crates/pptx/src/read/shapes.rs` (`tblPr`):
   keep the element's `a:extLst` children, bounded as the shape-level slot is. Extensions holding
   relationship references are dropped, as they are now.
4. `crates/drawingml/src/write.rs` (`ppr`, `rpr`) and `crates/pptx/src/write/shapes.rs`
   (`a:tblPr`): write the kept children as the element's last child, after any extensions the
   writer emits itself. An element with only kept extensions is written open rather than empty.
5. `crates/pptx/src/read/shapes.rs` (`base`) and the matching writer: keep the `p:cNvPr`
   `a:extLst` children other than the decorative extension in the existing `Shape::raw_ext`, and
   write them back after it, for every element that has one: `p:sp`, `p:pic`, `p:grpSp`,
   `p:cxnSp`, `p:graphicFrame` and `p:contentPart`. Groups matter as much as shapes: an app can
   keep its own settings on a group.
6. Tests: each element keeps an unknown extension across open and save; one with a relationship
   reference is dropped; an oversized one is bounded; documents without extensions write
   byte-identically to today.

Nothing changes for apps that don't use the slots. Each app keeps its own mapping into them, in
line with AGENTS.md ("keep app-specific policy and model mapping in the app").

I can send this as a pull request if it's welcome.
