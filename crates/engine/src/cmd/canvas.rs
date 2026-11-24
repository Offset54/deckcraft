//! SlideCraft canvas commands (Phase 3): selecting inside groups without ungrouping, and
//! measuring the distance between objects.
//!
//! A group's members can be selected and edited in place: `edit.selectChildren` (Enter on a
//! selected group), `edit.selectParent` (Shift+Enter or Esc on a member), a click on another
//! member once one is selected, and a double-click into a group. That works because groups keep
//! their members in slide space (`deckcraft_model::canvas`); a rotated or flipped group can't, so
//! its members are not entered.

use deckcraft_geom::{Point, Rect, Xfrm};
use deckcraft_model::canvas::{Align, AutoLayout, ChildLayout, Constraints, Flow, Frame, Pin, Sizing, Track};
use deckcraft_model::{Shape, ShapeId, ShapeKind, canvas};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{
    CommandSpec, NO_ID_SPACE, bad, bool_param, cmd, color_param, has_selection, has_slide, id_param, ids_param, rect_param, str_param, xfrm_of,
};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            noundo "edit.selectChildren",
            "Select Children",
            ["Arrange", "Group Objects"],
            None,
            "{ids?: groups (else the selected ones)} → {selected}: select the members of the groups, to edit them without ungrouping (Enter on a group). A rotated or flipped group's members can't be edited in place: ungroup it",
            has_selection,
            select_children
        ),
        cmd!(
            noundo "edit.selectParent",
            "Select Parent",
            ["Arrange", "Group Objects"],
            None,
            "{ids?} → {selected}: select the groups that hold the selected objects (Shift+Enter, or Esc on a member); objects at the top level stay selected",
            has_selection,
            select_parent
        ),
        cmd!(
            "frame.create",
            "Frame Selection",
            ["Arrange", "Frame"],
            None,
            "{ids?: objects on the slide (else the selection), rect?: [x,y,w,h] pt (an empty frame), padding?: pt, clip?: bool, radius?: pt, fill?: color, name?} → {id}: wrap objects in a frame, a group with its own box, fill and clipping that keeps its size when its members move",
            has_slide,
            frame_create
        ),
        cmd!(
            "frame.set",
            "Frame Options",
            ["Arrange", "Frame"],
            None,
            "{ids?: groups or frames (else the selection), clip?: bool, radius?: pt, frame?: bool (false turns a frame back into a group)} → {ids}",
            has_selection,
            frame_set
        ),
        cmd!(
            "layout.constraints",
            "Constraints",
            ["Arrange", "Frame"],
            None,
            "{ids?: members of frames (else the selection), horizontal?: start|end|both|center|scale, vertical?: start|end|both|center|scale} → {ids}: how members follow their frame when it is resized (start: keep the left/top distance; end: the right/bottom; both: stretch; center; scale)",
            has_selection,
            set_constraints
        ),
        cmd!(
            "layout.set",
            "Auto Layout",
            ["Arrange", "Auto Layout"],
            None,
            "{ids?: a frame or group to lay out (else the selection; several objects or one that is not a group are framed first), flow?: horizontal|vertical|grid, wrap?: bool, gap?: pt, crossGap?: pt, padding?: pt|[top,right,bottom,left], align?: start|center|end|spaceBetween, crossAlign?: start|center|end|stretch, columns?: [track], rows?: [track] (a track is points, \"hug\" or \"2fr\"), width?: fixed|hug, height?: fixed|hug} → {ids}: lay out a frame's members automatically",
            has_selection,
            layout_set
        ),
        cmd!(
            "layout.child",
            "Auto Layout Member",
            ["Arrange", "Auto Layout"],
            None,
            "{ids?: members of auto layout frames (else the selection), width?: fixed|fill, height?: fixed|fill, absolute?: bool (ignore the layout), column?: index|null, row?: index|null, columnSpan?: n, rowSpan?: n} → {ids}",
            has_selection,
            layout_child
        ),
        cmd!(
            "layout.remove",
            "Remove Auto Layout",
            ["Arrange", "Auto Layout"],
            None,
            "{ids?: frames (else the selection)} → {ids}: the members stay where the layout put them",
            has_selection,
            layout_remove
        ),
        cmd!(
            "format.blur",
            "Blur",
            ["Shape Format", "Shape Styles", "Shape Effects"],
            None,
            "{ids?, layer?: pt|null (blur the shape itself), background?: pt|null (blur what is behind it, frosted glass; top-level objects)} → {ids}: radii 0–250 pt; null or 0 removes",
            has_selection,
            set_blur
        ),
        cmd!(
            "format.noise",
            "Noise",
            ["Shape Format", "Shape Styles", "Shape Effects"],
            None,
            "{ids?, amount: 0–1|null} → {ids}: film grain over the fill; null or 0 removes",
            has_selection,
            set_noise
        ),
        cmd!(
            query "arrange.measure",
            "Measure Distance",
            [],
            None,
            "{ids?: objects measured from (else the selection), to?: id (else the slide)} → {from: [x,y,w,h], to: [x,y,w,h], gaps: [{axis: x|y, from: [x,y], to: [x,y], value: pt}]} (Alt while pointing shows the same on the canvas)",
            has_slide,
            measure_cmd
        ),
    ]
}

