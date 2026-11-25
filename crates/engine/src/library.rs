//! SlideCraft's layout library (Phase 4.4): 34 ready-made slides built from frames, auto layout,
//! the theme kit's variables (`deckcraft_model::design_kit`) and nine library components, so
//! they reflow when their text changes and restyle with the theme and its Light/Dark modes.
//!
//! Every colour is a theme colour or a kit colour variable; font sizes, gaps, padding and corner
//! radii are bound to the kit's type, spacing and radius variables. Repeated parts (stat card,
//! feature card, quote, person, step, agenda item, logo, price tier, check item) are instances of
//! components the library adds to the deck the first time a layout needs them, so editing one
//! main restyles every slide that uses it.
//!
//! Library slides are slide content on the Blank layout (PowerPoint sees groups of ordinary
//! shapes), not master layouts with placeholders; `slide.fromLibrary` makes them.

use std::collections::HashMap;
use std::sync::Arc;

use deckcraft_color::SchemeSlot;
use deckcraft_geom::Xfrm;
use deckcraft_model::canvas::{Align as LAlign, AutoLayout, ChildLayout, Flow, Frame, Sizing, Track};
use deckcraft_model::component::{Component, Instance, PropDef, PropValue};
use deckcraft_model::design_kit;
use deckcraft_model::style::{Gradient, GradientShape, GradientStop};
use deckcraft_model::text::{Align, Anchor, AutoFit, ParaProps, Paragraph, Run, RunKind, RunProps, Spacing, TextBody};
use deckcraft_model::variables::{BindField, Binding, Collection, DesignSystem, Mode, Variable, var_color};
use deckcraft_model::{ColorRef, Fill, Geom, Line, Presentation, Shape, ShapeId, ShapeKind, Slide};

/// A library layout: id, name shown in the gallery, and its builder.
pub struct LayoutDef {
    pub id: &'static str,
    pub name: &'static str,
    build: fn(&mut B) -> Option<Vec<Shape>>,
}

/// Every layout in the library, in gallery order.
pub fn layouts() -> &'static [LayoutDef] {
    LAYOUTS
}

const LAYOUTS: &[LayoutDef] = &[
    LayoutDef { id: "title", name: "Title", build: title },
    LayoutDef { id: "titleBand", name: "Title with Band", build: title_band },
    LayoutDef { id: "section", name: "Section", build: section },
    LayoutDef { id: "sectionBand", name: "Section Band", build: section_band },
    LayoutDef { id: "agenda", name: "Agenda", build: agenda },
    LayoutDef { id: "bullets", name: "Title and Bullets", build: bullets },
    LayoutDef { id: "twoColumns", name: "Two Columns", build: two_columns },
    LayoutDef { id: "threeUp", name: "Three Up", build: three_up },
    LayoutDef { id: "fourUp", name: "Four Up", build: four_up },
    LayoutDef { id: "stats", name: "Three Stats", build: stats },
    LayoutDef { id: "bigNumber", name: "Big Number", build: big_number },
    LayoutDef { id: "quote", name: "Quote", build: quote },
    LayoutDef { id: "quotePhoto", name: "Quote with Image", build: quote_photo },
    LayoutDef { id: "imageLeft", name: "Image Left", build: image_left },
    LayoutDef { id: "imageRight", name: "Image Right", build: image_right },
    LayoutDef { id: "fullBleed", name: "Full-Bleed Image", build: full_bleed },
    LayoutDef { id: "twoImages", name: "Two Images", build: two_images },
    LayoutDef { id: "gallery", name: "Gallery", build: gallery },
    LayoutDef { id: "comparison", name: "Comparison", build: comparison },
    LayoutDef { id: "timeline", name: "Timeline", build: timeline },
    LayoutDef { id: "process", name: "Process", build: process },
    LayoutDef { id: "team", name: "Team", build: team },
    LayoutDef { id: "teamBios", name: "Team with Bios", build: team_bios },
    LayoutDef { id: "logos", name: "Logo Wall", build: logos },
    LayoutDef { id: "pricing", name: "Pricing", build: pricing },
    LayoutDef { id: "keyFacts", name: "Key Facts", build: key_facts },
    LayoutDef { id: "chart", name: "Chart and Takeaways", build: chart },
    LayoutDef { id: "statement", name: "Statement", build: statement },
    LayoutDef { id: "questions", name: "Questions", build: questions },
    LayoutDef { id: "closing", name: "Closing", build: closing },
    LayoutDef { id: "contact", name: "Contact", build: contact },
    LayoutDef { id: "checklist", name: "Checklist", build: checklist },
    LayoutDef { id: "beforeAfter", name: "Before and After", build: before_after },
    LayoutDef { id: "dashboard", name: "Dashboard", build: dashboard },
];

// ---------- the kit in the deck ----------

/// Make sure the deck has the theme kit's collection (from its first master's theme), adding any
/// variable it lacks; returns the kit's variable ids by name. `None` when ids run out.
pub fn ensure_kit(doc: &mut Presentation) -> Option<HashMap<String, u32>> {
    let theme = doc.masters.first().map(|m| m.theme.clone()).unwrap_or_default();
    let kit = design_kit::kit(&theme);
    let mut ds: DesignSystem = doc.design_system().cloned().unwrap_or_default();
    let coll = match ds.kit() {
        Some(c) => c.clone(),
        None => {
            let id = doc.alloc_id()?;
            let light = Mode { id: doc.alloc_id()?, name: design_kit::LIGHT.into() };
            let dark = Mode { id: doc.alloc_id()?, name: design_kit::DARK.into() };
            // Named "Theme", unless the deck already has a collection of that name.
            let name = if ds.collection_named(design_kit::COLLECTION).is_some() {
                format!("{} kit", design_kit::COLLECTION)
            } else {
                design_kit::COLLECTION.to_string()
            };
            let c = Collection { id, name, modes: vec![light, dark], kit: true };
            ds.collections.push(c.clone());
            c
        }
    };
    let light = coll.mode_named(design_kit::LIGHT).or(coll.modes.first()).map(|m| m.id)?;
    let dark = coll.mode_named(design_kit::DARK).map(|m| m.id);
    let mut out = HashMap::new();
    for k in kit {
        let id = match ds.variable_named(k.name, Some(coll.id)) {
            Some(v) => v.id,
            None => {
                let id = doc.alloc_id()?;
                let mut values = vec![(light, k.light.clone())];
                if let Some(d) = dark {
                    values.push((d, k.dark.clone()));
                }
                ds.variables.push(Variable {
                    id,
                    name: k.name.into(),
                    collection: coll.id,
                    kind: k.kind,
                    values,
                    theme: k.theme,
                    description: String::new(),
                });
                id
            }
        };
        out.insert(k.name.to_string(), id);
    }
    if doc.design_system() != Some(&ds) {
        doc.design = Some(Arc::new(ds));
    }
    Some(out)
}

