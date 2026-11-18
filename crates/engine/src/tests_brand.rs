//! SlideCraft carries no ArtCraft marks and doesn't point people at the ArtCraft website or
//! community: DeckCraft's brand licence (`docs/brand/LICENSE-brand.txt`, "Forks and modified
//! versions") requires that of a published modified version. It may say in plain text that it is
//! based on DeckCraft by the ArtCraft team, which `deckcraft_model::brand::BASED_ON` does.
//!
//! Installers (`packaging/`) still carry DeckCraft's identity and links until the packaging step
//! changes them (`docs/decisions/001-product-identity.md`); they are not checked here yet.

use std::path::{Path, PathBuf};

fn rust_sources(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_sources(&path, files);
        } else if path.extension().is_some_and(|e| e == "rs") {
            files.push(path);
        }
    }
}

/// What a modified version must not show: the ArtCraft logo files, and links to the ArtCraft site
/// and community.
const MARKS: &[&str] = &["getartcraft.com", "discord.gg/artcraft", "artcraft-logo", "artcraft-mark"];

#[test]
fn shipped_code_and_the_readme_carry_no_artcraft_marks_or_links() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = vec![root.join("README.md")];
    for dir in ["crates", "apps"] {
        for entry in std::fs::read_dir(root.join(dir)).unwrap() {
            let src = entry.unwrap().path().join("src");
            if src.is_dir() {
                rust_sources(&src, &mut files);
            }
        }
    }
    assert!(files.iter().any(|f| f.ends_with("engine/src/links.rs")), "{files:?}");
    let mut found = vec![];
    // This file names the marks it looks for.
    for f in files.into_iter().filter(|f| !f.ends_with("engine/src/tests_brand.rs")) {
        let text = std::fs::read_to_string(&f).unwrap();
        for (n, line) in text.lines().enumerate() {
            if MARKS.iter().any(|m| line.contains(m)) {
                found.push(format!("{}:{}: {}", f.display(), n + 1, line.trim()));
            }
        }
    }
    assert!(found.is_empty(), "ArtCraft marks or links: {found:#?}");
}

/// Only the licence text stays in `docs/brand/`: it is the terms the README and NOTICE point to.
#[test]
fn the_artcraft_logo_files_are_gone() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/brand");
    let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    names.sort();
    assert_eq!(names, ["LICENSE-brand.txt"]);
}
