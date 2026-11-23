//! `.pptx` round trips of whole decks, judged by what the slides look like.
//!
//! - Every sample deck (the built-in sample and `samples/*.deckcraft`) is written as `.pptx`,
//!   opened again, and each slide must still match its golden picture (`golden.rs`). A second
//!   save of the reopened deck must give the same deck again.
//! - `DECKCRAFT_PPTX_CORPUS=<folder>` runs the same check on every `.pptx` in that folder: decks
//!   from outside this repository, which `AGENTS.md` §1.1 keeps out of it. Each deck is opened and
//!   its animation effects are checked against the ones counted in the package XML (what the
//!   first open lost). It is then saved, opened again, and compared: slide size, sections, layouts,
//!   embedded fonts, and slide by slide count, text, shapes, notes, transition, what the slide show
//!   plays (effect by effect) and pixels. Every loss is printed; the test fails when any deck lost
//!   something. Without the variable it does nothing.

use std::path::PathBuf;

use deckcraft_anim::{Sequence, Timeline};
use deckcraft_engine::render::golden::{Tolerance, compare, decode_png};
use deckcraft_engine::render::{Image, RenderOpts, render_slide};
use deckcraft_model::anim::build_filled;
use deckcraft_model::{Presentation, Shape, ShapeId, Slide};
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
    out.extend(deck_losses(before, after));
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
        // What the slide show plays, effect by effect, rather than how many animations the
        // Animation Pane lists: three paragraph effects and one by-paragraph build of the same
        // three paragraphs play the same.
        let (pa, pb) = (playback(before, a), playback(after, b));
        if pa != pb {
            out.push(format!("slide {n}: playback changed: {}", first_difference(&pa, &pb)));
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

/// One effect as the slide show plays it: (target shape, paragraph), sequence (trigger shape),
/// click step, start and span in milliseconds, repeats, and the effect's class, name and option.
/// Shapes are named by their place in the slide's shape tree, so that ids renumbered on save do
/// not count as a change.
#[derive(Debug, PartialEq)]
struct Played {
    target: (Option<usize>, Option<usize>),
    trigger: Option<Option<usize>>,
    step: usize,
    start_ms: i64,
    span_ms: i64,
    repeats: i64,
    what: String,
}

fn tree_index(shapes: &[Shape], id: ShapeId) -> Option<usize> {
    let mut order = vec![];
    deckcraft_model::walk(shapes, &mut |s, _| order.push(s.id));
    order.iter().position(|&s| s == id)
}

fn playback(doc: &Presentation, slide: &Slide) -> Vec<Played> {
    let tl = Timeline::new(slide, doc.slide_size.width, doc.slide_size.height);
    tl.effects()
        .into_iter()
        .map(|e| {
            let a = slide.animations.get(e.anim);
            Played {
                target: (tree_index(&slide.shapes, e.shape), e.paragraph),
                trigger: match e.sequence {
                    Sequence::Main => None,
                    Sequence::Trigger(t) => Some(tree_index(&slide.shapes, t)),
                },
                step: e.step,
                start_ms: (e.start * 1000.0).round() as i64,
                span_ms: (e.span * 1000.0).round() as i64,
                repeats: (e.iterations * 1000.0).round() as i64,
                what: a.map(|a| format!("{:?} {} {}", a.class, a.effect, a.option)).unwrap_or_default(),
            }
        })
        .collect()
}

fn first_difference(a: &[Played], b: &[Played]) -> String {
    match a.iter().zip(b).position(|(x, y)| x != y) {
        Some(i) => format!("effect {} of {}: {:?} became {:?}", i + 1, a.len(), a[i], b[i]),
        None => format!("{} effects became {}", a.len(), b.len()),
    }
}

/// What `after` lost against `before` that belongs to the whole deck.
fn deck_losses(before: &Presentation, after: &Presentation) -> Vec<String> {
    let mut out = vec![];
    if before.slide_size != after.slide_size {
        out.push(format!("slide size {:?} became {:?}", before.slide_size, after.slide_size));
    }
    let sections = |p: &Presentation| p.sections.iter().map(|s| (s.name.clone(), s.slides.len())).collect::<Vec<_>>();
    if sections(before) != sections(after) {
        out.push(format!("sections {:?} became {:?}", sections(before), sections(after)));
    }
    let layouts = |p: &Presentation| p.masters.iter().map(|m| m.layouts.len()).collect::<Vec<_>>();
    if layouts(before) != layouts(after) {
        out.push(format!("layouts per master {:?} became {:?}", layouts(before), layouts(after)));
    }
    let fonts = |p: &Presentation| p.embedded_fonts.iter().map(|f| f.typeface.clone()).collect::<Vec<_>>();
    if fonts(before) != fonts(after) {
        out.push(format!("embedded fonts {:?} became {:?}", fonts(before), fonts(after)));
    }
    out
}

/// Animation effects in a `.pptx` package, slide by slide in show order, counted from the XML:
/// every time node with a `presetClass`, as (shape id in the file, paragraph).
fn package_effects(bytes: &[u8]) -> Vec<Vec<(u32, Option<u32>)>> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut read = |name: &str| -> Option<Vec<u8>> {
        let mut f = z.by_name(name).ok()?;
        let mut v = vec![];
        std::io::Read::read_to_end(&mut f, &mut v).ok()?;
        Some(v)
    };
    let pres = deckcraft_pptx::parse_xml(&read("ppt/presentation.xml").unwrap()).unwrap();
    let rels = deckcraft_pptx::parse_xml(&read("ppt/_rels/presentation.xml.rels").unwrap()).unwrap();
    let target = |rid: &str| {
        rels.root.elements().find(|r| r.attr("Id") == Some(rid)).and_then(|r| r.attr("Target")).map(|t| {
            let t = t.trim_start_matches('/');
            if t.starts_with("ppt/") { t.to_string() } else { format!("ppt/{t}") }
        })
    };
    let ids: Vec<String> =
        pres.root.child("sldIdLst").map(|l| l.children_named("sldId").filter_map(|s| s.attr("r:id").and_then(&target)).collect()).unwrap_or_default();
    ids.iter()
        .map(|part| {
            let Some(xml) = read(part).and_then(|b| deckcraft_pptx::parse_xml(&b).ok()) else { return vec![] };
            let Some(tn) = xml.root.child("timing").and_then(|t| t.child("tnLst")) else { return vec![] };
            let mut ctns = vec![];
            tn.find_all("cTn", &mut ctns);
            ctns.into_iter()
                .filter(|c| c.has_attr("presetClass"))
                .filter_map(|c| {
                    let t = c.find("spTgt")?;
                    Some((t.u32("spid")?, t.path(&["txEl", "pRg"]).and_then(|p| p.u32("st"))))
                })
                .collect()
        })
        .collect()
}