/// Rewrite the kit's values from `theme` (Design ▸ Themes with the kit): every kit variable gets
/// the theme's Light and Dark values; variables keep their ids, so bindings stay.
pub fn refresh_kit(doc: &mut Presentation, theme: &deckcraft_model::Theme) -> Option<()> {
    ensure_kit(doc)?;
    let mut ds = doc.design_system().cloned()?;
    let coll = ds.kit()?.clone();
    let light = coll.mode_named(design_kit::LIGHT).or(coll.modes.first()).map(|m| m.id)?;
    let dark = coll.mode_named(design_kit::DARK).map(|m| m.id);
    for k in design_kit::kit(theme) {
        if let Some(v) = ds.variables.iter_mut().find(|v| v.collection == coll.id && v.name == k.name) {
            v.set_value(light, k.light.clone());
            if let Some(d) = dark {
                v.set_value(d, k.dark.clone());
            }
        }
    }
    doc.design = Some(Arc::new(ds));
    Some(())
}

// ---------- the builder ----------

/// What text plays on a slide: its font, size variable, colour and weight.
#[derive(Clone, Copy, PartialEq)]
enum Role {
    Display,
    Title,
    Heading,
    Body,
    Muted,
    Caption,
    Big,
    Kicker,
    OnAccent,
    OnAccentTitle,
}

pub(crate) struct B<'a> {
    doc: &'a mut Presentation,
    vars: HashMap<String, u32>,
    sx: f64,
    sy: f64,
}

impl B<'_> {
    fn id(&mut self) -> Option<ShapeId> {
        self.doc.alloc_id().map(ShapeId)
    }
    fn var(&self, name: &str) -> Option<u32> {
        self.vars.get(name).copied()
    }
    /// A box on the 960 × 540 design grid, scaled to the deck's slide size.
    fn r(&self, x: f64, y: f64, w: f64, h: f64) -> Xfrm {
        Xfrm::new(x * self.sx, y * self.sy, w * self.sx, h * self.sy)
    }
    fn color(&self, name: &str) -> ColorRef {
        let last = self.var(name).and_then(|id| self.doc.design_system().and_then(|d| d.color(id, &[]))).unwrap_or(deckcraft_color::Rgba::BLACK);
        match self.var(name) {
            Some(id) => var_color(id, last),
            None => ColorRef::scheme(SchemeSlot::Tx1),
        }
    }
    fn bind(&self, sh: &mut Shape, field: BindField, var: &str) {
        if let Some(v) = self.var(var) {
            sh.bindings.push(Binding { field, variable: v });
            if let Some(val) = self.doc.design_system().and_then(|d| d.resolve(v, &[])).cloned() {
                deckcraft_model::variables::apply_one(sh, field, &val);
            }
        }
    }

    fn text(&mut self, x: Xfrm, text: &str, role: Role) -> Option<Shape> {
        let (font, size_var, bold, color) = match role {
            Role::Display => ("+mj-lt", "type/display", true, ColorRef::scheme(SchemeSlot::Tx1)),
            Role::Title => ("+mj-lt", "type/title", true, ColorRef::scheme(SchemeSlot::Tx1)),
            Role::Heading => ("+mj-lt", "type/heading", true, ColorRef::scheme(SchemeSlot::Tx1)),
            Role::Body => ("+mn-lt", "type/body", false, ColorRef::scheme(SchemeSlot::Tx1)),
            Role::Muted => ("+mn-lt", "type/body", false, ColorRef::scheme(SchemeSlot::Tx2)),
            Role::Caption => ("+mn-lt", "type/caption", false, ColorRef::scheme(SchemeSlot::Tx2)),
            Role::Big => ("+mj-lt", "type/big-number", true, self.color("color/accent-text")),
            Role::Kicker => ("+mn-lt", "type/caption", true, self.color("color/accent-text")),
            Role::OnAccent => ("+mn-lt", "type/heading", false, self.color("color/on-accent")),
            Role::OnAccentTitle => ("+mj-lt", "type/display", true, self.color("color/on-accent")),
        };
        let props = RunProps { font: Some(font.into()), bold: Some(bold), fill: Some(Fill::solid(color)), ..Default::default() };
        let paragraphs = text
            .split('\n')
            .map(|line| Paragraph {
                runs: vec![Run { text: line.into(), props: props.clone(), kind: RunKind::Text }],
                end_props: props.clone(),
                props: ParaProps {
                    space_after: Some(Spacing::Pts(if role == Role::Body || role == Role::Muted { 6.0 } else { 0.0 })),
                    ..Default::default()
                },
                ..Default::default()
            })
            .collect();
        let mut body = TextBody { paragraphs, ..Default::default() };
        body.body.wrap = Some(true);
        body.body.autofit = Some(AutoFit::Shape);
        body.body.anchor = Some(Anchor::Top);
        body.body.inset_l = Some(0.0);
        body.body.inset_r = Some(0.0);
        let mut sh =
            Shape { id: self.id()?, xfrm: Some(x), text_box: true, fill: None, text: Some(body), geom: Geom::preset("rect"), ..Default::default() };
        self.bind(&mut sh, BindField::FontSize, size_var);
        Some(sh)
    }
    fn centered(&mut self, x: Xfrm, text: &str, role: Role) -> Option<Shape> {
        let mut sh = self.text(x, text, role)?;
        if let Some(t) = sh.text.as_mut() {
            for p in &mut t.paragraphs {
                p.props.align = Some(Align::Center);
            }
        }
        Some(sh)
    }
    fn named(mut sh: Shape, name: &str) -> Shape {
        sh.name = name.into();
        sh
    }
    fn rect(&mut self, x: Xfrm, fill: Fill) -> Option<Shape> {
        Some(Shape { id: self.id()?, xfrm: Some(x), fill: Some(fill), line: Some(Line::none()), geom: Geom::preset("rect"), ..Default::default() })
    }
    fn ellipse(&mut self, x: Xfrm, fill: Fill) -> Option<Shape> {
        Some(Shape { id: self.id()?, xfrm: Some(x), fill: Some(fill), line: Some(Line::none()), geom: Geom::preset("ellipse"), ..Default::default() })
    }
    /// A frame with auto layout around `children` (laid out by the engine's canvas pass).
    fn frame(&mut self, x: Xfrm, children: Vec<Shape>, al: AutoLayout, fill: Option<Fill>, radius: Option<&str>) -> Option<Shape> {
        let mut sh = Shape {
            id: self.id()?,
            xfrm: Some(x),
            kind: ShapeKind::Group { children, child: x },
            frame: Some(Frame::default()),
            fill: Some(fill.unwrap_or(Fill::None)),
            line: Some(Line::none()),
            auto_layout: Some(al),
            ..Default::default()
        };
        if let Some(r) = radius {
            self.bind(&mut sh, BindField::CornerRadius, r);
        }
        Some(sh)
    }
    fn stack(&mut self, x: Xfrm, children: Vec<Shape>, gap: &str, pad: Option<&str>, fill: Option<Fill>, radius: Option<&str>) -> Option<Shape> {
        let al =
            AutoLayout { flow: Flow::Vertical, gap: 0.0, padding: [0.0; 4], cross_align: LAlign::Start, height: Sizing::Hug, ..Default::default() };
        let children = children.into_iter().map(fill_w).collect();
        let mut f = self.frame(x, children, al, fill, radius)?;
        self.bind(&mut f, BindField::Gap, gap);
        if let Some(p) = pad {
            self.bind(&mut f, BindField::Padding, p);
        }
        Some(f)
    }
    fn row(&mut self, x: Xfrm, children: Vec<Shape>, gap: &str) -> Option<Shape> {
        let al =
            AutoLayout { flow: Flow::Horizontal, gap: 0.0, padding: [0.0; 4], cross_align: LAlign::Start, height: Sizing::Hug, ..Default::default() };
        let children = children.into_iter().map(fill_w).collect();
        let mut f = self.frame(x, children, al, None, None)?;
        f.name = "Row".into();
        self.bind(&mut f, BindField::Gap, gap);
        Some(f)
    }
    fn grid(&mut self, x: Xfrm, children: Vec<Shape>, columns: usize, gap: &str) -> Option<Shape> {
        let al = AutoLayout {
            flow: Flow::Grid,
            gap: 0.0,
            cross_gap: 0.0,
            padding: [0.0; 4],
            columns: vec![Track::Fr(1.0); columns.max(1)],
            rows: vec![],
            cross_align: LAlign::Start,
            align: LAlign::Start,
            height: Sizing::Hug,
            ..Default::default()
        };
        let children = children.into_iter().map(fill_w).collect();
        let mut f = self.frame(x, children, al, None, None)?;
        f.name = "Grid".into();
        self.bind(&mut f, BindField::Gap, gap);
        self.bind(&mut f, BindField::CrossGap, gap);
        Some(f)
    }
    /// A stand-in for a photo: a soft gradient of two accents in a frame with rounded corners.
    /// (The library ships no pictures; people drop their own in.)
    fn image(&mut self, x: Xfrm, a: SchemeSlot, b: SchemeSlot, radius: Option<&str>) -> Option<Shape> {
        let g = Fill::Gradient(Gradient {
            stops: vec![GradientStop { pos: 0.0, color: ColorRef::scheme(a) }, GradientStop { pos: 1.0, color: ColorRef::scheme(b) }],
            shape: GradientShape::Linear { angle: 45.0, scaled: false },
            flip: None,
            rotate_with_shape: true,
            tile: [0.0; 4],
        });
        let mut f = self.frame(x, vec![], AutoLayout::default(), Some(g), radius)?;
        f.auto_layout = None;
        f.name = "Image".into();
        f.descr = "Image placeholder".into();
        Some(f)
    }
    fn title(&mut self, text: &str) -> Option<Shape> {
        let x = self.r(60.0, 40.0, 840.0, 60.0);
        Some(Self::named(self.text(x, text, Role::Title)?, "Title"))
    }
    fn surface(&self) -> Fill {
        Fill::solid(self.color("color/surface"))
    }
    /// An instance of library component `key`, at `x`, with text property values.
    fn instance(&mut self, key: &str, variant: &[(&str, &str)], x: Xfrm, props: &[(&str, &str)]) -> Option<Shape> {
        let comp = ensure_component(self, key, variant)?;
        let inst = Instance {
            component: comp.id,
            props: props.iter().map(|(k, v)| (k.to_string(), PropValue::Text(v.to_string()))).collect(),
            ..Default::default()
        };
        let frame = Shape { id: self.id()?, name: comp.name.clone(), xfrm: Some(x), ..Default::default() };
        let lib = self.doc.design_system().map(|d| d.components.clone()).unwrap_or_default();
        deckcraft_model::component::materialize(&lib, &comp, &frame, &inst, self.doc)
    }
}

