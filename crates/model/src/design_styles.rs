//! SlideCraft paint, effect and grid styles (Phase 4.2): named, reusable looks in the deck's
//! design system, as in FigCraft's styles.
//!
//! - A **paint style** is a fill (solid, gradient…; its colours may name colour variables, so a
//!   paint style restyles with a mode switch). A shape uses it as its fill or its outline.
//! - An **effect style** is a set of effects (shadows, glow, blur…).
//! - A **grid style** is a layout grid (columns, rows, gutter, margin) shown over a slide or a
//!   frame while editing; "Tidy up" and the layout library place things on it.
//!
//! Paragraph and character styles are Phase 2's `text_style`; Phase 4 lets their font size,
//! font family and colour follow variables (`TextStyle::bindings`).
//!
//! A shape names the styles it uses (`Shape::styles`). The engine's design pass writes a style's
//! look into the shapes that use it when the style changes, and forgets the link when the shape's
//! own look is changed some other way (as Figma detaches a style when you edit the property).

use serde::{Deserialize, Serialize};

use crate::style::{Effects, Fill};
use crate::{Shape, ShapeKind};

/// Most styles a deck holds.
pub const MAX_STYLES: usize = 4096;
/// Most columns or rows in a grid style.
pub const MAX_GRID_LINES: u32 = 48;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LayoutGrid {
    /// Columns (0: none).
    pub columns: u32,
    /// Rows (0: none).
    pub rows: u32,
    /// Space between columns and between rows, in points.
    pub gutter: f64,
    /// Space around the grid inside the slide or frame, in points.
    pub margin: f64,
}

impl LayoutGrid {
    /// Made safe: counts bounded, lengths finite and non-negative.
    pub fn clean(self) -> LayoutGrid {
        let len = |v: f64| if v.is_finite() { v.clamp(0.0, 10_000.0) } else { 0.0 };
        LayoutGrid {
            columns: self.columns.min(MAX_GRID_LINES),
            rows: self.rows.min(MAX_GRID_LINES),
            gutter: len(self.gutter),
            margin: len(self.margin),
        }
    }

    /// The column bands `(x0, x1)` inside a box at `x` of width `w`.
    pub fn columns_in(&self, x: f64, w: f64) -> Vec<(f64, f64)> {
        bands(self.columns, x + self.margin, w - 2.0 * self.margin, self.gutter)
    }
    /// The row bands `(y0, y1)` inside a box at `y` of height `h`.
    pub fn rows_in(&self, y: f64, h: f64) -> Vec<(f64, f64)> {
        bands(self.rows, y + self.margin, h - 2.0 * self.margin, self.gutter)
    }
}

