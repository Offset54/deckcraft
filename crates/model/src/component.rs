//! SlideCraft components (Phase 4.3): reusable slide parts (title block, stat card, quote, logo
//! row, footer…) with variants and properties, placed as instances that take overrides, as in
//! FigCraft.
//!
//! - A **component** in the deck's library ([`crate::variables::DesignSystem::components`]) has a
//!   root frame (`root`) whose members are its content. Components that share a `set` name are
//!   the **variants** of one component set; each says its value for every variant property
//!   (`Size = Large`, `Tone = Dark`).
//! - **Properties** expose parts of the content: a text property sets the text of its target
//!   layers, a boolean property shows or hides them, and an instance-swap property picks which
//!   component a nested instance shows.
//! - An **instance** is a frame on a slide (`Shape::instance`) whose members are a copy of its
//!   component's content with the instance's property values and **overrides** (text, fill,
//!   visibility of a layer) applied. Editing the component updates every instance; overrides
//!   survive. Layers are matched by the component's own layer ids, mapped to the instance's ids
//!   ([`Instance::map`]); a variant or swap carries overrides across by layer name.
//! - The component's **main** may sit on a slide (`Shape::main_of`): editing it there with any
//!   tool updates the library, and with it every instance.
//!
//! Re-implemented against the host's nested shape tree (decision 002); FigCraft's component code
//! lives on its `imbl` document (MPL-2.0).

use serde::{Deserialize, Serialize};

use crate::style::Fill;
use crate::text::TextBody;
use crate::{Shape, ShapeId, ShapeKind};

/// Most components a deck holds.
pub const MAX_COMPONENTS: usize = 4096;
/// Most properties, overrides or variant values one component or instance keeps.
pub const MAX_PROPS: usize = 64;
pub const MAX_OVERRIDES: usize = 1024;
/// Deepest nesting of instances inside components that is expanded.
pub const MAX_NESTING: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PropKind {
    Text,
    Boolean,
    /// Which component a nested instance shows.
    Swap,
}

impl PropKind {
    pub fn name(self) -> &'static str {
        match self {
            PropKind::Text => "text",
            PropKind::Boolean => "boolean",
            PropKind::Swap => "swap",
        }
    }
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "text" => PropKind::Text,
            "boolean" | "bool" => PropKind::Boolean,
            "swap" | "instance" => PropKind::Swap,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PropValue {
    Text(String),
    Boolean(bool),
    /// A component id.
    Swap(u32),
}

