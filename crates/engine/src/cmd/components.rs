//! SlideCraft components (Phase 4.3): make a component from the selection, give it variants and
//! properties, place instances, set their properties, override, swap, reset and detach them. The
//! model is `deckcraft_model::component`; the engine's design pass (`crate::design_system`)
//! updates every instance when a component changes, and turns edits inside an instance into
//! overrides.

use std::sync::Arc;

use deckcraft_geom::Xfrm;
use deckcraft_model::component::{
    Component, Instance, MAX_COMPONENTS, MAX_PROPS, PropDef, PropKind, PropValue, carry_over, find_variant, materialize,
};
use deckcraft_model::variables::{DesignSystem, MAX_NAME, MAX_STRING};
use deckcraft_model::{Presentation, Shape, ShapeId, ShapeKind};
use serde_json::{Value, json};

use super::{CommandSpec, NO_ID_SPACE, bad, cmd, color_param, has_doc, has_selection, has_slide, rect_param, str_param};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "component.create",
            "Create Component",
            ["Arrange", "Components"],
            Some("Cmd+Alt+K"),
            "{ids?: (else the selection; several objects, or one that is not a frame, are framed first), name?, set?: component set name (makes it a variant), variant?: {property: value}, description?} → {id: component, main: shape id}: the selection becomes the main component, which updates every instance when edited",
            has_selection,
            create
        ),
        cmd!(
            "component.update",
            "Edit Component",
            ["Arrange", "Components"],
            None,
            "{component: id|name, name?, set?, variant?: {property: value}, description?} → {id}",
            has_doc,
            update
        ),
        cmd!(
            "component.addProperty",
            "Add Component Property",
            ["Arrange", "Components"],
            None,
            "{component: id|name, name, type: text|boolean|swap, default?: (text, true/false, or a component), layers: [layer id or name] (the component's layers it drives)} → {name}",
            has_doc,
            add_property
        ),
        cmd!(
            "component.removeProperty",
            "Remove Component Property",
            ["Arrange", "Components"],
            None,
            "{component: id|name, name}",
            has_doc,
            remove_property
        ),
        cmd!(
            "component.delete",
            "Delete Component",
            ["Arrange", "Components"],
            None,
            "{component: id|name} → {detached: n} (its instances become plain frames; its main stays on the slide as a frame)",
            has_doc,
            delete
        ),
        cmd!(
            "component.editMain",
            "Go to Main Component",
            ["Arrange", "Components"],
            None,
            "{component?: id|name (else the selected instance's)} → {main: shape id, slide: index}: select the main on its slide, placing it on the current slide when it is on none",
            has_doc,
            edit_main
        ),
        cmd!(
            "component.insert",
            "Insert Instance",
            ["Insert", "Components"],
            None,
            "{component: id|name, rect?: [x,y,w,h] pt | at?: [x,y] pt (else centred at its own size), props?: {name: value}, variant?: {property: value}} → {id}",
            has_slide,
            insert
        ),
        cmd!(
            "instance.set",
            "Instance Properties",
            ["Arrange", "Components"],
            None,
            "{ids?: instances (else the selection), props?: {name: value|null (null: back to the default)}, variant?: {property: value}} → {ids}",
            has_selection,
            instance_set
        ),
        cmd!(
            "instance.swap",
            "Swap Instance",
            ["Arrange", "Components"],
            None,
            "{ids?, component: id|name} → {ids}: show another component; overrides carry over to layers of the same name",
            has_selection,
            instance_swap
        ),
        cmd!(
            "instance.override",
            "Override Layer",
            ["Arrange", "Components"],
            None,
            "{ids?: an instance, layer: component layer id or name, text?: text|null, fill?: color|null, hidden?: bool|null (null clears that override)} → {ids}",
            has_selection,
            instance_override
        ),
        cmd!(
            "instance.reset",
            "Reset Instance",
            ["Arrange", "Components"],
            None,
            "{ids?} → {ids}: drop every override and property value",
            has_selection,
            instance_reset
        ),
        cmd!(
            "instance.detach",
            "Detach Instance",
            ["Arrange", "Components"],
            None,
            "{ids?} → {ids}: the instance becomes a plain frame that no longer follows its component",
            has_selection,
            instance_detach
        ),
        cmd!(
            query "component.list",
            "Components",
            [],
            None,
            "{} → {components: [{id, name, set?, variant?, props: [{name, type, default}], size: [w,h], instances, main?: {slide, id}}]}",
            has_doc,
            list
        ),
    ]
}

