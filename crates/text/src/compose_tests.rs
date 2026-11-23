//! The paragraph composer ([`crate::compose`]) as seen through [`crate::layout`].

use deckcraft_geom::Rect;
use deckcraft_model::defaults;
use deckcraft_model::resolve::Ctx;
use deckcraft_model::text::{Align, Bullet, Composer, Paragraph, TextBody};
use deckcraft_model::{LayoutType, Presentation, Shape, Xfrm};

use crate::{NoFields, Opts, TextLayout, layout};

const TEXT: &str = "Slides are read from across a room, so every line should be easy to take in at a glance. \
Even spacing and fewer short lines make a paragraph look calm and deliberate.";

fn deck(paras: &[&str], w: f64) -> (Presentation, Shape) {
    let mut p = Presentation::default();
    let lay = defaults::layout_of_kind(&p, LayoutType::TitleAndContent).expect("layout");
    let mut s = defaults::new_slide(&mut p, lay).expect("slide");
    let mut body = s.shapes.remove(1);
    body.xfrm = Some(Xfrm::new(0.0, 0.0, w, 2000.0));
    let mut tb = TextBody::from_text(&paras.join("\n"));
    for para in &mut tb.paragraphs {
        para.props.bullet = Some(Bullet::None);
        para.props.margin_left = Some(0.0);
        para.props.indent = Some(0.0);
        for r in &mut para.runs {
            r.props.size = Some(20.0);
        }
    }
    body.text = Some(tb);
    s.shapes = vec![body.clone()];
    p.slides = vec![std::sync::Arc::new(s)];
    (p, body)
}

fn set(sh: &mut Shape, f: impl Fn(&mut Paragraph)) {
    if let Some(tb) = sh.text.as_mut() {
        tb.paragraphs.iter_mut().for_each(f);
    }
}

fn lay(p: &Presentation, sh: &Shape, w: f64) -> TextLayout {
    let ctx = Ctx::for_slide(p, &p.slides[0]).expect("ctx");
    layout(
        &ctx,
        sh,
        sh.text.as_ref().expect("text"),
        &Opts { rect: Rect::new(0.0, 0.0, w, 2000.0), fields: &NoFields, prompt_color: None, no_shrink: false },
    )
}

/// Usable line width: the box less the default insets.
fn measure(w: f64) -> f64 {
    w - 2.0 * 7.2
}

/// Sum of squared leftover space over every line but each paragraph's last.
fn raggedness(l: &TextLayout, w: f64) -> f64 {
    let m = measure(w);
    let mut sum = 0.0;
    for (i, li) in l.lines.iter().enumerate() {
        let last_of_para = l.lines.get(i + 1).is_none_or(|n| n.para != li.para);
        if !last_of_para {
            let used = li.extent.1 - li.extent.0;
            sum += (m - used).powi(2);
        }
    }
    sum
}

/// Every character of the text is drawn by exactly one glyph cell or is a space/line end; the
/// drawn characters, in order, are the text without spaces.
fn drawn_text(l: &TextLayout) -> String {
    let mut cells: Vec<(usize, usize, char)> = l.runs.iter().flat_map(|r| r.cells.iter().map(move |c| (r.para, c.0, c.1))).collect();
    cells.sort_unstable();
    cells.dedup();
    cells.into_iter().map(|c| c.2).filter(|c| !c.is_whitespace()).collect()
}

fn hyphens_drawn(l: &TextLayout) -> usize {
    l.runs.iter().filter(|r| r.cells.is_empty() && r.glyphs.len() == 1 && r.glyphs[0].0 == r.face.glyph_for('-')).count()
}

#[test]
fn an_unset_composer_lays_out_exactly_like_the_single_line_composer() {
    let (p, sh) = deck(&[TEXT], 300.0);
    let mut single = sh.clone();
    set(&mut single, |para| para.props.composer = Some(Composer::SingleLine));
    let (a, b) = (lay(&p, &sh, 300.0), lay(&p, &single, 300.0));
    let spans = |l: &TextLayout| l.lines.iter().map(|x| (x.start, x.end, x.extent.0.to_bits(), x.extent.1.to_bits())).collect::<Vec<_>>();
    assert_eq!(spans(&a), spans(&b), "PowerPoint decks keep their line breaks unless the composer is chosen");
}

#[test]
fn the_paragraph_composer_evens_out_ragged_lines() {
    let mut better = 0;
    for w in [220.0, 260.0, 300.0, 340.0, 380.0] {
        let (p, mut sh) = deck(&[TEXT], w);
        let greedy = lay(&p, &sh, w);
        set(&mut sh, |para| para.props.composer = Some(Composer::Paragraph));
        let composed = lay(&p, &sh, w);
        assert_eq!(drawn_text(&composed), drawn_text(&greedy), "the composer keeps every character, in order");
        for li in &composed.lines {
            assert!(li.extent.1 <= w - 7.2 + 0.5, "a ragged line stays inside the box at {w}: {:?}", li.extent);
        }
        let (g, c) = (raggedness(&greedy, w), raggedness(&composed, w));
        assert!(c <= g + 1e-6, "total fit is never more ragged than first fit at {w}: {c} > {g}");
        if c < g - 1.0 {
            better += 1;
        }
    }
    assert!(better > 0, "at some width the composer finds a more even break than first fit");
}

