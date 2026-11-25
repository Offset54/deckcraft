//! SlideCraft variables (Phase 4): named values with modes, grouped in collections, that shape
//! properties bind to, so one change restyles everything bound to it.
//!
//! - A **collection** has one or more **modes** (Light and Dark, Compact and Comfortable…). Every
//!   variable belongs to one collection and has a value per mode.
//! - A variable is a colour, number, string or boolean. A value may be an **alias** of another
//!   variable (of the same type, in any collection), resolved in that collection's mode.
//! - The deck has an active mode per collection; a slide may override it ([`crate::Slide::modes`]).
//! - **Colours** bind through [`crate::ColorBase::Var`]: any colour in the deck can name a
//!   variable, and it is resolved when the slide is drawn (`resolve::Ctx::color`), the way theme
//!   colours are. A mode switch therefore rewrites no slide. A colour variable may also map to one
//!   of the theme's 12 colour slots (and a string variable to the heading or body font): the
//!   engine keeps every master's theme equal to the active mode of those variables, so everything
//!   that uses theme colours restyles too, and `.pptx` gets a real theme.
//! - **Numbers, strings and booleans** bind through [`Binding`]s on a shape (corner radius, gaps,
//!   padding, sizes, font size and family, text, visibility). These drive layout, so the engine
//!   writes their resolved value into the bound field (`apply_bindings`), on the slides that bind
//!   a variable whose value changed.
//!
//! Behaviour follows FigCraft's variables; the code is our own (FigCraft's document store is
//! MPL-2.0 `imbl`, outside the licence allowlist).

use deckcraft_color::{ColorScheme, Rgba, SchemeSlot};
use serde::{Deserialize, Serialize};

use crate::style::{ColorBase, ColorRef, Fill};
use crate::{Shape, ShapeKind};

/// Most collections, variables and modes a deck holds (hostile files are cut to these).
pub const MAX_COLLECTIONS: usize = 256;
pub const MAX_VARIABLES: usize = 8192;
pub const MAX_MODES: usize = 64;
/// Longest variable, collection or mode name, in characters.
pub const MAX_NAME: usize = 256;
/// Longest chain of aliases followed.
pub const MAX_ALIAS_DEPTH: usize = 16;
/// Longest string value, in characters.
pub const MAX_STRING: usize = 10_000;
/// Number values are kept within ±this.
pub const MAX_NUMBER: f64 = 1.0e6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VarKind {
    Color,
    Number,
    String,
    Boolean,
}

impl VarKind {
    pub fn name(self) -> &'static str {
        match self {
            VarKind::Color => "color",
            VarKind::Number => "number",
            VarKind::String => "string",
            VarKind::Boolean => "boolean",
        }
    }
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "color" | "colour" => VarKind::Color,
            "number" | "float" => VarKind::Number,
            "string" | "text" => VarKind::String,
            "boolean" | "bool" => VarKind::Boolean,
            _ => return None,
        })
    }
}

/// A variable's value in one mode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VarValue {
    Color(Rgba),
    Number(f64),
    String(String),
    Boolean(bool),
    /// The value of another variable.
    Alias(u32),
}