fn design(doc: &mut Presentation) -> &mut DesignSystem {
    Arc::make_mut(doc.design.get_or_insert_with(Default::default))
}

fn lib(doc: &Presentation) -> &[Component] {
    doc.design_system().map(|d| d.components.as_slice()).unwrap_or_default()
}

fn component_id(doc: &Presentation, v: Option<&Value>, cmd: &str) -> Result<u32> {
    let l = lib(doc);
    let found = match v {
        Some(Value::Number(n)) => n.as_u64().and_then(|n| u32::try_from(n).ok()).and_then(|n| l.iter().find(|c| c.id == n)),
        Some(Value::String(s)) => l.iter().find(|c| c.name.eq_ignore_ascii_case(s) || c.display_name().eq_ignore_ascii_case(s)),
        _ => return Err(bad(cmd, "missing `component`")),
    };
    found.map(|c| c.id).ok_or_else(|| bad(cmd, format!("no component {}", v.map(Value::to_string).unwrap_or_default())))
}

fn text_param(p: &Value, key: &str, cmd: &str, max: usize) -> Result<Option<String>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.chars().count() <= max => Ok(Some(s.trim().to_string())),
        Some(_) => Err(bad(cmd, format!("`{key}` must be text of at most {max} characters"))),
    }
}

fn variant_param(p: &Value, cmd: &str) -> Result<Option<Vec<(String, String)>>> {
    match p.get("variant") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(o)) if o.len() <= MAX_PROPS => o
            .iter()
            .map(|(k, v)| match v.as_str() {
                Some(v) if !k.is_empty() && k.chars().count() <= MAX_NAME && v.chars().count() <= MAX_NAME => Ok((k.clone(), v.to_string())),
                _ => Err(bad(cmd, "`variant` maps property names to text values")),
            })
            .collect::<Result<Vec<_>>>()
            .map(Some),
        Some(_) => Err(bad(cmd, "`variant` maps property names to text values")),
    }
}

/// The selection as one frame on the current slide: the selected frame, or the objects framed.
fn frame_selection(s: &mut Session, p: &Value, cmd: &str) -> Result<ShapeId> {
    let ids = super::targets(s, p)?;
    if ids.is_empty() {
        return Err(bad(cmd, "select what becomes the component"));
    }
    if let [one] = ids.as_slice() {
        let sh = super::shape_of(s, *one)?;
        if sh.instance.is_some() {
            return Err(bad(cmd, "an instance can't become a component; detach it first"));
        }
        if sh.main_of.is_some() {
            return Err(bad(cmd, "that is already a main component"));
        }
        if matches!(sh.kind, ShapeKind::Group { .. }) {
            if sh.frame.is_none() {
                s.execute("frame.set", &json!({"ids": [one.0]}))?;
            }
            return Ok(*one);
        }
    }
    let r = s.execute("frame.create", &json!({"ids": ids.iter().map(|i| i.0).collect::<Vec<_>>()}))?;
    r["id"].as_u64().and_then(|v| u32::try_from(v).ok()).map(ShapeId).ok_or_else(|| bad(cmd, "could not frame the selection"))
}

