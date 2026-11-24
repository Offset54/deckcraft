//! SlideCraft canvas helpers (Phase 3): group child space kept equal to slide space, so a shape
//! inside a group is edited, drawn and measured in the same coordinates as a top-level shape.
//!
//! PresentationML gives each group a child coordinate space (`chOff`/`chExt`) that maps onto its
//! box. PowerPoint writes it equal to the box when it makes a group; it drifts apart when a group
//! is moved or resized. While they differ, a member's stored box is not where it is drawn. For an
//! unrotated, unflipped group the mapping is a scale and a translation, so the members can be
//! moved into slide space without changing what is drawn: [`normalize_groups`] does that.

use deckcraft_geom::Xfrm;
use serde::{Deserialize, Serialize};

use crate::{Shape, ShapeId, ShapeKind};

/// Deepest group nesting the canvas walks (like the model's other walks).
pub const MAX_DEPTH: usize = 64;

/// Display P3 colours a shape was given (SlideCraft). The shape's own fill and outline hold the
/// nearest sRGB colour, which is what is drawn and what `.pptx` gets; these keep the wide-gamut
/// originals, encoded Display P3 components 0–1. Dropped when the fill or outline is changed
/// some other way.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct P3Colors {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<[f64; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<[f64; 3]>,
}

/// An editable boolean shape's operation (SlideCraft): how a group's members combine into the
/// one shape it draws.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BoolOp {
    #[default]
    Union,
    /// The first member, less the others.
    Subtract,
    Intersect,
    /// Where an odd number of members overlap.
    Exclude,
}

/// A frame (SlideCraft, after FigCraft): a group with its own box. It draws its own fill and
/// outline (the group shape's `fill` and `line`) behind its members, can clip them to its box,
/// and is not refitted to its members: resizing it moves its members by their [`Constraints`]
/// instead of scaling them. On `.pptx` export it is a group with its members at their resolved
/// boxes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Frame {
    /// Members are clipped to the frame's box.
    pub clip: bool,
    /// Corner radius of the frame's box, in points (0–1000).
    pub radius: f64,
}

impl Frame {
    pub const MAX_RADIUS: f64 = 1000.0;
    /// The corner radius, made safe to use for a `w` × `h` box.
    pub fn radius(&self, w: f64, h: f64) -> f64 {
        let r = if self.radius.is_finite() { self.radius.clamp(0.0, Self::MAX_RADIUS) } else { 0.0 };
        r.min(w.abs().min(h.abs()) / 2.0).max(0.0)
    }
}

/// How a member of a frame follows the frame when it is resized, along one axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Pin {
    /// Keeps its distance from the left (top) edge and its size.
    #[default]
    Start,
    /// Keeps its distance from the right (bottom) edge and its size.
    End,
    /// Keeps both distances: it stretches.
    Both,
    /// Keeps its offset from the centre and its size.
    Center,
    /// Keeps its position and size as a share of the frame.
    Scale,
}

/// A frame member's constraints (absent: start on both axes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Constraints {
    pub horizontal: Pin,
    pub vertical: Pin,
}

/// How an auto layout frame arranges its members.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Flow {
    /// In a row, left to right.
    #[default]
    Horizontal,
    /// In a column, top to bottom.
    Vertical,
    /// On a grid of column and row tracks.
    Grid,
}

/// Where items sit along the flow; the `Space*` values spread the free space.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    /// Free space between items (main axis only).
    SpaceBetween,
    /// Stretch across (cross axis and grid cells only).
    Stretch,
}

/// How a box sizes along one axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Sizing {
    /// Its own size.
    #[default]
    Fixed,
    /// A frame: just big enough for its members and padding.
    Hug,
    /// A member: takes a share of the free space of its auto layout frame.
    Fill,
}

/// One grid track: `fixed` points, `hug` (its largest item), or `fr` shares of the free space.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum Track {
    Fixed(f64),
    Hug,
    Fr(f64),
}

impl Default for Track {
    fn default() -> Self {
        Track::Fr(1.0)
    }
}