/// One measured distance on the canvas: a line from `from` to `to` along `axis`, `value` points
/// long.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gap {
    pub axis: char,
    pub from: (f64, f64),
    pub to: (f64, f64),
    pub value: f64,
}

/// Distances from box `a` to box `b`, as a designer reads them:
/// - one box inside the other: from each edge of the inner box to the same edge of the outer one;
/// - apart: the gap between them on each axis where they don't overlap;
/// - overlapping: from each edge of `a` to the same edge of `b` where they differ.
///
/// Lines run through the middle of the boxes' common span, or of `a` where there is none.
/// Zero-length distances are left out. Non-finite boxes give nothing.
pub fn measure(a: Rect, b: Rect) -> Vec<Gap> {
    let ok = |r: &Rect| [r.x0, r.y0, r.x1, r.y1].iter().all(|v| v.is_finite());
    if !ok(&a) || !ok(&b) {
        return vec![];
    }
    let mut out = vec![];
    let mut push = |axis: char, from: Point, to: Point| {
        let value = if axis == 'x' { (to.x - from.x).abs() } else { (to.y - from.y).abs() };
        if value > 1e-6 {
            out.push(Gap { axis, from: (from.x, from.y), to: (to.x, to.y), value });
        }
    };
    let contains = |o: &Rect, i: &Rect| o.x0 <= i.x0 && o.y0 <= i.y0 && o.x1 >= i.x1 && o.y1 >= i.y1;
    // A span's middle when the two ranges overlap, else `fallback`.
    let mid = |a0: f64, a1: f64, b0: f64, b1: f64, fallback: f64| {
        let (lo, hi) = (a0.max(b0), a1.min(b1));
        if lo <= hi { (lo + hi) / 2.0 } else { fallback }
    };
    if contains(&b, &a) || contains(&a, &b) {
        let (inner, outer) = if contains(&b, &a) { (a, b) } else { (b, a) };
        let (iy, ix) = (inner.center().y, inner.center().x);
        push('x', Point::new(outer.x0, iy), Point::new(inner.x0, iy));
        push('x', Point::new(inner.x1, iy), Point::new(outer.x1, iy));
        push('y', Point::new(ix, outer.y0), Point::new(ix, inner.y0));
        push('y', Point::new(ix, inner.y1), Point::new(ix, outer.y1));
        return out;
    }
    let y = mid(a.y0, a.y1, b.y0, b.y1, a.center().y);
    let x = mid(a.x0, a.x1, b.x0, b.x1, a.center().x);
    let apart_x = a.x1 <= b.x0 || b.x1 <= a.x0;
    let apart_y = a.y1 <= b.y0 || b.y1 <= a.y0;
    if a.x1 <= b.x0 {
        push('x', Point::new(a.x1, y), Point::new(b.x0, y));
    } else if b.x1 <= a.x0 {
        push('x', Point::new(b.x1, y), Point::new(a.x0, y));
    }
    if a.y1 <= b.y0 {
        push('y', Point::new(x, a.y1), Point::new(x, b.y0));
    } else if b.y1 <= a.y0 {
        push('y', Point::new(x, b.y1), Point::new(x, a.y0));
    }
    if !apart_x && !apart_y {
        let (ay, ax) = (a.center().y, a.center().x);
        push('x', Point::new(a.x0, ay), Point::new(b.x0, ay));
        push('x', Point::new(a.x1, ay), Point::new(b.x1, ay));
        push('y', Point::new(ax, a.y0), Point::new(ax, b.y0));
        push('y', Point::new(ax, a.y1), Point::new(ax, b.y1));
    }
    out
}

