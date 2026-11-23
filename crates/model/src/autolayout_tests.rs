#![allow(clippy::unwrap_used)]

use super::*;
use crate::canvas::ChildLayout;

fn item(w: f64, h: f64) -> Item {
    Item { w, h, layout: ChildLayout::default() }
}

fn fill_w(w: f64, h: f64) -> Item {
    Item { w, h, layout: ChildLayout { width: Sizing::Fill, ..Default::default() } }
}

fn al(flow: Flow) -> AutoLayout {
    AutoLayout { flow, gap: 10.0, cross_gap: 10.0, padding: [5.0; 4], ..Default::default() }
}

fn boxes(s: &Solution) -> Vec<(f64, f64, f64, f64)> {
    s.items.iter().map(|p| p.map(|p| (p.x, p.y, p.w, p.h)).unwrap_or((-1.0, -1.0, -1.0, -1.0))).collect()
}

#[test]
fn a_row_places_members_gap_apart_inside_the_padding() {
    let s = solve(&al(Flow::Horizontal), 300.0, 100.0, &[item(50.0, 20.0), item(60.0, 30.0)]);
    assert_eq!(boxes(&s), vec![(5.0, 5.0, 50.0, 20.0), (65.0, 5.0, 60.0, 30.0)]);
    assert_eq!((s.w, s.h), (300.0, 100.0));
}

#[test]
fn a_hugging_frame_fits_its_members() {
    let mut a = al(Flow::Vertical);
    a.width = Sizing::Hug;
    a.height = Sizing::Hug;
    let s = solve(&a, 999.0, 999.0, &[item(50.0, 20.0), item(80.0, 30.0)]);
    assert_eq!((s.w, s.h), (90.0, 70.0));
    assert_eq!(boxes(&s), vec![(5.0, 5.0, 50.0, 20.0), (5.0, 35.0, 80.0, 30.0)]);
}

#[test]
fn fill_members_share_the_free_space() {
    let s = solve(&al(Flow::Horizontal), 300.0, 50.0, &[item(50.0, 20.0), fill_w(1.0, 20.0), fill_w(1.0, 20.0)]);
    // Inside 290; 50 fixed and 2 gaps of 10 leave 220 for two fills.
    assert_eq!(boxes(&s), vec![(5.0, 5.0, 50.0, 20.0), (65.0, 5.0, 110.0, 20.0), (185.0, 5.0, 110.0, 20.0)]);
    // In a hugging row there's nothing to share: fill keeps its size.
    let mut a = al(Flow::Horizontal);
    a.width = Sizing::Hug;
    let s = solve(&a, 300.0, 50.0, &[fill_w(40.0, 20.0)]);
    assert_eq!(boxes(&s), vec![(5.0, 5.0, 40.0, 20.0)]);
    assert_eq!(s.w, 50.0);
    // Across a hugging column, fill takes the widest other member's width.
    let mut a = al(Flow::Vertical);
    a.width = Sizing::Hug;
    let s = solve(&a, 0.0, 100.0, &[item(80.0, 10.0), fill_w(10.0, 10.0)]);
    assert_eq!(s.items[1].unwrap().w, 80.0);
    assert_eq!(s.w, 90.0);
}

#[test]
fn alignment_along_and_across_the_flow() {
    let mut a = al(Flow::Horizontal);
    a.align = Align::Center;
    a.cross_align = Align::End;
    let s = solve(&a, 210.0, 100.0, &[item(50.0, 20.0), item(50.0, 40.0)]);
    // Free 200 - 110 = 90, half before.
    assert_eq!(boxes(&s), vec![(50.0, 75.0, 50.0, 20.0), (110.0, 55.0, 50.0, 40.0)]);
    a.align = Align::SpaceBetween;
    a.cross_align = Align::Stretch;
    let s = solve(&a, 210.0, 100.0, &[item(50.0, 20.0), item(50.0, 40.0), item(50.0, 10.0)]);
    assert_eq!(boxes(&s), vec![(5.0, 5.0, 50.0, 90.0), (80.0, 5.0, 50.0, 90.0), (155.0, 5.0, 50.0, 90.0)]);
}

#[test]
fn wrapping_starts_new_lines() {
    let mut a = al(Flow::Horizontal);
    a.wrap = true;
    a.height = Sizing::Hug;
    let s = solve(&a, 130.0, 0.0, &[item(50.0, 20.0), item(50.0, 30.0), item(50.0, 20.0)]);
    // 120 inside holds two (110); the third wraps under the taller line.
    assert_eq!(boxes(&s), vec![(5.0, 5.0, 50.0, 20.0), (65.0, 5.0, 50.0, 30.0), (5.0, 45.0, 50.0, 20.0)]);
    assert_eq!(s.h, 5.0 + 30.0 + 10.0 + 20.0 + 5.0);
    // A member wider than the frame still gets a line of its own.
    let s = solve(&a, 130.0, 0.0, &[item(500.0, 20.0), item(50.0, 20.0)]);
    assert_eq!(s.items[1].unwrap().y, 35.0);
}

