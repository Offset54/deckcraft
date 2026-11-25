//! SlideCraft paint, effect and grid styles (Phase 4.2), and paragraph and character styles that
//! follow variables. The model is `deckcraft_model::design_styles`; the engine's design pass
//! (`crate::design_system`) writes a changed style into the shapes that use it and forgets the
//! link of a shape whose look was changed some other way.

use std::sync::Arc;

use deckcraft_model::design_styles::{DesignStyle, LayoutGrid, MAX_STYLES, StyleKind, StyleRefs};
use deckcraft_model::variables::{BindField, Binding, DesignSystem, MAX_NAME, MAX_STRING, VarKind, var_color};
use deckcraft_model::{Fill, Presentation};
use serde_json::{Value, json};

use super::{CommandSpec, NO_ID_SPACE, bad, cmd, color_param, f64_param, has_doc, str_param};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "designStyle.create",
            "Create Style",
            ["Design", "Styles"],
            None,
            "{type: paint|effect|grid, name, from?: shape id (a paint style takes its fill, an effect style its effects; else the selection's first shape), color?: color (a paint style of one colour; {variable: id} follows a variable), columns?: n, rows?: n, gutter?: pt, margin?: pt (grid styles), description?} → {id}",
            has_doc,
            create
        ),
        cmd!(
            "designStyle.update",
            "Edit Style",
            ["Design", "Styles"],
            None,
            "{style: id|name, name?, from?: shape id, color?: color, columns?: n, rows?: n, gutter?: pt, margin?: pt, description?} → {id}: every shape that uses the style follows",
            has_doc,
            update
        ),
        cmd!(
            "designStyle.apply",
            "Apply Style",
            ["Design", "Styles"],
            None,
            "{ids?, style: id|name|null, as?: fill|line (paint styles; default fill) | style: grid style id|name|null, slides?: [index]|all (a grid style on slides, else on the selected frames)} → {ids}: null detaches (the look stays)",
            has_doc,
            apply
        ),
        cmd!("designStyle.delete", "Delete Style", ["Design", "Styles"], None, "{style: id|name} (shapes keep its look)", has_doc, delete),
        cmd!(
            query "designStyle.list",
            "Styles",
            [],
            None,
            "{} → {styles: [{id, name, type, users}]}",
            has_doc,
            list
        ),
        cmd!(
            "style.bind",
            "Bind Text Style to Variable",
            ["Design", "Styles"],
            None,
            "{id: text style id, field: fontSize|fontFamily|color, variable: id|name|null} → {id}: the paragraph or character style follows the variable (null detaches)",
            has_doc,
            bind_text_style
        ),
    ]
}

fn design(doc: &mut Presentation) -> &mut DesignSystem {
    Arc::make_mut(doc.design.get_or_insert_with(Default::default))
}

fn style_id(doc: &Presentation, v: Option<&Value>, cmd: &str) -> Result<u32> {
    let ds = doc.design_system().ok_or_else(|| bad(cmd, "the deck has no styles"))?;
    let found = match v {
        Some(Value::Number(n)) => n.as_u64().and_then(|n| u32::try_from(n).ok()).and_then(|n| ds.styles.iter().find(|s| s.id == n)),
        Some(Value::String(s)) => ds.styles.iter().find(|x| x.name.eq_ignore_ascii_case(s)),
        _ => return Err(bad(cmd, "missing `style`")),
    };
    found.map(|s| s.id).ok_or_else(|| bad(cmd, format!("no style {}", v.map(Value::to_string).unwrap_or_default())))
}

fn grid_of(p: &Value, base: LayoutGrid) -> LayoutGrid {
    let n = |k: &str, d: u32| p.get(k).and_then(Value::as_u64).map_or(d, |v| u32::try_from(v).unwrap_or(u32::MAX));
    LayoutGrid {
        columns: n("columns", base.columns),
        rows: n("rows", base.rows),
        gutter: f64_param(p, "gutter").unwrap_or(base.gutter),
        margin: f64_param(p, "margin").unwrap_or(base.margin),
    }
    .clean()
}

