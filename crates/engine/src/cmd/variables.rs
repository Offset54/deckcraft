//! SlideCraft variables (Phase 4.1): collections with modes, colour/number/string/boolean
//! variables with aliases, binding shape properties to them, and switching modes for the deck or
//! one slide. The model is `deckcraft_model::variables`; the engine's design pass
//! (`crate::design_system`) keeps themes and bound fields in step after every command.
//!
//! Variables, collections and modes are named in params by id (a number) or by name (a string).

use std::sync::Arc;

use deckcraft_color::Rgba;
use deckcraft_model::variables::{
    BindField, Binding, Collection, DesignSystem, MAX_BINDINGS, MAX_COLLECTIONS, MAX_MODES, MAX_NAME, MAX_STRING, MAX_VARIABLES, Mode, ThemeSlot,
    VarKind, VarValue, Variable, var_color, visit_bound_colors,
};
use deckcraft_model::{ColorBase, ColorRef, Fill, Presentation, Shape, ShapeKind};
use serde_json::{Value, json};

use super::{CommandSpec, NO_ID_SPACE, bad, cmd, has_doc, has_selection, str_param};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "variable.createCollection",
            "New Variable Collection",
            ["Design", "Variables"],
            None,
            "{name, modes?: [names] (default [\"Mode 1\"])} → {id, modes: [{id, name}]}",
            has_doc,
            create_collection
        ),
        cmd!(
            "variable.renameCollection",
            "Rename Collection",
            ["Design", "Variables"],
            None,
            "{collection: id|name, name}",
            has_doc,
            rename_collection
        ),
        cmd!(
            "variable.deleteCollection",
            "Delete Collection",
            ["Design", "Variables"],
            None,
            "{collection: id|name} → {deleted: n} (its variables go too; what was bound keeps its current value)",
            has_doc,
            delete_collection
        ),
        cmd!(
            "variable.addMode",
            "Add Mode",
            ["Design", "Variables"],
            None,
            "{collection: id|name, name, from?: mode (copy its values; default the first mode)} → {id}",
            has_doc,
            add_mode
        ),
        cmd!("variable.renameMode", "Rename Mode", ["Design", "Variables"], None, "{collection: id|name, mode: id|name, name}", has_doc, rename_mode),
        cmd!(
            "variable.deleteMode",
            "Delete Mode",
            ["Design", "Variables"],
            None,
            "{collection: id|name, mode: id|name} (a collection keeps at least one)",
            has_doc,
            delete_mode
        ),
        cmd!(
            "variable.create",
            "New Variable",
            ["Design", "Variables"],
            None,
            "{collection: id|name, name (a path such as color/accent), type: color|number|string|boolean, value?: (every mode; a colour is #RRGGBB, an alias {alias: variable}), values?: {mode: value}, theme?: dk1|lt1|dk2|lt2|accent1…accent6|hlink|folHlink|majorFont|minorFont (the theme slot it drives), description?} → {id}",
            has_doc,
            create_variable
        ),
        cmd!(
            "variable.set",
            "Edit Variable",
            ["Design", "Variables"],
            None,
            "{variable: id|name, mode?: id|name (else the deck's active mode), value?: (as in variable.create), name?, description?, theme?: slot|null} → {id}",
            has_doc,
            set_variable
        ),
        cmd!(
            "variable.delete",
            "Delete Variable",
            ["Design", "Variables"],
            None,
            "{variable: id|name} (what was bound to it keeps its current value; aliases of it take that value)",
            has_doc,
            delete_variable
        ),
        cmd!(
            "variable.mode",
            "Switch Mode",
            ["Design", "Variables"],
            None,
            "{collection: id|name, mode: id|name|null, slides?: [index]|all (else the whole deck; null mode on slides follows the deck again)} → {mode}",
            has_doc,
            switch_mode
        ),
        cmd!(
            "variable.bind",
            "Bind to Variable",
            ["Design", "Variables"],
            None,
            "{ids?, field: fill|line|textColor (colours) | field: cornerRadius|gap|crossGap|padding|width|height|lineWidth|fontSize (numbers) | field: fontFamily|text (strings) | field: visible (booleans), variable: id|name} → {ids}",
            has_selection,
            bind
        ),
        cmd!(
            "variable.unbind",
            "Detach Variable",
            ["Design", "Variables"],
            None,
            "{ids?, field?: (as in variable.bind; else every binding)} → {ids}: the property keeps its current value",
            has_selection,
            unbind
        ),
        cmd!(
            query "variable.list",
            "Variables",
            [],
            None,
            "{} → {collections: [{id, name, modes, mode}], variables: [{id, name, collection, type, values: {mode: value}, resolved, theme?}]}",
            has_doc,
            list
        ),
    ]
}

