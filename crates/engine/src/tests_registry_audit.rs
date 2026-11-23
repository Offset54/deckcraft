//! The command registry audit (Phase 1, `docs/command-audit.md`).
//!
//! Every user action is a command that the GUI, the CLI, the control channel and MCP reach through
//! [`Session::execute`]. Most commands are one undo step each. The ones that are not fall into two
//! groups the registry marks itself: queries (`cmd!(query …)`: they answer and change nothing) and
//! commands that are not undo steps (`cmd!(noundo …)`: files, navigation, selection, playback).
//! These tests pin both lists, so a new command outside undo is a decision someone reviews here,
//! and check that the commands outside undo really leave the deck alone. They also keep the
//! frontends from changing a document any way but through a command.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::json;

use crate::{Session, command_specs, sample};

/// Queries: they answer from the deck and change nothing.
const QUERIES: &[&str] = &[
    // Inspecting the deck, a slide, a shape or its text.
    "document.inspect",
    "slide.inspect",
    "slide.actions",
    "slide.colorAt",
    "shape.inspect",
    "shape.points",
    "shape.sites",
    "smartArt.nodes",
    // The deck's paragraph and character styles and the caret's (Phase 2).
    "style.list",
    "text.get",
    "text.find",
    "format.state",
    "format.ruler",
    "format.recentColors",
    "picture.cropInfo",
    "animation.get",
    "comment.list",
    "guide.list",
    // Distances between objects (Phase 3): geometry read off the slide.
    "arrange.measure",
    "font.used",
    "media.info",
    "file.info",
    // Catalogues the UI offers.
    "commands.list",
    "file.templates",
    "file.recovery.list",
    "slide.reuseList",
    "insert.icons",
    "shape.presets",
    "picture.styles",
    "picture.recolors",
    "design.themes",
    "design.variants",
    "design.ideas",
    "transition.list",
    "animation.list",
    // Checks.
    "review.accessibility",
    "review.readingOrder",
    "review.spelling",
    "review.dictionary",
    "review.thesaurus",
    "review.revisions",
    // Output without a path: the bytes or pixels come back in the answer.
    "file.saveBytes",
    "file.render",
    // An input method's composition: the text it composes is one undo step when it ends
    // (`Session::end_composition`), not one per keystroke.
    "text.compose",
    "text.deleteSurrounding",
];

/// Commands that are not undo steps of their own.
const NOT_UNDO_STEPS: &[&str] = &[
    // Files and windows: they open, save, close or switch documents; the undo history belongs to
    // each document.
    "file.new",
    "file.open",
    "file.adopt",
    "file.openBytes",
    "file.save",
    "file.saveAs",
    "file.saveTemplate",
    "file.export",
    "file.close",
    "file.revert",
    "file.recovery.save",
    "file.recovery.open",
    "file.recovery.discard",
    "shape.saveAsPicture",
    "window.next",
    "font.add",
    "font.rescan",
    // Undo itself, and the clipboard.
    "edit.undo",
    "edit.redo",
    "edit.copy",
    // Selection, navigation and views: they move the selection or the edited target, not the deck.
    "edit.select",
    "edit.selectAll",
    // Selecting inside groups and back out (Phase 3): selection only.
    "edit.selectChildren",
    "edit.selectParent",
    "edit.deselect",
    "slide.go",
    "slide.next",
    "slide.previous",
    "slide.first",
    "slide.last",
    "slide.selectRange",
    "slide.selectSlides",
    "table.selectCells",
    "text.edit",
    "text.exit",
    "text.move",
    "text.select",
    "text.selectAll",
    "text.selectWord",
    "text.selectParagraph",
    "comment.next",
    "comment.prev",
    "view.slideMaster",
    "view.closeMaster",
    "show.fromStart",
    "show.fromCurrent",
    // Tools that wait for the next click (the painted shapes are the undo step).
    "format.painter",
    "animation.painter",
    // Typing: AutoCorrect joins the undo step of the text it corrects; a commit ends an input
    // method's composition, which is one step (`Session::end_composition`).
    "text.autocorrect",
    "text.commit",
    // Review: the dictionary is the user's, not the deck's; Compare holds the revised copy apart
    // until a change is accepted, which is the undo step.
    "review.addToDictionary",
    "review.compare",
    "review.reject",
    "review.endCompare",
    // Media playback.
    "media.play",
    "media.pause",
    "media.toggle",
    "media.stop",
    "media.seek",
    "media.gifPlay",
    // Recent Colors (a fork patch): remembering a picked colour is not an edit of the slides, so
    // it is not undone with them. It is the one command here that changes the deck.
    "format.recentColor",
];

/// Commands from the lists above that touch files, the font database or other documents, which
/// the behaviour check below doesn't run.
const OUTSIDE_THE_DECK: &[&str] = &[
    "file.new",
    "file.open",
    "file.adopt",
    "file.openBytes",
    "file.save",
    "file.saveAs",
    "file.saveTemplate",
    "file.export",
    "file.close",
    "file.revert",
    "file.recovery.save",
    "file.recovery.open",
    "file.recovery.discard",
    "shape.saveAsPicture",
    "window.next",
    "font.add",
    "font.rescan",
];