/// The slide-space bounds of the selection (or `ids`), and of `to` (or the page).
fn measure_boxes(s: &Session, p: &Value) -> Result<(Rect, Rect)> {
    let st = s.doc()?;
    let ids = ids_param(p, "ids").unwrap_or_else(|| st.selection.shapes.clone());
    let xs: Vec<_> = ids.iter().filter_map(|i| st.shape(*i)).map(|sh| xfrm_of(&st.doc, &st.selection, sh)).collect();
    let from = super::arrange::union_bounds(&xs).ok_or_else(|| bad("arrange.measure", "nothing to measure from: select an object or pass `ids`"))?;
    let to = match id_param(p, "to", "arrange.measure")? {
        Some(id) => {
            let sh = st.shape(id).ok_or_else(|| bad("arrange.measure", format!("no object {id}")))?;
            xfrm_of(&st.doc, &st.selection, sh).bounds()
        }
        None => {
            let size = crate::page_size(st);
            Rect::new(0.0, 0.0, size.width, size.height)
        }
    };
    Ok((from, to))
}

fn measure_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    let (from, to) = measure_boxes(s, p)?;
    let r = |r: Rect| json!([r.x0, r.y0, r.width(), r.height()]);
    let gaps: Vec<Value> = measure(from, to)
        .into_iter()
        .map(|g| json!({"axis": g.axis.to_string(), "from": [g.from.0, g.from.1], "to": [g.to.0, g.to.1], "value": g.value}))
        .collect();
    Ok(json!({"from": r(from), "to": r(to), "gaps": gaps}))
}

/// The shapes a command acts on: `ids`, else the selection. Every id must exist.
fn chosen(s: &Session, p: &Value, cmd: &str) -> Result<Vec<ShapeId>> {
    let st = s.doc()?;
    let ids = ids_param(p, "ids").unwrap_or_else(|| st.selection.shapes.clone());
    if let Some(bad_id) = ids.iter().find(|i| st.shape(**i).is_none()) {
        return Err(bad(cmd, format!("no object {bad_id}")));
    }
    Ok(ids)
}

fn finite_len(p: &Value, key: &str, cmd: &str, max: f64) -> Result<Option<f64>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => match v.as_f64().filter(|v| v.is_finite() && (0.0..=max).contains(v)) {
            Some(v) => Ok(Some(v)),
            None => Err(bad(cmd, format!("`{key}` must be a number of points from 0 to {max}"))),
        },
    }
}