#[test]
fn absolute_members_are_left_alone() {
    let mut abs = item(10.0, 10.0);
    abs.layout.absolute = true;
    let s = solve(&al(Flow::Horizontal), 300.0, 100.0, &[abs, item(50.0, 20.0)]);
    assert_eq!(s.items[0], None);
    assert_eq!(s.items[1].unwrap().x, 5.0);
}

#[test]
fn grid_tracks_fixed_hug_and_fr() {
    let mut a = al(Flow::Grid);
    a.columns = vec![Track::Fixed(100.0), Track::Fr(1.0), Track::Fr(2.0)];
    // Inside 400 - 10 = 390, minus 2 gaps (20) and 100 fixed = 270 for 3 fr: 90 and 180.
    let s = solve(&a, 400.0, 300.0, &[item(20.0, 20.0), item(20.0, 30.0), item(20.0, 20.0), item(20.0, 40.0)]);
    let b = boxes(&s);
    assert_eq!(b[0], (5.0, 5.0, 20.0, 20.0));
    assert_eq!(b[1], (115.0, 5.0, 20.0, 30.0));
    assert_eq!(b[2], (215.0, 5.0, 20.0, 20.0));
    // Unlisted rows hug: the first row is 30 tall, the second starts 10 below it.
    assert_eq!(b[3], (5.0, 45.0, 20.0, 40.0));
    // Stretch takes the whole cell.
    a.align = Align::Stretch;
    let s = solve(&a, 400.0, 300.0, &[item(20.0, 20.0), item(20.0, 20.0), item(20.0, 20.0)]);
    assert_eq!(boxes(&s).iter().map(|b| b.2).collect::<Vec<_>>(), vec![100.0, 90.0, 180.0]);
}

#[test]
fn grid_cells_can_be_named_and_spanned_and_a_grid_can_hug() {
    let mut a = al(Flow::Grid);
    a.columns = vec![Track::Hug, Track::Hug];
    a.width = Sizing::Hug;
    a.height = Sizing::Hug;
    let mut wide = item(70.0, 10.0);
    wide.layout.column_span = 2;
    let mut corner = item(30.0, 30.0);
    corner.layout.column = Some(1);
    corner.layout.row = Some(1);
    let s = solve(&a, 0.0, 0.0, &[wide, item(40.0, 20.0), corner]);
    let b = boxes(&s);
    // Row 0: the wide one; row 1: the auto-placed one in column 0, the named one in column 1.
    assert_eq!(b[0].1, 5.0);
    assert_eq!((b[1].0, b[1].1), (5.0, 25.0));
    assert_eq!((b[2].0, b[2].1), (55.0, 25.0));
    // Hugging: columns 40 and 30 plus a gap; rows 10 and 30 plus a gap; plus padding.
    assert_eq!((s.w, s.h), (5.0 + 40.0 + 10.0 + 30.0 + 5.0, 5.0 + 10.0 + 10.0 + 30.0 + 5.0));
}

#[test]
fn hostile_input_stays_finite_and_bounded() {
    let mut a = al(Flow::Grid);
    a.columns = vec![Track::Fr(f64::NAN), Track::Fixed(-5.0), Track::Fr(f64::INFINITY)];
    a.gap = f64::NAN;
    a.padding = [f64::INFINITY, -1.0, 1e300, f64::NAN];
    let mut far = item(f64::NAN, -3.0);
    far.layout.row = Some(u32::MAX);
    far.layout.column = Some(u32::MAX);
    far.layout.row_span = u32::MAX;
    let s = solve(&a, f64::INFINITY, -10.0, &[far, item(1e300, 1e300)]);
    for p in s.items.iter().flatten() {
        assert!([p.x, p.y, p.w, p.h].iter().all(|v| v.is_finite()), "{p:?}");
    }
    assert!(s.w.is_finite() && s.h.is_finite());
    for flow in [Flow::Horizontal, Flow::Vertical] {
        let mut a = al(flow);
        a.wrap = true;
        a.gap = -1.0;
        let s = solve(&a, f64::NAN, 1.0, &[item(f64::INFINITY, 1.0), fill_w(1.0, f64::NAN)]);
        assert!(s.items.iter().flatten().all(|p| [p.x, p.y, p.w, p.h].iter().all(|v| v.is_finite())));
    }
    // Many members on a one-column grid stop at the track limit.
    let mut a = al(Flow::Grid);
    a.columns = vec![Track::Hug];
    let many = vec![item(1.0, 1.0); MAX_TRACKS + 10];
    let s = solve(&a, 10.0, 10.0, &many);
    assert_eq!(s.items.iter().filter(|p| p.is_some()).count(), MAX_TRACKS);
    assert!(solve(&al(Flow::Horizontal), 10.0, 10.0, &[]).items.is_empty());
}
