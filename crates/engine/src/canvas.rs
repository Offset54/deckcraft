//! SlideCraft canvas passes (Phase 3). After every edit, the slides, masters and layouts the edit
//! changed are settled:
//! 1. group child space is kept equal to slide space (`deckcraft_model::canvas::normalize_groups`),
//!    so shapes inside groups can be selected and edited in place, and frames' members follow
//!    their constraints;
//! 2. auto layout frames place their members (`deckcraft_model::autolayout::solve`), innermost
//!    frames first so a hugging frame has its size before its parent lays it out;
//! 3. a vector network whose outline was edited some other way becomes a plain path, and
//!    editable boolean shapes recompute their result from their operands;
//! 4. a Display P3 colour whose fill or outline was changed some other way is forgotten.
//!
//! Untouched slides are skipped by pointer comparison, so the cost follows the edit, not the deck
//! ("relayout only the dirty nodes", at slide granularity).

use std::sync::Arc;

use deckcraft_model::autolayout::{Item, solve};
use deckcraft_model::canvas::{MAX_DEPTH, group_is_axis_aligned, normalize_groups};
use deckcraft_model::{Master, Presentation, Shape, ShapeKind, Xfrm};

/// Settle one shape tree.
pub(crate) fn settle_list(list: &mut [Shape]) {
    normalize_groups(list);
    layout_tree(list, 0);
    crate::cmd::vector::settle(list, 0);
    crate::cmd::boolean::settle(list, 0);
    crate::cmd::color_p3::settle(list, 0);
}

/// Lay out every auto layout frame in `list`, innermost first.
fn layout_tree(list: &mut [Shape], depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    for sh in list.iter_mut() {
        if let ShapeKind::Group { children, .. } = &mut sh.kind {
            layout_tree(children, depth + 1);
        }
        if sh.auto_layout.is_some() {
            apply_layout(sh, depth);
        }
    }
}

/// Place the members of auto layout frame `sh` and size it (hug). Its top-left corner stays put.
fn apply_layout(sh: &mut Shape, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    let (Some(al), Some(g)) = (sh.auto_layout.clone(), sh.xfrm) else { return };
    if !group_is_axis_aligned(&g) {
        return;
    }
    let ShapeKind::Group { children, child } = &mut sh.kind else { return };
    let items: Vec<Item> = children
        .iter()
        .map(|c| match c.xfrm {
            Some(x) => Item { w: x.w, h: x.h, layout: c.layout_child.unwrap_or_default() },
            // A member without its own box (a placeholder) keeps its place.
            None => Item { layout: deckcraft_model::canvas::ChildLayout { absolute: true, ..Default::default() }, ..Default::default() },
        })
        .collect();
    let sol = solve(&al, g.w, g.h, &items);
    for (c, p) in children.iter_mut().zip(sol.items) {
        let (Some(p), Some(old)) = (p, c.xfrm) else { continue };
        let b = Xfrm { x: g.x + p.x, y: g.y + p.y, w: p.w, h: p.h, ..old };
        if [b.x, b.y, b.w, b.h].iter().all(|v| v.is_finite()) && b != old {
            set_box(c, b, depth + 1);
        }
    }
    let frame = Xfrm { w: sol.w, h: sol.h, ..g };
    if [frame.w, frame.h].iter().all(|v| v.is_finite()) {
        sh.xfrm = Some(frame);
        // Members were placed in slide space: the child space is the box.
        *child = Xfrm { x: frame.x, y: frame.y, w: frame.w, h: frame.h, ..*child };
    }
}

/// Move or resize a member to `b`. A group's members follow (scaled, or by their constraints in a
/// frame), and a resized auto layout frame lays itself out again.
fn set_box(c: &mut Shape, b: Xfrm, depth: usize) {
    let resized = c.xfrm.is_none_or(|o| (o.w - b.w).abs() > 1e-9 || (o.h - b.h).abs() > 1e-9);
    c.xfrm = Some(b);
    if matches!(c.kind, ShapeKind::Group { .. }) {
        normalize_groups(std::slice::from_mut(c));
        if resized && c.auto_layout.is_some() {
            apply_layout(c, depth);
        }
    }
}

/// Settle every slide, master and layout of `new` that differs from `old`.
pub(crate) fn settle_changed(old: &Presentation, new: &mut Presentation) {
    let old_slides: std::collections::HashMap<_, _> = old.slides.iter().map(|s| (s.id, s)).collect();
    for slide in new.slides.iter_mut() {
        if old_slides.get(&slide.id).is_some_and(|b| Arc::ptr_eq(b, slide)) {
            continue;
        }
        settle_list(&mut Arc::make_mut(slide).shapes);
    }
    for master in new.masters.iter_mut() {
        if old.masters.iter().any(|m| Arc::ptr_eq(m, master)) {
            continue;
        }
        settle_master(Arc::make_mut(master));
    }
    for (before, after) in [(old.notes_master.as_ref(), new.notes_master.as_mut()), (old.handout_master.as_ref(), new.handout_master.as_mut())] {
        if let Some(after) = after
            && !before.is_some_and(|b| Arc::ptr_eq(b, after))
        {
            settle_master(Arc::make_mut(after));
        }
    }
}

fn settle_master(m: &mut Master) {
    settle_list(&mut m.shapes);
    for l in m.layouts.iter_mut() {
        settle_list(&mut l.shapes);
    }
}

/// Settle a whole presentation (a file just opened).
pub(crate) fn settle_all(p: &mut Presentation) {
    for slide in p.slides.iter_mut() {
        if needs_settling(&slide.shapes) {
            settle_list(&mut Arc::make_mut(slide).shapes);
        }
    }
    for master in p.masters.iter_mut().chain(p.notes_master.iter_mut()).chain(p.handout_master.iter_mut()) {
        if needs_settling(&master.shapes) || master.layouts.iter().any(|l| needs_settling(&l.shapes)) {
            settle_master(Arc::make_mut(master));
        }
    }
}

/// Whether a tree has anything to settle (a group, so a frame): most slides have none, and are
/// not copied.
fn needs_settling(list: &[Shape]) -> bool {
    list.iter().any(|s| matches!(s.kind, deckcraft_model::ShapeKind::Group { .. }) || s.vector.is_some() || s.p3.is_some())
}