impl VarValue {
    /// The kind of a direct value (`None` for an alias).
    pub fn kind(&self) -> Option<VarKind> {
        Some(match self {
            VarValue::Color(_) => VarKind::Color,
            VarValue::Number(_) => VarKind::Number,
            VarValue::String(_) => VarKind::String,
            VarValue::Boolean(_) => VarKind::Boolean,
            VarValue::Alias(_) => return None,
        })
    }
    pub fn as_color(&self) -> Option<Rgba> {
        if let VarValue::Color(c) = self { Some(*c) } else { None }
    }
    pub fn as_number(&self) -> Option<f64> {
        if let VarValue::Number(n) = self { Some(*n).filter(|n| n.is_finite()) } else { None }
    }
    pub fn as_str(&self) -> Option<&str> {
        if let VarValue::String(s) = self { Some(s) } else { None }
    }
    pub fn as_bool(&self) -> Option<bool> {
        if let VarValue::Boolean(b) = self { Some(*b) } else { None }
    }
    /// Make a hostile value safe: numbers finite and bounded, strings bounded.
    fn sanitize(&mut self) {
        match self {
            VarValue::Number(n) => *n = if n.is_finite() { n.clamp(-MAX_NUMBER, MAX_NUMBER) } else { 0.0 },
            VarValue::String(s) => truncate(s, MAX_STRING),
            _ => {}
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Mode {
    pub id: u32,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Collection {
    pub id: u32,
    pub name: String,
    /// At least one; the first is the default.
    pub modes: Vec<Mode>,
    /// SlideCraft's theme kit (`design_kit`): the collection Design ▸ Themes and the layout library
    /// keep up to date. Marked rather than found by name, so a deck's own collection called
    /// "Theme" is never overwritten.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub kit: bool,
}

impl Collection {
    pub fn mode(&self, id: u32) -> Option<&Mode> {
        self.modes.iter().find(|m| m.id == id)
    }
    pub fn mode_named(&self, name: &str) -> Option<&Mode> {
        self.modes.iter().find(|m| m.name.eq_ignore_ascii_case(name))
    }
}

/// Where a variable maps into the deck's theme (for the theme gallery, colour pickers and `.pptx`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeSlot {
    /// One of the 12 theme colours (colour variables).
    Color(SchemeSlot),
    /// The heading font (string variables).
    MajorFont,
    /// The body font (string variables).
    MinorFont,
}

impl ThemeSlot {
    pub fn name(self) -> &'static str {
        match self {
            ThemeSlot::Color(s) => s.xml_name(),
            ThemeSlot::MajorFont => "majorFont",
            ThemeSlot::MinorFont => "minorFont",
        }
    }
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "majorFont" | "headings" => Some(ThemeSlot::MajorFont),
            "minorFont" | "body" => Some(ThemeSlot::MinorFont),
            s => SchemeSlot::from_xml(s).filter(|s| THEME_COLOR_SLOTS.contains(s)).map(ThemeSlot::Color),
        }
    }
    pub fn kind(self) -> VarKind {
        if matches!(self, ThemeSlot::Color(_)) { VarKind::Color } else { VarKind::String }
    }
}

/// The theme colour slots a variable can map to, in theme order.
pub const THEME_COLOR_SLOTS: [SchemeSlot; 12] = [
    SchemeSlot::Dk1,
    SchemeSlot::Lt1,
    SchemeSlot::Dk2,
    SchemeSlot::Lt2,
    SchemeSlot::Accent1,
    SchemeSlot::Accent2,
    SchemeSlot::Accent3,
    SchemeSlot::Accent4,
    SchemeSlot::Accent5,
    SchemeSlot::Accent6,
    SchemeSlot::Hlink,
    SchemeSlot::FolHlink,
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Variable {
    pub id: u32,
    /// A path such as `color/accent` or `space/md`: slashes group variables in the panel.
    pub name: String,
    pub collection: u32,
    pub kind: VarKind,
    /// One value per mode of the collection (a missing mode takes the first mode's value).
    pub values: Vec<(u32, VarValue)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<ThemeSlot>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
}

impl Default for Variable {
    fn default() -> Self {
        Variable { id: 0, name: String::new(), collection: 0, kind: VarKind::Color, values: vec![], theme: None, description: String::new() }
    }
}

impl Variable {
    /// The stored value for `mode`, else the first one.
    pub fn value_in(&self, mode: u32) -> Option<&VarValue> {
        self.values.iter().find(|(m, _)| *m == mode).or_else(|| self.values.first()).map(|(_, v)| v)
    }
    pub fn set_value(&mut self, mode: u32, v: VarValue) {
        match self.values.iter_mut().find(|(m, _)| *m == mode) {
            Some(slot) => slot.1 = v,
            None => self.values.push((mode, v)),
        }
    }
}

/// The deck's design system: variables now; styles and components join it in later arcs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DesignSystem {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub collections: Vec<Collection>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub variables: Vec<Variable>,
    /// The deck's active mode per collection (a collection not listed uses its first mode).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub modes: Vec<(u32, u32)>,
    /// Paint, effect and grid styles (Phase 4.2).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub styles: Vec<crate::design_styles::DesignStyle>,
    /// Main components (Phase 4.3).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<crate::component::Component>,
}