fn bands(n: u32, start: f64, len: f64, gutter: f64) -> Vec<(f64, f64)> {
    if n == 0 || !len.is_finite() || len <= 0.0 {
        return vec![];
    }
    let n = n.min(MAX_GRID_LINES);
    let gaps = gutter * f64::from(n - 1);
    let cell = ((len - gaps) / f64::from(n)).max(0.0);
    (0..n)
        .map(|i| {
            let a = start + f64::from(i) * (cell + gutter);
            (a, a + cell)
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum StyleKind {
    Paint { fill: Fill },
    Effect { effects: Effects },
    Grid { grid: LayoutGrid },
}

impl StyleKind {
    pub fn name(&self) -> &'static str {
        match self {
            StyleKind::Paint { .. } => "paint",
            StyleKind::Effect { .. } => "effect",
            StyleKind::Grid { .. } => "grid",
        }
    }
}

/// A named style in the deck's design system.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignStyle {
    pub id: u32,
    pub name: String,
    pub kind: StyleKind,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// The styles a shape (or a slide, for its grid) uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StyleRefs {
    /// Paint style of the fill.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<u32>,
    /// Paint style of the outline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect: Option<u32>,
    /// Grid style of a frame.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid: Option<u32>,
}

impl StyleRefs {
    pub fn is_empty(&self) -> bool {
        *self == StyleRefs::default()
    }
}

pub(crate) fn sanitize(styles: &mut Vec<DesignStyle>) {
    styles.truncate(MAX_STYLES);
    let mut seen = std::collections::HashSet::new();
    styles.retain(|s| seen.insert(s.id));
    for s in styles.iter_mut() {
        crate::variables::truncate(&mut s.name, crate::variables::MAX_NAME);
        crate::variables::truncate(&mut s.description, crate::variables::MAX_STRING);
        if let StyleKind::Grid { grid } = &mut s.kind {
            *grid = grid.clean();
        }
    }
}

/// The outline fill a style gives: a paint style's fill.
fn paint(styles: &[DesignStyle], id: u32) -> Option<&Fill> {
    styles.iter().find(|s| s.id == id).and_then(|s| if let StyleKind::Paint { fill } = &s.kind { Some(fill) } else { None })
}
fn effect(styles: &[DesignStyle], id: u32) -> Option<&Effects> {
    styles.iter().find(|s| s.id == id).and_then(|s| if let StyleKind::Effect { effects } = &s.kind { Some(effects) } else { None })
}

/// Write each style's look into the shapes in `list` (and their groups) that use it; a link to a
/// style that no longer exists is dropped. Whether anything changed.
pub fn apply_styles(styles: &[DesignStyle], list: &mut [Shape]) -> bool {
    let mut changed = false;
    each(list, 0, &mut |sh| {
        let Some(mut r) = sh.styles else { return };
        if let Some(id) = r.fill {
            match paint(styles, id) {
                Some(f) if sh.fill.as_ref() != Some(f) => {
                    sh.fill = Some(f.clone());
                    changed = true;
                }
                Some(_) => {}
                None => r.fill = None,
            }
        }
        if let Some(id) = r.line {
            match paint(styles, id) {
                Some(f) => {
                    let l = sh.line.get_or_insert_with(Default::default);
                    if l.fill.as_ref() != Some(f) {
                        l.fill = Some(f.clone());
                        if l.width.is_none() {
                            l.width = Some(1.0);
                        }
                        changed = true;
                    }
                }
                None => r.line = None,
            }
        }
        if let Some(id) = r.effect {
            match effect(styles, id) {
                Some(e) if sh.effects.as_ref() != Some(e) => {
                    sh.effects = Some(e.clone());
                    changed = true;
                }
                Some(_) => {}
                None => r.effect = None,
            }
        }
        if r.grid.is_some_and(|g| !styles.iter().any(|s| s.id == g && matches!(s.kind, StyleKind::Grid { .. }))) {
            r.grid = None;
        }
        let r = (!r.is_empty()).then_some(r);
        if r != sh.styles {
            sh.styles = r;
            changed = true;
        }
    });
    changed
}

/// Forget the links of shapes whose look no longer matches their style (changed by a command that
/// doesn't know styles, such as Shape Fill). Whether anything changed.
pub fn detach_edited(styles: &[DesignStyle], list: &mut [Shape]) -> bool {
    let mut changed = false;
    each(list, 0, &mut |sh| {
        let Some(mut r) = sh.styles else { return };
        if r.fill.and_then(|id| paint(styles, id)).is_some_and(|f| sh.fill.as_ref() != Some(f)) {
            r.fill = None;
        }
        if r.line.and_then(|id| paint(styles, id)).is_some_and(|f| sh.line.as_ref().and_then(|l| l.fill.as_ref()) != Some(f)) {
            r.line = None;
        }
        if r.effect.and_then(|id| effect(styles, id)).is_some_and(|e| sh.effects.as_ref() != Some(e)) {
            r.effect = None;
        }
        let r = (!r.is_empty()).then_some(r);
        if r != sh.styles {
            sh.styles = r;
            changed = true;
        }
    });
    changed
}

/// Whether any shape in `list` uses a style.
pub fn uses_styles(list: &[Shape]) -> bool {
    let mut found = false;
    crate::walk(list, &mut |s, _| found |= s.styles.is_some());
    found
}

fn each(list: &mut [Shape], depth: usize, f: &mut dyn FnMut(&mut Shape)) {
    if depth > crate::canvas::MAX_DEPTH {
        return;
    }
    for sh in list.iter_mut() {
        f(sh);
        if let ShapeKind::Group { children, .. } = &mut sh.kind {
            each(children, depth + 1, f);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::ColorRef;
    use deckcraft_color::Rgba;

    fn red() -> Fill {
        Fill::solid(ColorRef::rgb(Rgba::rgb(255, 0, 0)))
    }

    #[test]
    fn grid_bands_split_the_box() {
        let g = LayoutGrid { columns: 3, rows: 0, gutter: 10.0, margin: 20.0 };
        assert_eq!(g.columns_in(0.0, 330.0), vec![(20.0, 110.0), (120.0, 210.0), (220.0, 310.0)]);
        assert!(g.rows_in(0.0, 100.0).is_empty());
        let hostile = LayoutGrid { columns: u32::MAX, rows: 1, gutter: f64::NAN, margin: -5.0 }.clean();
        assert_eq!(hostile.columns, MAX_GRID_LINES);
        assert_eq!((hostile.gutter, hostile.margin), (0.0, 0.0));
        assert_eq!(LayoutGrid { columns: 2, ..Default::default() }.columns_in(0.0, -1.0), vec![]);
    }

    #[test]
    fn styles_write_into_shapes_and_edits_detach_them() {
        let mut styles = vec![DesignStyle { id: 1, name: "Brand".into(), kind: StyleKind::Paint { fill: red() }, description: String::new() }];
        let mut list = vec![Shape { styles: Some(StyleRefs { fill: Some(1), line: Some(1), ..Default::default() }), ..Default::default() }];
        assert!(apply_styles(&styles, &mut list));
        assert_eq!(list[0].fill, Some(red()));
        assert_eq!(list[0].line.as_ref().and_then(|l| l.fill.clone()), Some(red()));
        assert!(!apply_styles(&styles, &mut list), "settled");
        // The style changes: so do its shapes.
        let blue = Fill::solid(ColorRef::rgb(Rgba::rgb(0, 0, 255)));
        styles[0].kind = StyleKind::Paint { fill: blue.clone() };
        assert!(apply_styles(&styles, &mut list));
        assert_eq!(list[0].fill, Some(blue.clone()));
        // The shape's fill is changed directly: the fill link goes, the outline's stays.
        list[0].fill = Some(red());
        assert!(detach_edited(&styles, &mut list));
        assert_eq!(list[0].styles, Some(StyleRefs { line: Some(1), ..Default::default() }));
        // The style is deleted: its links go, the look stays.
        assert!(apply_styles(&[], &mut list));
        assert_eq!(list[0].styles, None);
        assert_eq!(list[0].line.as_ref().and_then(|l| l.fill.clone()), Some(blue));
    }
}