/// The look a style takes from params: `color`, else shape `from` (else the selection's first).
fn look(s: &Session, p: &Value, kind: &str, cmd: &str) -> Result<Option<StyleKind>> {
    if kind == "grid" {
        return Ok(None);
    }
    if let Some(c) = color_param(p, "color") {
        if kind != "paint" {
            return Err(bad(cmd, "`color` is for paint styles"));
        }
        return Ok(Some(StyleKind::Paint { fill: Fill::solid(c) }));
    }
    if p.get("color").is_some() {
        return Err(bad(cmd, "`color` is not a colour"));
    }
    let st = s.doc()?;
    let from = match super::id_param(p, "from", cmd)? {
        Some(id) => Some(id),
        None => st.selection.shapes.first().copied(),
    };
    let Some(id) = from else { return Ok(None) };
    let sh = st.shape(id).ok_or_else(|| bad(cmd, format!("no shape {id}")))?;
    Ok(Some(match kind {
        "paint" => StyleKind::Paint { fill: sh.fill.clone().ok_or_else(|| bad(cmd, format!("shape {id} has no fill of its own")))? },
        _ => StyleKind::Effect { effects: sh.effects.clone().ok_or_else(|| bad(cmd, format!("shape {id} has no effects of its own")))? },
    }))
}

fn name_of(p: &Value, cmd: &str) -> Result<Option<String>> {
    match p.get("name") {
        None => Ok(None),
        Some(v) => {
            let n = v.as_str().map(str::trim).filter(|n| !n.is_empty()).ok_or_else(|| bad(cmd, "`name` must be text"))?;
            if n.chars().count() > MAX_NAME {
                return Err(bad(cmd, format!("`name` is longer than {MAX_NAME} characters")));
            }
            Ok(Some(n.to_string()))
        }
    }
}

fn create(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "designStyle.create";
    let kind =
        str_param(p, "type").filter(|k| matches!(*k, "paint" | "effect" | "grid")).ok_or_else(|| bad(C, "`type` must be paint, effect or grid"))?;
    let name = name_of(p, C)?.ok_or_else(|| bad(C, "missing `name`"))?;
    let look = match kind {
        "grid" => StyleKind::Grid { grid: grid_of(p, LayoutGrid { columns: 12, rows: 0, gutter: 16.0, margin: 48.0 }) },
        _ => look(s, p, kind, C)?.ok_or_else(|| bad(C, "give `color` or `from`, or select a shape"))?,
    };
    let description: String = str_param(p, "description").unwrap_or_default().chars().take(MAX_STRING).collect();
    s.edit(|doc, _| {
        let ds = doc.design_system();
        if ds.is_some_and(|d| d.styles.len() >= MAX_STYLES) {
            return Err(bad(C, format!("a deck holds at most {MAX_STYLES} styles")));
        }
        if ds.is_some_and(|d| d.styles.iter().any(|x| x.name.eq_ignore_ascii_case(&name))) {
            return Err(bad(C, format!("there is already a style `{name}`")));
        }
        let id = doc.alloc_id().ok_or_else(|| bad(C, NO_ID_SPACE))?;
        design(doc).styles.push(DesignStyle { id, name, kind: look, description });
        Ok(json!({"id": id}))
    })
}

fn update(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "designStyle.update";
    let id = style_id(&s.doc()?.doc, p.get("style"), C)?;
    let cur = s.doc()?.doc.design_system().and_then(|d| d.styles.iter().find(|x| x.id == id)).cloned().ok_or_else(|| bad(C, "no style"))?;
    let mut next = cur.clone();
    if let Some(n) = name_of(p, C)? {
        if s.doc()?.doc.design_system().is_some_and(|d| d.styles.iter().any(|x| x.id != id && x.name.eq_ignore_ascii_case(&n))) {
            return Err(bad(C, format!("there is already a style `{n}`")));
        }
        next.name = n;
    }
    if let Some(d) = str_param(p, "description") {
        next.description = d.chars().take(MAX_STRING).collect();
    }
    match &cur.kind {
        StyleKind::Grid { grid } => next.kind = StyleKind::Grid { grid: grid_of(p, *grid) },
        k => {
            if p.get("color").is_some() || p.get("from").is_some() {
                next.kind = look(s, p, k.name(), C)?.ok_or_else(|| bad(C, "give `color` or `from`"))?;
            }
        }
    }
    if next == cur {
        return Ok(json!({"id": id}));
    }
    s.edit(|doc, _| {
        if let Some(x) = design(doc).styles.iter_mut().find(|x| x.id == id) {
            *x = next;
        }
        Ok(json!({"id": id}))
    })
}