impl PropValue {
    pub fn kind(&self) -> PropKind {
        match self {
            PropValue::Text(_) => PropKind::Text,
            PropValue::Boolean(_) => PropKind::Boolean,
            PropValue::Swap(_) => PropKind::Swap,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropDef {
    pub name: String,
    pub default: PropValue,
    /// The component's layers it drives (ids inside `Component::root`).
    pub targets: Vec<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Component {
    pub id: u32,
    pub name: String,
    /// The component set it is a variant of (empty: a component on its own).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub set: String,
    /// Its value for each variant property of the set.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub variant: Vec<(String, String)>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub props: Vec<PropDef>,
    /// The root frame: its box is the component's size, its members the content.
    pub root: Shape,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
}

impl Component {
    /// The name shown in the Assets panel: `Set / Size=Large, Tone=Dark` for a variant.
    pub fn display_name(&self) -> String {
        if self.set.is_empty() {
            return self.name.clone();
        }
        let v: Vec<String> = self.variant.iter().map(|(k, v)| format!("{k}={v}")).collect();
        format!("{} / {}", self.set, if v.is_empty() { self.name.clone() } else { v.join(", ") })
    }
    pub fn prop(&self, name: &str) -> Option<&PropDef> {
        self.props.iter().find(|p| p.name.eq_ignore_ascii_case(name))
    }
    /// A layer of the content by id.
    pub fn layer(&self, id: u32) -> Option<&Shape> {
        crate::find_shape(self.root.children(), ShapeId(id))
    }
    /// A layer of the content by name (the first in drawing order).
    pub fn layer_named(&self, name: &str) -> Option<&Shape> {
        let mut out = None;
        crate::walk(self.root.children(), &mut |s, _| {
            if out.is_none() && s.name == name {
                out = Some(s);
            }
        });
        out
    }
}

/// An override of one layer of an instance.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Override {
    /// The component's layer id.
    pub layer: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<TextBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<Fill>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
}

impl Override {
    fn is_empty(&self) -> bool {
        self.text.is_none() && self.fill.is_none() && self.hidden.is_none()
    }
}

/// What makes a frame an instance of a component.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Instance {
    pub component: u32,
    /// Property values that differ from the defaults.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub props: Vec<(String, PropValue)>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<Override>,
    /// Component layer id → this instance's layer id, so ids stay put across updates.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub map: Vec<(u32, u32)>,
}

impl Instance {
    /// The override of `layer`, added when it has none.
    pub fn override_mut(&mut self, layer: u32) -> Option<&mut Override> {
        if !self.overrides.iter().any(|o| o.layer == layer) {
            self.overrides.push(Override { layer, ..Default::default() });
        }
        self.overrides.iter_mut().find(|o| o.layer == layer)
    }
    pub fn prop(&self, name: &str) -> Option<&PropValue> {
        self.props.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v)
    }
    /// The component layer an instance layer comes from.
    pub fn layer_of(&self, own: ShapeId) -> Option<u32> {
        self.map.iter().find(|(_, i)| *i == own.0).map(|(c, _)| *c)
    }
}

pub(crate) fn sanitize(c: &mut Vec<Component>) {
    c.truncate(MAX_COMPONENTS);
    let mut seen = std::collections::HashSet::new();
    c.retain(|x| seen.insert(x.id) && matches!(x.root.kind, ShapeKind::Group { .. }));
    for x in c.iter_mut() {
        crate::variables::truncate(&mut x.name, crate::variables::MAX_NAME);
        crate::variables::truncate(&mut x.set, crate::variables::MAX_NAME);
        crate::variables::truncate(&mut x.description, crate::variables::MAX_STRING);
        x.variant.truncate(MAX_PROPS);
        x.props.truncate(MAX_PROPS);
        for p in &mut x.props {
            p.targets.truncate(MAX_OVERRIDES);
        }
        // A component is not an instance of anything, and not a main on a slide.
        x.root.instance = None;
        x.root.main_of = None;
    }
}

pub(crate) fn max_shape_id(c: &Component) -> u32 {
    let mut m = c.root.id.0;
    crate::walk(c.root.children(), &mut |s, _| m = m.max(s.id.0));
    m
}

/// Ids for new layers of an instance.
pub trait IdSource {
    fn next_id(&mut self) -> Option<u32>;
}

impl IdSource for crate::Presentation {
    fn next_id(&mut self) -> Option<u32> {
        self.alloc_id()
    }
}

/// Build instance `inst`'s frame from `comp` (looked up in `library` for nested instances): the
/// root's look and layout, the content with properties and overrides applied, and the instance's
/// own id, name, box and `instance`. `None` when ids run out.
pub fn materialize(library: &[Component], comp: &Component, frame: &Shape, inst: &Instance, ids: &mut dyn IdSource) -> Option<Shape> {
    materialize_at(library, comp, frame, inst, ids, 0)
}

fn materialize_at(library: &[Component], comp: &Component, frame: &Shape, inst: &Instance, ids: &mut dyn IdSource, depth: usize) -> Option<Shape> {
    let mut out = comp.root.clone();
    out.id = frame.id;
    out.name = frame.name.clone();
    out.xfrm = frame.xfrm.or(comp.root.xfrm);
    out.layout_child = frame.layout_child;
    out.constraints = frame.constraints;
    out.hidden = frame.hidden;
    out.locked = frame.locked;
    out.main_of = None;
    let mut inst = inst.clone();
    let ShapeKind::Group { children, .. } = &mut out.kind else { return Some(out) };
    // Properties, then overrides, on the component's own layer ids.
    for def in &comp.props {
        let v = inst.prop(&def.name).filter(|v| v.kind() == def.default.kind()).unwrap_or(&def.default).clone();
        for t in &def.targets {
            let Some(layer) = crate::find_shape_mut(children, ShapeId(*t)) else { continue };
            match &v {
                PropValue::Text(s) => {
                    crate::variables::apply_one(layer, crate::variables::BindField::Text, &crate::variables::VarValue::String(s.clone()));
                }
                PropValue::Boolean(b) => layer.hidden = !b,
                PropValue::Swap(c) => {
                    if let Some(n) = layer.instance.as_mut() {
                        n.component = *c;
                    }
                }
            }
        }
    }
    for o in &inst.overrides {
        let Some(layer) = crate::find_shape_mut(children, ShapeId(o.layer)) else { continue };
        if let Some(t) = &o.text {
            layer.text = Some(t.clone());
        }
        if let Some(f) = &o.fill {
            layer.fill = Some(f.clone());
        }
        if let Some(h) = o.hidden {
            layer.hidden = h;
        }
    }
    // Nested instances show their own component.
    if depth < MAX_NESTING {
        expand_nested(library, children, ids, depth)?;
    }
    // The instance's own ids.
    renumber(children, &mut inst.map, ids)?;
    out.instance = Some(Box::new(inst));
    Some(out)
}

fn expand_nested(library: &[Component], list: &mut [Shape], ids: &mut dyn IdSource, depth: usize) -> Option<()> {
    for sh in list.iter_mut() {
        if let Some(n) = sh.instance.clone() {
            if let Some(c) = library.iter().find(|c| c.id == n.component) {
                let built = materialize_at(library, c, sh, &n, ids, depth + 1)?;
                *sh = built;
            }
            continue;
        }
        if let ShapeKind::Group { children, .. } = &mut sh.kind {
            expand_nested(library, children, ids, depth)?;
        }
    }
    Some(())
}

/// Give every layer under `list` its instance id from `map`, adding fresh ids for new layers;
/// entries for layers that are gone are dropped.
fn renumber(list: &mut [Shape], map: &mut Vec<(u32, u32)>, ids: &mut dyn IdSource) -> Option<()> {
    let mut used = vec![];
    fn go(list: &mut [Shape], map: &mut Vec<(u32, u32)>, used: &mut Vec<(u32, u32)>, ids: &mut dyn IdSource, depth: usize) -> Option<()> {
        if depth > crate::canvas::MAX_DEPTH {
            return Some(());
        }
        for sh in list.iter_mut() {
            let src = sh.id.0;
            let id = match map.iter().find(|(c, _)| *c == src) {
                Some((_, i)) => *i,
                None => ids.next_id()?,
            };
            used.push((src, id));
            sh.id = ShapeId(id);
            // A nested instance keeps its own map; only its frame is renumbered here.
            if sh.instance.is_some() {
                continue;
            }
            if let ShapeKind::Group { children, .. } = &mut sh.kind {
                go(children, map, used, ids, depth + 1)?;
            }
        }
        Some(())
    }
    go(list, map, &mut used, ids, 0)?;
    *map = used;
    Some(())
}

/// Overrides an instance's layers now carry: text, fill and visibility that differ from what the
/// component (with the instance's properties and earlier overrides) gives. For capturing edits
/// made with ordinary commands (typing into an instance). Whether `inst` changed.
pub fn capture_overrides(library: &[Component], comp: &Component, frame: &Shape, inst: &mut Instance) -> bool {
    struct NoIds;
    impl IdSource for NoIds {
        fn next_id(&mut self) -> Option<u32> {
            None
        }
    }
    // What the instance would be without new edits, in its own ids.
    let Some(expected) = materialize(library, comp, frame, inst, &mut NoIds) else { return false };
    let mut changed = false;
    let mut want: Vec<(u32, &Shape)> = vec![];
    crate::walk(expected.children(), &mut |s, _| want.push((s.id.0, s)));
    let mut have: Vec<&Shape> = vec![];
    crate::walk(frame.children(), &mut |s, _| have.push(s));
    for s in have {
        let Some(layer) = inst.layer_of(s.id) else { continue };
        let Some((_, e)) = want.iter().find(|(id, _)| *id == s.id.0) else { continue };
        if s.text != e.text
            && s.text.is_some()
            && let Some(o) = inst.override_mut(layer)
        {
            o.text = s.text.clone();
            changed = true;
        }
        if s.fill != e.fill
            && s.fill.is_some()
            && let Some(o) = inst.override_mut(layer)
        {
            o.fill = s.fill.clone();
            changed = true;
        }
        if s.hidden != e.hidden
            && let Some(o) = inst.override_mut(layer)
        {
            o.hidden = Some(s.hidden);
            changed = true;
        }
    }
    inst.overrides.retain(|o| !o.is_empty());
    inst.overrides.truncate(MAX_OVERRIDES);
    changed
}

/// Carry `inst`'s overrides and map from component `from` to component `to` by layer name (a
/// variant switch or a swap). Overrides of layers `to` doesn't have are dropped.
pub fn carry_over(from: &Component, to: &Component, inst: &mut Instance) {
    let by_name = |id: u32| from.layer(id).map(|l| l.name.clone()).filter(|n| !n.is_empty()).and_then(|n| to.layer_named(&n)).map(|l| l.id.0);
    inst.overrides = inst.overrides.iter().filter_map(|o| Some(Override { layer: by_name(o.layer)?, ..o.clone() })).collect();
    inst.map = inst.map.iter().filter_map(|(c, i)| Some((by_name(*c)?, *i))).collect();
    let mut seen = std::collections::HashSet::new();
    inst.map.retain(|(c, i)| seen.insert(*c) && seen.insert(u32::MAX - *i));
    inst.component = to.id;
    // Properties the new component doesn't have go.
    inst.props.retain(|(n, v)| to.prop(n).is_some_and(|d| d.default.kind() == v.kind()));
}

/// The variant of `set` whose values match `current` with `want` applied.
pub fn find_variant<'a>(library: &'a [Component], set: &str, current: &[(String, String)], want: &[(String, String)]) -> Option<&'a Component> {
    let mut goal: Vec<(String, String)> = current.to_vec();
    for (k, v) in want {
        match goal.iter_mut().find(|(gk, _)| gk.eq_ignore_ascii_case(k)) {
            Some(slot) => slot.1 = v.clone(),
            None => goal.push((k.clone(), v.clone())),
        }
    }
    let matches =
        |c: &&Component| goal.iter().all(|(k, v)| c.variant.iter().any(|(ck, cv)| ck.eq_ignore_ascii_case(k) && cv.eq_ignore_ascii_case(v)));
    library.iter().filter(|c| !set.is_empty() && c.set == set).find(matches)
}

#[cfg(test)]
#[path = "component_tests.rs"]
mod tests;