const COLOR_FIELDS: [&str; 3] = ["fill", "line", "textColor"];

fn design(doc: &mut Presentation) -> &mut DesignSystem {
    Arc::make_mut(doc.design.get_or_insert_with(Default::default))
}

fn name_param(p: &Value, cmd: &str) -> Result<String> {
    let n = str_param(p, "name").map(str::trim).filter(|n| !n.is_empty()).ok_or_else(|| bad(cmd, "missing `name`"))?;
    if n.chars().count() > MAX_NAME {
        return Err(bad(cmd, format!("`name` is longer than {MAX_NAME} characters")));
    }
    Ok(n.to_string())
}

/// A collection named by id or name.
fn collection_id(ds: Option<&DesignSystem>, v: Option<&Value>, cmd: &str) -> Result<u32> {
    let ds = ds.ok_or_else(|| bad(cmd, "the deck has no variables"))?;
    let c = match v {
        Some(Value::Number(n)) => n.as_u64().and_then(|n| u32::try_from(n).ok()).and_then(|n| ds.collection(n)),
        Some(Value::String(s)) => ds.collection_named(s),
        _ => return Err(bad(cmd, "missing `collection`")),
    };
    c.map(|c| c.id).ok_or_else(|| bad(cmd, format!("no collection {}", v.map(Value::to_string).unwrap_or_default())))
}

fn mode_id(c: &Collection, v: Option<&Value>, cmd: &str) -> Result<u32> {
    let m = match v {
        Some(Value::Number(n)) => n.as_u64().and_then(|n| u32::try_from(n).ok()).and_then(|n| c.mode(n)),
        Some(Value::String(s)) => c.mode_named(s),
        _ => return Err(bad(cmd, "missing `mode`")),
    };
    m.map(|m| m.id).ok_or_else(|| bad(cmd, format!("collection `{}` has no mode {}", c.name, v.map(Value::to_string).unwrap_or_default())))
}

/// A variable named by id or by name (`collection/name` paths are names).
fn variable_id(ds: Option<&DesignSystem>, v: Option<&Value>, cmd: &str) -> Result<u32> {
    let ds = ds.ok_or_else(|| bad(cmd, "the deck has no variables"))?;
    let found = match v {
        Some(Value::Number(n)) => n.as_u64().and_then(|n| u32::try_from(n).ok()).and_then(|n| ds.variable(n)),
        Some(Value::String(s)) => ds.variable_named(s, None),
        _ => return Err(bad(cmd, "missing `variable`")),
    };
    found.map(|v| v.id).ok_or_else(|| bad(cmd, format!("no variable {}", v.map(Value::to_string).unwrap_or_default())))
}