fn apply(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "designStyle.apply";
    let doc = s.doc()?.doc.clone();
    let style = match p.get("style") {
        Some(Value::Null) => None,
        v => Some(style_id(&doc, v, C)?),
    };
    let kind = style.and_then(|id| doc.design_system().and_then(|d| d.styles.iter().find(|x| x.id == id)).map(|x| x.kind.clone()));
    let is_grid = matches!(kind, Some(StyleKind::Grid { .. })) || (style.is_none() && p.get("slides").is_some());
    if let Some(sl) = p.get("slides") {
        if !is_grid {
            return Err(bad(C, "only a grid style goes on slides"));
        }
        let n = doc.slides.len();
        let idx: Vec<usize> = if sl.as_str() == Some("all") {
            (0..n).collect()
        } else {
            sl.as_array()
                .ok_or_else(|| bad(C, "`slides` is a list of indices or \"all\""))?
                .iter()
                .map(|i| i.as_u64().and_then(|i| usize::try_from(i).ok()).filter(|i| *i < n))
                .collect::<Option<_>>()
                .ok_or_else(|| bad(C, "`slides` has an index out of range"))?
        };
        return s.edit(|doc, _| {
            for i in &idx {
                if let Some(slide) = doc.slides.get_mut(*i)
                    && slide.grid_style != style
                {
                    Arc::make_mut(slide).grid_style = style;
                }
            }
            Ok(json!({"slides": idx}))
        });
    }
    let target = str_param(p, "as").unwrap_or("fill");
    if !matches!(target, "fill" | "line") {
        return Err(bad(C, "`as` must be fill or line"));
    }
    let ids = super::targets(s, p)?;
    if ids.is_empty() {
        return Err(bad(C, "select shapes, or give `slides` for a grid style"));
    }
    let styles = doc.design_system().map(|d| d.styles.clone()).unwrap_or_default();
    super::edit_shapes(s, p, C, |sh| {
        let mut r = sh.styles.unwrap_or_default();
        match (&kind, style) {
            (Some(StyleKind::Paint { .. }), id) if target == "line" => r.line = id,
            (Some(StyleKind::Paint { .. }), id) => r.fill = id,
            (Some(StyleKind::Effect { .. }), id) => r.effect = id,
            (Some(StyleKind::Grid { .. }), id) => {
                if sh.frame.is_none() {
                    return Err(bad(C, format!("a grid style goes on a frame or a slide; shape {} is not a frame", sh.id)));
                }
                r.grid = id;
            }
            (None, None) => match target {
                "line" if p.get("as").is_some() => r.line = None,
                _ => r = StyleRefs::default(),
            },
            (None, Some(_)) => {}
        }
        sh.styles = (!r.is_empty()).then_some(r);
        deckcraft_model::design_styles::apply_styles(&styles, std::slice::from_mut(sh));
        Ok(())
    })?;
    Ok(json!({"ids": ids}))
}

fn delete(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "designStyle.delete";
    s.edit(|doc, _| {
        let id = style_id(doc, p.get("style"), C)?;
        design(doc).styles.retain(|x| x.id != id);
        for slide in doc.slides.iter_mut() {
            if slide.grid_style == Some(id) {
                Arc::make_mut(slide).grid_style = None;
            }
        }
        // The design pass drops the shapes' links and keeps their look.
        if doc.design_system().is_some_and(DesignSystem::is_empty) {
            doc.design = None;
            // No design pass then: drop the links here.
            for slide in doc.slides.iter_mut() {
                let mut shapes = slide.shapes.clone();
                if deckcraft_model::design_styles::apply_styles(&[], &mut shapes) {
                    Arc::make_mut(slide).shapes = shapes;
                }
            }
        }
        Ok(json!({"deleted": id}))
    })
}