/// Auto layout on a frame (SlideCraft, after FigCraft): its members are placed by flow, wrap or
/// grid, with padding and gaps; the frame can hug them. Members that opt out
/// ([`ChildLayout::absolute`]) keep their place.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AutoLayout {
    pub flow: Flow,
    /// Horizontal or vertical flow: start a new line when the next item doesn't fit.
    pub wrap: bool,
    /// Space between items along the flow (grid: between columns), in points.
    pub gap: f64,
    /// Space between wrapped lines (grid: between rows), in points.
    pub cross_gap: f64,
    /// `[top, right, bottom, left]` in points.
    pub padding: [f64; 4],
    /// Along the flow.
    pub align: Align,
    /// Across the flow (grid: within each cell).
    pub cross_align: Align,
    /// Grid: column tracks (at least one).
    pub columns: Vec<Track>,
    /// Grid: row tracks; rows past the last listed one are `hug`.
    pub rows: Vec<Track>,
    /// How the frame itself sizes, horizontally and vertically.
    pub width: Sizing,
    pub height: Sizing,
}

impl Default for AutoLayout {
    fn default() -> Self {
        AutoLayout {
            flow: Flow::Horizontal,
            wrap: false,
            gap: 12.0,
            cross_gap: 12.0,
            padding: [12.0; 4],
            align: Align::Start,
            cross_align: Align::Start,
            columns: vec![Track::Fr(1.0), Track::Fr(1.0)],
            rows: vec![],
            width: Sizing::Fixed,
            height: Sizing::Fixed,
        }
    }
}

/// A member's part in its frame's auto layout (absent: fixed size, placed by the layout).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ChildLayout {
    pub width: Sizing,
    pub height: Sizing,
    /// Ignores the auto layout: keeps its own place (and follows its constraints).
    pub absolute: bool,
    /// Grid: the cell's column and row (0-based); `None` places it in the next free cell.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row: Option<u32>,
    /// Grid: how many columns and rows it spans (0 counts as 1).
    pub column_span: u32,
    pub row_span: u32,
}

/// Within this many points, a group's child space counts as equal to its box.
const EPS: f64 = 1e-6;

fn same_box(a: &Xfrm, b: &Xfrm) -> bool {
    (a.x - b.x).abs() <= EPS && (a.y - b.y).abs() <= EPS && (a.w - b.w).abs() <= EPS && (a.h - b.h).abs() <= EPS
}

fn finite(x: &Xfrm) -> bool {
    [x.x, x.y, x.w, x.h, x.rot].iter().all(|v| v.is_finite())
}

/// Whether a group can carry its members in slide space: unrotated, unflipped, and a finite box.
pub fn group_is_axis_aligned(x: &Xfrm) -> bool {
    finite(x) && x.rot.rem_euclid(360.0).abs() <= EPS && !x.flip_h && !x.flip_v
}

/// `c` (in the child space `ch` of an axis-aligned group with box `g`) in the group's parent space.
fn map_axis_aligned(g: &Xfrm, ch: &Xfrm, c: &Xfrm) -> Xfrm {
    let sx = if ch.w.abs() > 1e-9 { g.w / ch.w } else { 1.0 };
    let sy = if ch.h.abs() > 1e-9 { g.h / ch.h } else { 1.0 };
    let (cx, cy) = (c.x + c.w / 2.0, c.y + c.h / 2.0);
    let (nx, ny) = (g.x + (cx - ch.x) * sx, g.y + (cy - ch.y) * sy);
    let (w, h) = (c.w * sx.abs(), c.h * sy.abs());
    Xfrm { x: nx - w / 2.0, y: ny - h / 2.0, w, h, ..*c }
}

