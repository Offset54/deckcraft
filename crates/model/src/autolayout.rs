//! The auto layout solver (SlideCraft, Phase 3). Re-implemented against the host model with
//! FigCraft's `figcraft-layout` as the behavioural reference: that crate places nodes of its own
//! document, so it could not be lifted (recon §1).
//!
//! [`solve`] is pure: it takes a frame's settings, its size and its members' sizes and returns
//! where each member goes and how big the frame is. The engine applies the answer to the shape
//! tree (`deckcraft_engine::canvas`).
//!
//! - **Flow** (horizontal or vertical): members in a line, `gap` apart, inside `padding`, aligned
//!   along the flow (`align`: start, centre, end, or space between) and across it (`cross_align`:
//!   start, centre, end, stretch). With `wrap`, a member that doesn't fit starts a new line,
//!   `cross_gap` below (or beside) the last.
//! - **Grid**: column and row tracks, each `fixed` points, `hug` (its largest member) or `fr`
//!   shares of the free space, like CSS. Members fill cells in reading order, or name their cell,
//!   and can span several. `align` places a member across its cell, `cross_align` down it.
//! - **Sizing**: a member is `fixed` (its own size) or `fill` (shares the free space along the
//!   flow, or takes the line's or cell's extent across it). A frame is `fixed` or `hug`s its
//!   members.
//!
//! Every input is hostile: non-finite or negative sizes count as 0, gaps and padding are clamped,
//! and the grid is capped at [`MAX_TRACKS`] columns and rows.

use crate::canvas::{Align, AutoLayout, ChildLayout, Flow, Sizing, Track};

/// Most grid columns or rows (members past them are not placed).
pub const MAX_TRACKS: usize = 256;
/// Largest gap or padding, in points.
pub const MAX_SPACE: f64 = 10_000.0;

/// A member as the solver sees it: its current size and its layout settings.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Item {
    pub w: f64,
    pub h: f64,
    pub layout: ChildLayout,
}

/// Where a member goes, relative to the frame's top-left corner.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Placed {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// The solver's answer: a box for each member (`None` for absolute ones, and for grid members
/// past the track limit) and the frame's size.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Solution {
    pub items: Vec<Option<Placed>>,
    pub w: f64,
    pub h: f64,
}

fn len(v: f64) -> f64 {
    if v.is_finite() { v.clamp(0.0, 1e7) } else { 0.0 }
}

fn space(v: f64) -> f64 {
    if v.is_finite() { v.clamp(0.0, MAX_SPACE) } else { 0.0 }
}

/// Lay out the members of a `w` × `h` frame.
pub fn solve(al: &AutoLayout, w: f64, h: f64, items: &[Item]) -> Solution {
    match al.flow {
        Flow::Horizontal => flow(al, w, h, items, true),
        Flow::Vertical => flow(al, w, h, items, false),
        Flow::Grid => grid(al, w, h, items),
    }
}

/// One member along the flow: main and cross sizes and whether each fills.
#[derive(Clone, Copy)]
struct FlowItem {
    index: usize,
    main: f64,
    cross: f64,
    fill_main: bool,
    fill_cross: bool,
}