fn fill_w(mut s: Shape) -> Shape {
    let mut l = s.layout_child.unwrap_or_default();
    l.width = Sizing::Fill;
    s.layout_child = Some(l);
    s
}

// ---------- library components ----------

/// The component `key` (with `variant` values), added to the deck's library on first use.
fn ensure_component(b: &mut B, key: &str, variant: &[(&str, &str)]) -> Option<Component> {
    let set = component_set(key);
    let found = b.doc.design_system().and_then(|d| {
        d.components
            .iter()
            .find(|c| {
                if variant.is_empty() {
                    c.set.is_empty() && c.name == set
                } else {
                    c.set == set && variant.iter().all(|(k, v)| c.variant.iter().any(|(ck, cv)| ck == k && cv == v))
                }
            })
            .cloned()
    });
    if let Some(c) = found {
        return Some(c);
    }
    let (root, props) = build_component(b, key, variant)?;
    let id = b.doc.alloc_id()?;
    let name = if variant.is_empty() { set.to_string() } else { format!("{set} {}", variant.iter().map(|(_, v)| *v).collect::<Vec<_>>().join(" ")) };
    let c = Component {
        id,
        name: name.clone(),
        set: if variant.is_empty() { String::new() } else { set.to_string() },
        variant: variant.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        props,
        root: Shape { name, ..root },
        description: "SlideCraft layout library".into(),
    };
    let mut ds = b.doc.design_system().cloned().unwrap_or_default();
    ds.components.push(c.clone());
    b.doc.design = Some(Arc::new(ds));
    Some(c)
}

fn component_set(key: &str) -> &'static str {
    match key {
        "stat" => "Stat",
        "feature" => "Feature card",
        "quote" => "Quote",
        "person" => "Person",
        "step" => "Step",
        "agenda" => "Agenda item",
        "logo" => "Logo",
        "price" => "Price tier",
        _ => "Check item",
    }
}

/// A component's root frame (at the origin, its members laid out) and its text properties.
fn build_component(b: &mut B, key: &str, variant: &[(&str, &str)]) -> Option<(Shape, Vec<PropDef>)> {
    let large = !variant.iter().any(|(_, v)| *v == "Small");
    let highlight = variant.iter().any(|(_, v)| *v == "Highlight");
    let (root, names): (Shape, Vec<&str>) = match key {
        "stat" => comp_stat(b, large, highlight)?,
        "feature" => comp_feature(b, large, highlight)?,
        "quote" => comp_quote(b, large, highlight)?,
        "person" => comp_person(b, large, highlight)?,
        "step" => comp_step(b, large, highlight)?,
        "agenda" => comp_agenda(b, large, highlight)?,
        "logo" => comp_logo(b, large, highlight)?,
        "price" => comp_price(b, large, highlight)?,
        _ => comp_check(b, large, highlight)?,
    };
    let mut root = root;
    // Text sized to its theme, members laid out, in the component's own space.
    crate::library_fit::fit_tree(b.doc, std::slice::from_mut(&mut root));
    crate::canvas::settle_list(std::slice::from_mut(&mut root));
    let props = names
        .iter()
        .filter_map(|n| {
            let layer = find_named(root.children(), n)?;
            Some(PropDef {
                name: (*n).to_string(),
                default: PropValue::Text(layer.text.as_ref().map(TextBody::text).unwrap_or_default()),
                targets: vec![layer.id.0],
            })
        })
        .collect();
    Some((root, props))
}

fn comp_stat(b: &mut B, large: bool, highlight: bool) -> Option<(Shape, Vec<&'static str>)> {
    let _ = (large, highlight);
    let (w, h) = if large { (260.0, 170.0) } else { (190.0, 120.0) };
    let v = B::named(b.text(b.r(0.0, 0.0, w, 70.0), "42%", if large { Role::Big } else { Role::Title })?, "Value");
    let mut v = v;
    if large {
        b.bind(&mut v, BindField::FontSize, "type/display");
    } else if let Some(t) = v.text.as_mut() {
        let c = b.color("color/accent-text");
        for p in &mut t.paragraphs {
            for r in &mut p.runs {
                r.props.fill = Some(Fill::solid(c.clone()));
            }
        }
    }
    let l = B::named(b.text(b.r(0.0, 0.0, w, 30.0), "growth this year", if large { Role::Muted } else { Role::Caption })?, "Label");
    let surface = b.surface();
    Some((b.stack(b.r(0.0, 0.0, w, h), vec![v, l], "space/xs", Some("space/md"), Some(surface), Some("radius/md"))?, vec!["Value", "Label"]))
}