impl DesignSystem {
    pub fn is_empty(&self) -> bool {
        *self == DesignSystem::default()
    }
    pub fn collection(&self, id: u32) -> Option<&Collection> {
        self.collections.iter().find(|c| c.id == id)
    }
    /// The theme kit's collection (`design_kit`), when the deck has one.
    pub fn kit(&self) -> Option<&Collection> {
        self.collections.iter().find(|c| c.kit)
    }
    pub fn collection_named(&self, name: &str) -> Option<&Collection> {
        self.collections.iter().find(|c| c.name.eq_ignore_ascii_case(name))
    }
    pub fn variable(&self, id: u32) -> Option<&Variable> {
        self.variables.iter().find(|v| v.id == id)
    }
    pub fn variable_mut(&mut self, id: u32) -> Option<&mut Variable> {
        self.variables.iter_mut().find(|v| v.id == id)
    }
    /// A variable by name (case-insensitive), optionally within a collection.
    pub fn variable_named(&self, name: &str, collection: Option<u32>) -> Option<&Variable> {
        self.variables.iter().find(|v| v.name.eq_ignore_ascii_case(name) && collection.is_none_or(|c| v.collection == c))
    }

    /// The deck's active mode of a collection.
    pub fn deck_mode(&self, collection: u32) -> Option<u32> {
        let c = self.collection(collection)?;
        self.modes.iter().find(|(k, m)| *k == collection && c.mode(*m).is_some()).map(|(_, m)| *m).or_else(|| c.modes.first().map(|m| m.id))
    }

    /// The mode of `collection` under `overrides` (a slide's), else the deck's.
    pub fn mode_of(&self, collection: u32, overrides: &[(u32, u32)]) -> Option<u32> {
        let c = self.collection(collection)?;
        overrides.iter().find(|(k, m)| *k == collection && c.mode(*m).is_some()).map(|(_, m)| *m).or_else(|| self.deck_mode(collection))
    }

    /// The value of variable `id` with aliases followed (at most [`MAX_ALIAS_DEPTH`]), in the
    /// mode `overrides` give its collection, else the deck's. `None` for a missing variable, a
    /// cycle, or a chain too long.
    pub fn resolve(&self, id: u32, overrides: &[(u32, u32)]) -> Option<&VarValue> {
        let mut id = id;
        for _ in 0..=MAX_ALIAS_DEPTH {
            let v = self.variable(id)?;
            let mode = self.mode_of(v.collection, overrides)?;
            match v.value_in(mode)? {
                VarValue::Alias(next) => id = *next,
                value => return Some(value),
            }
        }
        None
    }
    pub fn color(&self, id: u32, overrides: &[(u32, u32)]) -> Option<Rgba> {
        self.resolve(id, overrides).and_then(VarValue::as_color)
    }

    /// Whether `from` aliasing `to` would make a cycle.
    pub fn alias_cycles(&self, from: u32, to: u32) -> bool {
        // Any mode's alias counts: walk every alias edge breadth first from `to`.
        let mut seen = std::collections::HashSet::new();
        let mut stack = vec![to];
        while let Some(id) = stack.pop() {
            if id == from {
                return true;
            }
            if !seen.insert(id) || seen.len() > MAX_VARIABLES {
                continue;
            }
            if let Some(v) = self.variable(id) {
                stack.extend(v.values.iter().filter_map(|(_, val)| if let VarValue::Alias(a) = val { Some(*a) } else { None }));
            }
        }
        false
    }

    /// The theme colours and fonts the variables give, in the deck's modes overridden by
    /// `overrides`: `(slot, value)` for each variable mapped into the theme.
    pub fn theme_values(&self, overrides: &[(u32, u32)]) -> Vec<(ThemeSlot, VarValue)> {
        self.variables
            .iter()
            .filter_map(|v| Some((v.theme?, self.resolve(v.id, overrides)?.clone())))
            .filter(|(t, val)| val.kind() == Some(t.kind()))
            .collect()
    }