fn create(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "component.create";
    let name = text_param(p, "name", C, MAX_NAME)?;
    let set = text_param(p, "set", C, MAX_NAME)?.unwrap_or_default();
    let variant = variant_param(p, C)?.unwrap_or_default();
    let description = text_param(p, "description", C, MAX_STRING)?.unwrap_or_default();
    if lib(&s.doc()?.doc).len() >= MAX_COMPONENTS {
        return Err(bad(C, format!("a deck holds at most {MAX_COMPONENTS} components")));
    }
    // Framing and making the component are one undo step (nested commands).
    let main = frame_selection(s, p, C)?;
    {
        let sh = super::shape_of(s, main)?;
        s.edit(|doc, sel| {
            let id = doc.alloc_id().ok_or_else(|| bad(C, NO_ID_SPACE))?;
            let name = name
                .clone()
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| if sh.name.is_empty() { format!("Component {id}") } else { sh.name.clone() });
            let mut root = sh.clone();
            root.main_of = None;
            root.name = name.clone();
            design(doc).components.push(Component {
                id,
                name: name.clone(),
                set: set.clone(),
                variant: variant.clone(),
                props: vec![],
                root,
                description: description.clone(),
            });
            let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad(C, "no slide"))?;
            if let Some(m) = deckcraft_model::find_shape_mut(list, main) {
                m.main_of = Some(id);
                m.name = name;
            }
            sel.shapes = vec![main];
            Ok(json!({"id": id, "main": main.0}))
        })
    }
}

fn update(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "component.update";
    let name = text_param(p, "name", C, MAX_NAME)?.filter(|n| !n.is_empty());
    let set = text_param(p, "set", C, MAX_NAME)?;
    let variant = variant_param(p, C)?;
    let description = text_param(p, "description", C, MAX_STRING)?;
    s.edit(|doc, _| {
        let id = component_id(doc, p.get("component"), C)?;
        let c = design(doc).components.iter_mut().find(|c| c.id == id).ok_or_else(|| bad(C, "no component"))?;
        if let Some(n) = name {
            c.name = n.clone();
            c.root.name = n;
        }
        if let Some(st) = set {
            c.set = st;
        }
        if let Some(v) = variant {
            c.variant = v;
        }
        if let Some(d) = description {
            c.description = d;
        }
        Ok(json!({"id": id}))
    })
}

/// A layer of component `c` named by id or by name.
fn layer_of(c: &Component, v: &Value) -> Option<u32> {
    match v {
        Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()).filter(|id| c.layer(*id).is_some()),
        Value::String(s) => c.layer_named(s).map(|l| l.id.0),
        _ => None,
    }
}

fn prop_value(doc: &Presentation, kind: PropKind, v: &Value, cmd: &str) -> Result<PropValue> {
    Ok(match kind {
        PropKind::Text => {
            PropValue::Text(v.as_str().filter(|t| t.chars().count() <= MAX_STRING).ok_or_else(|| bad(cmd, "a text property takes text"))?.to_string())
        }
        PropKind::Boolean => PropValue::Boolean(v.as_bool().ok_or_else(|| bad(cmd, "a boolean property takes true or false"))?),
        PropKind::Swap => PropValue::Swap(component_id(doc, Some(v), cmd)?),
    })
}