/// A value of `kind` from params: `#RRGGBB` for colours, a number, a string, a boolean, or
/// `{alias: variable}`.
fn value_of(ds: &DesignSystem, kind: VarKind, v: &Value, me: Option<u32>, cmd: &str) -> Result<VarValue> {
    if let Some(a) = v.get("alias") {
        let target = variable_id(Some(ds), Some(a), cmd)?;
        let tk = ds.variable(target).map(|t| t.kind);
        if tk != Some(kind) {
            return Err(bad(cmd, format!("an alias must name a {} variable", kind.name())));
        }
        if let Some(me) = me
            && (me == target || ds.alias_cycles(me, target))
        {
            return Err(bad(cmd, "that alias would refer back to this variable"));
        }
        return Ok(VarValue::Alias(target));
    }
    let wrong = || bad(cmd, format!("`value` is not a {} value: {v}", kind.name()));
    Ok(match kind {
        VarKind::Color => VarValue::Color(v.as_str().and_then(Rgba::from_hex).ok_or_else(wrong)?),
        VarKind::Number => {
            let n = v.as_f64().filter(|n| n.is_finite()).ok_or_else(wrong)?;
            VarValue::Number(n.clamp(-deckcraft_model::variables::MAX_NUMBER, deckcraft_model::variables::MAX_NUMBER))
        }
        VarKind::String => {
            let s = v.as_str().ok_or_else(wrong)?;
            if s.chars().count() > MAX_STRING {
                return Err(bad(cmd, format!("`value` is longer than {MAX_STRING} characters")));
            }
            VarValue::String(s.to_string())
        }
        VarKind::Boolean => VarValue::Boolean(v.as_bool().ok_or_else(wrong)?),
    })
}

fn theme_param(p: &Value, kind: VarKind, cmd: &str) -> Result<Option<Option<ThemeSlot>>> {
    match p.get("theme") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(s)) => {
            let t = ThemeSlot::from_name(s).ok_or_else(|| bad(cmd, format!("no theme slot `{s}`")))?;
            if t.kind() != kind {
                return Err(bad(cmd, format!("theme slot `{s}` takes a {} variable", t.kind().name())));
            }
            Ok(Some(Some(t)))
        }
        Some(v) => Err(bad(cmd, format!("`theme` is not a slot name: {v}"))),
    }
}

fn value_json(v: &VarValue) -> Value {
    match v {
        VarValue::Color(c) => json!(format!("#{}", c.hex())),
        VarValue::Number(n) => json!(n),
        VarValue::String(s) => json!(s),
        VarValue::Boolean(b) => json!(b),
        VarValue::Alias(a) => json!({"alias": a}),
    }
}

fn create_collection(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "variable.createCollection";
    let name = name_param(p, C)?;
    let modes: Vec<String> = match p.get("modes") {
        None | Some(Value::Null) => vec!["Mode 1".into()],
        Some(Value::Array(a)) => a
            .iter()
            .map(|m| m.as_str().map(|m| m.trim().to_string()).filter(|m| !m.is_empty()))
            .collect::<Option<_>>()
            .ok_or_else(|| bad(C, "`modes` must be names"))?,
        Some(v) => return Err(bad(C, format!("`modes` is not a list: {v}"))),
    };
    if modes.is_empty() || modes.len() > MAX_MODES || modes.iter().any(|m| m.chars().count() > MAX_NAME) {
        return Err(bad(C, format!("give 1 to {MAX_MODES} mode names of at most {MAX_NAME} characters")));
    }
    s.edit(|doc, _| {
        if doc.design_system().is_some_and(|d| d.collections.len() >= MAX_COLLECTIONS) {
            return Err(bad(C, format!("a deck holds at most {MAX_COLLECTIONS} collections")));
        }
        if doc.design_system().is_some_and(|d| d.collection_named(&name).is_some()) {
            return Err(bad(C, format!("there is already a collection `{name}`")));
        }
        let id = doc.alloc_id().ok_or_else(|| bad(C, NO_ID_SPACE))?;
        let mut ms = vec![];
        for m in &modes {
            ms.push(Mode { id: doc.alloc_id().ok_or_else(|| bad(C, NO_ID_SPACE))?, name: m.clone() });
        }
        let out = json!({"id": id, "modes": ms.iter().map(|m| json!({"id": m.id, "name": m.name})).collect::<Vec<_>>()});
        design(doc).collections.push(Collection { id, name, modes: ms, kit: false });
        Ok(out)
    })
}

