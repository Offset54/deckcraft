//! The brief's design check for Phase 4 (§5): every layout of the library in every built-in
//! theme, in its Light and Dark modes. Each slide is checked for
//! - **overflow**: every text box is tall enough for its text at its width;
//! - **clipping**: every visible object lies on the slide, and every member of a frame inside it;
//! - **contrast**: every run of text has WCAG AA contrast against what is drawn behind it
//!   (4.5:1, or 3:1 for large text: 18 pt, or 14 pt bold). Behind it is the topmost filled object
//!   below it in drawing order that contains the text's centre (both ends of a gradient are
//!   checked), else the slide background.
//!
//! `SLIDECRAFT_CONTACT_SHEET=<file.png>` also writes the contact sheet: one row per theme and
//! mode (Light, then Dark, themes in gallery order), one column per layout in library order.

use deckcraft_engine::Session;
use deckcraft_engine::model::resolve::{self, Ctx};
use deckcraft_engine::model::{Fill, Presentation, Rgba, Shape, ShapeKind, Slide};
use deckcraft_geom::Xfrm;
use serde_json::json;

/// The deck of every library layout in `theme`, `mode`; the first (title) slide removed.
fn deck(theme: &str, mode: &str) -> Presentation {
    let mut s = Session::with_new();
    s.execute("design.kit", &json!({"theme": theme, "mode": mode})).unwrap();
    for l in deckcraft_engine::library::layouts() {
        s.execute("slide.fromLibrary", &json!({"layout": l.id})).unwrap();
    }
    s.execute("slide.go", &json!({"index": 0})).unwrap();
    s.execute("slide.delete", &json!({"index": 0})).unwrap();
    (*s.doc().unwrap().doc).clone()
}

/// Shapes in drawing order with their visibility (a hidden frame hides its members) and their
/// frame's box.
fn flatten<'a>(list: &'a [Shape], hidden: bool, parent: Option<Xfrm>, out: &mut Vec<(&'a Shape, bool, Option<Xfrm>)>) {
    for sh in list {
        let h = hidden || sh.hidden;
        out.push((sh, h, parent));
        if let ShapeKind::Group { children, .. } = &sh.kind {
            let p = if sh.frame.is_some() { sh.xfrm } else { parent };
            flatten(children, h, p, out);
        }
    }
}

fn fill_colors(ctx: &Ctx, f: &Fill, ph: Option<Rgba>) -> Vec<Rgba> {
    match f {
        Fill::Solid { color } => vec![ctx.color(color, ph)],
        Fill::Gradient(g) => g.stops.iter().map(|s| ctx.color(&s.color, ph)).collect(),
        _ => vec![],
    }
}

fn contains(x: &Xfrm, px: f64, py: f64) -> bool {
    px >= x.x && px <= x.x + x.w && py >= x.y && py <= x.y + x.h
}