fn list(s: &mut Session, _p: &Value) -> Result<Value> {
    let doc = &s.doc()?.doc;
    let Some(ds) = doc.design_system() else { return Ok(json!({"styles": []})) };
    let mut users: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    for slide in &doc.slides {
        if let Some(g) = slide.grid_style {
            *users.entry(g).or_default() += 1;
        }
        deckcraft_model::walk(&slide.shapes, &mut |sh, _| {
            if let Some(r) = sh.styles {
                for id in [r.fill, r.line, r.effect, r.grid].into_iter().flatten() {
                    *users.entry(id).or_default() += 1;
                }
            }
        });
    }
    let styles: Vec<Value> = ds
        .styles
        .iter()
        .map(|x| {
            let mut o = json!({"id": x.id, "name": x.name, "type": x.kind.name(), "users": users.get(&x.id).copied().unwrap_or(0)});
            if let (StyleKind::Grid { grid }, Some(m)) = (&x.kind, o.as_object_mut()) {
                m.insert("grid".into(), json!({"columns": grid.columns, "rows": grid.rows, "gutter": grid.gutter, "margin": grid.margin}));
            }
            o
        })
        .collect();
    Ok(json!({"styles": styles}))
}

fn bind_text_style(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "style.bind";
    let sid = str_param(p, "id").ok_or_else(|| bad(C, "missing `id`"))?.to_string();
    let field = str_param(p, "field")
        .filter(|f| matches!(*f, "fontSize" | "fontFamily" | "color"))
        .ok_or_else(|| bad(C, "`field` must be fontSize, fontFamily or color"))?;
    let doc = s.doc()?.doc.clone();
    let mut style = doc.text_style(&sid).ok_or_else(|| bad(C, format!("no text style `{sid}`")))?;
    let var = match p.get("variable") {
        Some(Value::Null) => None,
        v => {
            let ds = doc.design_system().ok_or_else(|| bad(C, "the deck has no variables"))?;
            let found = match v {
                Some(Value::Number(n)) => n.as_u64().and_then(|n| u32::try_from(n).ok()).and_then(|n| ds.variable(n)),
                Some(Value::String(name)) => ds.variable_named(name, None),
                _ => return Err(bad(C, "missing `variable`")),
            };
            let var = found.ok_or_else(|| bad(C, "no such variable"))?;
            let want = match field {
                "color" => VarKind::Color,
                "fontSize" => VarKind::Number,
                _ => VarKind::String,
            };
            if var.kind != want {
                return Err(bad(C, format!("`{field}` takes a {} variable", want.name())));
            }
            Some((var.id, ds.resolve(var.id, &[]).cloned()))
        }
    };
    match field {
        "color" => {
            style.run.fill = var
                .as_ref()
                .map(|(id, v)| Fill::solid(var_color(*id, v.as_ref().and_then(|v| v.as_color()).unwrap_or(deckcraft_color::Rgba::BLACK))));
            if var.is_none() {
                // Detached: the colour it showed stays.
                if let Some(Fill::Solid { color }) = doc.text_style(&sid).and_then(|t| t.run.fill) {
                    let rgb = doc
                        .design_system()
                        .and_then(|d| if let deckcraft_model::ColorBase::Var { id, .. } = color.base { d.color(id, &[]) } else { None });
                    style.run.fill = Some(Fill::solid(rgb.map_or(color, deckcraft_model::ColorRef::rgb)));
                }
            }
        }
        f => {
            let bf = if f == "fontSize" { BindField::FontSize } else { BindField::FontFamily };
            style.bindings.retain(|b| b.field != bf);
            if let Some((id, v)) = &var {
                style.bindings.push(Binding { field: bf, variable: *id });
                match (bf, v) {
                    (BindField::FontSize, Some(v)) => style.run.size = v.as_number().map(|n| n.clamp(1.0, 4000.0)),
                    (_, Some(v)) => style.run.font = v.as_str().filter(|s| !s.is_empty()).map(str::to_string).or(style.run.font.take()),
                    _ => {}
                }
            }
        }
    }
    s.edit(move |doc, _| {
        match doc.text_styles.iter_mut().find(|x| x.id == style.id) {
            Some(x) => *x = style,
            None => doc.text_styles.push(style),
        }
        Ok(json!({"id": sid}))
    })
}

#[cfg(test)]
#[path = "design_styles_tests.rs"]
mod tests;
