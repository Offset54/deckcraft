//! Editable boolean shapes (SlideCraft, Phase 3, after FigCraft): a group whose members are the
//! operands of a union, subtraction, intersection or exclusion, drawn as the one result shape.
//! The operands stay: select them inside the group (`edit.selectChildren`, a double-click) and
//! move, resize or reshape them, and the result follows. `boolean.flatten` makes the result a
//! plain shape, like Merge Shapes.
//!
//! The result is the group's own geometry (`Shape::geom`), recomputed after every edit by the
//! canvas pass (`crate::canvas`) from the operands' outlines with the same polygon overlay Merge
//! Shapes uses (`i_overlay`). It is drawn with the group's own fill, outline and effects.

use deckcraft_geom::Xfrm;
use deckcraft_model::canvas::BoolOp;
use deckcraft_model::{CustomPath, Geom, Shape, ShapeId, ShapeKind};
use serde_json::{Value, json};

use super::merge::{Area, merge_areas};
use super::{CommandSpec, NO_ID_SPACE, bad, cmd, has_selection, has_two, ids_param, targets, xfrm_of};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "boolean.create",
            "Boolean Group",
            ["Shape Format", "Insert Shapes", "Boolean"],
            None,
            "{op: union|subtract|intersect|exclude, ids?: two or more objects on the slide (else the selection; in order: subtract takes the rest from the first)} → {id}: an editable boolean shape; its operands stay and can be edited inside it",
            has_two,
            create
        ),
        cmd!(
            "boolean.setOp",
            "Boolean Operation",
            ["Shape Format", "Insert Shapes", "Boolean"],
            None,
            "{op: union|subtract|intersect|exclude, ids?: boolean shapes (else the selection)} → {ids}",
            has_selection,
            set_op
        ),
        cmd!(
            "boolean.flatten",
            "Flatten",
            ["Shape Format", "Insert Shapes", "Boolean"],
            None,
            "{ids?: boolean shapes (else the selection)} → {ids}: replace each with a plain shape of its result; the operands go",
            has_selection,
            flatten
        ),
    ]
}

fn op_param(p: &Value, cmd: &str) -> Result<BoolOp> {
    let v = p.get("op").cloned().ok_or_else(|| bad(cmd, "give `op`: union, subtract, intersect or exclude"))?;
    serde_json::from_value(v).map_err(|_| bad(cmd, "`op` must be union, subtract, intersect or exclude"))
}

/// The filled outline of `sh` (and of its members, for a group) in slide points.
pub(crate) fn outline(sh: &Shape, depth: usize, out: &mut Area) {
    if depth > deckcraft_model::canvas::MAX_DEPTH || sh.hidden {
        return;
    }
    match &sh.kind {
        // A nested boolean shape counts as its result; another group as all its members.
        ShapeKind::Group { children, .. } if sh.boolean.is_none() => {
            for c in children {
                outline(c, depth + 1, out);
            }
        }
        _ => {
            let Some(x) = sh.xfrm else { return };
            let geo = deckcraft_render::shape_geometry(sh, x.w, x.h);
            let a = x.affine();
            for sp in geo.paths.iter().filter(|p| p.fill != deckcraft_geom::preset::FillMode::None) {
                super::merge::flatten(&sp.path, a, out);
            }
        }
    }
}

/// The result of boolean group `sh` as its own geometry, in its box's space. `None` when it
/// can't be computed (no box, operands without area).
pub(crate) fn result_geom(sh: &Shape) -> Option<Geom> {
    let op = sh.boolean?;
    let g = sh.xfrm?;
    let ShapeKind::Group { children, .. } = &sh.kind else { return None };
    let areas: Vec<Area> = children
        .iter()
        .map(|c| {
            let mut a = Area::new();
            outline(c, 1, &mut a);
            a
        })
        .collect();
    let word = match op {
        BoolOp::Union => "union",
        BoolOp::Subtract => "subtract",
        BoolOp::Intersect => "intersect",
        BoolOp::Exclude => "combine",
    };
    let result = merge_areas(word, &areas).into_iter().next().unwrap_or_default();
    Some(area_in_box(&result, &g))
}