fn comp_feature(b: &mut B, large: bool, highlight: bool) -> Option<(Shape, Vec<&'static str>)> {
    let _ = (large, highlight);
    let dot = B::named(b.ellipse(b.r(0.0, 0.0, 28.0, 28.0), Fill::solid(ColorRef::scheme(SchemeSlot::Accent1)))?, "Icon");
    let mut dot = dot;
    dot.layout_child = Some(ChildLayout::default());
    let h = B::named(b.text(b.r(0.0, 0.0, 220.0, 34.0), "Clear", Role::Heading)?, "Heading");
    let t = B::named(b.text(b.r(0.0, 0.0, 220.0, 80.0), "One idea per slide, said plainly.", Role::Muted)?, "Body");
    let surface = b.surface();
    let mut f = b.stack(b.r(0.0, 0.0, 260.0, 230.0), vec![], "space/sm", Some("space/md"), Some(surface), Some("radius/md"))?;
    if let ShapeKind::Group { children, .. } = &mut f.kind {
        children.extend([dot, fill_w(h), fill_w(t)]);
    }
    Some((f, vec!["Heading", "Body"]))
}

fn comp_quote(b: &mut B, large: bool, highlight: bool) -> Option<(Shape, Vec<&'static str>)> {
    let _ = (large, highlight);
    let mut m = B::named(b.text(b.r(0.0, 0.0, 700.0, 60.0), "\u{201C}", Role::Big)?, "Mark");
    // The mark at display size: at big-number size its line leaves a gap above the quote.
    m.bindings.clear();
    b.bind(&mut m, BindField::FontSize, "type/display");
    let q = B::named(
        b.text(b.r(0.0, 0.0, 700.0, 110.0), "Good design is as little design as possible, and then a little less.", Role::Heading)?,
        "Quote",
    );
    let a = B::named(b.text(b.r(0.0, 0.0, 700.0, 24.0), "A designer we admire", Role::Caption)?, "Attribution");
    Some((b.stack(b.r(0.0, 0.0, 760.0, 260.0), vec![m, q, a], "space/sm", None, None, None)?, vec!["Quote", "Attribution"]))
}

fn comp_person(b: &mut B, large: bool, highlight: bool) -> Option<(Shape, Vec<&'static str>)> {
    let _ = (large, highlight);
    let mut av = B::named(
        b.ellipse(
            b.r(0.0, 0.0, 96.0, 96.0),
            Fill::Gradient(Gradient {
                stops: vec![
                    GradientStop { pos: 0.0, color: ColorRef::scheme(SchemeSlot::Accent1) },
                    GradientStop { pos: 1.0, color: ColorRef::scheme(SchemeSlot::Accent2) },
                ],
                shape: GradientShape::Linear { angle: 45.0, scaled: false },
                flip: None,
                rotate_with_shape: true,
                tile: [0.0; 4],
            }),
        )?,
        "Photo",
    );
    av.layout_child = Some(ChildLayout::default());
    let n = B::named(b.centered(b.r(0.0, 0.0, 180.0, 30.0), "Alex Rivera", Role::Heading)?, "Name");
    let r = B::named(b.centered(b.r(0.0, 0.0, 180.0, 24.0), "Head of Design", Role::Caption)?, "Role");
    let mut f = b.stack(b.r(0.0, 0.0, 190.0, 210.0), vec![], "space/sm", Some("space/sm"), None, None)?;
    if let Some(al) = f.auto_layout.as_mut() {
        al.cross_align = LAlign::Center;
    }
    if let ShapeKind::Group { children, .. } = &mut f.kind {
        children.extend([av, fill_w(n), fill_w(r)]);
    }
    Some((f, vec!["Name", "Role"]))
}

fn comp_step(b: &mut B, large: bool, highlight: bool) -> Option<(Shape, Vec<&'static str>)> {
    let _ = (large, highlight);
    let mut num = B::named(b.ellipse(b.r(0.0, 0.0, 56.0, 56.0), Fill::solid(ColorRef::scheme(SchemeSlot::Accent1)))?, "Badge");
    let mut nt = b.centered(b.r(0.0, 0.0, 56.0, 56.0), "1", Role::OnAccent)?;
    // A short label on one line in the circle; the circle keeps its size.
    nt.bindings.clear();
    b.bind(&mut nt, BindField::FontSize, "type/body");
    if let Some(t) = nt.text.take() {
        let body = deckcraft_model::text::BodyProps {
            anchor: Some(Anchor::Middle),
            wrap: Some(false),
            autofit: Some(AutoFit::None),
            inset_l: Some(0.0),
            inset_r: Some(0.0),
            ..t.body
        };
        num.text = Some(TextBody { body, ..t });
    }
    num.bindings = nt.bindings;
    num.layout_child = Some(ChildLayout::default());
    let h = B::named(b.text(b.r(0.0, 0.0, 190.0, 30.0), "Discover", Role::Heading)?, "Heading");
    let t = B::named(b.text(b.r(0.0, 0.0, 190.0, 70.0), "Listen first, then frame the problem.", Role::Muted)?, "Body");
    let mut f = b.stack(b.r(0.0, 0.0, 200.0, 190.0), vec![], "space/sm", None, None, None)?;
    if let ShapeKind::Group { children, .. } = &mut f.kind {
        children.extend([num, fill_w(h), fill_w(t)]);
    }
    Some((f, vec!["Badge", "Heading", "Body"]))
}

fn comp_agenda(b: &mut B, large: bool, highlight: bool) -> Option<(Shape, Vec<&'static str>)> {
    let _ = (large, highlight);
    let n = B::named(b.text(b.r(0.0, 0.0, 60.0, 34.0), "01", Role::Kicker)?, "Number");
    let mut n = n;
    b.bind(&mut n, BindField::FontSize, "type/heading");
    let t = B::named(b.text(b.r(0.0, 0.0, 560.0, 34.0), "Where we are", Role::Body)?, "Title");
    let tm = B::named(b.text(b.r(0.0, 0.0, 120.0, 34.0), "10 min", Role::Caption)?, "Time");
    let al =
        AutoLayout { flow: Flow::Horizontal, gap: 0.0, padding: [0.0; 4], cross_align: LAlign::Center, height: Sizing::Hug, ..Default::default() };
    let mut tt = t;
    tt.layout_child = Some(ChildLayout { width: Sizing::Fill, ..Default::default() });
    let mut f = b.frame(b.r(0.0, 0.0, 840.0, 54.0), vec![n, tt, tm], al, Some(Fill::None), None)?;
    b.bind(&mut f, BindField::Gap, "space/md");
    Some((f, vec!["Number", "Title", "Time"]))
}

fn comp_logo(b: &mut B, large: bool, highlight: bool) -> Option<(Shape, Vec<&'static str>)> {
    let _ = (large, highlight);
    let n = B::named(b.centered(b.r(0.0, 0.0, 150.0, 30.0), "Northwind", Role::Heading)?, "Name");
    let mut n = n;
    if let Some(t) = n.text.as_mut() {
        for p in &mut t.paragraphs {
            for r in &mut p.runs {
                r.props.fill = Some(Fill::solid(ColorRef::scheme(SchemeSlot::Tx2)));
            }
        }
    }
    let surface = b.surface();
    let mut f = b.stack(b.r(0.0, 0.0, 190.0, 76.0), vec![n], "space/xs", Some("space/md"), Some(surface), Some("radius/sm"))?;
    if let Some(al) = f.auto_layout.as_mut() {
        al.align = LAlign::Center;
    }
    Some((f, vec!["Name"]))
}

