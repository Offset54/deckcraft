# 007: Installer identity — prepared, held until tested on real machines

> **Decided:** 2026-11-24 by the owner (asked in the owner-decisions session) · **Status:**
> change prepared as a patch, **not applied**. It waits for the owner to install it on macOS,
> Windows and Linux. · **Closes:** decision 001, "Pending: installers"

## Decision

The installer identity change is written in full and kept as a patch,
[`../packaging/slidecraft-installer-identity.patch`](../packaging/slidecraft-installer-identity.patch).
It is applied to the work branch only after the owner has built and installed it on real machines
and the checklist below passes. Nothing here is a release: publishing a build needs its own
explicit yes.

## Options put to the owner

| Option | Result |
|---|---|
| **Prepare, hold for testing (chosen)** | The change exists and is reviewable now. Nothing in the main work branch changes until it has been installed for real. |
| Do it now, let CI build it | Only `packaging-lint` and the unit tests would check it. An MSI that won't upgrade, or a Flatpak that won't start, would only be found by a user. |
| Defer to Phase 8 | No work now. The identity mismatch (decision 001's `APP_ID` against DeckCraft's desktop file) would stay. |

## What the patch changes

Made on 2026-11-24 from `claude/new-session-lm99ab` at `e3371cd`. It's one commit (21 files,
+116/−95, 13 renames). Apply it with `git am docs/packaging/slidecraft-installer-identity.patch`.