/// `area` as a custom path relative to box `g`.
fn area_in_box(area: &Area, g: &Xfrm) -> Geom {
    let f = |v: f64| {
        let s = format!("{v:.2}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        if s.is_empty() || s == "-0" { "0".to_string() } else { s.to_string() }
    };
    let mut d = String::new();
    for c in area {
        for (i, p) in c.iter().enumerate() {
            d += &format!("{}{} {} ", if i == 0 { "M " } else { "L " }, f(p[0] - g.x), f(p[1] - g.y));
        }
        d += "Z ";
    }
    let (w, h) = (g.w.max(0.01), g.h.max(0.01));
    Geom::Custom { paths: vec![CustomPath { w, h, d: d.trim_end().to_string(), ..Default::default() }] }
}

fn create(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "boolean.create";
    let op = op_param(p, CMD)?;
    let ids = targets(s, p)?;
    s.edit(|doc, sel| {
        let current = super::current_shapes(doc, sel).cloned().unwrap_or_default();
        // Operands in the order named (subtract keeps the first), not slide order.
        let mut members: Vec<Shape> = ids.iter().filter_map(|id| current.iter().find(|x| x.id == *id).cloned()).collect();
        if members.len() < 2 || members.len() != ids.len() {
            return Err(bad(CMD, "select two or more objects at the top level of the slide"));
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
        let b = super::arrange::union_bounds(&xs).ok_or_else(|| bad(CMD, "nothing to combine"))?;
        let gx = Xfrm::new(b.x0, b.y0, b.width(), b.height());
        let first = members.first().cloned().unwrap_or_default();
        let id = ShapeId(doc.alloc_id().ok_or_else(|| bad(CMD, NO_ID_SPACE))?);
        let mut g = Shape {
            id,
            name: format!("{} {}", label(op), current.iter().filter(|x| x.boolean.is_some()).count() + 1),
            xfrm: Some(gx),
            kind: ShapeKind::Group { children: members, child: gx },
            // The result takes the first operand's look, as Merge Shapes does.
            fill: first.fill.clone(),
            line: first.line.clone(),
            effects: first.effects.clone(),
            style: first.style.clone(),
            boolean: Some(op),
            ..Default::default()
        };
        if let Some(geom) = result_geom(&g) {
            g.geom = geom;
        }
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad(CMD, "no slide"))?;
        let at = list.iter().rposition(|x| ids.contains(&x.id)).map(|i| i + 1).unwrap_or(list.len());
        list.insert(at, g);
        list.retain(|x| !ids.contains(&x.id));
        sel.shapes = vec![id];
        sel.text = None;
        sel.cells = None;
        Ok(json!({"id": id}))
    })
}

fn label(op: BoolOp) -> &'static str {
    match op {
        BoolOp::Union => "Union",
        BoolOp::Subtract => "Subtract",
        BoolOp::Intersect => "Intersect",
        BoolOp::Exclude => "Exclude",
    }
}

fn booleans(s: &Session, p: &Value, cmd: &str) -> Result<Vec<ShapeId>> {
    let st = s.doc()?;
    let ids = ids_param(p, "ids").unwrap_or_else(|| st.selection.shapes.clone());
    if ids.is_empty() {
        return Err(bad(cmd, "select a boolean shape"));
    }
    if let Some(id) = ids.iter().find(|i| st.shape(**i).is_none_or(|sh| sh.boolean.is_none())) {
        return Err(bad(cmd, format!("object {id} is not a boolean shape")));
    }
    Ok(ids)
}

fn set_op(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "boolean.setOp";
    let op = op_param(p, CMD)?;
    let ids = booleans(s, p, CMD)?;
    s.edit(|doc, sel| {
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad(CMD, "no slide"))?;
        for id in &ids {
            if let Some(sh) = deckcraft_model::find_shape_mut(list, *id) {
                sh.boolean = Some(op);
            }
        }
        Ok(json!({"ids": ids}))
    })
}

fn flatten(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "boolean.flatten";
    let ids = booleans(s, p, CMD)?;
    s.edit(|doc, sel| {
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad(CMD, "no slide"))?;
        for id in &ids {
            if let Some(sh) = deckcraft_model::find_shape_mut(list, *id) {
                if let Some(geom) = result_geom(sh) {
                    sh.geom = geom;
                }
                sh.kind = ShapeKind::Shape;
                sh.boolean = None;
            }
        }
        Ok(json!({"ids": ids}))
    })
}

/// Keep every boolean group's result in step with its operands (the canvas pass).
pub(crate) fn settle(list: &mut [Shape], depth: usize) {
    if depth > deckcraft_model::canvas::MAX_DEPTH {
        return;
    }
    for sh in list.iter_mut() {
        if let ShapeKind::Group { children, .. } = &mut sh.kind {
            settle(children, depth + 1);
        }
        if sh.boolean.is_some()
            && let Some(geom) = result_geom(sh)
            && geom != sh.geom
        {
            sh.geom = geom;
        }
    }
}

#[cfg(test)]
#[path = "boolean_tests.rs"]
mod tests;