/// `c` (a member of a frame whose box was `old` and is now `g`) placed by its constraints.
pub fn constrain(g: &Xfrm, old: &Xfrm, c: &Xfrm, k: Constraints) -> Xfrm {
    let scaled = map_axis_aligned(g, old, c);
    let axis = |pin: Pin, g0: f64, gl: f64, o0: f64, ol: f64, c0: f64, cl: f64, s0: f64, sl: f64| -> (f64, f64) {
        let (lead, trail) = (c0 - o0, (o0 + ol) - (c0 + cl));
        match pin {
            Pin::Start => (g0 + lead, cl),
            Pin::End => (g0 + gl - trail - cl, cl),
            Pin::Both => (g0 + lead, (gl - lead - trail).max(0.0)),
            Pin::Center => (g0 + gl / 2.0 + ((c0 + cl / 2.0) - (o0 + ol / 2.0)) - cl / 2.0, cl),
            Pin::Scale => (s0, sl),
        }
    };
    let (x, w) = axis(k.horizontal, g.x, g.w, old.x, old.w, c.x, c.w, scaled.x, scaled.w);
    let (y, h) = axis(k.vertical, g.y, g.h, old.y, old.h, c.y, c.h, scaled.y, scaled.h);
    Xfrm { x, y, w, h, ..*c }
}

/// Put every axis-aligned group's members (at any depth) into its parent's space, so the
/// group's child space equals its box. What is drawn does not change, except in a frame, whose
/// members follow their [`Constraints`] instead of scaling with it ([`constrain`]). Rotated or flipped groups,
/// SmartArt graphics (regenerated inside their own box) and groups with a non-finite box are left
/// as they are. Returns whether anything changed.
pub fn normalize_groups(list: &mut [Shape]) -> bool {
    normalize_list(list, 0)
}

fn normalize_list(list: &mut [Shape], depth: usize) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }
    let mut changed = false;
    for sh in list.iter_mut() {
        let smart = sh.diagram.is_some();
        let own = sh.xfrm;
        let framed = sh.frame.is_some();
        let ShapeKind::Group { children, child } = &mut sh.kind else { continue };
        if let Some(g) = own
            && !smart
            && group_is_axis_aligned(&g)
            && finite(child)
            && !same_box(&g, child)
        {
            for c in children.iter_mut() {
                if let Some(cx) = c.xfrm.filter(finite) {
                    // A frame's members follow their constraints; a group's scale with it.
                    let mapped = if framed { constrain(&g, child, &cx, c.constraints.unwrap_or_default()) } else { map_axis_aligned(&g, child, &cx) };
                    if finite(&mapped) {
                        c.xfrm = Some(mapped);
                    }
                }
            }
            *child = Xfrm { x: g.x, y: g.y, w: g.w, h: g.h, ..*child };
            changed = true;
        }
        changed |= normalize_list(children, depth + 1);
    }
    changed
}

/// Whether every group around `id` (in `list`) carries its members in slide space, so `id` can be
/// selected and edited on its own. `false` when `id` isn't found.
pub fn editable_in_place(list: &[Shape], id: ShapeId) -> bool {
    fn rec(list: &[Shape], id: ShapeId, depth: usize) -> Option<bool> {
        if depth > MAX_DEPTH {
            return None;
        }
        for sh in list {
            if sh.id == id {
                return Some(true);
            }
            if let ShapeKind::Group { children, child } = &sh.kind
                && let Some(inner) = rec(children, id, depth + 1)
            {
                let flat = sh.xfrm.is_some_and(|g| group_is_axis_aligned(&g) && same_box(&g, child));
                return Some(inner && flat);
            }
        }
        None
    }
    rec(list, id, 0).unwrap_or(false)
}