fn rename_collection(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "variable.renameCollection";
    let name = name_param(p, C)?;
    s.edit(|doc, _| {
        let id = collection_id(doc.design_system(), p.get("collection"), C)?;
        if doc.design_system().and_then(|d| d.collection_named(&name)).is_some_and(|c| c.id != id) {
            return Err(bad(C, format!("there is already a collection `{name}`")));
        }
        if let Some(c) = design(doc).collections.iter_mut().find(|c| c.id == id) {
            c.name = name;
        }
        Ok(json!({"id": id}))
    })
}

fn delete_collection(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "variable.deleteCollection";
    s.edit(|doc, _| {
        let id = collection_id(doc.design_system(), p.get("collection"), C)?;
        let gone: Vec<u32> =
            doc.design_system().map(|d| d.variables.iter().filter(|v| v.collection == id).map(|v| v.id).collect()).unwrap_or_default();
        detach(doc, &gone);
        let d = design(doc);
        d.collections.retain(|c| c.id != id);
        d.variables.retain(|v| v.collection != id);
        d.modes.retain(|(c, _)| *c != id);
        for slide in doc.slides.iter_mut() {
            if slide.modes.iter().any(|(c, _)| *c == id) {
                Arc::make_mut(slide).modes.retain(|(c, _)| *c != id);
            }
        }
        tidy(doc);
        Ok(json!({"deleted": gone.len()}))
    })
}

fn add_mode(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "variable.addMode";
    let name = name_param(p, C)?;
    s.edit(|doc, _| {
        let cid = collection_id(doc.design_system(), p.get("collection"), C)?;
        let c = doc.design_system().and_then(|d| d.collection(cid)).cloned().ok_or_else(|| bad(C, "no collection"))?;
        if c.modes.len() >= MAX_MODES {
            return Err(bad(C, format!("a collection holds at most {MAX_MODES} modes")));
        }
        if c.mode_named(&name).is_some() {
            return Err(bad(C, format!("`{}` already has a mode `{name}`", c.name)));
        }
        let from = match p.get("from") {
            None | Some(Value::Null) => c.modes.first().map(|m| m.id),
            v => Some(mode_id(&c, v, C)?),
        };
        let id = doc.alloc_id().ok_or_else(|| bad(C, NO_ID_SPACE))?;
        let d = design(doc);
        if let Some(c) = d.collections.iter_mut().find(|c| c.id == cid) {
            c.modes.push(Mode { id, name });
        }
        for v in d.variables.iter_mut().filter(|v| v.collection == cid) {
            if let Some(val) = from.and_then(|f| v.value_in(f)).cloned() {
                v.set_value(id, val);
            }
        }
        Ok(json!({"id": id}))
    })
}

fn rename_mode(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "variable.renameMode";
    let name = name_param(p, C)?;
    s.edit(|doc, _| {
        let cid = collection_id(doc.design_system(), p.get("collection"), C)?;
        let c = doc.design_system().and_then(|d| d.collection(cid)).cloned().ok_or_else(|| bad(C, "no collection"))?;
        let mid = mode_id(&c, p.get("mode"), C)?;
        if c.mode_named(&name).is_some_and(|m| m.id != mid) {
            return Err(bad(C, format!("`{}` already has a mode `{name}`", c.name)));
        }
        if let Some(m) = design(doc).collections.iter_mut().find(|c| c.id == cid).and_then(|c| c.modes.iter_mut().find(|m| m.id == mid)) {
            m.name = name;
        }
        Ok(json!({"id": mid}))
    })
}

