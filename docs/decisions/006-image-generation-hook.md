# 006: Image-generation bridge — a generic, local-only command hook

> **Decided:** 2026-11-24 by the owner (asked in the owner-decisions session) · **Status:**
> decided; applies to Phase 7 · **Fills:** the brief's `{{OPTIONAL: …}}` placeholder

## Decision

The brief's optional item stays, as a **generic hook that runs a local program the user
configures**. It names no product: no ArtCraft name, mark or link appears in the command, the UI,
the settings or the docs.

## Placeholder value

The brief is kept verbatim ([`../brief/slidecraft-agent-prompt.md`](../brief/slidecraft-agent-prompt.md)
§4, Phase 7). Its placeholder reads:

> {{OPTIONAL: ArtCraft bridge. A command that sends a placeholder and a prompt to a locally
> installed ArtCraft app (or any image-generation backend I configure) and drops the result in as
> an image fill. Opt-in only, with no network calls by default.}}

The value that fills it, as decision 001 recorded the other placeholders:

> **Keep, generalised:** a command that sends a placeholder's size and a text prompt to a local
> image-generation program the user has configured, and drops the image it returns into the
> placeholder as an image fill. Off until configured. SlideCraft itself makes no network calls.
> No vendor or product is named.

## Options put to the owner

| Option | Result |
|---|---|
| **Generic local hook (chosen)** | Keeps the useful part of the brief and avoids the trademark question entirely |
| Keep as written (ArtCraft bridge) | The brief (§1.6) and the brand licence (`docs/brand/LICENSE-brand.txt`) bar the ArtCraft name from a modified version's product UI without written permission. Decision 001 removed the marks. A named bridge would bring them back. |
| Delete | Phase 7 loses an opt-in feature that costs little |

## Shape of the feature (for Phase 7)

- **One command** (id to be fixed in Phase 7, e.g. `picture.generateFill`). It takes a target
  placeholder or shape and a prompt, and it's registered with MCP like every other command.
- **Configured, not discovered.** In settings, the user names a program path and arguments. The
  program reads a small JSON request (prompt, width and height in pixels, an output path inside a
  temporary directory) and writes a PNG or JPEG. Without a configured program, the command is
  disabled and says why.
- **No network from SlideCraft.** The app runs a local process and nothing else. Whatever the
  configured program does is the user's choice. The docs say so plainly.
- **Hostile output.** The returned file goes through the normal picture import: size and pixel
  caps, and a decode error is reported, never a panic. There's a time limit, and the process is
  killed when it's reached. It's one undo step.
- **Not in wasm.** The web build has no local processes, so the command is reported as
  unavailable there.
- **Wording.** "Compatible with ArtCraft"–style wording is allowed in docs (brief §1.6), but the
  docs give a neutral example program, not a named product.