    /// Write the theme values into `theme` (deck modes); whether anything changed.
    pub fn apply_to_theme(&self, theme: &mut crate::Theme) -> bool {
        let mut changed = false;
        for (slot, value) in self.theme_values(&[]) {
            match (slot, value) {
                (ThemeSlot::Color(s), VarValue::Color(c)) => {
                    if theme.colors.get(s) != c {
                        theme.colors.set(s, c);
                        changed = true;
                    }
                }
                (ThemeSlot::MajorFont, VarValue::String(f)) if !f.is_empty() && theme.fonts.major.latin != f => {
                    theme.fonts.major.latin = f;
                    changed = true;
                }
                (ThemeSlot::MinorFont, VarValue::String(f)) if !f.is_empty() && theme.fonts.minor.latin != f => {
                    theme.fonts.minor.latin = f;
                    changed = true;
                }
                _ => {}
            }
        }
        changed
    }

    /// Take the theme's colours and fonts into the active mode of the variables mapped to them
    /// (after Design ▸ Themes or Colors changed the theme): direct values only, aliases stay.
    pub fn take_from_theme(&mut self, theme: &crate::Theme) -> bool {
        let mut changed = false;
        let modes: Vec<(u32, Option<u32>)> = self.collections.iter().map(|c| (c.id, self.deck_mode(c.id))).collect();
        for v in &mut self.variables {
            let Some(slot) = v.theme else { continue };
            let Some(mode) = modes.iter().find(|(c, _)| *c == v.collection).and_then(|(_, m)| *m) else { continue };
            let want = match slot {
                ThemeSlot::Color(s) => VarValue::Color(theme.colors.get(s)),
                ThemeSlot::MajorFont => VarValue::String(theme.fonts.major.latin.clone()),
                ThemeSlot::MinorFont => VarValue::String(theme.fonts.minor.latin.clone()),
            };
            match v.value_in(mode) {
                Some(VarValue::Alias(_)) => {}
                Some(cur) if *cur == want => {}
                _ => {
                    v.set_value(mode, want);
                    changed = true;
                }
            }
        }
        changed
    }

    /// The theme colour scheme `base` with slide `overrides` applied, when they change any
    /// colour (`None` otherwise, so callers keep using the theme as it is).
    pub fn scheme_with(&self, base: &ColorScheme, overrides: &[(u32, u32)]) -> Option<ColorScheme> {
        if overrides.is_empty() {
            return None;
        }
        let mut out = base.clone();
        let mut changed = false;
        for (slot, value) in self.theme_values(overrides) {
            if let (ThemeSlot::Color(s), VarValue::Color(c)) = (slot, value)
                && out.get(s) != c
            {
                out.set(s, c);
                changed = true;
            }
        }
        changed.then_some(out)
    }