fn sorted(v: impl IntoIterator<Item = &'static str>) -> Vec<&'static str> {
    let mut v: Vec<_> = v.into_iter().collect();
    v.sort_unstable();
    v
}

#[test]
fn the_commands_outside_undo_are_the_reviewed_ones() {
    let specs = command_specs();
    let queries = sorted(specs.iter().filter(|c| !c.journal && !c.undoable).map(|c| c.id));
    let not_undo = sorted(specs.iter().filter(|c| c.journal && !c.undoable).map(|c| c.id));
    assert_eq!(queries, sorted(QUERIES.iter().copied()), "queries changed: add the new one to QUERIES with why it changes nothing");
    assert_eq!(not_undo, sorted(NOT_UNDO_STEPS.iter().copied()), "commands outside undo changed: add the new one to NOT_UNDO_STEPS with why");
    // A command that is an undo step is also journaled (replayable as a script).
    let unjournaled: Vec<&str> = specs.iter().filter(|c| c.undoable && !c.journal).map(|c| c.id).collect();
    assert!(unjournaled.is_empty(), "{unjournaled:?}");
}

/// Run on the sample deck with a shape selected and with the parameters agents most often pass,
/// every query and every command outside undo leaves the deck as it was, except
/// `format.recentColor`, which changes only the recent colours. None of them adds an undo step.
#[test]
fn the_commands_outside_undo_leave_the_deck_alone() {
    let mut s = Session::new();
    sample::open_sample(&mut s).unwrap();
    s.execute("slide.go", &json!({"index": 1})).unwrap();
    let id = s.doc().unwrap().current_slide().unwrap().shapes.iter().find(|x| x.text.is_some()).unwrap().id.0;
    s.execute("edit.select", &json!({"ids": [id]})).unwrap();
    let start = s.doc().unwrap().clone();
    let params = [
        json!({}),
        json!({"ids": [id]}),
        json!({"id": id}),
        json!({"index": 0}),
        json!({"text": "Abc", "color": "#123456", "word": "aurora", "path": [[0, 0], [1, 1]]}),
    ];
    let mut changed = vec![];
    for c in command_specs().iter().filter(|c| !c.undoable && !OUTSIDE_THE_DECK.contains(&c.id)) {
        for p in &params {
            *s.doc_mut().unwrap() = start.clone();
            let _ = s.execute(c.id, p);
            let st = s.doc().unwrap();
            assert_eq!(st.history.undo.len(), start.history.undo.len(), "{} {p} added an undo step", c.id);
            if Arc::ptr_eq(&start.doc, &st.doc) || *start.doc == *st.doc {
                continue;
            }
            let mut only_colours = (*st.doc).clone();
            only_colours.recent_colors = start.doc.recent_colors.clone();
            if c.id == "format.recentColor" && only_colours == *start.doc {
                continue;
            }
            changed.push(format!("{} {p}", c.id));
        }
    }
    assert!(changed.is_empty(), "changed the deck without an undo step: {changed:#?}");
}

/// The source files of a crate's `src`, without its test files.
fn sources(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if !name.contains("tests") {
                sources(&path, files);
            }
        } else if path.extension().is_some_and(|e| e == "rs") && !name.contains("tests") && !name.starts_with("test") {
            files.push(path);
        }
    }
}

/// The shipped part of a source file: everything before its first inline test module. A
/// `#[cfg(test)]` import, or a test module declared in a file of its own (`mod tests;`), doesn't
/// start one.
fn shipped(text: &str) -> &str {
    let mut from = 0;
    while let Some(i) = text[from..].find("#[cfg(test)]") {
        let at = from + i;
        let next = text[at + "#[cfg(test)]".len()..].trim_start();
        let first_line = next.lines().next().unwrap_or("").trim_end();
        let import = ["use ", "pub use ", "pub(crate) use ", "pub(super) use "].iter().any(|p| next.starts_with(p));
        let declared = (next.starts_with("mod ") || next.starts_with("pub mod ") || next.starts_with("pub(crate) mod ")) && first_line.ends_with(';');
        if import || declared {
            from = at + 1;
            continue;
        }
        return &text[..at];
    }
    text
}

/// The UI, the MCP server and the apps change a document only by running commands, so every
/// action they offer is one an agent can take too, with the same undo step. `Session::edit` and
/// `doc_mut` are the engine's own ways in; a frontend that used them would make an action no
/// command can reproduce.
#[test]
fn frontends_change_documents_only_through_commands() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = vec![];
    for dir in ["crates/ui-egui/src", "crates/mcp/src", "apps/deckcraft/src", "apps/deckcraft-cli/src", "apps/deckcraft-web/src"] {
        sources(&root.join(dir), &mut files);
    }
    assert!(files.iter().any(|f| f.ends_with("ui-egui/src/app.rs")), "{files:?}");
    let mut offenders = vec![];
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap();
        for (n, line) in shipped(&text).lines().enumerate() {
            let line = line.trim_start();
            let edits = [".edit(|", ".doc_mut()", "history.undo.push", "history.redo.push"].iter().any(|w| line.contains(w));
            if edits && !line.starts_with("//") {
                offenders.push(format!("{}:{}: {line}", path.display(), n + 1));
            }
        }
    }
    assert!(offenders.is_empty(), "run a command (Session::execute) instead: {offenders:#?}");
}