fn comp_price(b: &mut B, large: bool, highlight: bool) -> Option<(Shape, Vec<&'static str>)> {
    let _ = (large, highlight);
    let (fill, title_role, body_role) = if highlight {
        (Fill::solid(ColorRef::scheme(SchemeSlot::Accent1)), Role::OnAccent, Role::OnAccent)
    } else {
        (b.surface(), Role::Heading, Role::Muted)
    };
    let tier = B::named(b.text(b.r(0.0, 0.0, 220.0, 30.0), "Team", title_role)?, "Tier");
    let mut price = B::named(b.text(b.r(0.0, 0.0, 220.0, 64.0), "$12", if highlight { Role::OnAccentTitle } else { Role::Title })?, "Price");
    b.bind(&mut price, BindField::FontSize, "type/title");
    let mut feats = B::named(b.text(b.r(0.0, 0.0, 220.0, 120.0), "Shared themes\nComponents\nUnlimited decks", body_role)?, "Features");
    if highlight {
        // On the accent, text is bold so it counts as large text (WCAG: 14 pt bold), where
        // a theme's accent gives white only 3:1–4.5:1.
        b.bind(&mut feats, BindField::FontSize, "type/body");
        if let Some(t) = feats.text.as_mut() {
            for p in &mut t.paragraphs {
                for r in &mut p.runs {
                    r.props.bold = Some(true);
                }
                p.end_props.bold = Some(true);
            }
        }
    }
    Some((
        b.stack(b.r(0.0, 0.0, 250.0, 320.0), vec![tier, price, feats], "space/sm", Some("space/md"), Some(fill), Some("radius/md"))?,
        vec!["Tier", "Price", "Features"],
    ))
}

fn comp_check(b: &mut B, large: bool, highlight: bool) -> Option<(Shape, Vec<&'static str>)> {
    let _ = (large, highlight);
    let mut mark = B::named(b.ellipse(b.r(0.0, 0.0, 22.0, 22.0), Fill::solid(ColorRef::scheme(SchemeSlot::Accent1)))?, "Mark");
    mark.layout_child = Some(ChildLayout::default());
    let mut t = B::named(b.text(b.r(0.0, 0.0, 780.0, 30.0), "Every slide has one message", Role::Body)?, "Text");
    t.layout_child = Some(ChildLayout { width: Sizing::Fill, ..Default::default() });
    let al =
        AutoLayout { flow: Flow::Horizontal, gap: 0.0, padding: [0.0; 4], cross_align: LAlign::Center, height: Sizing::Hug, ..Default::default() };
    let mut f = b.frame(b.r(0.0, 0.0, 840.0, 40.0), vec![mark, t], al, Some(Fill::None), None)?;
    b.bind(&mut f, BindField::Gap, "space/sm");
    Some((f, vec!["Text"]))
}

fn find_named<'a>(list: &'a [Shape], name: &str) -> Option<&'a Shape> {
    let mut out = None;
    deckcraft_model::walk(list, &mut |s, _| {
        if out.is_none() && s.name == name {
            out = Some(s);
        }
    });
    out
}

// ---------- the layouts ----------

fn accent_bar(b: &mut B, x: f64, y: f64) -> Option<Shape> {
    let r = b.r(x, y, 64.0, 6.0);
    b.rect(r, Fill::solid(ColorRef::scheme(SchemeSlot::Accent1)))
}

fn title(b: &mut B) -> Option<Vec<Shape>> {
    let bar = accent_bar(b, 60.0, 176.0)?;
    let k = B::named(b.text(b.r(60.0, 196.0, 840.0, 24.0), "PHASE 4 · DESIGN SYSTEM", Role::Kicker)?, "Kicker");
    let t = B::named(b.text(b.r(60.0, 226.0, 840.0, 70.0), "A deck that styles itself", Role::Display)?, "Title");
    let s = B::named(b.text(b.r(60.0, 310.0, 840.0, 30.0), "Variables, components and a library of layouts", Role::Muted)?, "Subtitle");
    let st = b.stack(b.r(60.0, 176.0, 840.0, 200.0), vec![bar, k, t, s], "space/sm", None, None, None)?;
    Some(vec![st])
}

fn title_band(b: &mut B) -> Option<Vec<Shape>> {
    let band = b.rect(b.r(0.0, 0.0, 400.0, 540.0), Fill::solid(ColorRef::scheme(SchemeSlot::Accent1)))?;
    let t = b.text(b.r(48.0, 190.0, 310.0, 120.0), "Annual\nreview", Role::OnAccentTitle)?;
    let k = b.text(b.r(460.0, 210.0, 440.0, 24.0), "2026 · FOR THE WHOLE TEAM", Role::Kicker)?;
    let s = b.text(b.r(460.0, 240.0, 440.0, 90.0), "What we shipped, what we learned, and where we go next.", Role::Heading)?;
    Some(vec![band, t, k, s])
}

fn section(b: &mut B) -> Option<Vec<Shape>> {
    let n = b.text(b.r(60.0, 120.0, 400.0, 120.0), "02", Role::Big)?;
    let t = b.text(b.r(60.0, 260.0, 760.0, 56.0), "What we learned", Role::Title)?;
    let s = b.text(b.r(60.0, 330.0, 640.0, 60.0), "Three lessons from a year of building decks with our customers.", Role::Muted)?;
    Some(vec![n, t, s])
}

fn section_band(b: &mut B) -> Option<Vec<Shape>> {
    let bg = b.rect(b.r(0.0, 0.0, 960.0, 540.0), Fill::solid(ColorRef::scheme(SchemeSlot::Accent1)))?;
    let t = b.centered(b.r(80.0, 220.0, 800.0, 80.0), "Next: the roadmap", Role::OnAccentTitle)?;
    Some(vec![bg, t])
}

fn agenda(b: &mut B) -> Option<Vec<Shape>> {
    let t = b.title("Agenda")?;
    let items = [("01", "Where we are", "10 min"), ("02", "What we learned", "15 min"), ("03", "The plan", "20 min"), ("04", "Questions", "15 min")];
    let mut rows = vec![];
    for (n, title, time) in items {
        rows.push(b.instance("agenda", &[], b.r(0.0, 0.0, 840.0, 54.0), &[("Number", n), ("Title", title), ("Time", time)])?);
    }
    let list = b.stack(b.r(60.0, 130.0, 840.0, 340.0), rows, "space/sm", None, None, None)?;
    Some(vec![t, list])
}

fn bullets(b: &mut B) -> Option<Vec<Shape>> {
    let t = b.title("Why it matters")?;
    let mut body = b.text(
        b.r(60.0, 130.0, 840.0, 300.0),
        "Consistency without effort: every slide follows the theme\nChange once, update everywhere: components and styles\nLight and Dark from one switch\nLayouts that reflow when the words change",
        Role::Body,
    )?;
    if let Some(tb) = body.text.as_mut() {
        for p in &mut tb.paragraphs {
            p.props.bullet = Some(deckcraft_model::text::Bullet::Char { char: "•".into() });
            p.props.margin_left = Some(22.0);
            p.props.indent = Some(-22.0);
            p.props.bullet_color = Some(ColorRef::scheme(SchemeSlot::Accent1));
            p.props.space_after = Some(Spacing::Pts(12.0));
        }
    }
    Some(vec![t, body])
}