    /// Cut a hostile design system down to the limits and drop what doesn't hold together:
    /// collections without modes, variables of missing collections, values of the wrong kind,
    /// theme slots of the wrong kind.
    pub fn sanitize(&mut self) {
        self.collections.truncate(MAX_COLLECTIONS);
        for c in &mut self.collections {
            truncate(&mut c.name, MAX_NAME);
            c.modes.truncate(MAX_MODES);
            for m in &mut c.modes {
                truncate(&mut m.name, MAX_NAME);
            }
            let mut seen = std::collections::HashSet::new();
            c.modes.retain(|m| seen.insert(m.id));
        }
        self.collections.retain(|c| !c.modes.is_empty());
        let mut seen = std::collections::HashSet::new();
        self.collections.retain(|c| seen.insert(c.id));
        // One kit at most.
        let mut seen_kit = false;
        for c in &mut self.collections {
            if c.kit && std::mem::replace(&mut seen_kit, true) {
                c.kit = false;
            }
        }
        self.variables.truncate(MAX_VARIABLES);
        let colls: Vec<(u32, Vec<u32>)> = self.collections.iter().map(|c| (c.id, c.modes.iter().map(|m| m.id).collect())).collect();
        let mut seen = std::collections::HashSet::new();
        self.variables.retain(|v| seen.insert(v.id) && colls.iter().any(|(c, _)| *c == v.collection));
        for v in &mut self.variables {
            truncate(&mut v.name, MAX_NAME);
            truncate(&mut v.description, MAX_STRING);
            let modes = colls.iter().find(|(c, _)| *c == v.collection).map(|(_, m)| m.as_slice()).unwrap_or_default();
            let kind = v.kind;
            v.values.retain(|(m, val)| modes.contains(m) && val.kind().is_none_or(|k| k == kind));
            v.values.truncate(MAX_MODES);
            for (_, val) in &mut v.values {
                val.sanitize();
            }
            if v.theme.is_some_and(|t| t.kind() != kind) {
                v.theme = None;
            }
        }
        // An alias must name a variable of the same kind.
        let kinds: std::collections::HashMap<u32, VarKind> = self.variables.iter().map(|v| (v.id, v.kind)).collect();
        for v in &mut self.variables {
            let kind = v.kind;
            v.values.retain(|(_, val)| !matches!(val, VarValue::Alias(a) if kinds.get(a) != Some(&kind)));
        }
        self.modes.retain(|(c, m)| colls.iter().any(|(k, ms)| k == c && ms.contains(m)));
        self.modes.truncate(MAX_COLLECTIONS);
        crate::design_styles::sanitize(&mut self.styles);
        crate::component::sanitize(&mut self.components);
    }

    /// Every id the design system uses (for `Presentation::fix_next_id`).
    pub fn max_id(&self) -> u32 {
        let mut m = 0;
        for c in &self.collections {
            m = m.max(c.id);
            for md in &c.modes {
                m = m.max(md.id);
            }
        }
        for v in &self.variables {
            m = m.max(v.id);
        }
        for s in &self.styles {
            m = m.max(s.id);
        }
        for c in &self.components {
            m = m.max(c.id).max(crate::component::max_shape_id(c));
        }
        m
    }
}

pub(crate) fn truncate(s: &mut String, max: usize) {
    if let Some((i, _)) = s.char_indices().nth(max) {
        s.truncate(i);
    }
}

// ---------- number, string and boolean bindings ----------

/// A shape property a number, string or boolean variable can drive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BindField {
    /// A frame's corner radius (pt).
    CornerRadius,
    /// An auto layout frame's gap between members (pt).
    Gap,
    /// An auto layout frame's gap between rows or columns when wrapping, or grid rows (pt).
    CrossGap,
    /// An auto layout frame's padding on every side (pt).
    Padding,
    /// Width and height of the box (pt).
    Width,
    Height,
    /// Outline width (pt).
    LineWidth,
    /// Every run's font size (pt).
    FontSize,
    /// Every run's font family.
    FontFamily,
    /// The text, as plain text (each line a paragraph).
    Text,
    /// Shown or hidden.
    Visible,
}

pub const BIND_FIELDS: [BindField; 11] = [
    BindField::CornerRadius,
    BindField::Gap,
    BindField::CrossGap,
    BindField::Padding,
    BindField::Width,
    BindField::Height,
    BindField::LineWidth,
    BindField::FontSize,
    BindField::FontFamily,
    BindField::Text,
    BindField::Visible,
];

impl BindField {
    pub fn name(self) -> &'static str {
        match self {
            BindField::CornerRadius => "cornerRadius",
            BindField::Gap => "gap",
            BindField::CrossGap => "crossGap",
            BindField::Padding => "padding",
            BindField::Width => "width",
            BindField::Height => "height",
            BindField::LineWidth => "lineWidth",
            BindField::FontSize => "fontSize",
            BindField::FontFamily => "fontFamily",
            BindField::Text => "text",
            BindField::Visible => "visible",
        }
    }
    pub fn from_name(s: &str) -> Option<Self> {
        BIND_FIELDS.into_iter().find(|f| f.name() == s)
    }
    /// The kind of variable the field takes.
    pub fn kind(self) -> VarKind {
        match self {
            BindField::FontFamily | BindField::Text => VarKind::String,
            BindField::Visible => VarKind::Boolean,
            _ => VarKind::Number,
        }
    }
    /// The range a number is clamped to before it is written.
    fn range(self) -> (f64, f64) {
        match self {
            BindField::FontSize => (1.0, 4000.0),
            BindField::CornerRadius => (0.0, crate::canvas::Frame::MAX_RADIUS),
            BindField::Width | BindField::Height => (0.0, 1.0e5),
            BindField::LineWidth => (0.0, 1584.0),
            _ => (0.0, 1.0e4),
        }
    }
}

