//! SlideCraft's theme kit, layout library and Tidy up (Phase 4.4).
//!
//! - `design.kit`: the deck's theme as variables with Light and Dark modes
//!   (`deckcraft_model::design_kit`), optionally from a built-in theme.
//! - `slide.fromLibrary`: a new slide from one of the library's 34 layouts (`crate::library`).
//! - `arrange.tidy`: line a messy selection up in an auto layout grid, in reading order, with
//!   even gaps.

use std::sync::Arc;

use deckcraft_geom::Xfrm;
use deckcraft_model::{ShapeId, ShapeKind};
use serde_json::{Value, json};

use super::{CommandSpec, NO_ID_SPACE, bad, cmd, f64_param, has_doc, has_two, str_param, usize_param, xfrm_of};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "design.kit",
            "Theme Variables",
            ["Design", "Variables"],
            None,
            "{theme?: built-in theme name (else the deck's own theme), mode?: Light|Dark} → {collection, variables}: the theme as variables with a Light and a Dark mode (colours mapped to its 12 slots, both fonts, spacing, radius and type scales), as the layout library uses",
            has_doc,
            kit
        ),
        cmd!(
            "slide.fromLibrary",
            "New Slide from Library",
            ["Home", "Slides"],
            None,
            "{layout: id or name (see slide.library), at?: index (else after the current slide)} → {index, id}: a slide built from frames, auto layout, the theme's variables and library components",
            has_doc,
            from_library
        ),
        cmd!(query "slide.library", "Layout Library", [], None, "{} → {layouts: [{id, name}]}", always_ok, library_list),
        cmd!(
            "arrange.tidy",
            "Tidy Up",
            ["Arrange", "Align"],
            Some("Ctrl+Alt+T"),
            "{ids?: two or more objects (else the selection), columns?: n (else as they stand), gap?: pt (else the typical gap between them)} → {id: the frame, columns, rows, gap}: line them up in reading order in an auto layout grid with even gaps",
            has_two,
            tidy
        ),
    ]
}

fn always_ok(_: &Session) -> std::result::Result<(), String> {
    Ok(())
}

fn kit(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "design.kit";
    let theme = match str_param(p, "theme") {
        Some(name) => Some(
            deckcraft_model::theme::builtin_themes()
                .into_iter()
                .find(|t| t.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| bad(C, format!("no theme `{name}`")))?,
        ),
        None => None,
    };
    let mode = str_param(p, "mode").map(str::to_string);
    if mode.as_deref().is_some_and(|m| !m.eq_ignore_ascii_case("light") && !m.eq_ignore_ascii_case("dark")) {
        return Err(bad(C, "`mode` must be Light or Dark"));
    }
    s.edit(|doc, sel| {
        if let Some(t) = &theme {
            let mi = super::design::master_index(doc, sel);
            let m = doc.masters.get_mut(mi).ok_or_else(|| bad(C, "no master"))?;
            let m = Arc::make_mut(m);
            m.theme = t.clone();
            m.name = t.name.clone();
            m.color_map = vec![];
        }
        let base = doc.masters.first().map(|m| m.theme.clone()).unwrap_or_default();
        crate::library::refresh_kit(doc, theme.as_ref().unwrap_or(&base)).ok_or_else(|| bad(C, NO_ID_SPACE))?;
        let mut ds = doc.design_system().cloned().unwrap_or_default();
        let coll = ds.kit().cloned().ok_or_else(|| bad(C, "no kit"))?;
        if let Some(m) = &mode {
            let mid = coll.mode_named(m).map(|m| m.id).ok_or_else(|| bad(C, "no such mode"))?;
            ds.modes.retain(|(c, _)| *c != coll.id);
            if coll.modes.first().map(|f| f.id) != Some(mid) {
                ds.modes.push((coll.id, mid));
            }
            doc.design = Some(Arc::new(ds.clone()));
        }
        let n = ds.variables.iter().filter(|v| v.collection == coll.id).count();
        Ok(json!({"collection": coll.id, "variables": n}))
    })
}

fn library_list(_s: &mut Session, _p: &Value) -> Result<Value> {
    Ok(json!({"layouts": crate::library::layouts().iter().map(|l| json!({"id": l.id, "name": l.name})).collect::<Vec<_>>()}))
}

fn from_library(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "slide.fromLibrary";
    let name = str_param(p, "layout").ok_or_else(|| bad(C, "missing `layout` (see slide.library)"))?;
    let def = crate::library::find(name).ok_or_else(|| bad(C, format!("no library layout `{name}` (see slide.library)")))?;
    let at = match p.get("at") {
        None | Some(Value::Null) => None,
        Some(_) => Some(usize_param(p, "at").ok_or_else(|| bad(C, "`at` is a slide index"))?),
    };
    s.edit(|doc, sel| {
        let slide = crate::library::new_slide(doc, def).ok_or_else(|| bad(C, NO_ID_SPACE))?;
        let id = slide.id;
        let n = doc.slides.len();
        let index = at.unwrap_or(if n == 0 { 0 } else { sel.slide.saturating_add(1) }).min(n);
        doc.slides.insert(index, Arc::new(slide));
        sel.slide = index;
        sel.shapes.clear();
        sel.text = None;
        sel.target = crate::Target::Slides;
        Ok(json!({"index": index, "id": id.0}))
    })
}