fn column(b: &mut B, head: &str, text: &str) -> Option<Shape> {
    let h = b.text(b.r(0.0, 0.0, 400.0, 34.0), head, Role::Heading)?;
    let t = b.text(b.r(0.0, 0.0, 400.0, 150.0), text, Role::Muted)?;
    b.stack(b.r(0.0, 0.0, 400.0, 220.0), vec![h, t], "space/sm", None, None, None)
}

fn two_columns(b: &mut B) -> Option<Vec<Shape>> {
    let t = b.title("Two ways to start")?;
    let a = column(b, "From a theme", "Pick a theme, switch between Light and Dark, and every layout follows.")?;
    let c = column(b, "From your brand", "Set your colours, fonts and spacing as variables; the library uses them.")?;
    let row = b.row(b.r(60.0, 140.0, 840.0, 240.0), vec![a, c], "space/lg")?;
    Some(vec![t, row])
}

fn three_up(b: &mut B) -> Option<Vec<Shape>> {
    let t = b.title("What makes it work")?;
    let mut cards = vec![];
    for (h, body) in [
        ("Clear", "One idea per slide, said plainly."),
        ("Consistent", "Styles and components keep slides alike."),
        ("Quick", "Layouts reflow as the words change."),
    ] {
        cards.push(b.instance("feature", &[], b.r(0.0, 0.0, 260.0, 230.0), &[("Heading", h), ("Body", body)])?);
    }
    let row = b.row(b.r(60.0, 140.0, 840.0, 240.0), cards, "space/md")?;
    Some(vec![t, row])
}

fn four_up(b: &mut B) -> Option<Vec<Shape>> {
    let t = b.title("Four things to remember")?;
    let mut cards = vec![];
    for (h, body) in [
        ("Themes", "Colours and fonts as variables."),
        ("Modes", "Light and Dark in one click."),
        ("Components", "Edit once, update everywhere."),
        ("Layouts", "34 slides ready to fill."),
    ] {
        cards.push(b.instance("feature", &[], b.r(0.0, 0.0, 400.0, 160.0), &[("Heading", h), ("Body", body)])?);
    }
    let g = b.grid(b.r(60.0, 120.0, 840.0, 380.0), cards, 2, "space/sm")?;
    Some(vec![t, g])
}

fn stats(b: &mut B) -> Option<Vec<Shape>> {
    let t = b.title("The year in numbers")?;
    let mut cards = vec![];
    for (v, l) in [("3×", "faster to build a deck"), ("92%", "of slides from the library"), ("0", "off-brand colours")] {
        cards.push(b.instance("stat", &[("Size", "Large")], b.r(0.0, 0.0, 260.0, 170.0), &[("Value", v), ("Label", l)])?);
    }
    let row = b.row(b.r(60.0, 160.0, 840.0, 190.0), cards, "space/md")?;
    Some(vec![t, row])
}

fn big_number(b: &mut B) -> Option<Vec<Shape>> {
    let n = b.centered(b.r(60.0, 150.0, 840.0, 130.0), "1,204", Role::Big)?;
    let c = b.centered(b.r(160.0, 300.0, 640.0, 60.0), "teams switched to the library this quarter", Role::Muted)?;
    Some(vec![n, c])
}

fn quote(b: &mut B) -> Option<Vec<Shape>> {
    let q = b.instance("quote", &[], b.r(100.0, 130.0, 760.0, 260.0), &[])?;
    Some(vec![q])
}

fn quote_photo(b: &mut B) -> Option<Vec<Shape>> {
    let img = b.image(b.r(60.0, 60.0, 300.0, 420.0), SchemeSlot::Accent2, SchemeSlot::Accent1, Some("radius/lg"))?;
    let q = b.instance(
        "quote",
        &[],
        b.r(400.0, 140.0, 500.0, 260.0),
        &[("Quote", "We stopped arguing about fonts and started talking about the story."), ("Attribution", "A customer, after a month")],
    )?;
    Some(vec![img, q])
}

fn image_side(b: &mut B, left: bool) -> Option<Vec<Shape>> {
    let ix = if left { 0.0 } else { 520.0 };
    let tx = if left { 500.0 } else { 60.0 };
    let img = b.image(b.r(ix, 0.0, 440.0, 540.0), SchemeSlot::Accent1, SchemeSlot::Accent3, None)?;
    let k = b.text(b.r(0.0, 0.0, 400.0, 24.0), "FIELD NOTES", Role::Kicker)?;
    let t = b.text(b.r(0.0, 0.0, 400.0, 100.0), "Design in the open", Role::Title)?;
    let s =
        b.text(b.r(0.0, 0.0, 400.0, 120.0), "Share the deck early. Variables keep everyone's slides on brand while the story changes.", Role::Muted)?;
    let st = b.stack(b.r(tx, 150.0, 400.0, 280.0), vec![k, t, s], "space/sm", None, None, None)?;
    Some(vec![img, st])
}
fn image_left(b: &mut B) -> Option<Vec<Shape>> {
    image_side(b, true)
}
fn image_right(b: &mut B) -> Option<Vec<Shape>> {
    image_side(b, false)
}

fn full_bleed(b: &mut B) -> Option<Vec<Shape>> {
    let img = b.image(b.r(0.0, 0.0, 960.0, 540.0), SchemeSlot::Accent1, SchemeSlot::Accent2, None)?;
    let t = b.text(b.r(0.0, 0.0, 380.0, 50.0), "Somewhere new", Role::Title)?;
    let c = b.text(b.r(0.0, 0.0, 380.0, 30.0), "Offsite, spring 2026", Role::Muted)?;
    let bg = Fill::solid(ColorRef::scheme(SchemeSlot::Bg1));
    let card = b.stack(b.r(48.0, 380.0, 440.0, 120.0), vec![t, c], "space/xs", Some("space/md"), Some(bg), Some("radius/md"))?;
    Some(vec![img, card])
}

fn captioned(b: &mut B, a: SchemeSlot, c: SchemeSlot, caption: &str, w: f64, h: f64) -> Option<Shape> {
    let img = b.image(b.r(0.0, 0.0, w, h), a, c, Some("radius/md"))?;
    let cap = b.text(b.r(0.0, 0.0, w, 26.0), caption, Role::Caption)?;
    b.stack(b.r(0.0, 0.0, w, h + 40.0), vec![img, cap], "space/xs", None, None, None)
}

fn two_images(b: &mut B) -> Option<Vec<Shape>> {
    let t = b.title("Two places, one team")?;
    let a = captioned(b, SchemeSlot::Accent1, SchemeSlot::Accent2, "The studio in the morning", 410.0, 260.0)?;
    let c = captioned(b, SchemeSlot::Accent3, SchemeSlot::Accent4, "The workshop after lunch", 410.0, 260.0)?;
    let row = b.row(b.r(60.0, 130.0, 840.0, 310.0), vec![a, c], "space/md")?;
    Some(vec![t, row])
}