fn add_property(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "component.addProperty";
    let name = text_param(p, "name", C, MAX_NAME)?.filter(|n| !n.is_empty()).ok_or_else(|| bad(C, "missing `name`"))?;
    let kind = str_param(p, "type").and_then(PropKind::from_name).ok_or_else(|| bad(C, "`type` must be text, boolean or swap"))?;
    s.edit(|doc, _| {
        let id = component_id(doc, p.get("component"), C)?;
        let c = lib(doc).iter().find(|c| c.id == id).cloned().ok_or_else(|| bad(C, "no component"))?;
        if c.prop(&name).is_some() {
            return Err(bad(C, format!("`{}` already has a property `{name}`", c.name)));
        }
        if c.props.len() >= MAX_PROPS {
            return Err(bad(C, format!("a component has at most {MAX_PROPS} properties")));
        }
        let layers = p.get("layers").and_then(Value::as_array).ok_or_else(|| bad(C, "`layers` lists the layers it drives"))?;
        let targets: Vec<u32> =
            layers.iter().map(|l| layer_of(&c, l).ok_or_else(|| bad(C, format!("`{}` has no layer {l}", c.name)))).collect::<Result<_>>()?;
        if targets.is_empty() {
            return Err(bad(C, "give at least one layer"));
        }
        for t in &targets {
            let l = c.layer(*t);
            let fits = match kind {
                PropKind::Text => l.is_some_and(|l| l.text.is_some()),
                PropKind::Boolean => true,
                PropKind::Swap => l.is_some_and(|l| l.instance.is_some()),
            };
            if !fits {
                return Err(bad(C, format!("layer {t} can't take a {} property", kind.name())));
            }
        }
        let default = match p.get("default") {
            Some(v) if !v.is_null() => prop_value(doc, kind, v, C)?,
            _ => match kind {
                PropKind::Text => {
                    PropValue::Text(targets.first().and_then(|t| c.layer(*t)).and_then(|l| l.text.as_ref()).map(|t| t.text()).unwrap_or_default())
                }
                PropKind::Boolean => PropValue::Boolean(true),
                PropKind::Swap => {
                    PropValue::Swap(targets.first().and_then(|t| c.layer(*t)).and_then(|l| l.instance.as_ref()).map_or(0, |i| i.component))
                }
            },
        };
        if let Some(c) = design(doc).components.iter_mut().find(|c| c.id == id) {
            c.props.push(PropDef { name: name.clone(), default, targets });
        }
        Ok(json!({"name": name}))
    })
}

fn remove_property(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "component.removeProperty";
    let name = str_param(p, "name").ok_or_else(|| bad(C, "missing `name`"))?.to_string();
    s.edit(|doc, _| {
        let id = component_id(doc, p.get("component"), C)?;
        let c = design(doc).components.iter_mut().find(|c| c.id == id).ok_or_else(|| bad(C, "no component"))?;
        let n = c.props.len();
        c.props.retain(|d| !d.name.eq_ignore_ascii_case(&name));
        if n == c.props.len() {
            return Err(bad(C, format!("no property `{name}`")));
        }
        Ok(json!({"name": name}))
    })
}

fn each_list(doc: &mut Presentation, f: &mut dyn FnMut(&mut Vec<Shape>)) {
    for slide in doc.slides.iter_mut() {
        let mut shapes = slide.shapes.clone();
        f(&mut shapes);
        if shapes != slide.shapes {
            Arc::make_mut(slide).shapes = shapes;
        }
    }
    for m in doc.masters.iter_mut() {
        let mut mm = (**m).clone();
        f(&mut mm.shapes);
        for l in &mut mm.layouts {
            f(&mut l.shapes);
        }
        if mm != **m {
            *m = Arc::new(mm);
        }
    }
}

fn walk_mut(list: &mut [Shape], depth: usize, f: &mut dyn FnMut(&mut Shape)) {
    if depth > deckcraft_model::canvas::MAX_DEPTH {
        return;
    }
    for sh in list.iter_mut() {
        f(sh);
        if let ShapeKind::Group { children, .. } = &mut sh.kind {
            walk_mut(children, depth + 1, f);
        }
    }
}

fn delete(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "component.delete";
    s.edit(|doc, _| {
        let id = component_id(doc, p.get("component"), C)?;
        let mut n = 0;
        each_list(doc, &mut |list| {
            walk_mut(list, 0, &mut |sh| {
                if sh.instance.as_ref().is_some_and(|i| i.component == id) {
                    sh.instance = None;
                    n += 1;
                }
                if sh.main_of == Some(id) {
                    sh.main_of = None;
                }
            })
        });
        let d = design(doc);
        d.components.retain(|c| c.id != id);
        if d.is_empty() {
            doc.design = None;
        }
        Ok(json!({"detached": n}))
    })
}