fn delete_mode(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "variable.deleteMode";
    s.edit(|doc, _| {
        let cid = collection_id(doc.design_system(), p.get("collection"), C)?;
        let c = doc.design_system().and_then(|d| d.collection(cid)).cloned().ok_or_else(|| bad(C, "no collection"))?;
        let mid = mode_id(&c, p.get("mode"), C)?;
        if c.modes.len() <= 1 {
            return Err(bad(C, "a collection keeps at least one mode"));
        }
        let d = design(doc);
        if let Some(c) = d.collections.iter_mut().find(|c| c.id == cid) {
            c.modes.retain(|m| m.id != mid);
        }
        for v in d.variables.iter_mut().filter(|v| v.collection == cid) {
            v.values.retain(|(m, _)| *m != mid);
        }
        d.modes.retain(|m| *m != (cid, mid));
        for slide in doc.slides.iter_mut() {
            if slide.modes.contains(&(cid, mid)) {
                Arc::make_mut(slide).modes.retain(|m| *m != (cid, mid));
            }
        }
        Ok(json!({"deleted": mid}))
    })
}

fn create_variable(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "variable.create";
    let name = name_param(p, C)?;
    let kind = str_param(p, "type").and_then(VarKind::from_name).ok_or_else(|| bad(C, "`type` must be color, number, string or boolean"))?;
    let theme = theme_param(p, kind, C)?.flatten();
    let description = str_param(p, "description").unwrap_or_default().chars().take(MAX_STRING).collect::<String>();
    s.edit(|doc, _| {
        let cid = collection_id(doc.design_system(), p.get("collection"), C)?;
        let ds = doc.design_system().cloned().unwrap_or_default();
        if ds.variables.len() >= MAX_VARIABLES {
            return Err(bad(C, format!("a deck holds at most {MAX_VARIABLES} variables")));
        }
        if ds.variable_named(&name, Some(cid)).is_some() {
            return Err(bad(C, format!("the collection already has a variable `{name}`")));
        }
        let c = ds.collection(cid).cloned().ok_or_else(|| bad(C, "no collection"))?;
        let mut values = vec![];
        if let Some(v) = p.get("value").filter(|v| !v.is_null()) {
            let val = value_of(&ds, kind, v, None, C)?;
            values = c.modes.iter().map(|m| (m.id, val.clone())).collect();
        }
        if let Some(per) = p.get("values") {
            let o = per.as_object().ok_or_else(|| bad(C, "`values` must map modes to values"))?;
            for (m, v) in o {
                let mid = mode_id(&c, Some(&json!(m)), C)
                    .or_else(|_| m.parse::<u32>().ok().filter(|id| c.mode(*id).is_some()).ok_or_else(|| bad(C, format!("no mode `{m}`"))))?;
                let val = value_of(&ds, kind, v, None, C)?;
                match values.iter_mut().find(|(k, _)| *k == mid) {
                    Some(slot) => slot.1 = val,
                    None => values.push((mid, val)),
                }
            }
        }
        if values.is_empty() {
            return Err(bad(C, "give `value` or `values`"));
        }
        let id = doc.alloc_id().ok_or_else(|| bad(C, NO_ID_SPACE))?;
        design(doc).variables.push(Variable { id, name, collection: cid, kind, values, theme, description });
        Ok(json!({"id": id}))
    })
}

fn set_variable(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "variable.set";
    s.edit(|doc, _| {
        let ds = doc.design_system().cloned().unwrap_or_default();
        let id = variable_id(Some(&ds), p.get("variable"), C)?;
        let var = ds.variable(id).cloned().ok_or_else(|| bad(C, "no variable"))?;
        let c = ds.collection(var.collection).cloned().ok_or_else(|| bad(C, "no collection"))?;
        let mut var2 = var.clone();
        if let Some(v) = p.get("value").filter(|v| !v.is_null()) {
            let mode = match p.get("mode") {
                None | Some(Value::Null) => ds.deck_mode(c.id).ok_or_else(|| bad(C, "no mode"))?,
                m => mode_id(&c, m, C)?,
            };
            var2.set_value(mode, value_of(&ds, var.kind, v, Some(id), C)?);
        }
        if p.get("name").is_some() {
            let name = name_param(p, C)?;
            if ds.variable_named(&name, Some(c.id)).is_some_and(|o| o.id != id) {
                return Err(bad(C, format!("the collection already has a variable `{name}`")));
            }
            var2.name = name;
        }
        if let Some(d) = str_param(p, "description") {
            var2.description = d.chars().take(MAX_STRING).collect();
        }
        if let Some(t) = theme_param(p, var.kind, C)? {
            var2.theme = t;
        }
        if var2 == var {
            return Ok(json!({"id": id}));
        }
        if let Some(v) = design(doc).variable_mut(id) {
            *v = var2;
        }
        Ok(json!({"id": id}))
    })
}