fn frame_create(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "frame.create";
    let rect = rect_param(p, "rect");
    let ids = if rect.is_some() && p.get("ids").is_none() { vec![] } else { chosen(s, p, CMD)? };
    if ids.is_empty() && rect.is_none() {
        return Err(bad(CMD, "select objects to frame, or give `rect` for an empty frame"));
    }
    let padding = finite_len(p, "padding", CMD, 1000.0)?.unwrap_or(0.0);
    let radius = finite_len(p, "radius", CMD, Frame::MAX_RADIUS)?.unwrap_or(0.0);
    let clip = bool_param(p, "clip").unwrap_or(false);
    let fill = color_param(p, "fill").map(|color| deckcraft_model::Fill::Solid { color });
    if p.get("fill").is_some_and(|v| !v.is_null()) && fill.is_none() {
        return Err(bad(CMD, "`fill` is not a colour"));
    }
    let name = str_param(p, "name").map(|n| n.chars().take(256).collect::<String>());
    s.edit(|doc, sel| {
        let current = super::current_shapes(doc, sel).cloned().unwrap_or_default();
        let mut members: Vec<Shape> = current.iter().filter(|x| ids.contains(&x.id)).cloned().collect();
        if members.len() != ids.len() {
            return Err(bad(CMD, "only objects at the top level of the slide can be framed together"));
        }
        if super::arrange::group_depth(&members) + 1 > super::arrange::MAX_GROUP_DEPTH {
            return Err(bad(CMD, format!("groups can be nested at most {} deep", super::arrange::MAX_GROUP_DEPTH)));
        }
        for m in &mut members {
            if m.xfrm.is_none() {
                m.xfrm = Some(xfrm_of(doc, sel, m));
            }
            m.ph = None;
        }
        let xs: Vec<Xfrm> = members.iter().filter_map(|m| m.xfrm).collect();
        let b = match (rect, super::arrange::union_bounds(&xs)) {
            (Some(r), _) => Rect::new(r.x, r.y, r.x + r.w.max(0.0), r.y + r.h.max(0.0)),
            (None, Some(u)) => u.inflate(padding, padding),
            (None, None) => return Err(bad(CMD, "nothing to frame")),
        };
        let fx = Xfrm::new(b.x0, b.y0, b.width(), b.height());
        if ![fx.x, fx.y, fx.w, fx.h].iter().all(|v| v.is_finite()) {
            return Err(bad(CMD, "the frame's box is not finite"));
        }
        let id = ShapeId(doc.alloc_id().ok_or_else(|| bad(CMD, NO_ID_SPACE))?);
        let n = current.iter().filter(|x| x.frame.is_some()).count();
        let f = Shape {
            id,
            name: name.unwrap_or_else(|| format!("Frame {}", n + 1)),
            xfrm: Some(fx),
            kind: ShapeKind::Group { children: members, child: fx },
            fill: fill.or(Some(deckcraft_model::Fill::None)),
            frame: Some(Frame { clip, radius }),
            ..Default::default()
        };
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad(CMD, "no slide"))?;
        let at = list.iter().rposition(|x| ids.contains(&x.id)).map(|i| i + 1).unwrap_or(list.len());
        list.insert(at, f);
        list.retain(|x| !ids.contains(&x.id));
        sel.shapes = vec![id];
        sel.text = None;
        sel.cells = None;
        Ok(json!({"id": id}))
    })
}

fn frame_set(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "frame.set";
    let ids = chosen(s, p, CMD)?;
    let clip = bool_param(p, "clip");
    let radius = finite_len(p, "radius", CMD, Frame::MAX_RADIUS)?;
    let keep = bool_param(p, "frame").unwrap_or(true);
    {
        let st = s.doc()?;
        if let Some(id) = ids.iter().find(|i| !st.shape(**i).is_some_and(Shape::is_group)) {
            return Err(bad(CMD, format!("object {id} is not a group or a frame")));
        }
    }
    s.edit(|doc, sel| {
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad(CMD, "no slide"))?;
        for id in &ids {
            let Some(sh) = deckcraft_model::find_shape_mut(list, *id) else { continue };
            if !keep {
                sh.frame = None;
                sh.auto_layout = None;
                if let ShapeKind::Group { children, .. } = &mut sh.kind {
                    for c in children.iter_mut() {
                        c.constraints = None;
                        c.layout_child = None;
                    }
                }
                continue;
            }
            let mut f = sh.frame.unwrap_or_default();
            if let Some(c) = clip {
                f.clip = c;
            }
            if let Some(r) = radius {
                f.radius = r;
            }
            if sh.frame.is_none() && sh.fill.is_none() {
                sh.fill = Some(deckcraft_model::Fill::None);
            }
            sh.frame = Some(f);
        }
        Ok(json!({"ids": ids}))
    })
}

fn pin_param(p: &Value, key: &str) -> Result<Option<Pin>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => serde_json::from_value::<Pin>(v.clone())
            .map(Some)
            .map_err(|_| bad("layout.constraints", format!("`{key}` must be start, end, both, center or scale"))),
    }
}

fn set_constraints(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "layout.constraints";
    let ids = chosen(s, p, CMD)?;
    let (h, v) = (pin_param(p, "horizontal")?, pin_param(p, "vertical")?);
    {
        let st = s.doc()?;
        for id in &ids {
            let parent = canvas::ancestors(st.shapes(), *id).and_then(|c| c.last().copied());
            if !parent.and_then(|g| st.shape(g)).is_some_and(|g| g.frame.is_some()) {
                return Err(bad(CMD, format!("object {id} is not in a frame")));
            }
        }
    }
    s.edit(|doc, sel| {
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad(CMD, "no slide"))?;
        for id in &ids {
            let Some(sh) = deckcraft_model::find_shape_mut(list, *id) else { continue };
            let mut k: Constraints = sh.constraints.unwrap_or_default();
            if let Some(h) = h {
                k.horizontal = h;
            }
            if let Some(v) = v {
                k.vertical = v;
            }
            sh.constraints = (k != Constraints::default()).then_some(k);
        }
        Ok(json!({"ids": ids}))
    })
}