/// What the first open lost against the package: animation effects per slide. A by-paragraph
/// build counts once per paragraph it plays, sub-paragraphs that play with their parent included,
/// as the file lists them.
fn first_open_losses(bytes: &[u8], doc: &Presentation) -> Vec<String> {
    let mut out = vec![];
    let in_file = package_effects(bytes);
    if in_file.len() != doc.slides.len() {
        out.push(format!("the package lists {} slides, {} opened", in_file.len(), doc.slides.len()));
        return out;
    }
    for (i, (file, slide)) in in_file.iter().zip(&doc.slides).enumerate() {
        let tl = Timeline::new(slide, doc.slide_size.width, doc.slide_size.height);
        let effects = tl.effects();
        let mut shapes: Vec<ShapeId> = effects.iter().map(|e| e.shape).collect();
        shapes.sort_by_key(|s| s.0);
        shapes.dedup();
        // Sub-paragraphs that play with their parent paragraph are not effects of their own; the
        // file has one for each that holds text (`build_filled`), as the writer writes them.
        let riders: usize = shapes
            .iter()
            .map(|&s| {
                let own: Vec<usize> = effects.iter().filter(|e| e.shape == s).filter_map(|e| e.paragraph).collect();
                let mut paragraphs = vec![];
                deckcraft_model::walk(&slide.shapes, &mut |sh, _| {
                    if sh.id == s {
                        paragraphs = sh.text.as_ref().map(|t| t.paragraphs.clone()).unwrap_or_default();
                    }
                });
                tl.paragraph_targets(s).into_iter().filter(|p| !own.contains(p) && paragraphs.get(*p).is_some_and(build_filled)).count()
            })
            .sum();
        let played = effects.len() + riders;
        if file.len() != played {
            let mut missing: Vec<String> = file
                .iter()
                .filter(|(spid, p)| match p {
                    Some(p) => !tl.paragraph_targets(ShapeId(*spid)).contains(&(*p as usize)),
                    None => !effects.iter().any(|e| e.shape == ShapeId(*spid) && e.paragraph.is_none()),
                })
                .map(|(spid, p)| match p {
                    Some(p) => format!("shape {spid} paragraph {p}"),
                    None => format!("shape {spid}"),
                })
                .collect();
            missing.dedup();
            out.push(format!(
                "slide {}: {} animation effects in the file, {} played (not played: {})",
                i + 1,
                file.len(),
                played,
                if missing.is_empty() { "-".to_string() } else { missing.join(", ") }
            ));
        }
    }
    out
}