fn flow(al: &AutoLayout, w: f64, h: f64, items: &[Item], horizontal: bool) -> Solution {
    let [pt, pr, pb, pl] = al.padding.map(space);
    let (gap, cross_gap) = (space(al.gap), space(al.cross_gap));
    // Main axis is x for a horizontal flow.
    let (pad_main0, pad_main1, pad_cross0, pad_cross1) = if horizontal { (pl, pr, pt, pb) } else { (pt, pb, pl, pr) };
    let (frame_main, frame_cross) = if horizontal { (len(w), len(h)) } else { (len(h), len(w)) };
    let (hug_main, hug_cross) =
        if horizontal { (al.width == Sizing::Hug, al.height == Sizing::Hug) } else { (al.height == Sizing::Hug, al.width == Sizing::Hug) };
    let members: Vec<FlowItem> = items
        .iter()
        .enumerate()
        .filter(|(_, it)| !it.layout.absolute)
        .map(|(index, it)| {
            let (mw, mh) = (len(it.w), len(it.h));
            let (main, cross) = if horizontal { (mw, mh) } else { (mh, mw) };
            let (sm, sc) = if horizontal { (it.layout.width, it.layout.height) } else { (it.layout.height, it.layout.width) };
            // Fill along the flow means nothing in a frame that hugs that way (there's no free
            // space to share): such a member keeps its size. Across the flow, fill takes the
            // line's extent, which in a hugging frame is its largest other member's.
            FlowItem { index, main, cross, fill_main: sm == Sizing::Fill && !hug_main, fill_cross: sc == Sizing::Fill }
        })
        .collect();
    let inner_main = (frame_main - pad_main0 - pad_main1).max(0.0);
    let inner_cross = (frame_cross - pad_cross0 - pad_cross1).max(0.0);
    // Break into lines. Only a frame with a fixed length along the flow wraps.
    let wraps = al.wrap && !hug_main;
    let mut lines: Vec<Vec<FlowItem>> = vec![];
    let mut cur: Vec<FlowItem> = vec![];
    let mut used = 0.0;
    for it in members {
        let size = if it.fill_main { 0.0 } else { it.main };
        let need = if cur.is_empty() { size } else { used + gap + size };
        if wraps && !cur.is_empty() && need > inner_main + 1e-9 {
            lines.push(std::mem::take(&mut cur));
            used = size;
        } else {
            used = need;
        }
        cur.push(it);
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(cur);
    }
    let single = lines.len() == 1;
    // Each line: main sizes (fill shares), its cross extent.
    let mut out = vec![None; items.len()];
    let mut line_boxes: Vec<(Vec<(usize, f64, f64, f64, bool)>, f64)> = vec![];
    let mut widest: f64 = 0.0;
    for line in &lines {
        let n = line.len();
        let fixed: f64 = line.iter().filter(|i| !i.fill_main).map(|i| i.main).sum();
        let gaps = gap * n.saturating_sub(1) as f64;
        let fills = line.iter().filter(|i| i.fill_main).count();
        let free = (inner_main - fixed - gaps).max(0.0);
        let share = if fills > 0 { free / fills as f64 } else { 0.0 };
        let sizes: Vec<f64> = line.iter().map(|i| if i.fill_main { share } else { i.main }).collect();
        let total: f64 = sizes.iter().sum::<f64>() + gaps;
        widest = widest.max(total);
        // Where the line starts along the flow, and the gap actually used.
        let (mut at, step) = if hug_main || fills > 0 {
            (pad_main0, gap)
        } else {
            let slack = (inner_main - total).max(0.0);
            match al.align {
                Align::Center => (pad_main0 + slack / 2.0, gap),
                Align::End => (pad_main0 + slack, gap),
                Align::SpaceBetween if n > 1 => (pad_main0, gap + slack / (n - 1) as f64),
                _ => (pad_main0, gap),
            }
        };
        let natural_cross = line.iter().filter(|i| !i.fill_cross).map(|i| i.cross).fold(0.0, f64::max);
        // A single line in a frame with a fixed cross size spans the whole inside.
        let extent = if single && !hug_cross { inner_cross } else { natural_cross };
        let mut placed = vec![];
        for (it, size) in line.iter().zip(sizes) {
            placed.push((it.index, at, size, it.cross, it.fill_cross));
            at += size + step;
        }
        line_boxes.push((placed, extent));
    }
    let lines_cross: f64 = line_boxes.iter().map(|(_, e)| e).sum::<f64>() + cross_gap * line_boxes.len().saturating_sub(1) as f64;
    let mut cross_at = pad_cross0;
    if !hug_cross && !single {
        // Wrapped lines sit at the start, centre or end of the frame, like their members.
        let slack = (inner_cross - lines_cross).max(0.0);
        cross_at += match al.cross_align {
            Align::Center => slack / 2.0,
            Align::End => slack,
            _ => 0.0,
        };
    }
    for (placed, extent) in &line_boxes {
        for &(index, main_at, main, cross, fill_cross) in placed {
            let stretch = fill_cross || al.cross_align == Align::Stretch;
            let c = if stretch { *extent } else { cross };
            let off = if stretch {
                0.0
            } else {
                match al.cross_align {
                    Align::Center => (extent - c) / 2.0,
                    Align::End => extent - c,
                    _ => 0.0,
                }
            };
            let (x, y, pw, ph) = if horizontal { (main_at, cross_at + off, main, c) } else { (cross_at + off, main_at, c, main) };
            if let Some(slot) = out.get_mut(index) {
                *slot = Some(Placed { x, y, w: pw, h: ph });
            }
        }
        cross_at += extent + cross_gap;
    }
    let main_size = if hug_main { widest + pad_main0 + pad_main1 } else { frame_main };
    let cross_size = if hug_cross { lines_cross + pad_cross0 + pad_cross1 } else { frame_cross };
    let (fw, fh) = if horizontal { (main_size, cross_size) } else { (cross_size, main_size) };
    Solution { items: out, w: fw, h: fh }
}