/// The chain of groups from the top level down to `id`'s parent (empty for a top-level shape),
/// or `None` when `id` isn't in `list`.
pub fn ancestors(list: &[Shape], id: ShapeId) -> Option<Vec<ShapeId>> {
    fn rec(list: &[Shape], id: ShapeId, path: &mut Vec<ShapeId>, depth: usize) -> bool {
        if depth > MAX_DEPTH {
            return false;
        }
        for sh in list {
            if sh.id == id {
                return true;
            }
            if let ShapeKind::Group { children, .. } = &sh.kind {
                path.push(sh.id);
                if rec(children, id, path, depth + 1) {
                    return true;
                }
                path.pop();
            }
        }
        false
    }
    let mut path = vec![];
    rec(list, id, &mut path, 0).then_some(path)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn rect(id: u32, x: Xfrm) -> Shape {
        Shape { id: ShapeId(id), xfrm: Some(x), ..Default::default() }
    }

    fn group(id: u32, x: Xfrm, child: Xfrm, children: Vec<Shape>) -> Shape {
        Shape { id: ShapeId(id), xfrm: Some(x), kind: ShapeKind::Group { children, child }, ..Default::default() }
    }

    #[test]
    fn a_moved_and_scaled_group_brings_its_members_into_slide_space() {
        // Child space 0..100 x 0..50 drawn at 200,100 twice as large.
        let mut list =
            vec![group(1, Xfrm::new(200.0, 100.0, 200.0, 100.0), Xfrm::new(0.0, 0.0, 100.0, 50.0), vec![rect(2, Xfrm::new(10.0, 5.0, 20.0, 10.0))])];
        assert!(normalize_groups(&mut list));
        let ShapeKind::Group { children, child } = &list[0].kind else { panic!() };
        assert!(same_box(child, &Xfrm::new(200.0, 100.0, 200.0, 100.0)));
        assert!(same_box(&children[0].xfrm.unwrap(), &Xfrm::new(220.0, 110.0, 40.0, 20.0)));
        assert!(editable_in_place(&list, ShapeId(2)));
        // Already normal: nothing to do.
        assert!(!normalize_groups(&mut list));
    }

    #[test]
    fn nested_groups_are_normalised_all_the_way_down() {
        let inner = group(3, Xfrm::new(0.0, 0.0, 50.0, 50.0), Xfrm::new(0.0, 0.0, 50.0, 50.0), vec![rect(4, Xfrm::new(10.0, 10.0, 10.0, 10.0))]);
        let mut list = vec![group(1, Xfrm::new(100.0, 0.0, 100.0, 100.0), Xfrm::new(0.0, 0.0, 100.0, 100.0), vec![inner])];
        assert!(!editable_in_place(&list, ShapeId(4)));
        normalize_groups(&mut list);
        assert!(editable_in_place(&list, ShapeId(4)));
        let ShapeKind::Group { children, .. } = &list[0].kind else { panic!() };
        let ShapeKind::Group { children: deep, .. } = &children[0].kind else { panic!() };
        assert!(same_box(&deep[0].xfrm.unwrap(), &Xfrm::new(110.0, 10.0, 10.0, 10.0)));
        assert_eq!(ancestors(&list, ShapeId(4)), Some(vec![ShapeId(1), ShapeId(3)]));
        assert_eq!(ancestors(&list, ShapeId(1)), Some(vec![]));
        assert_eq!(ancestors(&list, ShapeId(99)), None);
    }

    #[test]
    fn rotated_groups_and_hostile_numbers_are_left_alone() {
        let mut g = group(1, Xfrm::new(0.0, 0.0, 10.0, 10.0), Xfrm::new(5.0, 5.0, 10.0, 10.0), vec![rect(2, Xfrm::new(5.0, 5.0, 1.0, 1.0))]);
        if let Some(x) = g.xfrm.as_mut() {
            x.rot = 30.0;
        }
        let mut list = vec![g, group(3, Xfrm::new(f64::NAN, 0.0, 1.0, 1.0), Xfrm::new(0.0, 0.0, 0.0, 0.0), vec![])];
        assert!(!normalize_groups(&mut list));
        assert!(!editable_in_place(&list, ShapeId(2)));
        // A zero-sized child space maps without dividing by zero.
        let mut list = vec![group(1, Xfrm::new(10.0, 10.0, 10.0, 10.0), Xfrm::new(0.0, 0.0, 0.0, 0.0), vec![rect(2, Xfrm::new(0.0, 0.0, 1.0, 1.0))])];
        normalize_groups(&mut list);
        assert!(list.iter().all(|s| s.xfrm.is_some_and(|x| finite(&x))));
    }

    #[test]
    fn deep_nesting_is_bounded() {
        let mut s = rect(0, Xfrm::new(0.0, 0.0, 1.0, 1.0));
        for i in 1..200 {
            s = group(i, Xfrm::new(1.0, 1.0, 1.0, 1.0), Xfrm::new(0.0, 0.0, 1.0, 1.0), vec![s]);
        }
        let mut list = vec![s];
        normalize_groups(&mut list);
        assert!(!editable_in_place(&list, ShapeId(0)));
    }
}
