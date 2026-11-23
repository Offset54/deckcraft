# 003: Grid view and single-slide view are window state

> **Decided:** 2026-11-23 by the agent on the recommendation below, under the owner's Phase 3
> go-ahead · **Status:** recommendation in force; **the owner may overturn it** (see "For the
> owner") · Raised in [`reports/phase-1.md`](../reports/phase-1.md) §4

## Decision

Which view a window shows (Grid view, the zoomable board of every slide, or Single-slide view,
the slide being edited with its filmstrip) is **window state**. It lives in the UI's persisted
state (`UiState::view`, saved with the app's settings), not in the presentation. Opening a deck
does not change the view, saving does not record it, and two windows on one deck may show
different views.

Grid view is the host's Slide Sorter: it already shows every slide as a board with sections
(add, rename, fold, drag), reordering by drag, multi-select and zoom. Phase 3 makes the switch
one keystroke (`view.grid`, **Cmd/Ctrl+Alt+B**, toggling between the two) and widens the board's
zoom.

## The choice

| | Window state (chosen) | Document state |
|---|---|---|
| Matches | PowerPoint and Figma Slides: a view is how you look at a deck, not part of it | Keynote's "last view" behaviour |
| `.pptx` | Nothing to write. (PowerPoint keeps its last view in `viewProps.xml`, which the host doesn't model; it is not needed for fidelity) | A `viewProps.xml` mapping both ways, for a value PowerPoint itself ignores on most opens |
| Undo, dirty flag | Switching views never makes a deck "modified" or adds an undo step | Either every switch dirties the deck, or the field needs a special case outside undo |
| Agents | `call_app` `view.grid` drives a running app; headless sessions have no view, and building a deck never depends on one | Headless agents could set it, but nothing headless would show it |
| Several windows on one deck (`view.newWindow`) | Each window keeps its own view | One shared value fights between windows |
| Cost to reverse | Adding a document field later is a pure addition | Removing a saved field later leaves it in old files |

## Consequences

- `view.grid` is a UI command (listed in `UI_COMMANDS`, reachable through the control channel
  and MCP `call_app`), not an engine command. It is not in the engine's registry audit, like
  `view.sorter` and `view.normal`.
- Selection carries over: the slides selected in the grid are the ones the filmstrip shows
  selected, because both read the engine's `Selection::slides`.
- Sections, slide order and hidden slides changed in Grid view are document edits through the
  existing commands (`section.*`, `slide.move`, `slide.hide`), with undo.

## For the owner

This was the recommendation in the Phase 1 report; the Phase 3 instructions asked for it to be
decided in a record and noted for you. Say if you want a deck to reopen in the view it was saved
in; that would add an optional `.slidecraft` field and leave `.pptx` alone.