fn enum_param<T: serde::de::DeserializeOwned>(p: &Value, key: &str, cmd: &str, allowed: &str) -> Result<Option<T>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => serde_json::from_value::<T>(v.clone()).map(Some).map_err(|_| bad(cmd, format!("`{key}` must be one of {allowed}"))),
    }
}

/// A grid track: a number of points, `"hug"`, `"2fr"` or `"120pt"`.
fn track_value(v: &Value) -> Option<Track> {
    if let Some(n) = v.as_f64() {
        return (n.is_finite() && (0.0..=100_000.0).contains(&n)).then_some(Track::Fixed(n));
    }
    let t = v.as_str()?.trim().to_ascii_lowercase();
    if t == "hug" || t == "auto" {
        return Some(Track::Hug);
    }
    let num = |s: &str| s.trim().parse::<f64>().ok().filter(|n| n.is_finite());
    if let Some(f) = t.strip_suffix("fr") {
        return num(f).filter(|n| (0.0..=1000.0).contains(n)).map(Track::Fr);
    }
    num(t.strip_suffix("pt").unwrap_or(&t)).filter(|n| (0.0..=100_000.0).contains(n)).map(Track::Fixed)
}

fn tracks_param(p: &Value, key: &str, cmd: &str) -> Result<Option<Vec<Track>>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(a)) if a.len() <= deckcraft_model::autolayout::MAX_TRACKS => a
            .iter()
            .map(|v| track_value(v).ok_or_else(|| bad(cmd, format!("`{key}`: {v} is not a track (points, \"hug\" or \"Nfr\")"))))
            .collect::<Result<Vec<_>>>()
            .map(Some),
        Some(_) => Err(bad(cmd, format!("`{key}` must be a list of at most {} tracks", deckcraft_model::autolayout::MAX_TRACKS))),
    }
}

fn padding_param(p: &Value, cmd: &str) -> Result<Option<[f64; 4]>> {
    let ok = |v: &Value| v.as_f64().filter(|n| n.is_finite() && (0.0..=deckcraft_model::autolayout::MAX_SPACE).contains(n));
    match p.get("padding") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(a)) if a.len() == 4 => {
            let v: Vec<f64> = a.iter().filter_map(ok).collect();
            match v.as_slice() {
                [t, r, b, l] => Ok(Some([*t, *r, *b, *l])),
                _ => Err(bad(cmd, "`padding` values must be points from 0 to 10000")),
            }
        }
        Some(v) => ok(v).map(|n| Some([n; 4])).ok_or_else(|| bad(cmd, "`padding` must be points or [top, right, bottom, left]")),
    }
}