#[test]
fn composed_justified_lines_fill_the_measure() {
    let w = 300.0;
    let (p, mut sh) = deck(&[TEXT], w);
    set(&mut sh, |para| {
        para.props.composer = Some(Composer::Paragraph);
        para.props.align = Some(Align::Justify);
    });
    let l = lay(&p, &sh, w);
    assert!(l.lines.len() >= 3);
    for li in &l.lines[..l.lines.len() - 1] {
        let used = li.extent.1 - li.extent.0;
        assert!((used - measure(w)).abs() < 0.5, "justified line fills the measure: {used} vs {}", measure(w));
    }
    let last = l.lines.last().expect("a last line");
    assert!(last.extent.1 - last.extent.0 < measure(w), "the last line is not stretched");
}

#[test]
fn hyphenation_adds_drawn_hyphens_but_no_text() {
    let w = 170.0;
    let words = "Internationalization considerations complicate straightforward communication.";
    let (p, mut sh) = deck(&[words], w);
    set(&mut sh, |para| para.props.composer = Some(Composer::Paragraph));
    let plain = lay(&p, &sh, w);
    assert_eq!(hyphens_drawn(&plain), 0, "no hyphenation unless it is turned on");
    set(&mut sh, |para| para.props.hyphenate = Some(true));
    let hyph = lay(&p, &sh, w);
    assert!(hyphens_drawn(&hyph) > 0, "long words are hyphenated at line ends");
    assert_eq!(drawn_text(&hyph), drawn_text(&plain), "a hyphen is drawn, not inserted into the text");
    for li in &hyph.lines {
        assert!(li.extent.1 <= w - 7.2 + 0.5, "the hyphen fits inside the box: {:?}", li.extent);
    }
    // Hyphenation needs the paragraph composer: the single-line composer ignores it.
    set(&mut sh, |para| para.props.composer = None);
    assert_eq!(hyphens_drawn(&lay(&p, &sh, w)), 0);
}

#[test]
fn optical_margins_hang_opening_quotes_into_the_margin() {
    let w = 400.0;
    let (p, mut sh) = deck(&["\u{201C}Quoted words open this paragraph.\u{201D}"], w);
    set(&mut sh, |para| para.props.composer = Some(Composer::Paragraph));
    let flush = lay(&p, &sh, w);
    set(&mut sh, |para| para.props.optical_margin = Some(true));
    let hung = lay(&p, &sh, w);
    let (a, b) = (flush.lines[0].extent.0, hung.lines[0].extent.0);
    assert!(b < a - 1.0, "the opening quote hangs left of the text edge: {b} vs {a}");
}

#[test]
fn paragraphs_with_tabs_keep_the_single_line_composer() {
    let w = 260.0;
    let (p, mut sh) = deck(&["Name\tValue and a long description that has to wrap onto another line"], w);
    let single = lay(&p, &sh, w);
    set(&mut sh, |para| para.props.composer = Some(Composer::Paragraph));
    let composed = lay(&p, &sh, w);
    let spans = |l: &TextLayout| l.lines.iter().map(|x| (x.start, x.end)).collect::<Vec<_>>();
    assert_eq!(spans(&single), spans(&composed));
}

#[test]
fn hostile_paragraphs_compose_without_losing_text() {
    let long = "x".repeat(400);
    let cases = ["", "     ", "\u{b}\u{b}", "a\u{b}", long.as_str(), "word \u{AD}\u{AD}\u{AD} \u{b} end", "\u{201C}\u{201C}\u{201C}"];
    for case in cases {
        let (p, mut sh) = deck(&[case], 60.0);
        let single = lay(&p, &sh, 60.0);
        set(&mut sh, |para| {
            para.props.composer = Some(Composer::Paragraph);
            para.props.hyphenate = Some(true);
            para.props.optical_margin = Some(true);
            para.props.align = Some(Align::Justify);
        });
        let composed = lay(&p, &sh, 60.0);
        assert_eq!(drawn_text(&composed), drawn_text(&single), "{case:?}");
        assert!(!composed.lines.is_empty(), "{case:?}");
        for li in &composed.lines {
            assert!(li.start <= li.end, "{case:?}");
            assert!(li.extent.0.is_finite() && li.extent.1.is_finite(), "{case:?}");
        }
    }
}

#[test]
fn a_trailing_line_break_opens_an_empty_last_line_in_both_composers() {
    let (p, mut sh) = deck(&["Two lines\u{b}"], 400.0);
    let single = lay(&p, &sh, 400.0);
    set(&mut sh, |para| para.props.composer = Some(Composer::Paragraph));
    let composed = lay(&p, &sh, 400.0);
    let spans = |l: &TextLayout| l.lines.iter().map(|x| (x.start, x.end)).collect::<Vec<_>>();
    assert_eq!(spans(&single), spans(&composed));
}