/// A shape property bound to a variable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    pub field: BindField,
    pub variable: u32,
}

/// Most bindings one shape keeps.
pub const MAX_BINDINGS: usize = 32;

/// Write the resolved value of every binding in `list` (and in groups, to `crate::canvas::MAX_DEPTH`)
/// into its field. Whether anything changed.
pub fn apply_bindings(ds: &DesignSystem, list: &mut [Shape], overrides: &[(u32, u32)]) -> bool {
    apply_list(ds, list, overrides, 0)
}

fn apply_list(ds: &DesignSystem, list: &mut [Shape], overrides: &[(u32, u32)], depth: usize) -> bool {
    if depth > crate::canvas::MAX_DEPTH {
        return false;
    }
    let mut changed = false;
    for sh in list.iter_mut() {
        if !sh.bindings.is_empty() {
            let bindings = sh.bindings.clone();
            for b in bindings.iter().take(MAX_BINDINGS) {
                if let Some(v) = ds.resolve(b.variable, overrides) {
                    changed |= apply_one(sh, b.field, v);
                }
            }
        }
        if let ShapeKind::Group { children, .. } = &mut sh.kind {
            changed |= apply_list(ds, children, overrides, depth + 1);
        }
    }
    changed
}

/// Whether any shape in `list` (or its groups) has a binding.
pub fn has_bindings(list: &[Shape]) -> bool {
    let mut found = false;
    crate::walk(list, &mut |s, _| found |= !s.bindings.is_empty());
    found
}

fn set<T: PartialEq>(slot: &mut T, v: T) -> bool {
    if *slot == v {
        false
    } else {
        *slot = v;
        true
    }
}

/// Write one resolved value into `field` of `sh`; whether it changed.
pub fn apply_one(sh: &mut Shape, field: BindField, v: &VarValue) -> bool {
    if let Some(n) = v.as_number() {
        let (lo, hi) = field.range();
        let n = n.clamp(lo, hi);
        return match field {
            BindField::CornerRadius => sh.frame.as_mut().is_some_and(|f| set(&mut f.radius, n)),
            BindField::Gap => sh.auto_layout.as_mut().is_some_and(|a| set(&mut a.gap, n)),
            BindField::CrossGap => sh.auto_layout.as_mut().is_some_and(|a| set(&mut a.cross_gap, n)),
            BindField::Padding => sh.auto_layout.as_mut().is_some_and(|a| set(&mut a.padding, [n; 4])),
            BindField::Width => sh.xfrm.as_mut().is_some_and(|x| set(&mut x.w, n)),
            BindField::Height => sh.xfrm.as_mut().is_some_and(|x| set(&mut x.h, n)),
            BindField::LineWidth => set(&mut sh.line.get_or_insert_with(Default::default).width, Some(n)),
            BindField::FontSize => each_run(sh, |r| set(&mut r.size, Some(n))),
            _ => false,
        };
    }
    if let Some(s) = v.as_str() {
        return match field {
            BindField::FontFamily if !s.is_empty() => each_run(sh, |r| set(&mut r.font, Some(s.to_string()))),
            BindField::Text => set_text(sh, s),
            _ => false,
        };
    }
    if let (Some(b), BindField::Visible) = (v.as_bool(), field) {
        return set(&mut sh.hidden, !b);
    }
    false
}