fn layout_set(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "layout.set";
    let flow: Option<Flow> = enum_param(p, "flow", CMD, "horizontal, vertical or grid")?;
    let align: Option<Align> = enum_param(p, "align", CMD, "start, center, end or spaceBetween")?;
    let cross: Option<Align> = enum_param(p, "crossAlign", CMD, "start, center, end or stretch")?;
    let width: Option<Sizing> = enum_param(p, "width", CMD, "fixed or hug")?;
    let height: Option<Sizing> = enum_param(p, "height", CMD, "fixed or hug")?;
    if width == Some(Sizing::Fill) || height == Some(Sizing::Fill) {
        return Err(bad(CMD, "a frame's own size is fixed or hug (fill is for members: layout.child)"));
    }
    if align == Some(Align::Stretch) || cross == Some(Align::SpaceBetween) {
        return Err(bad(CMD, "`align` takes start, center, end or spaceBetween; `crossAlign` start, center, end or stretch"));
    }
    let gap = finite_len(p, "gap", CMD, deckcraft_model::autolayout::MAX_SPACE)?;
    let cross_gap = finite_len(p, "crossGap", CMD, deckcraft_model::autolayout::MAX_SPACE)?;
    let padding = padding_param(p, CMD)?;
    let columns = tracks_param(p, "columns", CMD)?;
    let rows = tracks_param(p, "rows", CMD)?;
    if columns.as_ref().is_some_and(Vec::is_empty) {
        return Err(bad(CMD, "a grid needs at least one column"));
    }
    let wrap = bool_param(p, "wrap");
    let mut ids = chosen(s, p, CMD)?;
    if ids.is_empty() {
        return Err(bad(CMD, "select a frame, a group or the objects to lay out"));
    }
    // One group or frame is laid out itself. Anything else (one other object, or several
    // objects, frames included) is framed first, as Figma's "add auto layout" does; one undo step
    // for both (nested commands).
    let one_group = {
        let st = s.doc()?;
        matches!(ids.as_slice(), [one] if st.shape(*one).is_some_and(Shape::is_group))
    };
    if !one_group {
        let r = s.execute("frame.create", &json!({"ids": ids}))?;
        let f = r.get("id").and_then(Value::as_u64).and_then(|v| u32::try_from(v).ok()).map(ShapeId).ok_or_else(|| bad(CMD, "no frame"))?;
        ids = vec![f];
    }
    s.edit(|doc, sel| {
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad(CMD, "no slide"))?;
        for id in &ids {
            let Some(sh) = deckcraft_model::find_shape_mut(list, *id) else { continue };
            if sh.xfrm.is_some_and(|x| !canvas::group_is_axis_aligned(&x)) {
                return Err(bad(CMD, "a rotated or flipped frame can't use auto layout"));
            }
            if sh.frame.is_none() {
                sh.frame = Some(Frame::default());
                if sh.fill.is_none() {
                    sh.fill = Some(deckcraft_model::Fill::None);
                }
            }
            let mut al = sh.auto_layout.clone().unwrap_or_default();
            if let Some(v) = flow {
                al.flow = v;
            }
            if let Some(v) = wrap {
                al.wrap = v;
            }
            if let Some(v) = gap {
                al.gap = v;
            }
            if let Some(v) = cross_gap {
                al.cross_gap = v;
            }
            if let Some(v) = padding {
                al.padding = v;
            }
            if let Some(v) = align {
                al.align = v;
            }
            if let Some(v) = cross {
                al.cross_align = v;
            }
            if let Some(v) = &columns {
                al.columns = v.clone();
            }
            if let Some(v) = &rows {
                al.rows = v.clone();
            }
            if let Some(v) = width {
                al.width = v;
            }
            if let Some(v) = height {
                al.height = v;
            }
            sh.auto_layout = Some(al);
        }
        Ok(json!({"ids": ids}))
    })
}

/// The auto layout frame holding `id`, if any.
fn layout_parent(st: &crate::DocState, id: ShapeId) -> Option<&AutoLayout> {
    let g = canvas::ancestors(st.shapes(), id)?.last().copied()?;
    st.shape(g)?.auto_layout.as_ref()
}

fn layout_child(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "layout.child";
    let ids = chosen(s, p, CMD)?;
    let width: Option<Sizing> = enum_param(p, "width", CMD, "fixed or fill")?;
    let height: Option<Sizing> = enum_param(p, "height", CMD, "fixed or fill")?;
    if width == Some(Sizing::Hug) || height == Some(Sizing::Hug) {
        return Err(bad(CMD, "a member is fixed or fill (hug is for frames: layout.set)"));
    }
    let absolute = bool_param(p, "absolute");
    let cell = |key: &str| -> Result<Option<Option<u32>>> {
        match p.get(key) {
            None => Ok(None),
            Some(Value::Null) => Ok(Some(None)),
            Some(v) => v
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .filter(|n| (*n as usize) < deckcraft_model::autolayout::MAX_TRACKS)
                .map(|n| Some(Some(n)))
                .ok_or_else(|| bad(CMD, format!("`{key}` must be a track index below {}", deckcraft_model::autolayout::MAX_TRACKS))),
        }
    };
    let (column, row) = (cell("column")?, cell("row")?);
    let span = |key: &str| -> Result<Option<u32>> {
        match p.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .filter(|n| (1..=deckcraft_model::autolayout::MAX_TRACKS as u32).contains(n))
                .map(Some)
                .ok_or_else(|| bad(CMD, format!("`{key}` must be from 1 to {}", deckcraft_model::autolayout::MAX_TRACKS))),
        }
    };
    let (column_span, row_span) = (span("columnSpan")?, span("rowSpan")?);
    {
        let st = s.doc()?;
        if let Some(id) = ids.iter().find(|i| layout_parent(st, **i).is_none()) {
            return Err(bad(CMD, format!("object {id} is not in an auto layout frame")));
        }
    }
    s.edit(|doc, sel| {
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad(CMD, "no slide"))?;
        for id in &ids {
            let Some(sh) = deckcraft_model::find_shape_mut(list, *id) else { continue };
            let mut c: ChildLayout = sh.layout_child.unwrap_or_default();
            if let Some(v) = width {
                c.width = v;
            }
            if let Some(v) = height {
                c.height = v;
            }
            if let Some(v) = absolute {
                c.absolute = v;
            }
            if let Some(v) = column {
                c.column = v;
            }
            if let Some(v) = row {
                c.row = v;
            }
            if let Some(v) = column_span {
                c.column_span = v;
            }
            if let Some(v) = row_span {
                c.row_span = v;
            }
            sh.layout_child = (c != ChildLayout::default()).then_some(c);
        }
        Ok(json!({"ids": ids}))
    })
}