/// Where the main of component `id` is: (slide index, shape id).
fn find_main(doc: &Presentation, id: u32) -> Option<(usize, ShapeId)> {
    doc.slides.iter().enumerate().find_map(|(i, sl)| {
        let mut found = None;
        deckcraft_model::walk(&sl.shapes, &mut |sh, _| {
            if found.is_none() && sh.main_of == Some(id) {
                found = Some(sh.id);
            }
        });
        found.map(|f| (i, f))
    })
}

fn selected_instance(s: &Session) -> Option<u32> {
    let st = s.doc().ok()?;
    st.selection.shapes.iter().find_map(|id| st.shape(*id).and_then(|sh| sh.instance.as_ref().map(|i| i.component)))
}

fn edit_main(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "component.editMain";
    let id = match p.get("component") {
        None | Some(Value::Null) => selected_instance(s).ok_or_else(|| bad(C, "select an instance, or give `component`"))?,
        v => component_id(&s.doc()?.doc, v, C)?,
    };
    if let Some((slide, main)) = find_main(&s.doc()?.doc, id) {
        s.execute("slide.go", &json!({"index": slide}))?;
        s.execute("edit.select", &json!({"ids": [main.0]}))?;
        return Ok(json!({"main": main.0, "slide": slide}));
    }
    let comp = lib(&s.doc()?.doc).iter().find(|c| c.id == id).cloned().ok_or_else(|| bad(C, "no component"))?;
    s.edit(|doc, sel| {
        let mut main = comp.root.clone();
        let fresh = doc.alloc_id().ok_or_else(|| bad(C, NO_ID_SPACE))?;
        // The main keeps the component's own layer ids (they are what instances map from), so only
        // its frame needs a fresh id; a deck holds them only once since the main is the only copy.
        main.id = ShapeId(fresh);
        main.main_of = Some(id);
        if let Some(c) = design(doc).components.iter_mut().find(|c| c.id == id) {
            c.root.id = ShapeId(fresh);
        }
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad(C, "no slide"))?;
        list.push(main);
        sel.shapes = vec![ShapeId(fresh)];
        Ok(json!({"main": fresh, "slide": sel.slide}))
    })
}

fn props_param(doc: &Presentation, comp: &Component, p: &Value, cmd: &str) -> Result<Vec<(String, Option<PropValue>)>> {
    let Some(o) = p.get("props") else { return Ok(vec![]) };
    let o = o.as_object().ok_or_else(|| bad(cmd, "`props` maps property names to values"))?;
    o.iter()
        .map(|(k, v)| {
            let def = comp.prop(k).ok_or_else(|| bad(cmd, format!("`{}` has no property `{k}`", comp.name)))?;
            if v.is_null() {
                return Ok((def.name.clone(), None));
            }
            Ok((def.name.clone(), Some(prop_value(doc, def.default.kind(), v, cmd)?)))
        })
        .collect()
}