#[test]
fn snapping_paragraphs_put_their_baselines_on_the_slide_grid() {
    let w = 300.0;
    let (mut p, mut sh) = deck(&[TEXT, TEXT], w);
    sh.xfrm = Some(Xfrm::new(0.0, 37.0, w, 2000.0));
    if let Some(m) = p.masters.first_mut() {
        std::sync::Arc::make_mut(m).baseline_grid = Some(deckcraft_model::BaselineGrid { increment: 30.0, offset: 5.0 });
    }
    let free = lay(&p, &sh, w);
    set(&mut sh, |para| para.props.snap_to_grid = Some(true));
    let snapped = lay(&p, &sh, w);
    assert_eq!(free.lines.len(), snapped.lines.len());
    for (a, b) in free.lines.iter().zip(&snapped.lines) {
        let slide_y = 37.0 + b.baseline;
        let k = (slide_y - 5.0) / 30.0;
        assert!((k - k.round()).abs() < 1e-6, "baseline {slide_y} is on the 30 pt grid from 5 pt");
        assert!(b.baseline >= a.baseline - 1e-9, "lines only move down");
    }
    // Without a grid, snapping changes nothing.
    if let Some(m) = p.masters.first_mut() {
        std::sync::Arc::make_mut(m).baseline_grid = None;
    }
    let none = lay(&p, &sh, w);
    let bl = |l: &TextLayout| l.lines.iter().map(|x| x.baseline.to_bits()).collect::<Vec<_>>();
    assert_eq!(bl(&none), bl(&free));
}

#[test]
fn text_keeps_clear_of_a_shape_it_wraps_around() {
    let w = 500.0;
    let long = format!("{TEXT} {TEXT} {TEXT}");
    for composer in [None, Some(Composer::Paragraph)] {
        let (mut p, mut sh) = deck(&[long.as_str()], w);
        set(&mut sh, |para| para.props.composer = composer);
        let plain = lay(&p, &sh, w);
        // A picture-sized shape on the right of the box, from y = 30 to 130, wrapped with 6 pt.
        let pic = Shape {
            id: deckcraft_model::ShapeId(900),
            xfrm: Some(Xfrm::new(300.0, 30.0, 150.0, 100.0)),
            text_wrap: Some(deckcraft_model::TextWrap { margin: 6.0 }),
            ..Default::default()
        };
        {
            let slide = std::sync::Arc::make_mut(&mut p.slides[0]);
            slide.shapes = vec![sh.clone(), pic];
        }
        let wrapped = lay(&p, &sh, w);
        assert_eq!(drawn_text(&wrapped), drawn_text(&plain), "{composer:?}: nothing is lost");
        let (o_l, o_t, o_b) = (300.0 - 6.0, 30.0 - 6.0, 130.0 + 6.0);
        let mut beside = 0;
        for li in &wrapped.lines {
            if li.top < o_b && li.bottom > o_t {
                beside += 1;
                assert!(li.extent.1 <= o_l + 0.5, "{composer:?}: a line beside the shape stops before it: {:?}", li.extent);
            }
        }
        assert!(beside >= 3, "{composer:?}: several lines run beside the shape");
        let below = wrapped.lines.iter().filter(|li| li.top >= o_b).map(|li| li.extent.1).fold(0.0, f64::max);
        assert!(below > o_l + 20.0, "{composer:?}: below the shape lines use the full width again");
        assert!(wrapped.lines.len() > plain.lines.len());
    }
}

#[test]
fn wrap_follows_where_anchoring_puts_the_lines() {
    let (w, h) = (500.0, 420.0);
    let long = format!("{TEXT} {TEXT} {TEXT}");
    let (mut p, mut sh) = deck(&[long.as_str()], w);
    sh.xfrm = Some(Xfrm::new(0.0, 0.0, w, h));
    if let Some(t) = sh.text.as_mut() {
        t.body.anchor = Some(deckcraft_model::text::Anchor::Middle);
    }
    let pic = Shape {
        id: deckcraft_model::ShapeId(900),
        xfrm: Some(Xfrm::new(300.0, 190.0, 150.0, 60.0)),
        text_wrap: Some(deckcraft_model::TextWrap { margin: 0.0 }),
        ..Default::default()
    };
    std::sync::Arc::make_mut(&mut p.slides[0]).shapes = vec![sh.clone(), pic];
    let ctx = Ctx::for_slide(&p, &p.slides[0]).expect("ctx");
    let l = layout(
        &ctx,
        &sh,
        sh.text.as_ref().expect("text"),
        &Opts { rect: Rect::new(0.0, 0.0, w, h), fields: &NoFields, prompt_color: None, no_shrink: true },
    );
    let beside: Vec<_> = l.lines.iter().filter(|li| li.top < 250.0 && li.bottom > 190.0).collect();
    assert!(!beside.is_empty());
    assert!(beside.iter().all(|li| li.extent.1 <= 300.5), "lines beside the shape stop before it");
    let above = l.lines.iter().filter(|li| li.bottom <= 190.0).map(|li| li.extent.1).fold(0.0, f64::max);
    assert!(above > 320.0, "lines above the shape use the full width: {above}");
}