fn delete_variable(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "variable.delete";
    s.edit(|doc, _| {
        let id = variable_id(doc.design_system(), p.get("variable"), C)?;
        detach(doc, &[id]);
        // Aliases of it take its value in each mode.
        let ds = doc.design_system().cloned().unwrap_or_default();
        let d = design(doc);
        for v in d.variables.iter_mut() {
            let coll = v.collection;
            for (m, val) in v.values.iter_mut() {
                if *val == VarValue::Alias(id) {
                    let mode = [(coll, *m)];
                    if let Some(r) = ds.resolve(id, &mode).cloned() {
                        *val = r;
                    }
                }
            }
            v.values.retain(|(_, val)| *val != VarValue::Alias(id));
        }
        d.variables.retain(|v| v.id != id);
        tidy(doc);
        Ok(json!({"deleted": id}))
    })
}

/// No design system left once its last part goes (so the file is as it was).
fn tidy(doc: &mut Presentation) {
    if doc.design_system().is_some_and(DesignSystem::is_empty) {
        doc.design = None;
    }
}

fn switch_mode(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "variable.mode";
    s.edit(|doc, _| {
        let cid = collection_id(doc.design_system(), p.get("collection"), C)?;
        let c = doc.design_system().and_then(|d| d.collection(cid)).cloned().ok_or_else(|| bad(C, "no collection"))?;
        let mode = match p.get("mode") {
            Some(Value::Null) => None,
            m => Some(mode_id(&c, m, C)?),
        };
        match p.get("slides") {
            None | Some(Value::Null) => {
                let mode = mode.ok_or_else(|| bad(C, "the deck needs a mode (`null` is for slides)"))?;
                let d = design(doc);
                d.modes.retain(|(k, _)| *k != cid);
                if c.modes.first().map(|m| m.id) != Some(mode) {
                    d.modes.push((cid, mode));
                }
            }
            Some(v) => {
                let n = doc.slides.len();
                let idx: Vec<usize> = if v.as_str() == Some("all") {
                    (0..n).collect()
                } else {
                    let a = v.as_array().ok_or_else(|| bad(C, "`slides` is a list of indices or \"all\""))?;
                    a.iter()
                        .map(|i| i.as_u64().and_then(|i| usize::try_from(i).ok()).filter(|i| *i < n))
                        .collect::<Option<_>>()
                        .ok_or_else(|| bad(C, "`slides` has an index out of range"))?
                };
                for i in idx {
                    if let Some(slide) = doc.slides.get_mut(i) {
                        let want: Vec<(u32, u32)> = slide.modes.iter().copied().filter(|(k, _)| *k != cid).chain(mode.map(|m| (cid, m))).collect();
                        if want != slide.modes {
                            Arc::make_mut(slide).modes = want;
                        }
                    }
                }
            }
        }
        Ok(json!({"mode": mode}))
    })
}