fn check(doc: &Presentation, slide: &Slide, label: &str, problems: &mut Vec<String>) {
    let ctx = Ctx::for_slide(doc, slide).unwrap();
    let (w, h) = (doc.slide_size.width, doc.slide_size.height);
    let mut flat = vec![];
    flatten(&slide.shapes, false, None, &mut flat);
    let (bg, bg_ph) = resolve::background(&ctx, Some(slide));
    let bg = fill_colors(&ctx, &bg, bg_ph);
    for (i, (sh, hidden, parent)) in flat.iter().enumerate() {
        if *hidden {
            continue;
        }
        let Some(x) = sh.xfrm else { continue };
        let tol = 0.5;
        if x.x < -tol || x.y < -tol || x.x + x.w > w + tol || x.y + x.h > h + tol {
            problems.push(format!("{label}: `{}` leaves the slide ({:.0},{:.0} {:.0}×{:.0})", sh.name, x.x, x.y, x.w, x.h));
        }
        if let Some(p) = parent
            && (x.x < p.x - tol || x.y < p.y - tol || x.x + x.w > p.x + p.w + tol || x.y + x.h > p.y + p.h + tol)
        {
            problems.push(format!("{label}: `{}` sticks out of its frame", sh.name));
        }
        let Some(body) = sh.text.as_ref().filter(|t| !t.text().trim().is_empty()) else { continue };
        if matches!(sh.kind, ShapeKind::Group { .. }) {
            continue;
        }
        // Overflow.
        let geo = deckcraft_render::shape_geometry(sh, x.w, x.h);
        let need = deckcraft_text::fit_height(&ctx, sh, body, geo.text_rect, &deckcraft_text::NoFields);
        let extra = (x.h - geo.text_rect.height()).max(0.0);
        if need + extra > x.h + 1.0 {
            problems.push(format!("{label}: `{}` overflows: needs {:.1} pt, has {:.1}", sh.name, need + extra, x.h));
        }
        // Contrast against what is behind it (the shape's own fill first).
        let (cx, cy) = (x.x + x.w / 2.0, x.y + x.h / 2.0);
        let own = resolve::fill(&ctx, sh);
        let behind: Vec<Rgba> = match own.0.as_ref().map(|f| fill_colors(&ctx, f, own.1)).filter(|c| !c.is_empty()) {
            Some(c) => c,
            None => flat[..i]
                .iter()
                .rev()
                .filter(|(o, hid, _)| !*hid && o.xfrm.is_some_and(|ox| contains(&ox, cx, cy)))
                .find_map(|(o, _, _)| {
                    let (f, ph) = resolve::fill(&ctx, o);
                    f.map(|f| fill_colors(&ctx, &f, ph)).filter(|c| !c.is_empty())
                })
                .unwrap_or_else(|| bg.clone()),
        };
        for para in &body.paragraphs {
            for run in para.runs.iter().filter(|r| !r.text.trim().is_empty()) {
                let rp = resolve::run(&ctx, sh, para, &run.props);
                let fg = resolve::text_color(&ctx, &rp);
                let size = rp.size.unwrap_or(18.0);
                let large = size >= 18.0 || (rp.bold == Some(true) && size >= 14.0);
                let need = if large { 3.0 } else { 4.5 };
                for b in &behind {
                    let c = fg.contrast(*b);
                    if c < need {
                        problems.push(format!(
                            "{label}: `{}` \"{}\" contrast {c:.2} < {need} (#{} on #{}, {size} pt)",
                            sh.name,
                            run.text.chars().take(24).collect::<String>(),
                            fg.hex(),
                            b.hex()
                        ));
                    }
                }
            }
        }
    }
}

#[test]
fn the_layout_library_reads_in_every_theme_light_and_dark() {
    let themes: Vec<String> = deckcraft_engine::model::theme::builtin_themes().into_iter().map(|t| t.name).collect();
    let layouts = deckcraft_engine::library::layouts();
    let sheet = std::env::var("SLIDECRAFT_CONTACT_SHEET").ok();
    let scale = 0.2;
    let (cw, ch) = ((960.0 * scale) as u32, (540.0 * scale) as u32);
    let gap = 4u32;
    let cols = layouts.len() as u32;
    let rows = themes.len() as u32 * 2;
    let (sw, shh) = (cols * (cw + gap) + gap, rows * (ch + gap) + gap);
    let mut pixels = if sheet.is_some() { vec![255u8; (sw * shh * 4) as usize] } else { vec![] };
    let mut problems = vec![];
    for (ti, theme) in themes.iter().enumerate() {
        for (mi, mode) in ["Light", "Dark"].iter().enumerate() {
            let doc = deck(theme, mode);
            assert_eq!(doc.slides.len(), layouts.len());
            for (li, (slide, l)) in doc.slides.iter().zip(layouts).enumerate() {
                check(&doc, slide, &format!("{theme} {mode} {}", l.id), &mut problems);
                if sheet.is_some() {
                    let img = deckcraft_engine::render::render_slide(&doc, li, &deckcraft_engine::render::RenderOpts { scale, ..Default::default() });
                    let (ox, oy) = (gap + li as u32 * (cw + gap), gap + (ti as u32 * 2 + mi as u32) * (ch + gap));
                    for y in 0..img.height.min(ch) {
                        for x in 0..img.width.min(cw) {
                            let s = ((y * img.width + x) * 4) as usize;
                            let d = (((oy + y) * sw + ox + x) * 4) as usize;
                            if let (Some(src), Some(dst)) = (img.pixels.get(s..s + 4), pixels.get_mut(d..d + 4)) {
                                dst.copy_from_slice(src);
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(path) = sheet {
        let img = deckcraft_engine::render::Image { width: sw, height: shh, pixels };
        std::fs::write(&path, img.to_png()).unwrap();
    }
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}