/// Three paragraphs of one shape, each faded in After Previous for 200 ms as separate effects
/// (corpus deck 01, slide 89). The saved file marks them as one paragraph build, which reopens
/// as one by-paragraph build; that build must play at the same times (0.2 s apart), not 0.5 s
/// apart.
#[test]
fn separate_after_previous_paragraph_effects_play_the_same_after_a_pptx_round_trip() {
    use deckcraft_model::anim::{AnimClass, AnimStart, Animation};
    use deckcraft_model::text::TextBody;
    let mut doc = deckcraft_model::defaults::new_presentation(None);
    let id = ShapeId(doc.alloc_id().unwrap());
    let shape = Shape {
        id,
        name: "List".into(),
        xfrm: Some(deckcraft_geom::Xfrm::new(50.0, 50.0, 400.0, 200.0)),
        text: Some(TextBody::from_text("one\ntwo\nthree")),
        ..Default::default()
    };
    let slide = std::sync::Arc::make_mut(&mut doc.slides[0]);
    slide.shapes.push(shape);
    slide.animations = (0..3)
        .map(|p| Animation {
            shape: id,
            class: AnimClass::Entrance,
            effect: "fade".into(),
            start: AnimStart::AfterPrevious,
            duration_ms: 200,
            paragraph: Some(p),
            ..Default::default()
        })
        .collect();
    let reopened = through_pptx(&doc);
    let (before, after) = (playback(&doc, &doc.slides[0]), playback(&reopened, &reopened.slides[0]));
    assert_eq!(before.iter().map(|p| p.start_ms).collect::<Vec<_>>(), vec![0, 200, 400]);
    assert!(before == after, "{}", first_difference(&before, &after));
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
        // Named before the work, so a deck that hangs or crashes is known by name.
        println!("{name}: opening");
        let started = std::time::Instant::now();
        let bytes = std::fs::read(f).unwrap();
        let (doc, damage) = match deckcraft_pptx::import_checked(&bytes) {
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
        let opened = first_open_losses(&bytes, &doc);
        let lost: Vec<String> = opened
            .into_iter()
            .map(|l| format!("on first open: {l}"))
            .chain(losses(&doc, &reopened, true).into_iter().map(|l| format!("on save and reopen: {l}")))
            .collect();
        println!(
            "{name}: {} slides, {} parts damaged on open, {} losses ({:.1} s)",
            doc.slides.len(),
            damage.len(),
            lost.len(),
            started.elapsed().as_secs_f64()
        );
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