/// Turn every use of variables `ids` into its current value: colours become plain colours,
/// bindings go.
fn detach(doc: &mut Presentation, ids: &[u32]) {
    if ids.is_empty() {
        return;
    }
    let Some(ds) = doc.design.clone() else { return };
    let fix = |list: &mut Vec<Shape>, modes: &[(u32, u32)]| -> bool {
        let mut changed = false;
        each_shape(list, &mut |sh| {
            visit_bound_colors(sh, &mut |_, c| {
                if let ColorBase::Var { id, last } = c.base
                    && ids.contains(&id)
                {
                    c.base = ColorBase::Rgb { rgb: ds.color(id, modes).unwrap_or(last) };
                    changed = true;
                }
            });
            let n = sh.bindings.len();
            sh.bindings.retain(|b| !ids.contains(&b.variable));
            changed |= n != sh.bindings.len();
        });
        changed
    };
    for slide in doc.slides.iter_mut() {
        let mut shapes = slide.shapes.clone();
        if fix(&mut shapes, &slide.modes) {
            Arc::make_mut(slide).shapes = shapes;
        }
    }
    for m in doc.masters.iter_mut() {
        let mut mm = (**m).clone();
        let mut changed = fix(&mut mm.shapes, &[]);
        for l in &mut mm.layouts {
            changed |= fix(&mut l.shapes, &[]);
        }
        if changed {
            *m = Arc::new(mm);
        }
    }
}

fn each_shape(list: &mut [Shape], f: &mut dyn FnMut(&mut Shape)) {
    fn go(list: &mut [Shape], f: &mut dyn FnMut(&mut Shape), depth: usize) {
        if depth > deckcraft_model::canvas::MAX_DEPTH {
            return;
        }
        for sh in list.iter_mut() {
            f(sh);
            if let ShapeKind::Group { children, .. } = &mut sh.kind {
                go(children, f, depth + 1);
            }
        }
    }
    go(list, f, 0);
}

/// Point a shape's colour `field` at variable `id`.
fn bind_color(sh: &mut Shape, field: &str, id: u32, last: Rgba) {
    let c = var_color(id, last);
    match field {
        "fill" => sh.fill = Some(Fill::solid(c)),
        "line" => {
            let l = sh.line.get_or_insert_with(Default::default);
            l.fill = Some(Fill::solid(c));
            if l.width.is_none() {
                l.width = Some(1.0);
            }
        }
        _ => {
            if let Some(t) = sh.text.as_mut() {
                for para in &mut t.paragraphs {
                    for r in &mut para.runs {
                        r.props.fill = Some(Fill::solid(c.clone()));
                    }
                    para.end_props.fill = Some(Fill::solid(c.clone()));
                }
            }
        }
    }
}

fn bind(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "variable.bind";
    let field = str_param(p, "field").ok_or_else(|| bad(C, "missing `field`"))?.to_string();
    let doc = s.doc()?.doc.clone();
    let ds = doc.design_system().ok_or_else(|| bad(C, "the deck has no variables"))?;
    let id = variable_id(Some(ds), p.get("variable"), C)?;
    let var = ds.variable(id).ok_or_else(|| bad(C, "no variable"))?;
    let modes = doc.slides.get(s.doc()?.selection.slide).map(|sl| sl.modes.clone()).unwrap_or_default();
    if COLOR_FIELDS.contains(&field.as_str()) {
        if var.kind != VarKind::Color {
            return Err(bad(C, format!("`{field}` takes a colour variable; `{}` is a {}", var.name, var.kind.name())));
        }
        let last = ds.color(id, &modes).unwrap_or(Rgba::BLACK);
        let ids = super::targets(s, p)?;
        super::edit_shapes(s, p, C, |sh| {
            bind_color(sh, &field, id, last);
            Ok(())
        })?;
        return Ok(json!({"ids": ids}));
    }
    let f = BindField::from_name(&field).ok_or_else(|| bad(C, format!("no bindable field `{field}`")))?;
    if var.kind != f.kind() {
        return Err(bad(C, format!("`{field}` takes a {} variable; `{}` is a {}", f.kind().name(), var.name, var.kind.name())));
    }
    let value = ds.resolve(id, &modes).cloned();
    let ids = super::targets(s, p)?;
    super::edit_shapes(s, p, C, |sh| {
        let applies = match f {
            BindField::CornerRadius => sh.frame.is_some(),
            BindField::Gap | BindField::CrossGap | BindField::Padding => sh.auto_layout.is_some(),
            BindField::FontSize | BindField::FontFamily | BindField::Text => sh.text.is_some() || f == BindField::Text,
            _ => true,
        };
        if !applies {
            return Err(bad(C, format!("shape {} has no {field} (a frame or auto layout frame is needed)", sh.id)));
        }
        sh.bindings.retain(|b| b.field != f);
        if sh.bindings.len() >= MAX_BINDINGS {
            return Err(bad(C, format!("a shape keeps at most {MAX_BINDINGS} bindings")));
        }
        sh.bindings.push(Binding { field: f, variable: id });
        if let Some(v) = &value {
            deckcraft_model::variables::apply_one(sh, f, v);
        }
        Ok(())
    })?;
    Ok(json!({"ids": ids}))
}