| Area | Before | After |
|---|---|---|
| App identifier: `APP_ID` (`apps/deckcraft/src/main.rs`), Wayland/X11 app id, `StartupWMClass` | `ai.storyteller.deckcraft` | `io.github.offset54.slidecraft` (= `deckcraft_model::brand::APP_ID`) |
| Linux desktop, AppStream and MIME files, icon names (`assets/app-icon/hicolor/*/apps/…`), `icons.sh`, `package.sh`, FreeBSD `package.sh`, `omastore.toml` | `ai.storyteller.deckcraft.*` | `io.github.offset54.slidecraft.*` (renamed, contents unchanged for icons) |
| Flatpak id and manifests | `ai.storyteller.deckcraft` | `io.github.offset54.slidecraft` |
| macOS `CFBundleIdentifier`, exported UTI | `ai.storyteller.deckcraft`, `….presentation` | `io.github.offset54.slidecraft`, `….presentation` |
| Visible names: desktop `Name`, AppStream `<name>`, `CFBundleName`/`CFBundleDisplayName`, MSI product, shortcut, Apps & Features, "Open with" | DeckCraft | SlideCraft |
| Developer / vendor / MSI manufacturer | Learning Machines LLC (`ai.storyteller`) | Offset54 (`io.github.offset54`) |
| Links: AppStream homepage, help, bug tracker, contact; MSI About, Update and Help links; nfpm homepage | `getartcraft.com`, `discord.gg/artcraft`, `storytold/deckcraft` | `github.com/Offset54/deckcraft` (and `/issues`) |
| Windows `UpgradeCode` | `6F9C69E7-7825-40D2-BF26-C274DA676425` (DeckCraft's) | `8D7E7FDD-7C66-40CE-AB2A-E8AC686EE93A` (new); the app component GUID is new too |
| Windows install folder and registry | `Programs\ArtCraft\DeckCraft`, `Software\DeckCraft` | `Programs\Offset54\SlideCraft`, `Software\SlideCraft`; `install-policy.json`, `.wxi`, `check-install-policy.ps1` and `INSTALLATION.md` follow |
| Windows ProgId | `DeckCraft.Presentation` | `SlideCraft.Presentation` |
| File associations | `.deckcraft` only, although Save writes `.slidecraft` (decision 001) | `.slidecraft` **and** `.deckcraft` on all three desktops (new MIME type `application/x-slidecraft`); the test `every_desktop_offers_the_same_file_types` checks both |

Unchanged on purpose: the binary names `deckcraft` and `deckcraft-cli` (decision 001), the icon
artwork, and `application/x-deckcraft`.

## What it breaks or migrates for existing installs

- **No in-place upgrade from a DeckCraft-identity build.** A new UpgradeCode, bundle id and
  Flatpak id make SlideCraft a different app to every OS. It installs **beside** an existing
  install, which has to be uninstalled by hand. Settings already moved to a `SlideCraft` folder in
  decision 001, so nothing user-level is lost.
- **The ArtCraft Launcher no longer manages this fork's Windows build.** The launcher finds apps
  by their suite folder and UpgradeCode. `custom-windows.yml` (patch 12 in
  [`../fork-patches.md`](../fork-patches.md)) builds `custom/patches` for the launcher's mirror,
  and patch 10 hands updates to the launcher when `CRAFT_CUSTOM_BUILD` is set. With this patch,
  SlideCraft's builds fall outside that arrangement.
- **Update channel: not changed by the patch, and still open.** `crates/ui-egui/src/lib.rs` uses
  `craft_update::suite::DECKCRAFT`. An official-mode build would offer official DeckCraft
  releases as updates, and installing one would replace SlideCraft's binaries with upstream's.
  Builds with `CRAFT_CUSTOM_BUILD` set leave updates to the launcher (patch 10), which no longer
  applies once the launcher doesn't manage the app. The safe options are (a) turn the in-app
  updater off for SlideCraft until it has its own release feed, or (b) point it at
  `Offset54/deckcraft` releases. Either is a code change to decide when this patch is applied.
- **macOS bundle and DMG names.** The patch renames what macOS shows (`CFBundleName`,
  `CFBundleDisplayName`), but the bundle is still built as `DeckCraft.app`, with executable
  `DeckCraft`, volume name `DeckCraft`, and a DMG layout (`packaging/macos/dmg/dmg-layout.DS_Store`)
  that places `DeckCraft.app`. Renaming those means changing `packaging/macos/package.sh` and
  `dmg/generate.py` and regenerating the layout (`python3 packaging/macos/dmg/generate.py`, which
  needs `resvg`, `ds_store` and `mac_alias`). That is the first follow-up when the patch is applied.

## CI

The patch changes no workflow. `packaging-lint.yml` runs `test-install-policy.ps1` against the
edited policy, so CI checks the Windows policy once the patch lands. `custom-windows.yml` and
`release.yml` are untouched: `AGENTS.md` keeps CI changes out of feature work, and what the fork's
own build should become (stop feeding the launcher, or publish SlideCraft builds elsewhere) is a
CI and publishing decision, so it needs its own explicit yes.

## Checklist before applying (owner, on real machines)

None of this can be built or tested in the cloud environment. What did run here on 2026-11-24,
with the patch applied on a local branch:
- `git apply --check`: applies cleanly to `e3371cd`;
- `cargo test -p deckcraft --bin deckcraft every_desktop`: 1 passed (macOS, Windows and Linux offer
  the same file types, `.slidecraft` included);
- `cargo test -p deckcraft-ui-egui --test flatpak_updates`: 1 passed (it reads the renamed Flatpak
  manifests);
- `cargo xtask assets`: all 69 asset files are attributed (the renamed icon paths).

What didn't run: the full `cargo xtask ci`, `packaging-lint`, WiX, `appstreamcli` and every
installer build.

1. **Windows:** `packaging/windows/package.ps1 -Arch x64`. Install the MSI for the current user,
   then for all users. Check the folders `Programs\Offset54\SlideCraft` and
   `Program Files\Offset54\SlideCraft`, the Start menu name, Apps & Features (name, links),
   double-clicking a `.slidecraft` file and a `.deckcraft` file, and "Open with" for `.pptx`.
   Install a second build of the same version over it (same-version upgrade). Run
   `check-install-policy.ps1`. Confirm a DeckCraft install on the same machine is untouched.
2. **macOS:** `packaging/macos/package.sh`. Check the app name in the menu bar and Dock, Finder's
   Open With for `.slidecraft`, `.deckcraft` and `.pptx`, and that DeckCraft can be installed
   beside it.
3. **Linux:** `packaging/linux/package.sh` (deb, rpm, AppImage), then the Flatpak bundle. Check the
   menu entry, the window icon and grouping (the app id has to match `StartupWMClass`), double-clicking
   `.slidecraft`, and `appstreamcli validate` on the metainfo.
4. Decide the update channel (above) and make the macOS bundle and DMG rename.
5. Then apply the patch on the work branch, run `CARGO_INCREMENTAL=0 cargo xtask ci`, and record the
   results here.