fn gallery(b: &mut B) -> Option<Vec<Shape>> {
    let t = b.title("Gallery")?;
    let mut items = vec![];
    for (a, c, cap) in [
        (SchemeSlot::Accent1, SchemeSlot::Accent2, "Sketches"),
        (SchemeSlot::Accent3, SchemeSlot::Accent4, "Prototypes"),
        (SchemeSlot::Accent5, SchemeSlot::Accent6, "Launch"),
    ] {
        items.push(captioned(b, a, c, cap, 270.0, 260.0)?);
    }
    let row = b.row(b.r(60.0, 130.0, 840.0, 310.0), items, "space/md")?;
    Some(vec![t, row])
}

fn comparison(b: &mut B) -> Option<Vec<Shape>> {
    let t = b.title("Before and with the design system")?;
    let side = |b: &mut B, head: &str, text: &str| -> Option<Shape> {
        let h = b.text(b.r(0.0, 0.0, 380.0, 34.0), head, Role::Heading)?;
        let mut h = h;
        if let Some(tb) = h.text.as_mut() {
            let c = b.color("color/accent-text");
            for p in &mut tb.paragraphs {
                for r in &mut p.runs {
                    r.props.fill = Some(Fill::solid(c.clone()));
                }
            }
        }
        let x = b.text(b.r(0.0, 0.0, 380.0, 180.0), text, Role::Body)?;
        let surface = b.surface();
        b.stack(b.r(0.0, 0.0, 410.0, 300.0), vec![h, x], "space/sm", Some("space/lg"), Some(surface), Some("radius/md"))
    };
    let a = side(b, "Before", "Every slide styled by hand\nColours drift from the brand\nA theme change means a day of edits")?;
    let c = side(b, "With variables", "Slides follow the theme\nOne palette, bound everywhere\nA theme change is one click")?;
    let row = b.row(b.r(60.0, 130.0, 840.0, 320.0), vec![a, c], "space/md")?;
    Some(vec![t, row])
}

fn steps(b: &mut B, items: &[(&str, &str, &str)]) -> Option<Vec<Shape>> {
    let mut out = vec![];
    for (n, h, body) in items {
        out.push(b.instance("step", &[], b.r(0.0, 0.0, 200.0, 190.0), &[("Badge", n), ("Heading", h), ("Body", body)])?);
    }
    Some(out)
}

fn timeline(b: &mut B) -> Option<Vec<Shape>> {
    let t = b.title("Timeline")?;
    let line = b.rect(b.r(60.0, 159.0, 840.0, 3.0), Fill::solid(ColorRef::scheme(SchemeSlot::Accent1)))?;
    let items = steps(
        b,
        &[
            ("Q1", "Research", "Interviews and audits"),
            ("Q2", "Design", "Variables and components"),
            ("Q3", "Build", "The layout library"),
            ("Q4", "Launch", "Every team on board"),
        ],
    )?;
    let row = b.row(b.r(60.0, 138.0, 840.0, 220.0), items, "space/md")?;
    Some(vec![t, line, row])
}

fn process(b: &mut B) -> Option<Vec<Shape>> {
    let t = b.title("How a deck comes together")?;
    let items = steps(
        b,
        &[
            ("1", "Outline", "Write the story as headlines"),
            ("2", "Pick layouts", "One library layout per idea"),
            ("3", "Refine", "Tidy up and check contrast"),
        ],
    )?;
    let row = b.row(b.r(60.0, 150.0, 840.0, 220.0), items, "space/xl")?;
    Some(vec![t, row])
}

fn people(b: &mut B, list: &[(&str, &str)]) -> Option<Vec<Shape>> {
    let mut out = vec![];
    for (n, r) in list {
        out.push(b.instance("person", &[], b.r(0.0, 0.0, 190.0, 210.0), &[("Name", n), ("Role", r)])?);
    }
    Some(out)
}

fn team(b: &mut B) -> Option<Vec<Shape>> {
    let t = b.title("The team")?;
    let p = people(b, &[("Alex Rivera", "Design"), ("Sam Okafor", "Engineering"), ("Mia Chen", "Research"), ("Jo Weber", "Product")])?;
    let row = b.row(b.r(60.0, 150.0, 840.0, 230.0), p, "space/md")?;
    Some(vec![t, row])
}

fn team_bios(b: &mut B) -> Option<Vec<Shape>> {
    let t = b.title("Who to ask")?;
    let mut cols = vec![];
    for (n, r, bio) in [
        ("Alex Rivera", "Design", "Owns the theme and the component library."),
        ("Sam Okafor", "Engineering", "Keeps the build fast and the files open."),
        ("Mia Chen", "Research", "Turns interviews into the next layouts."),
    ] {
        let p = b.instance("person", &[], b.r(0.0, 0.0, 260.0, 210.0), &[("Name", n), ("Role", r)])?;
        let x = b.centered(b.r(0.0, 0.0, 260.0, 60.0), bio, Role::Muted)?;
        cols.push(b.stack(b.r(0.0, 0.0, 260.0, 290.0), vec![p, x], "space/xs", None, None, None)?);
    }
    let row = b.row(b.r(60.0, 120.0, 840.0, 300.0), cols, "space/md")?;
    Some(vec![t, row])
}

fn logos(b: &mut B) -> Option<Vec<Shape>> {
    let t = b.title("Teams using the library")?;
    let mut l = vec![];
    for n in ["Northwind", "Bluebird", "Fjord", "Lumen", "Maple", "Orbit", "Quarry", "Tandem"] {
        l.push(b.instance("logo", &[], b.r(0.0, 0.0, 195.0, 76.0), &[("Name", n)])?);
    }
    let g = b.grid(b.r(60.0, 150.0, 840.0, 190.0), l, 4, "space/sm")?;
    Some(vec![t, g])
}

fn pricing(b: &mut B) -> Option<Vec<Shape>> {
    let t = b.title("Plans")?;
    let a = b.instance(
        "price",
        &[("Tone", "Default")],
        b.r(0.0, 0.0, 250.0, 320.0),
        &[("Tier", "Starter"), ("Price", "Free"), ("Features", "Built-in themes\nThe layout library\nOne editor")],
    )?;
    let c = b.instance(
        "price",
        &[("Tone", "Highlight")],
        b.r(0.0, 0.0, 250.0, 320.0),
        &[("Tier", "Team"), ("Price", "$12"), ("Features", "Shared variables\nComponents\nUnlimited editors")],
    )?;
    let d = b.instance(
        "price",
        &[("Tone", "Default")],
        b.r(0.0, 0.0, 250.0, 320.0),
        &[("Tier", "Company"), ("Price", "Ask"), ("Features", "Brand kits\nReview tools\nSupport")],
    )?;
    let row = b.row(b.r(60.0, 120.0, 840.0, 340.0), vec![a, c, d], "space/md")?;
    Some(vec![t, row])
}

fn key_facts(b: &mut B) -> Option<Vec<Shape>> {
    let t = b.title("Key facts")?;
    let mut cells = vec![];
    for (k, v) in [("Founded", "2019"), ("People", "48"), ("Offices", "Lisbon, Toronto"), ("Customers", "1,204 teams")] {
        let kk = b.text(b.r(0.0, 0.0, 400.0, 24.0), k, Role::Caption)?;
        let vv = b.text(b.r(0.0, 0.0, 400.0, 40.0), v, Role::Heading)?;
        cells.push(b.stack(b.r(0.0, 0.0, 400.0, 80.0), vec![kk, vv], "space/xs", None, None, None)?);
    }
    let g = b.grid(b.r(60.0, 150.0, 840.0, 220.0), cells, 2, "space/lg")?;
    Some(vec![t, g])
}