fn unbind(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "variable.unbind";
    let field = str_param(p, "field").map(str::to_string);
    if let Some(f) = &field
        && !COLOR_FIELDS.contains(&f.as_str())
        && BindField::from_name(f).is_none()
    {
        return Err(bad(C, format!("no bindable field `{f}`")));
    }
    let doc = s.doc()?.doc.clone();
    let modes = doc.slides.get(s.doc()?.selection.slide).map(|sl| sl.modes.clone()).unwrap_or_default();
    let ds = doc.design_system().cloned().unwrap_or_default();
    let ids = super::targets(s, p)?;
    super::edit_shapes(s, p, C, |sh| {
        let want = |path: &str| match field.as_deref() {
            None => true,
            Some("fill") => path == "fill",
            Some("line") => path == "line",
            Some("textColor") => path.starts_with("text/"),
            Some(_) => false,
        };
        visit_bound_colors(sh, &mut |path, c: &mut ColorRef| {
            if let ColorBase::Var { id, last } = c.base
                && want(path)
            {
                c.base = ColorBase::Rgb { rgb: ds.color(id, &modes).unwrap_or(last) };
            }
        });
        match field.as_deref().and_then(BindField::from_name) {
            Some(f) => sh.bindings.retain(|b| b.field != f),
            None if field.is_none() => sh.bindings.clear(),
            None => {}
        }
        Ok(())
    })?;
    Ok(json!({"ids": ids}))
}

fn list(s: &mut Session, _p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let doc = &st.doc;
    let Some(ds) = doc.design_system() else { return Ok(json!({"collections": [], "variables": []})) };
    let modes = doc.slides.get(st.selection.slide).map(|sl| sl.modes.clone()).unwrap_or_default();
    let collections: Vec<Value> = ds
        .collections
        .iter()
        .map(|c| {
            json!({
                "id": c.id,
                "name": c.name,
                "modes": c.modes.iter().map(|m| json!({"id": m.id, "name": m.name})).collect::<Vec<_>>(),
                "mode": ds.deck_mode(c.id),
                "slideMode": ds.mode_of(c.id, &modes),
            })
        })
        .collect();
    let variables: Vec<Value> = ds
        .variables
        .iter()
        .map(|v| {
            let c = ds.collection(v.collection);
            let values: serde_json::Map<String, Value> = v
                .values
                .iter()
                .map(|(m, val)| (c.and_then(|c| c.mode(*m)).map_or_else(|| m.to_string(), |m| m.name.clone()), value_json(val)))
                .collect();
            let mut o = json!({
                "id": v.id,
                "name": v.name,
                "collection": v.collection,
                "type": v.kind.name(),
                "values": values,
                "resolved": ds.resolve(v.id, &modes).map(value_json),
            });
            if let (Some(t), Some(m)) = (v.theme, o.as_object_mut()) {
                m.insert("theme".into(), json!(t.name()));
            }
            if !v.description.is_empty()
                && let Some(m) = o.as_object_mut()
            {
                m.insert("description".into(), json!(v.description));
            }
            o
        })
        .collect();
    Ok(json!({"collections": collections, "variables": variables}))
}

#[cfg(test)]
#[path = "variables_tests.rs"]
mod tests;