/// Sizes of grid tracks: `specs` (missing ones are `default`), `content` the largest member of
/// each, `avail` the space they share (`None` when the frame hugs: `fr` tracks then hug too).
fn tracks(specs: &[Track], n: usize, default: Track, content: &[f64], avail: Option<f64>) -> Vec<f64> {
    let spec = |i: usize| specs.get(i).copied().unwrap_or(default);
    let mut sizes = vec![0.0; n];
    let mut fr_total = 0.0;
    for (i, s) in sizes.iter_mut().enumerate() {
        let c = content.get(i).copied().unwrap_or(0.0);
        match spec(i) {
            Track::Fixed(v) => *s = len(v),
            Track::Hug => *s = c,
            Track::Fr(f) => {
                if avail.is_none() {
                    *s = c;
                } else {
                    fr_total += if f.is_finite() { f.clamp(0.0, 1e6) } else { 0.0 };
                }
            }
        }
    }
    if let Some(avail) = avail {
        let used: f64 = (0..n).filter(|i| !matches!(spec(*i), Track::Fr(_))).map(|i| sizes.get(i).copied().unwrap_or(0.0)).sum();
        let free = (avail - used).max(0.0);
        for (i, s) in sizes.iter_mut().enumerate() {
            if let Track::Fr(f) = spec(i) {
                let f = if f.is_finite() { f.clamp(0.0, 1e6) } else { 0.0 };
                *s = if fr_total > 0.0 { free * f / fr_total } else { 0.0 };
            }
        }
    }
    sizes
}

