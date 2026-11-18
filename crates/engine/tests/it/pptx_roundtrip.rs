//! `.pptx` round trips of whole decks, judged by what the slides look like.
//!
//! - Every sample deck (the built-in sample and `samples/*.deckcraft`) is written as `.pptx`,
//!   opened again, and each slide must still match its golden picture (`golden.rs`). A second
//!   save of the reopened deck must give the same deck again.
//! - `DECKCRAFT_PPTX_CORPUS=<folder>` runs the same check on every `.pptx` in that folder: decks
//!   from outside this repository, which `AGENTS.md` §1.1 keeps out of it. Each deck is opened,
//!   saved, opened again, and compared slide by slide (count, text, shapes, pixels). Every loss is
//!   printed; the test fails when any deck lost something. Without the variable it does nothing.

use std::path::PathBuf;

use deckcraft_engine::render::golden::{Tolerance, compare, decode_png};
use deckcraft_engine::render::{Image, RenderOpts, render_slide};
use deckcraft_model::{Presentation, Shape};
use deckcraft_text::datetime::DateTime;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn render(doc: &Presentation, index: usize) -> Image {
    let now = DateTime { year: 2026, month: 1, day: 2, hour: 9, minute: 30, second: 0 };
    render_slide(doc, index, &RenderOpts { scale: 1.0, now: Some(now), ..Default::default() })
}

fn through_pptx(doc: &Presentation) -> Presentation {
    deckcraft_pptx::import(&deckcraft_pptx::export(doc).unwrap()).unwrap()
}

fn sample_decks() -> Vec<(String, Presentation)> {
    let mut s = deckcraft_engine::Session::new();
    deckcraft_engine::sample::open_sample(&mut s).unwrap();
    let mut out = vec![("builtin".to_string(), (*s.doc().unwrap().doc).clone())];
    let mut files: Vec<PathBuf> = std::fs::read_dir(root().join("samples"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "deckcraft"))
        .collect();
    files.sort();
    for f in files {
        let doc = deckcraft_format::load(&std::fs::read(&f).unwrap()).unwrap();
        out.push((f.file_stem().unwrap().to_string_lossy().to_string(), doc));
    }
    out
}

/// The text of every shape on a slide, in tree order, groups included.
fn texts(shapes: &[Shape]) -> Vec<String> {
    let mut out = vec![];
    deckcraft_model::walk(shapes, &mut |s, _| {
        if let Some(t) = &s.text {
            out.push(t.text());
        }
    });
    out
}

fn shape_count(shapes: &[Shape]) -> usize {
    let mut n = 0;
    deckcraft_model::walk(shapes, &mut |_, _| n += 1);
    n
}

/// What `after` lost against `before`, slide by slide; empty when nothing.
fn losses(before: &Presentation, after: &Presentation, pixels: bool) -> Vec<String> {
    let mut out = vec![];
    if before.slides.len() != after.slides.len() {
        out.push(format!("{} slides became {}", before.slides.len(), after.slides.len()));
        return out;
    }
    for (i, (a, b)) in before.slides.iter().zip(&after.slides).enumerate() {
        let n = i + 1;
        if texts(&a.shapes) != texts(&b.shapes) {
            out.push(format!("slide {n}: text changed: {:?} → {:?}", texts(&a.shapes), texts(&b.shapes)));
        }
        if shape_count(&a.shapes) != shape_count(&b.shapes) {
            out.push(format!("slide {n}: {} shapes became {}", shape_count(&a.shapes), shape_count(&b.shapes)));
        }
        if a.notes.text() != b.notes.text() {
            out.push(format!("slide {n}: notes changed"));
        }
        if a.animations.len() != b.animations.len() {
            out.push(format!("slide {n}: {} animations became {}", a.animations.len(), b.animations.len()));
        }
        if a.transition.as_ref().map(|t| &t.kind) != b.transition.as_ref().map(|t| &t.kind) {
            out.push(format!(
                "slide {n}: transition {:?} became {:?}",
                a.transition.as_ref().map(|t| &t.kind),
                b.transition.as_ref().map(|t| &t.kind)
            ));
        }
        if pixels {
            match compare(&render(after, i), &render(before, i), Tolerance::SLIDE.channel) {
                Ok(d) if d.within(Tolerance::SLIDE) => {}
                Ok(d) => out.push(format!("slide {n}: {:.3}% of pixels differ", d.share() * 100.0)),
                Err(e) => out.push(format!("slide {n}: {e}")),
            }
        }
    }
    out
}

#[test]
fn sample_decks_survive_a_pptx_round_trip() {
    let fonts = deckcraft_fonts::CRAFT_FONTS.iter().any(|f| f.family == "Inter");
    let mut failures = vec![];
    for (deck, doc) in sample_decks() {
        let reopened = through_pptx(&doc);
        for l in losses(&doc, &reopened, false) {
            failures.push(format!("{deck}: {l}"));
        }
        // The reopened deck saves to the same deck again: nothing drifts from save to save.
        let twice = through_pptx(&reopened);
        if losses(&reopened, &twice, false).len() + usize::from(twice.slides.len() != reopened.slides.len()) > 0 {
            failures.push(format!("{deck}: a second .pptx save changed the deck again"));
        }
        // What the reopened deck looks like, against the stored pictures of the original.
        if fonts {
            for i in 0..reopened.slides.len() {
                let path = root().join("samples/golden").join(&deck).join(format!("slide-{:02}.png", i + 1));
                let Some(stored) = std::fs::read(&path).ok().and_then(|b| decode_png(&b)) else {
                    failures.push(format!("{}: missing", path.display()));
                    continue;
                };
                match compare(&render(&reopened, i), &stored, Tolerance::SLIDE.channel) {
                    Ok(d) if d.within(Tolerance::SLIDE) => {}
                    Ok(d) => failures.push(format!(
                        "{deck} slide {}: after .pptx, {:.3}% of pixels differ from the golden picture",
                        i + 1,
                        d.share() * 100.0
                    )),
                    Err(e) => failures.push(format!("{deck} slide {}: {e}", i + 1)),
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn an_external_corpus_survives_a_pptx_round_trip() {
    let Some(dir) = std::env::var_os("DECKCRAFT_PPTX_CORPUS") else { return };
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pptx")))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no .pptx files in {}", PathBuf::from(&dir).display());
    let mut lossy = 0;
    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy();
        let (doc, damage) = match deckcraft_pptx::import_checked(&std::fs::read(f).unwrap()) {
            Ok(r) => r,
            Err(e) => {
                println!("{name}: does not open: {e}");
                lossy += 1;
                continue;
            }
        };
        let reopened =
            match deckcraft_pptx::export(&doc).map_err(|e| e.to_string()).and_then(|b| deckcraft_pptx::import(&b).map_err(|e| e.to_string())) {
                Ok(r) => r,
                Err(e) => {
                    println!("{name}: save or reopen failed: {e}");
                    lossy += 1;
                    continue;
                }
            };
        let lost = losses(&doc, &reopened, true);
        println!("{name}: {} slides, {} parts damaged on open, {} losses", doc.slides.len(), damage.len(), lost.len());
        for d in &damage {
            println!("  damaged on open: {d}");
        }
        for l in &lost {
            println!("  {l}");
        }
        if !lost.is_empty() {
            lossy += 1;
        }
    }
    assert_eq!(lossy, 0, "{lossy} of {} decks lost something; the list is above (run with --nocapture)", files.len());
}
