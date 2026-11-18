//! The product's identity: its name, the identifier installers and the system know it by, its
//! document extension, and how it credits the app it is based on.
//!
//! This app is SlideCraft, a modified version of DeckCraft. DeckCraft's brand licence
//! (`docs/brand/LICENSE-brand.txt`, "Forks and modified versions") requires a published modified
//! version to leave out the ArtCraft marks and not present itself as an ArtCraft product; it may
//! say in plain text that it is based on DeckCraft by the ArtCraft team. Everything the app shows
//! about itself reads from here, so the name lives in one place and upstream's own strings stay
//! easy to merge.
//!
//! Crate and binary names (`deckcraft-*`, `deckcraft-cli`), the built-in theme called "DeckCraft"
//! and the identity inside the native package (its `mimetype` member) are not branding and keep
//! upstream's names: they are code and data shared with DeckCraft.

/// The product name as a string literal, for `concat!` (`concat!("About ", product_name!())`).
#[macro_export]
macro_rules! product_name {
    () => {
        "SlideCraft"
    };
}

/// The product name people see.
pub const NAME: &str = product_name!();

/// The reverse-DNS application identifier: the macOS bundle id, the Linux desktop and AppStream
/// id, the Windows installer's product, and the folder settings and recovery files go under. It
/// must differ from DeckCraft's (`ai.storyteller.deckcraft`), so the two apps install side by side.
pub const APP_ID: &str = "io.github.offset54.slidecraft";

/// The extension SlideCraft saves its own documents with. The bytes are the DeckCraft package
/// (`.deckcraft`), so files of either extension open in either app.
pub const NATIVE_EXT: &str = "slidecraft";

/// The folder name for settings, recovery files and dictionaries where folder names are lower
/// case by convention (Linux and the BSDs, under the XDG folders). macOS and Windows use [`NAME`].
pub const UNIX_DIR: &str = "slidecraft";

/// Every extension a native document can have, the one SlideCraft writes first.
pub const NATIVE_EXTS: &[&str] = &[NATIVE_EXT, "deckcraft"];

/// The credit to the app this one is based on, in the plain words the brand licence allows.
pub const BASED_ON: &str =
    concat!(product_name!(), " is based on DeckCraft by the ArtCraft team. It is not made, endorsed or supported by the ArtCraft team.");

/// DeckCraft's copyright and licence, which the MIT and Apache-2.0 licences require to be kept.
pub const UPSTREAM_COPYRIGHT: &str = "MIT OR Apache-2.0 · © 2026 ArtCraft Team and the DeckCraft contributors";

/// Whether `path` names a native document, by its extension (either spelling, any case).
pub fn is_native_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    NATIVE_EXTS.iter().any(|e| lower.strip_suffix(e).is_some_and(|stem| stem.ends_with('.')))
}

#[cfg(test)]
mod tests {
    use super::{APP_ID, BASED_ON, NAME, NATIVE_EXT, is_native_path};

    #[test]
    fn the_product_is_slidecraft_and_credits_deckcraft_without_claiming_artcraft() {
        assert_eq!(NAME, "SlideCraft");
        assert_eq!(concat!("About ", product_name!()), "About SlideCraft");
        assert_ne!(APP_ID, "ai.storyteller.deckcraft", "SlideCraft must install beside DeckCraft");
        assert!(BASED_ON.starts_with("SlideCraft is based on DeckCraft by the ArtCraft team"));
        assert!(BASED_ON.contains("not made, endorsed or supported by the ArtCraft team"));
        assert_eq!(NATIVE_EXT, "slidecraft");
    }

    #[test]
    fn both_native_extensions_are_recognised() {
        for p in ["talk.slidecraft", "a/b/TALK.SLIDECRAFT", "old.deckcraft", "x.DeckCraft"] {
            assert!(is_native_path(p), "{p}");
        }
        for p in ["talk.pptx", "slidecraft", "x.notslidecraft", "deck.deckcraft.saving", ""] {
            assert!(!is_native_path(p), "{p}");
        }
    }
}