fn chart(b: &mut B) -> Option<Vec<Shape>> {
    let t = b.title("Adoption by quarter")?;
    let mut bars = vec![];
    for (i, h) in [90.0, 150.0, 210.0, 280.0].iter().enumerate() {
        let slot = [SchemeSlot::Accent1, SchemeSlot::Accent1, SchemeSlot::Accent1, SchemeSlot::Accent2][i];
        let x = 80.0 + i as f64 * 100.0;
        bars.push(b.rect(b.r(x, 440.0 - h, 70.0, *h), Fill::solid(ColorRef::scheme(slot)))?);
        bars.push(b.centered(b.r(x - 10.0, 448.0, 90.0, 24.0), &format!("Q{}", i + 1), Role::Caption)?);
    }
    let base = b.rect(b.r(60.0, 440.0, 420.0, 2.0), Fill::solid(ColorRef::scheme(SchemeSlot::Tx2)))?;
    let k = b.text(b.r(0.0, 0.0, 360.0, 24.0), "TAKEAWAYS", Role::Kicker)?;
    let x = b.text(b.r(0.0, 0.0, 360.0, 200.0), "Adoption tripled in a year\nQ4 grew fastest\nThe library drove most of it", Role::Body)?;
    let st = b.stack(b.r(540.0, 150.0, 360.0, 260.0), vec![k, x], "space/sm", None, None, None)?;
    let mut out = vec![t, base];
    out.extend(bars);
    out.push(st);
    Some(out)
}

fn statement(b: &mut B) -> Option<Vec<Shape>> {
    let s = b.text(b.r(60.0, 170.0, 840.0, 160.0), "Make the right thing the easy thing.", Role::Display)?;
    let bar = accent_bar(b, 60.0, 150.0)?;
    Some(vec![bar, s])
}

fn questions(b: &mut B) -> Option<Vec<Shape>> {
    let q = b.centered(b.r(60.0, 180.0, 840.0, 80.0), "Questions?", Role::Display)?;
    let s = b.centered(b.r(160.0, 280.0, 640.0, 40.0), "Ask now, or write to us afterwards.", Role::Muted)?;
    Some(vec![q, s])
}

fn closing(b: &mut B) -> Option<Vec<Shape>> {
    let bar = accent_bar(b, 448.0, 170.0)?;
    let t = b.centered(b.r(60.0, 196.0, 840.0, 80.0), "Thank you", Role::Display)?;
    let s = b.centered(b.r(160.0, 300.0, 640.0, 40.0), "slides and notes: our team space", Role::Muted)?;
    Some(vec![bar, t, s])
}

fn contact(b: &mut B) -> Option<Vec<Shape>> {
    let t = b.title("Get in touch")?;
    let mut cards = vec![];
    for (h, body) in
        [("Sales", "Plans, pricing and trials."), ("Support", "Help with any deck, any day."), ("Press", "Stories, quotes and pictures.")]
    {
        cards.push(b.instance("feature", &[], b.r(0.0, 0.0, 260.0, 230.0), &[("Heading", h), ("Body", body)])?);
    }
    let row = b.row(b.r(60.0, 140.0, 840.0, 240.0), cards, "space/md")?;
    Some(vec![t, row])
}

fn checklist(b: &mut B) -> Option<Vec<Shape>> {
    let t = b.title("Before you present")?;
    let mut items = vec![];
    for s in ["Every slide has one message", "Text reads on every background", "Numbers have their source", "The last slide says what to do next"] {
        items.push(b.instance("check", &[], b.r(0.0, 0.0, 840.0, 40.0), &[("Text", s)])?);
    }
    let list = b.stack(b.r(60.0, 140.0, 840.0, 280.0), items, "space/sm", None, None, None)?;
    Some(vec![t, list])
}

fn before_after(b: &mut B) -> Option<Vec<Shape>> {
    let t = b.title("Before and after")?;
    let a = captioned(b, SchemeSlot::Accent5, SchemeSlot::Accent6, "Before: styled by hand", 410.0, 260.0)?;
    let c = captioned(b, SchemeSlot::Accent1, SchemeSlot::Accent2, "After: from the library", 410.0, 260.0)?;
    let row = b.row(b.r(60.0, 130.0, 840.0, 310.0), vec![a, c], "space/md")?;
    Some(vec![t, row])
}

fn dashboard(b: &mut B) -> Option<Vec<Shape>> {
    let t = b.title("This week")?;
    let mut cards = vec![];
    for (v, l) in [("48", "decks made"), ("12", "new layouts"), ("99.9%", "uptime"), ("4.8", "rating")] {
        cards.push(b.instance("stat", &[("Size", "Small")], b.r(0.0, 0.0, 190.0, 120.0), &[("Value", v), ("Label", l)])?);
    }
    let g = b.grid(b.r(60.0, 140.0, 840.0, 140.0), cards, 4, "space/sm")?;
    let note = b.text(b.r(60.0, 320.0, 840.0, 60.0), "Numbers from the team dashboard, Monday to Friday.", Role::Caption)?;
    Some(vec![t, g, note])
}

// ---------- making a slide ----------

/// The layout with id or name `name`.
pub fn find(name: &str) -> Option<&'static LayoutDef> {
    LAYOUTS.iter().find(|l| l.id.eq_ignore_ascii_case(name) || l.name.eq_ignore_ascii_case(name))
}

/// Build the shapes of `layout` for `doc` (adding the kit and components it needs to the deck).
/// `None` when ids run out.
pub(crate) fn build(doc: &mut Presentation, layout: &LayoutDef) -> Option<Vec<Shape>> {
    let vars = ensure_kit(doc)?;
    let (sx, sy) = (doc.slide_size.width / 960.0, doc.slide_size.height / 540.0);
    let (sx, sy) = (if sx.is_finite() && sx > 0.0 { sx } else { 1.0 }, if sy.is_finite() && sy > 0.0 { sy } else { 1.0 });
    let mut b = B { doc, vars, sx, sy };
    let mut shapes = (layout.build)(&mut b)?;
    crate::library_fit::fit_tree(b.doc, &mut shapes);
    crate::canvas::settle_list(&mut shapes);
    // Fill and layout members that were placed by the solver may now leave text short or tall:
    // one more fit and layout settles them.
    crate::library_fit::fit_tree(b.doc, &mut shapes);
    crate::canvas::settle_list(&mut shapes);
    Some(shapes)
}

/// A slide of `layout` on the deck's Blank layout (else its first).
pub(crate) fn new_slide(doc: &mut Presentation, layout: &LayoutDef) -> Option<Slide> {
    let shapes = build(doc, layout)?;
    let lay = doc
        .masters
        .first()
        .and_then(|m| m.layouts.iter().find(|l| l.kind == deckcraft_model::LayoutType::Blank).or(m.layouts.first()))
        .map(|l| l.id)?;
    let id = doc.alloc_id()?;
    Some(Slide { id: deckcraft_model::SlideId(id), layout: lay, name: layout.name.into(), shapes, ..Default::default() })
}