fn grid(al: &AutoLayout, w: f64, h: f64, items: &[Item]) -> Solution {
    let [pt, pr, pb, pl] = al.padding.map(space);
    let (col_gap, row_gap) = (space(al.gap), space(al.cross_gap));
    let ncol = al.columns.len().clamp(1, MAX_TRACKS);
    let (hug_w, hug_h) = (al.width == Sizing::Hug, al.height == Sizing::Hug);
    // Place members: named cells first claim their spot, the rest flow into free cells.
    let mut taken: Vec<Vec<bool>> = vec![];
    let occupied = |taken: &Vec<Vec<bool>>, r: usize, c: usize| taken.get(r).and_then(|row| row.get(c)).copied().unwrap_or(false);
    let mut cells: Vec<Option<(usize, usize, usize, usize)>> = vec![None; items.len()];
    let claim = |taken: &mut Vec<Vec<bool>>, r: usize, c: usize, rs: usize, cs: usize| {
        while taken.len() < r + rs {
            taken.push(vec![false; ncol]);
        }
        for row in taken.iter_mut().skip(r).take(rs) {
            for cell in row.iter_mut().skip(c).take(cs) {
                *cell = true;
            }
        }
    };
    let span = |s: u32, max: usize| (s.max(1) as usize).min(max);
    let order: Vec<usize> = (0..items.len())
        .filter(|i| items.get(*i).is_some_and(|it| it.layout.column.is_some() && it.layout.row.is_some()))
        .chain((0..items.len()).filter(|i| items.get(*i).is_some_and(|it| !(it.layout.column.is_some() && it.layout.row.is_some()))))
        .collect();
    let mut cursor = (0usize, 0usize);
    for i in order {
        let Some(it) = items.get(i) else { continue };
        if it.layout.absolute {
            continue;
        }
        let cs = span(it.layout.column_span, ncol);
        let rs = span(it.layout.row_span, MAX_TRACKS);
        let spot = match (it.layout.column, it.layout.row) {
            (Some(c), Some(r)) => Some(((r as usize).min(MAX_TRACKS - 1), (c as usize).min(ncol - cs))),
            _ => {
                // Next free cell in reading order where the span fits.
                let mut found = None;
                let (mut r, mut c) = cursor;
                while r < MAX_TRACKS {
                    if c + cs <= ncol && (0..rs).all(|dr| (0..cs).all(|dc| !occupied(&taken, r + dr, c + dc))) {
                        found = Some((r, c));
                        break;
                    }
                    c += 1;
                    if c >= ncol {
                        c = 0;
                        r += 1;
                    }
                }
                if let Some((r, c)) = found {
                    cursor = (r, c);
                }
                found
            }
        };
        let Some((r, c)) = spot else { continue };
        if r + rs > MAX_TRACKS {
            continue;
        }
        claim(&mut taken, r, c, rs, cs);
        if let Some(slot) = cells.get_mut(i) {
            *slot = Some((r, c, rs, cs));
        }
    }
    let nrow = al.rows.len().max(taken.len()).clamp(1, MAX_TRACKS);
    // What each track needs for its single-span members (fill members need nothing).
    let mut col_content = vec![0.0; ncol];
    let mut row_content = vec![0.0; nrow];
    for (i, cell) in cells.iter().enumerate() {
        let (Some((r, c, rs, cs)), Some(it)) = (cell, items.get(i)) else { continue };
        if *cs == 1
            && it.layout.width != Sizing::Fill
            && let Some(v) = col_content.get_mut(*c)
        {
            *v = f64::max(*v, len(it.w));
        }
        if *rs == 1
            && it.layout.height != Sizing::Fill
            && let Some(v) = row_content.get_mut(*r)
        {
            *v = f64::max(*v, len(it.h));
        }
    }
    let avail_w = (!hug_w).then(|| (len(w) - pl - pr - col_gap * (ncol - 1) as f64).max(0.0));
    let avail_h = (!hug_h).then(|| (len(h) - pt - pb - row_gap * (nrow - 1) as f64).max(0.0));
    let cols = tracks(&al.columns, ncol, Track::Fr(1.0), &col_content, avail_w);
    let rows = tracks(&al.rows, nrow, Track::Hug, &row_content, avail_h);
    let starts = |sizes: &[f64], from: f64, gap: f64| {
        let mut at = from;
        sizes
            .iter()
            .map(|s| {
                let a = at;
                at += s + gap;
                a
            })
            .collect::<Vec<f64>>()
    };
    let (cx, ry) = (starts(&cols, pl, col_gap), starts(&rows, pt, row_gap));
    let extent = |sizes: &[f64], from: usize, n: usize, gap: f64| sizes.iter().skip(from).take(n).sum::<f64>() + gap * n.saturating_sub(1) as f64;
    let mut out = vec![None; items.len()];
    for (i, cell) in cells.iter().enumerate() {
        let (Some((r, c, rs, cs)), Some(it)) = (cell, items.get(i)) else { continue };
        let (Some(x0), Some(y0)) = (cx.get(*c), ry.get(*r)) else { continue };
        let (cw, ch) = (extent(&cols, *c, *cs, col_gap), extent(&rows, *r, *rs, row_gap));
        let place = |own: f64, cell: f64, fill: bool, align: Align| -> (f64, f64) {
            if fill || align == Align::Stretch {
                return (0.0, cell);
            }
            let own = own.min(cell);
            match align {
                Align::Center => ((cell - own) / 2.0, own),
                Align::End => (cell - own, own),
                _ => (0.0, own),
            }
        };
        let (dx, pw) = place(len(it.w), cw, it.layout.width == Sizing::Fill, al.align);
        let (dy, ph) = place(len(it.h), ch, it.layout.height == Sizing::Fill, al.cross_align);
        if let Some(slot) = out.get_mut(i) {
            *slot = Some(Placed { x: x0 + dx, y: y0 + dy, w: pw, h: ph });
        }
    }
    let total = |sizes: &[f64], gap: f64| sizes.iter().sum::<f64>() + gap * sizes.len().saturating_sub(1) as f64;
    let fw = if hug_w { total(&cols, col_gap) + pl + pr } else { len(w) };
    let fh = if hug_h { total(&rows, row_gap) + pt + pb } else { len(h) };
    Solution { items: out, w: fw, h: fh }
}

#[cfg(test)]
#[path = "autolayout_tests.rs"]
mod tests;
