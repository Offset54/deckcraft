# 005: Collaboration — pinned comments only, no live sessions

> **Decided:** 2026-11-24 by the owner (asked in the owner-decisions session) · **Status:**
> decided; applies to Phase 7

## Decision

From the brief's Phase 7 collaboration item, SlideCraft keeps **comments pinned to slide
coordinates** and drops **live sessions**: shared cursors, per-person undo and a self-hostable
sync server. Live sessions are out of scope, not a flagged stretch goal. Bringing them back needs
a new decision record (see "Revisit if").

## What the brief asked

> **Collaboration (stretch):** port FigCraft's live-session model (cursors, per-person undo,
> self-hostable server) and comments pinned to slide coordinates. Gate it behind a feature flag
> until it's stable. (§3)
>
> Phase 7: FigCraft live sessions + comments, behind a flag.

## Options put to the owner

| Option | Effort | Risk |
|---|---|---|
| **Pinned comments only (chosen)** | Small. DeckCraft already has `Slide.comments` (author, date, `x`/`y`, `resolved`, `replies`, optional `shape`) and the commands `comment.add`, `comment.reply`, `comment.resolve`, `comment.delete`, `comment.list`, `comment.next` and `comment.prev`. What's left is polish: pins that follow a moved shape, and a filter for resolved comments. | Low. Nothing in the document model or undo changes. |
| Keep live sessions as a flagged stretch goal | Large: several weeks. FigCraft's `collab` (≈10.2k lines) and `collab-net` (≈9.7k lines) sync a flat object/property map keyed by stable ids. Decision 002 kept the nested tree, so this needs a new sync layer over the tree (ids for every node, per-node operations, conflict rules), a server, and per-person undo on top of the copy-on-write history. | High. It touches the file format, undo and every command. A flag hides it from users but not from the code. |
| Drop collaboration entirely | None | Loses comment polish that is cheap and useful to a non-designer reviewing a deck |

## Why

- The brief's test for every feature is "does it make a good-looking deck easier to get?". For a
  single author, review comments do that; live co-editing mostly doesn't.
- Decision 002 chose the nested tree knowing it makes live sync harder. Its "revisit if" clause
  is about exactly this. Dropping live sessions means that clause isn't triggered.
- FigCraft's document store depends on `imbl` (MPL-2.0), which is outside the licence allowlist
  (decision 002). Porting the sync core would mean rewriting it in any case.

## Consequences

- Phase 7 work under this heading: comment pins that move with their shape and keep slide
  coordinates on `.pptx` round trip, plus a resolved filter in the Comments pane. All of it goes
  through the existing `comment.*` commands and MCP.
- No `collab` or `collab-net` crate is ported. No sync server, presence or per-person undo.
- The feature flag the brief asked for isn't needed: nothing unstable ships.

## Revisit if

The owner wants real-time co-editing. That would be its own decision record, and it would also
reopen decision 002 (a flat store or per-node operations over the tree).
