# Command registry audit

> **Date:** 2026-11-18 · **Base:** upstream v0.19.0 plus the fork patches · **Phase:** 1

The brief's rule: every user action is a registered command with a stable id, undo support, and
exposure through the GUI, the CLI (`run --cmd`), the JSON control channel and the MCP server. This
is what the registry holds today, what the tests now pin, and the gaps that remain.

## The registry

| | Count | Where |
|---|---|---|
| Engine commands | 342 | `crates/engine/src/cmd/*`, `command_specs()` |
| — undo steps (journaled, undoable) | 240 | `cmd!` |
| — queries (change nothing) | 43 | `cmd!(query …)` |
| — not undo steps (files, navigation, selection, playback) | 59 | `cmd!(noundo …)` |
| UI-only commands | 53 | `crates/ui-egui/src/lib.rs`, `UI_COMMANDS` |
| MCP tools | 25 | `crates/mcp/src/tools.rs` |

## How each surface reaches a command

| Surface | Engine commands | UI-only commands |
|---|---|---|
| GUI (ribbon, menus, keys, pointer) | `Session::execute`, `Session::pointer`, `Session::key` | The app itself |
| CLI `deckcraft-cli run --cmd ID=PARAMS` | All 342 | None (no window) |
| JSON control channel (`deckcraft --control PORT`) | `engine.execute`, all 342 | All 53, by id |
| MCP, headless (`deckcraft-cli mcp`) | `run_command` and `batch`, all 342; 20 convenience tools call the same commands | None (no window) |
| MCP, connected (`deckcraft-cli mcp --connect PORT`) | All 342 | All 53, through `call_app` |

Every engine command is reachable from every surface by construction: `run_command` takes any id
the registry has. The MCP-only build of `samples/phase-1.deckcraft`
(`crates/mcp/tests/it/phase1_sample.rs`) exercises that path end to end.

## What the tests now hold (`crates/engine/src/tests_registry_audit.rs`)

1. **The commands outside undo are the reviewed ones.** The 43 queries and 59 commands that are
   not undo steps are listed in the test, grouped with the reason each is outside undo. A new
   command marked `query` or `noundo` fails the test until someone adds it with a reason. No
   command is an undo step without also being journaled (replayable as a script).
2. **The commands outside undo leave the deck alone.** On the sample deck, with a shape selected
   and with the parameters agents pass most (`{}`, `ids`, `id`, `index`, `text`/`color`/`word`/
   `path`), every one of them leaves the presentation as it was and adds no undo step. The one
   exception is `format.recentColor` (a fork patch), which changes only the deck's recent colours,
   by design. Commands that touch files, the font database or other documents are not run here;
   their own tests cover them.
3. **Frontends change documents only through commands.** The shipped code of `ui-egui`, `mcp` and
   the three apps never calls `Session::edit`, `doc_mut` or pushes undo history itself. A probe
   file that did was caught.

The existing tests this builds on: `every_command_id_is_registered_once`,
`every_params_doc_names_its_keys`, `every_command_survives_empty_and_hostile_params`, and the
soak test, which runs thousands of random commands and checks that undo then redo of every real
edit gives back the same document.

## Gaps

| Gap | Effect | Proposed |
|---|---|---|
| The 53 UI-only commands need a running app: views and zoom (21), panes and dialogs (25 `app.*`), the slide show and presenter view (4), animation preview, Ink Replay, the Thesaurus pane | A headless agent cannot open the slide show, switch to the sorter or open a dialog. None of them changes the deck, so building a deck headlessly is not affected | Keep: these are window state. When Phase 3 adds Grid view and Single-slide view, decide whether the view mode belongs to the document (saved with it, so headless) or to the window |
| Pointer gestures are reachable through `Session::pointer`, which MCP exposes as the `pointer` tool, but they are not registry entries with ids | An agent drags by sending pointer events, not by naming a gesture | No change: every gesture ends in the same edit a command makes, and the soak and UI tests drive them |
| Commands carry no machine-readable parameter schema, only a params doc string that `parse_params` reads | MCP clients see `params` as free-form JSON | Later: generate JSON Schema from the params docs for `list_commands` |
| `format.recentColor` changes the deck without an undo step | Undo doesn't forget a picked colour | Intended by the patch; recorded in `fork-patches.md` |
