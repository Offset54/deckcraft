//! Golden-image tests: every sample deck is drawn headlessly, slide by slide, and compared with
//! the pictures stored in `samples/golden/<deck>/slide-NN.png`.
//!
//! The decks are the built-in sample (`deckcraft-cli render --sample`) and every
//! `samples/*.deckcraft` file. A render may differ from its stored picture by a few levels on
//! anti-aliased edges, which `vello_cpu` draws differently with each machine's SIMD level; it fails
//! when more than [`Tolerance::SLIDE`]'s share of the slide differs. On a failure the render and a
//! picture of the differences are written next to the test binary's temporary folder, and the
//! message says where.
//!
//! `DECKCRAFT_BLESS_GOLDEN=1` rewrites the stored pictures. Only do that for a deliberate change to
//! how slides look, and look at the new pictures before committing them.
//!
//! The pictures depend on the fonts the build embeds from craft-fonts. A build without them draws
//! with other faces, so the comparison is skipped there unless `CRAFT_FONTS_REQUIRED` is set,
//! which turns the missing fonts into a failure.

use std::path::{Path, PathBuf};

use deckcraft_engine::Session;
use deckcraft_engine::render::golden::{Tolerance, compare, decode_png, diff_image};
use deckcraft_engine::render::{Image, RenderOpts, render_slide};
use deckcraft_model::Presentation;
use deckcraft_text::datetime::DateTime;

/// Pixels per point: the slide at its own size (960 × 540 for 16:9).
const SCALE: f64 = 1.0;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn golden_dir(deck: &str) -> PathBuf {
    root().join("samples").join("golden").join(deck)
}

/// The decks to check, by name: the built-in sample, then each `samples/*.deckcraft` by file stem.
fn decks() -> Vec<(String, Presentation)> {
    let mut s = Session::new();
    deckcraft_engine::sample::open_sample(&mut s).unwrap();
    let mut out = vec![("builtin".to_string(), (*s.doc().unwrap().doc).clone())];
    let mut files: Vec<PathBuf> = std::fs::read_dir(root().join("samples"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "deckcraft"))
        .collect();
    files.sort();
    for f in files {
        let name = f.file_stem().unwrap().to_string_lossy().to_string();
        let doc = deckcraft_format::load(&std::fs::read(&f).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        out.push((name, doc));
    }
    out
}

/// Whether this build embeds the fonts the stored pictures were drawn with.
fn fonts_embedded() -> bool {
    deckcraft_fonts::CRAFT_FONTS.iter().any(|f| f.family == "Inter")
}

fn render(doc: &Presentation, index: usize) -> Image {
    // Date fields show a fixed day, so the pictures don't change with the clock.
    let now = DateTime { year: 2026, month: 1, day: 2, hour: 9, minute: 30, second: 0 };
    render_slide(doc, index, &RenderOpts { scale: SCALE, now: Some(now), ..Default::default() })
}

/// Where a failing render and its differences are written.
fn failure_dir(deck: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("golden").join(deck)
}

fn slide_file(dir: &Path, index: usize) -> PathBuf {
    dir.join(format!("slide-{:02}.png", index + 1))
}

#[test]
fn sample_decks_render_like_their_golden_pictures() {
    if !fonts_embedded() {
        assert!(option_env!("CRAFT_FONTS_REQUIRED").is_none(), "CRAFT_FONTS_REQUIRED is set but Inter is not embedded");
        eprintln!("golden: skipped, this build has no craft-fonts (set CRAFT_FONTS_DIR)");
        return;
    }
    let bless = std::env::var_os("DECKCRAFT_BLESS_GOLDEN").is_some();
    let mut failures = vec![];
    let mut checked = 0;
    for (deck, doc) in decks() {
        let dir = golden_dir(&deck);
        if bless {
            std::fs::create_dir_all(&dir).unwrap();
            // A deck that lost slides must not keep pictures of them.
            for e in std::fs::read_dir(&dir).unwrap() {
                let p = e.unwrap().path();
                if p.extension().is_some_and(|x| x == "png") {
                    std::fs::remove_file(p).unwrap();
                }
            }
        }
        for i in 0..doc.slides.len() {
            let img = render(&doc, i);
            let path = slide_file(&dir, i);
            if bless {
                std::fs::write(&path, img.to_png()).unwrap();
            }
            let Some(stored) = std::fs::read(&path).ok().and_then(|b| decode_png(&b)) else {
                failures.push(format!("{}: missing or unreadable; run with DECKCRAFT_BLESS_GOLDEN=1", path.display()));
                continue;
            };
            checked += 1;
            let result = compare(&img, &stored, Tolerance::SLIDE.channel);
            let ok = result.as_ref().is_ok_and(|d| d.within(Tolerance::SLIDE));
            if !ok {
                let out = failure_dir(&deck);
                std::fs::create_dir_all(&out).unwrap();
                std::fs::write(slide_file(&out, i), img.to_png()).unwrap();
                if let Some(diff) = diff_image(&img, &stored, Tolerance::SLIDE.channel) {
                    std::fs::write(out.join(format!("slide-{:02}-diff.png", i + 1)), diff.to_png()).unwrap();
                }
                let what = match result {
                    Ok(d) => format!("{:.3}% of pixels differ (largest channel difference {})", d.share() * 100.0, d.max_delta),
                    Err(e) => e.to_string(),
                };
                failures.push(format!("{deck} slide {}: {what}; render and differences in {}", i + 1, out.display()));
            }
        }
        // A stored picture for a slide the deck no longer has is stale.
        let stale = slide_file(&dir, doc.slides.len());
        if stale.exists() {
            failures.push(format!("{}: the deck has {} slides; bless again to drop it", stale.display(), doc.slides.len()));
        }
    }
    assert!(checked > 0, "no golden pictures were compared");
    assert!(failures.is_empty(), "{} slide(s) differ from their golden pictures:\n{}", failures.len(), failures.join("\n"));
}

/// The same slide drawn twice in one process gives the same pixels: the comparison above measures
/// the renderer, not noise.
#[test]
fn a_slide_renders_the_same_twice() {
    let (_, doc) = decks().into_iter().next().unwrap();
    for i in 0..doc.slides.len() {
        let d = compare(&render(&doc, i), &render(&doc, i), 0).unwrap();
        assert_eq!(d.differing, 0, "slide {} drew differently the second time", i + 1);
    }
}
