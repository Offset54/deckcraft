# 002: Document tree — keep the nested shape tree

> **Decided:** 2026-11-23 by the owner ("proceed with all your recommendations", after the Phase 2
> report) · **Status:** decided; applies from Phase 3

## Decision

SlideCraft keeps the host's nested, PresentationML-shaped document: slides, layouts and masters
hold `Vec<Shape>`, and groups hold their children. FigCraft concepts are added as **optional
fields that default to absent**, following the mapping in [`recon.md`](../recon.md) §4.2. It
does **not** move to FigCraft's flat node map keyed by stable ids.

## The choice

| | Keep the nested tree (chosen) | Move to a flat node map |
|---|---|---|
| Existing code | Every command, the renderer, the `.pptx` reader and writer, and `.deckcraft` stay as they are | Every one of those is rewritten against the new store |
| `.pptx` fidelity | The model keeps PresentationML's shape, so round trips stay close to the file | A mapping layer on both sides of every save and load |
| Undo | Already copy-on-write and O(1) per step (`Arc<Slide>`); measured in Phase 1 | FigCraft's store is `imbl`, which is MPL-2.0 and outside the licence allowlist; a replacement would have to be written |
| Auto layout, components, collaboration | Need lookups by id and parent links, built as indexes over the tree when needed | Native |
| Cost to undo the choice later | A flat store could still be introduced under the same commands | Hard to undo once commands and files depend on it |

## Consequences for Phase 3 and later

- **Frames** are groups with optional size, fill, clip and an optional `AutoLayout`. Children
  carry an optional `ChildLayout` (hug, fill or fixed; alignment; grid cell). On `.pptx` export
  they are groups with their resolved boxes.
- **Constraints** are an optional field on shapes inside a frame, applied when the frame is
  resized.
- **Lookups by id and by parent** that the tree can't answer quickly (layout solving, component
  sync, Smart Animate matching) use indexes built from the tree for the operation and dropped
  afterwards. They are never a second source of truth.
- **Stable identity:** an optional `origin` id, copied when a shape or slide is duplicated, so
  Smart Animate can match by identity before name (Phase 5).
- Every new field is `skip_serializing_if` absent, so existing `.deckcraft` files stay
  byte-identical, and every field PowerPoint has no markup for survives SlideCraft's own `.pptx`
  round trip through an extension (the pattern Phase 2 used).

## Revisit if

Collaboration (a Phase 7 stretch goal) needs per-node operational transforms that indexes over
the tree can't support efficiently. That would be its own decision record.