/// Apply `f` to every run's properties, and to each paragraph's end-of-paragraph properties so
/// typing after the last run looks the same.
fn each_run(sh: &mut Shape, f: impl Fn(&mut crate::RunProps) -> bool) -> bool {
    let Some(t) = sh.text.as_mut() else { return false };
    let mut changed = false;
    for p in &mut t.paragraphs {
        for r in &mut p.runs {
            changed |= f(&mut r.props);
        }
        changed |= f(&mut p.end_props);
    }
    changed
}

/// Replace a shape's text with `s`, keeping the first paragraph's and first run's formatting.
fn set_text(sh: &mut Shape, s: &str) -> bool {
    let body = sh.text.get_or_insert_with(Default::default);
    if body.text() == s {
        return false;
    }
    let first = body.paragraphs.first().cloned().unwrap_or_default();
    let run_props = first.runs.first().map(|r| r.props.clone()).unwrap_or_default();
    body.paragraphs = s
        .split('\n')
        .take(10_000)
        .map(|line| {
            let mut p = crate::Paragraph { runs: vec![], ..first.clone() };
            if !line.is_empty() {
                p.runs.push(crate::Run { text: line.to_string(), props: run_props.clone(), kind: crate::text::RunKind::Text });
            }
            p
        })
        .collect();
    true
}

// ---------- colour bindings ----------

/// A colour that names variable `id`; `last` is its value when it was last resolved (what
/// readers without the variable see).
pub fn var_color(id: u32, last: Rgba) -> ColorRef {
    ColorRef { base: ColorBase::Var { id, last }, mods: vec![] }
}

/// Visit the colours of a shape a colour variable is bound through, with a stable path for
/// each: `fill`, `line`, `text/<paragraph>/<run>` and `text/<paragraph>/end`. Group members are
/// not visited (each shape keeps its own).
pub fn visit_bound_colors(sh: &mut Shape, f: &mut dyn FnMut(&str, &mut ColorRef)) {
    if let Some(Fill::Solid { color }) = sh.fill.as_mut() {
        f("fill", color);
    }
    if let Some(Fill::Solid { color }) = sh.line.as_mut().and_then(|l| l.fill.as_mut()) {
        f("line", color);
    }
    if let Some(t) = sh.text.as_mut() {
        for (pi, p) in t.paragraphs.iter_mut().enumerate().take(10_000) {
            for (ri, r) in p.runs.iter_mut().enumerate().take(10_000) {
                if let Some(Fill::Solid { color }) = r.props.fill.as_mut() {
                    f(&format!("text/{pi}/{ri}"), color);
                }
            }
            if let Some(Fill::Solid { color }) = p.end_props.fill.as_mut() {
                f(&format!("text/{pi}/end"), color);
            }
        }
    }
}

/// The colour variables bound in `sh` (not its members), as `(path, variable)`.
pub fn color_bindings(sh: &Shape) -> Vec<(String, u32)> {
    let mut sh = sh.clone();
    let mut out = vec![];
    visit_bound_colors(&mut sh, &mut |path, c| {
        if let ColorBase::Var { id, .. } = c.base {
            out.push((path.to_string(), id));
        }
    });
    out
}

/// Refresh the `last` value of every variable colour in `list` (and its groups): for writers
/// that can't resolve variables (`.pptx`). Whether anything changed.
pub fn refresh_last(ds: &DesignSystem, list: &mut [Shape], overrides: &[(u32, u32)]) -> bool {
    let mut changed = false;
    fn go(ds: &DesignSystem, list: &mut [Shape], o: &[(u32, u32)], depth: usize, changed: &mut bool) {
        if depth > crate::canvas::MAX_DEPTH {
            return;
        }
        for sh in list.iter_mut() {
            visit_bound_colors(sh, &mut |_, c| {
                if let ColorBase::Var { id, last } = &mut c.base
                    && let Some(v) = ds.color(*id, o)
                    && *last != v
                {
                    *last = v;
                    *changed = true;
                }
            });
            if let ShapeKind::Group { children, .. } = &mut sh.kind {
                go(ds, children, o, depth + 1, changed);
            }
        }
    }
    go(ds, list, overrides, 0, &mut changed);
    changed
}

#[cfg(test)]
#[path = "variables_tests.rs"]
mod tests;