fn insert(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "component.insert";
    let doc = s.doc()?.doc.clone();
    let mut cid = component_id(&doc, p.get("component"), C)?;
    let mut comp = lib(&doc).iter().find(|c| c.id == cid).cloned().ok_or_else(|| bad(C, "no component"))?;
    if let Some(v) = variant_param(p, C)? {
        let to = find_variant(lib(&doc), &comp.set, &comp.variant, &v).ok_or_else(|| bad(C, "no variant matches `variant`"))?;
        cid = to.id;
        comp = to.clone();
    }
    let props = props_param(&doc, &comp, p, C)?;
    let size = comp.root.xfrm.unwrap_or_else(|| Xfrm::new(0.0, 0.0, 200.0, 100.0));
    let rect = match (rect_param(p, "rect"), p.get("at").and_then(Value::as_array)) {
        (Some(r), _) => r,
        (None, Some(a)) => {
            let f = |i: usize| a.get(i).and_then(Value::as_f64).filter(|v| v.is_finite());
            Xfrm::new(f(0).ok_or_else(|| bad(C, "`at` is [x, y]"))?, f(1).ok_or_else(|| bad(C, "`at` is [x, y]"))?, size.w, size.h)
        }
        (None, None) => {
            let sz = doc.slide_size;
            Xfrm::new((sz.width - size.w) / 2.0, (sz.height - size.h) / 2.0, size.w, size.h)
        }
    };
    if !(rect.w > 0.0 && rect.h > 0.0 && rect.w <= 1.0e5 && rect.h <= 1.0e5) {
        return Err(bad(C, "the instance needs a positive size"));
    }
    s.edit(|doc, sel| {
        let id = doc.alloc_id().ok_or_else(|| bad(C, NO_ID_SPACE))?;
        let inst =
            Instance { component: cid, props: props.iter().filter_map(|(n, v)| Some((n.clone(), v.clone()?))).collect(), ..Default::default() };
        let frame = Shape { id: ShapeId(id), name: comp.name.clone(), xfrm: Some(rect), ..Default::default() };
        let library = lib(doc).to_vec();
        let mut built = materialize(&library, &comp, &frame, &inst, doc).ok_or_else(|| bad(C, NO_ID_SPACE))?;
        crate::canvas::settle_list(std::slice::from_mut(&mut built));
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad(C, "no slide"))?;
        list.push(built);
        sel.shapes = vec![ShapeId(id)];
        Ok(json!({"id": id}))
    })
}

/// Rebuild instance `id` in the edited tree with `f` applied to its `Instance`.
fn edit_instances(
    s: &mut Session,
    p: &Value,
    cmd: &str,
    f: &dyn Fn(&Presentation, &Component, &mut Instance) -> Result<Option<u32>>,
) -> Result<Value> {
    let ids = super::targets(s, p)?;
    if ids.is_empty() {
        return Err(bad(cmd, "select an instance"));
    }
    s.edit(|doc, sel| {
        let library = lib(doc).to_vec();
        for id in &ids {
            let Some(cur) = crate::cmd::current_shapes(doc, sel).and_then(|l| deckcraft_model::find_shape(l, *id)).cloned() else {
                return Err(bad(cmd, format!("no shape {id}")));
            };
            let mut inst = cur.instance.as_deref().cloned().ok_or_else(|| bad(cmd, format!("shape {id} is not an instance")))?;
            let comp = library.iter().find(|c| c.id == inst.component).cloned().ok_or_else(|| bad(cmd, "its component is gone"))?;
            let target = f(doc, &comp, &mut inst)?;
            let comp = match target {
                Some(t) => {
                    let to = library.iter().find(|c| c.id == t).cloned().ok_or_else(|| bad(cmd, "no such component"))?;
                    carry_over(&comp, &to, &mut inst);
                    to
                }
                None => comp,
            };
            let mut built = materialize(&library, &comp, &cur, &inst, doc).ok_or_else(|| bad(cmd, NO_ID_SPACE))?;
            crate::canvas::settle_list(std::slice::from_mut(&mut built));
            let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad(cmd, "no slide"))?;
            if let Some(slot) = deckcraft_model::find_shape_mut(list, *id) {
                *slot = built;
            }
        }
        Ok(json!({"ids": ids}))
    })
}

fn instance_set(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "instance.set";
    let variant = variant_param(p, C)?;
    edit_instances(s, p, C, &|doc, comp, inst| {
        for (name, v) in props_param(doc, comp, p, C)? {
            inst.props.retain(|(n, _)| !n.eq_ignore_ascii_case(&name));
            if let Some(v) = v {
                inst.props.push((name, v));
            }
        }
        match &variant {
            Some(v) => {
                let to = find_variant(lib(doc), &comp.set, &comp.variant, v).ok_or_else(|| bad(C, "no variant matches `variant`"))?;
                Ok((to.id != comp.id).then_some(to.id))
            }
            None => Ok(None),
        }
    })
}

fn instance_swap(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "instance.swap";
    let to = component_id(&s.doc()?.doc, p.get("component"), C)?;
    edit_instances(s, p, C, &|_, comp, _| Ok((to != comp.id).then_some(to)))
}