/// Rows of boxes as a reader goes through them: grouped by their vertical centres (a box joins
/// the row when its centre is within half the tallest box of the row's), each row left to right.
fn reading_rows(items: &[(ShapeId, Xfrm)]) -> Vec<Vec<(ShapeId, Xfrm)>> {
    let mut sorted: Vec<(ShapeId, Xfrm)> = items.to_vec();
    sorted.sort_by(|a, b| (a.1.y + a.1.h / 2.0).total_cmp(&(b.1.y + b.1.h / 2.0)));
    let mut rows: Vec<Vec<(ShapeId, Xfrm)>> = vec![];
    for it in sorted {
        let cy = it.1.y + it.1.h / 2.0;
        match rows.last_mut() {
            Some(row)
                if {
                    let mean = row.iter().map(|r| r.1.y + r.1.h / 2.0).sum::<f64>() / row.len().max(1) as f64;
                    let tall = row.iter().map(|r| r.1.h).fold(it.1.h, f64::max);
                    (cy - mean).abs() <= tall / 2.0
                } =>
            {
                row.push(it)
            }
            _ => rows.push(vec![it]),
        }
    }
    for r in &mut rows {
        r.sort_by(|a, b| a.1.x.total_cmp(&b.1.x));
    }
    rows
}

fn median(mut v: Vec<f64>) -> Option<f64> {
    v.retain(|x| x.is_finite());
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    v.get(v.len() / 2).copied()
}

fn tidy(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "arrange.tidy";
    let mut ids = super::targets(s, p)?;
    let mut seen = std::collections::HashSet::new();
    ids.retain(|i| seen.insert(*i));
    if ids.len() < 2 {
        return Err(bad(C, "select two or more objects"));
    }
    let (items, parent) = {
        let st = s.doc()?;
        let list = super::current_shapes(&st.doc, &st.selection).ok_or_else(|| bad(C, "no slide"))?;
        let mut parent = None;
        let mut items = vec![];
        for id in &ids {
            let sh = deckcraft_model::find_shape(list, *id).ok_or_else(|| bad(C, format!("no shape {id}")))?;
            let par = deckcraft_model::parent_of(list, *id).flatten();
            if items.is_empty() {
                parent = par;
            } else if par != parent {
                return Err(bad(C, "the objects must be in the same group or frame"));
            }
            let x = xfrm_of(&st.doc, &st.selection, sh);
            if x.rot.abs() > 1e-9 || ![x.x, x.y, x.w, x.h].iter().all(|v| v.is_finite()) {
                return Err(bad(C, format!("shape {id} is rotated; Tidy Up lines up unrotated objects")));
            }
            items.push((*id, x));
        }
        (items, parent)
    };
    if let Some(g) = parent
        && s.doc()?.shape(g).is_some_and(|f| f.auto_layout.is_some())
    {
        return Err(bad(C, "they are already in an auto layout frame"));
    }
    let rows = reading_rows(&items);
    let columns = match p.get("columns") {
        None | Some(Value::Null) => rows.iter().map(Vec::len).max().unwrap_or(1),
        Some(_) => usize_param(p, "columns").filter(|n| (1..=64).contains(n)).ok_or_else(|| bad(C, "`columns` is 1 to 64"))?,
    };
    let order: Vec<ShapeId> = rows.iter().flatten().map(|(i, _)| *i).collect();
    let gap = match f64_param(p, "gap") {
        Some(g) if (0.0..=1000.0).contains(&g) => g,
        Some(_) => return Err(bad(C, "`gap` is 0 to 1000 pt")),
        None => {
            let mut gaps = vec![];
            for row in &rows {
                for w in row.windows(2) {
                    if let [a, b] = w {
                        gaps.push(b.1.x - (a.1.x + a.1.w));
                    }
                }
            }
            for w in rows.windows(2) {
                if let [a, b] = w {
                    let bottom = a.iter().map(|r| r.1.y + r.1.h).fold(f64::MIN, f64::max);
                    let top = b.iter().map(|r| r.1.y).fold(f64::MAX, f64::min);
                    gaps.push(top - bottom);
                }
            }
            median(gaps).map_or(16.0, |g| g.clamp(4.0, 200.0).round())
        }
    };
    let nrows = order.len().div_ceil(columns.max(1));
    // Frame them (one undo step with the layout: nested commands), put the members in reading
    // order, then lay them out.
    let r = s.execute("frame.create", &json!({"ids": order.iter().map(|i| i.0).collect::<Vec<_>>(), "padding": 0}))?;
    let frame = r.get("id").and_then(Value::as_u64).and_then(|v| u32::try_from(v).ok()).map(ShapeId).ok_or_else(|| bad(C, "no frame"))?;
    s.edit(|doc, sel| {
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad(C, "no slide"))?;
        let f = deckcraft_model::find_shape_mut(list, frame).ok_or_else(|| bad(C, "no frame"))?;
        f.name = "Tidy".into();
        if let ShapeKind::Group { children, .. } = &mut f.kind {
            children.sort_by_key(|c| order.iter().position(|o| *o == c.id).unwrap_or(usize::MAX));
        }
        Ok(())
    })?;
    let (flow, cols): (&str, Vec<Value>) = if nrows <= 1 {
        ("horizontal", vec![])
    } else if columns == 1 {
        ("vertical", vec![])
    } else {
        ("grid", vec![json!("hug"); columns])
    };
    let mut lp =
        json!({"ids": [frame.0], "flow": flow, "gap": gap, "crossGap": gap, "padding": 0, "width": "hug", "height": "hug", "crossAlign": "center"});
    if let Some(o) = lp.as_object_mut()
        && !cols.is_empty()
    {
        o.insert("columns".into(), Value::Array(cols));
    }
    s.execute("layout.set", &lp)?;
    s.select(|_, sel| sel.shapes = vec![frame])?;
    Ok(json!({"id": frame.0, "columns": if flow == "vertical" { 1 } else { columns.min(order.len()) }, "rows": nrows, "gap": gap}))
}

#[cfg(test)]
#[path = "library_tests.rs"]
mod tests;