fn layout_remove(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "layout.remove";
    let ids = chosen(s, p, CMD)?;
    {
        let st = s.doc()?;
        if let Some(id) = ids.iter().find(|i| st.shape(**i).is_none_or(|sh| sh.auto_layout.is_none())) {
            return Err(bad(CMD, format!("object {id} has no auto layout")));
        }
    }
    s.edit(|doc, sel| {
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad(CMD, "no slide"))?;
        for id in &ids {
            if let Some(sh) = deckcraft_model::find_shape_mut(list, *id) {
                sh.auto_layout = None;
                if let ShapeKind::Group { children, .. } = &mut sh.kind {
                    for c in children.iter_mut() {
                        c.layout_child = None;
                    }
                }
            }
        }
        Ok(json!({"ids": ids}))
    })
}

/// `Some(None)` to remove (null or 0), `Some(Some(v))` to set, `None` when absent.
fn effect_amount(p: &Value, key: &str, cmd: &str, max: f64) -> Result<Option<Option<f64>>> {
    match p.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(v) => match v.as_f64().filter(|n| n.is_finite() && (0.0..=max).contains(n)) {
            Some(0.0) => Ok(Some(None)),
            Some(n) => Ok(Some(Some(n))),
            None => Err(bad(cmd, format!("`{key}` must be from 0 to {max}"))),
        },
    }
}

fn edit_effects(s: &mut Session, p: &Value, cmd: &'static str, f: impl Fn(&mut deckcraft_model::Effects)) -> Result<Value> {
    let ids = chosen(s, p, cmd)?;
    if ids.is_empty() {
        return Err(bad(cmd, "select a shape"));
    }
    s.edit(|doc, sel| {
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad(cmd, "no slide"))?;
        for id in &ids {
            if let Some(sh) = deckcraft_model::find_shape_mut(list, *id) {
                let mut e = sh.effects.clone().unwrap_or_default();
                f(&mut e);
                sh.effects = Some(e);
            }
        }
        Ok(json!({"ids": ids}))
    })
}

fn set_blur(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "format.blur";
    let max = deckcraft_model::style::MAX_BLUR;
    let (layer, background) = (effect_amount(p, "layer", CMD, max)?, effect_amount(p, "background", CMD, max)?);
    if layer.is_none() && background.is_none() {
        return Err(bad(CMD, "give `layer` or `background`"));
    }
    edit_effects(s, p, CMD, |e| {
        if let Some(v) = layer {
            e.layer_blur = v;
        }
        if let Some(v) = background {
            e.background_blur = v;
        }
    })
}

fn set_noise(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "format.noise";
    let amount = effect_amount(p, "amount", CMD, 1.0)?.ok_or_else(|| bad(CMD, "give `amount` from 0 to 1"))?;
    edit_effects(s, p, CMD, |e| e.noise = amount)
}

/// Why a group's members can't be entered.
pub(crate) const NOT_IN_PLACE: &str = "a rotated or flipped group's members can't be edited in place; ungroup it first";