fn instance_override(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "instance.override";
    let layer = p.get("layer").cloned().ok_or_else(|| bad(C, "missing `layer`"))?;
    let fill = match p.get("fill") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(_) => Some(Some(deckcraft_model::Fill::solid(color_param(p, "fill").ok_or_else(|| bad(C, "`fill` is not a colour"))?))),
    };
    let text = match p.get("text") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(v) => Some(Some(v.as_str().filter(|t| t.chars().count() <= MAX_STRING).ok_or_else(|| bad(C, "`text` must be text"))?.to_string())),
    };
    let hidden = match p.get("hidden") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(v) => Some(Some(v.as_bool().ok_or_else(|| bad(C, "`hidden` must be true or false"))?)),
    };
    edit_instances(s, p, C, &|_, comp, inst| {
        let l = layer_of(comp, &layer).ok_or_else(|| bad(C, format!("`{}` has no layer {layer}", comp.name)))?;
        let base = comp.layer(l).cloned();
        let o = inst.override_mut(l).ok_or_else(|| bad(C, "no override"))?;
        if let Some(f) = &fill {
            o.fill = f.clone();
        }
        if let Some(t) = &text {
            o.text = t.as_ref().map(|t| {
                let mut sh = base.clone().unwrap_or_default();
                deckcraft_model::variables::apply_one(
                    &mut sh,
                    deckcraft_model::variables::BindField::Text,
                    &deckcraft_model::variables::VarValue::String(t.clone()),
                );
                sh.text.unwrap_or_default()
            });
        }
        if let Some(h) = hidden {
            o.hidden = h;
        }
        inst.overrides.retain(|o| o.text.is_some() || o.fill.is_some() || o.hidden.is_some());
        Ok(None)
    })
}

fn instance_reset(s: &mut Session, p: &Value) -> Result<Value> {
    edit_instances(s, p, "instance.reset", &|_, _, inst| {
        inst.overrides.clear();
        inst.props.clear();
        Ok(None)
    })
}

fn instance_detach(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "instance.detach";
    let ids = super::targets(s, p)?;
    super::edit_shapes(s, p, C, |sh| {
        if sh.instance.take().is_none() {
            return Err(bad(C, format!("shape {} is not an instance", sh.id)));
        }
        Ok(())
    })?;
    Ok(json!({"ids": ids}))
}

fn list(s: &mut Session, _p: &Value) -> Result<Value> {
    let doc = &s.doc()?.doc;
    let mut counts: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    for sl in &doc.slides {
        deckcraft_model::walk(&sl.shapes, &mut |sh, _| {
            if let Some(i) = &sh.instance {
                *counts.entry(i.component).or_default() += 1;
            }
        });
    }
    let comps: Vec<Value> = lib(doc)
        .iter()
        .map(|c| {
            let x = c.root.xfrm.unwrap_or_default();
            let mut o = json!({
                "id": c.id,
                "name": c.name,
                "display": c.display_name(),
                "props": c.props.iter().map(|d| json!({"name": d.name, "type": d.default.kind().name(), "default": match &d.default {
                    PropValue::Text(t) => json!(t),
                    PropValue::Boolean(b) => json!(b),
                    PropValue::Swap(id) => json!(id),
                }})).collect::<Vec<_>>(),
                "size": [x.w, x.h],
                "instances": counts.get(&c.id).copied().unwrap_or(0),
            });
            if let Some(m) = o.as_object_mut() {
                if !c.set.is_empty() {
                    m.insert("set".into(), json!(c.set));
                    m.insert("variant".into(), Value::Object(c.variant.iter().map(|(k, v)| (k.clone(), json!(v))).collect()));
                }
                if let Some((slide, id)) = find_main(doc, c.id) {
                    m.insert("main".into(), json!({"slide": slide, "id": id.0}));
                }
            }
            o
        })
        .collect();
    Ok(json!({"components": comps}))
}

#[cfg(test)]
#[path = "components_tests.rs"]
mod tests;