fn select_children(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let ids = ids_param(p, "ids").unwrap_or_else(|| st.selection.shapes.clone());
    let mut out = vec![];
    let mut groups = 0;
    for id in &ids {
        let Some(sh) = st.shape(*id) else { return Err(bad("edit.selectChildren", format!("no object {id}"))) };
        let ShapeKind::Group { children, .. } = &sh.kind else { continue };
        groups += 1;
        let members: Vec<ShapeId> = children.iter().filter(|c| !c.hidden).map(|c| c.id).collect();
        if members.iter().any(|m| !canvas::editable_in_place(st.shapes(), *m)) {
            return Err(bad("edit.selectChildren", NOT_IN_PLACE));
        }
        out.extend(members);
    }
    if groups == 0 {
        return Err(bad("edit.selectChildren", "select a group"));
    }
    s.select(|_, sel| {
        sel.text = None;
        sel.cells = None;
        sel.shapes = out;
    })?;
    Ok(json!({"selected": s.doc()?.selection.shapes}))
}

fn select_parent(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let ids = ids_param(p, "ids").unwrap_or_else(|| st.selection.shapes.clone());
    let mut out: Vec<ShapeId> = vec![];
    for id in ids {
        let Some(chain) = canvas::ancestors(st.shapes(), id) else { return Err(bad("edit.selectParent", format!("no object {id}"))) };
        let pick = chain.last().copied().unwrap_or(id);
        if !out.contains(&pick) {
            out.push(pick);
        }
    }
    // A group and one of its own members can't both stay selected.
    let shapes = st.shapes();
    let keep: Vec<ShapeId> =
        out.iter().copied().filter(|id| canvas::ancestors(shapes, *id).is_none_or(|chain| !chain.iter().any(|a| out.contains(a)))).collect();
    s.select(|_, sel| {
        sel.text = None;
        sel.cells = None;
        sel.shapes = keep;
    })?;
    Ok(json!({"selected": s.doc()?.selection.shapes}))
}

/// Whether any selected object is inside a group.
pub(crate) fn selection_in_group(st: &crate::DocState) -> bool {
    st.selection.shapes.iter().any(|id| canvas::ancestors(st.shapes(), *id).is_some_and(|c| !c.is_empty()))
}

/// Whether the selection is only groups (so Enter selects their members).
pub(crate) fn selection_is_groups(st: &crate::DocState) -> bool {
    !st.selection.shapes.is_empty() && st.selection.shapes.iter().all(|id| st.shape(*id).is_some_and(|s| matches!(s.kind, ShapeKind::Group { .. })))
}

#[cfg(test)]
#[path = "canvas_tests.rs"]
mod session_tests;

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn apart_boxes_measure_their_gaps() {
        let g = measure(Rect::new(0.0, 0.0, 10.0, 10.0), Rect::new(30.0, 5.0, 40.0, 20.0));
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].axis, 'x');
        assert_eq!(g[0].value, 20.0);
        // The line runs through the common vertical span (5..10).
        assert_eq!(g[0].from, (10.0, 7.5));
        let g = measure(Rect::new(0.0, 0.0, 10.0, 10.0), Rect::new(20.0, 30.0, 25.0, 35.0));
        assert_eq!(g.iter().map(|g| (g.axis, g.value)).collect::<Vec<_>>(), vec![('x', 10.0), ('y', 20.0)]);
    }

    #[test]
    fn a_box_inside_another_measures_to_each_edge() {
        let g = measure(Rect::new(10.0, 20.0, 30.0, 40.0), Rect::new(0.0, 0.0, 100.0, 50.0));
        let v: Vec<f64> = g.iter().map(|g| g.value).collect();
        assert_eq!(v, vec![10.0, 70.0, 20.0, 10.0]);
        // Either way round.
        assert_eq!(measure(Rect::new(0.0, 0.0, 100.0, 50.0), Rect::new(10.0, 20.0, 30.0, 40.0)).len(), 4);
    }

    #[test]
    fn overlapping_boxes_measure_edge_to_edge_and_hostile_boxes_give_nothing() {
        let g = measure(Rect::new(0.0, 0.0, 20.0, 20.0), Rect::new(5.0, 0.0, 30.0, 20.0));
        assert_eq!(g.iter().map(|g| g.value).collect::<Vec<_>>(), vec![5.0, 10.0]);
        assert!(measure(Rect::new(f64::NAN, 0.0, 1.0, 1.0), Rect::new(0.0, 0.0, 1.0, 1.0)).is_empty());
        assert!(measure(Rect::new(0.0, 0.0, 1.0, 1.0), Rect::new(0.0, 0.0, 1.0, 1.0)).is_empty());
    }
}
