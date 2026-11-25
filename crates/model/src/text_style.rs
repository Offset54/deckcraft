//! Paragraph and character styles (SlideCraft typography; PowerPoint has no named text styles).
//!
//! A paragraph names a paragraph style (`ParaProps::style`) and a run a character style
//! (`RunProps::style`). Resolution order for a run: its own formatting, its character style, the
//! paragraph's `defRPr`, the paragraph style's run formatting, then the placeholder and master
//! chain ([`crate::resolve`]). A paragraph: its own formatting, its paragraph style, the chain.
//!
//! The built-in styles ([`builtin`]) use theme fonts (`+mj-lt`, `+mn-lt`) and theme colours, so
//! changing the theme restyles every styled paragraph. A deck changes a built-in style or adds its
//! own through [`crate::Presentation::text_styles`]; an entry there with a built-in id replaces
//! the built-in one.
//!
//! In `.pptx`, styles are flattened into direct formatting (PowerPoint shows the same text) and
//! the style ids and the deck's definitions ride in extensions; [`strip_equal`] undoes the
//! flattening when SlideCraft reads the file back.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::text::Paragraph;
use crate::text::{Composer, MAX_STYLE_ID, ParaProps, RunProps, Spacing};
use crate::{ColorRef, Fill, Presentation, SchemeSlot, Shape, ShapeKind};

/// A deck defines at most this many styles (hostile files).
pub const MAX_STYLES: usize = 256;
/// `basedOn` chains are followed at most this deep (cycles stop there too).
pub const MAX_BASED_ON: usize = 8;
/// Longest style name kept.
pub const MAX_NAME: usize = 128;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextStyleKind {
    /// Formats whole paragraphs: paragraph settings and the text in them.
    #[default]
    Paragraph,
    /// Formats runs of text inside a paragraph.
    Character,
}

impl TextStyleKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TextStyleKind::Paragraph => "paragraph",
            TextStyleKind::Character => "character",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TextStyle {
    /// Stable id that paragraphs and runs refer to (`body`, `bigNumber`, `myQuote`…).
    pub id: String,
    /// Name shown in the UI.
    pub name: String,
    pub kind: TextStyleKind,
    /// The style this one starts from; its settings fill what this one leaves unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub based_on: Option<String>,
    /// Paragraph settings (paragraph styles only; ignored on character styles).
    #[serde(skip_serializing_if = "is_default")]
    pub para: ParaProps,
    /// Character settings.
    #[serde(skip_serializing_if = "is_default")]
    pub run: RunProps,
    /// Font size and family driven by variables (SlideCraft Phase 4); the engine writes their
    /// values into `run`. A colour follows a variable through `run.fill` (`ColorBase::Var`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bindings: Vec<crate::variables::Binding>,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

/// A valid style id: 1–64 ASCII letters, digits, `-` or `_`.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_STYLE_ID && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Reads a style id, dropping one that is not [`valid_id`] (hostile files).
pub fn bounded_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(|id| id.filter(|v| valid_id(v)))
}

/// Reads a deck's style list, keeping at most [`MAX_STYLES`] valid ones and the first of each id.
pub fn bounded_styles<'de, D>(deserializer: D) -> Result<Vec<TextStyle>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let all = Vec::<TextStyle>::deserialize(deserializer)?;
    Ok(sanitize(all))
}

/// The styles of `all` a deck may hold: valid ids, no duplicates, bounded names and count.
pub fn sanitize(all: Vec<TextStyle>) -> Vec<TextStyle> {
    let mut out: Vec<TextStyle> = Vec::new();
    for mut s in all {
        if out.len() >= MAX_STYLES {
            break;
        }
        if !valid_id(&s.id) || out.iter().any(|o| o.id == s.id) {
            continue;
        }
        s.name = s.name.chars().take(MAX_NAME).collect();
        s.bindings.truncate(crate::variables::MAX_BINDINGS);
        if s.based_on.as_deref().is_some_and(|b| !valid_id(b) || b == s.id) {
            s.based_on = None;
        }
        // References inside a definition would make resolution recursive.
        s.para.style = None;
        s.run.style = None;
        if s.kind == TextStyleKind::Character {
            s.para = ParaProps::default();
        }
        out.push(s);
    }
    out
}

fn style(id: &str, name: &str, kind: TextStyleKind, para: ParaProps, run: RunProps) -> TextStyle {
    TextStyle { id: id.into(), name: name.into(), kind, based_on: None, para, run, bindings: vec![] }
}

fn color(slot: SchemeSlot) -> Option<Fill> {
    Some(Fill::solid(ColorRef::scheme(slot)))
}

/// The built-in styles, in the order the UI lists them. Sizes suit a 960 × 540 pt slide. Text
/// styles leave the colour to the shape (dark on the slide, light on a filled shape); Big Number
/// and Accent are coloured on purpose, and fall back to the shape's colour where theirs would
/// not read ([`crate::resolve::run`]).
pub fn builtin() -> Vec<TextStyle> {
    use TextStyleKind::{Character, Paragraph};
    let major = || Some("+mj-lt".to_string());
    let minor = || Some("+mn-lt".to_string());
    // Running text: the paragraph composer with optical margins.
    let composed = ParaProps { composer: Some(Composer::Paragraph), optical_margin: Some(true), ..Default::default() };
    vec![
        style(
            "title",
            "Title",
            Paragraph,
            ParaProps { line_spacing: Some(Spacing::Pct(0.9)), ..Default::default() },
            RunProps { font: major(), size: Some(44.0), bold: Some(true), ..Default::default() },
        ),
        style("subtitle", "Subtitle", Paragraph, ParaProps::default(), RunProps { font: minor(), size: Some(24.0), ..Default::default() }),
        style(
            "heading",
            "Heading",
            Paragraph,
            ParaProps { space_after: Some(Spacing::Pts(6.0)), ..Default::default() },
            RunProps { font: major(), size: Some(32.0), bold: Some(true), ..Default::default() },
        ),
        style(
            "body",
            "Body",
            Paragraph,
            ParaProps { space_after: Some(Spacing::Pts(8.0)), line_spacing: Some(Spacing::Pct(1.1)), ..composed.clone() },
            RunProps { font: minor(), size: Some(20.0), ..Default::default() },
        ),
        style("caption", "Caption", Paragraph, composed.clone(), RunProps { font: minor(), size: Some(14.0), ..Default::default() }),
        style(
            "quote",
            "Quote",
            Paragraph,
            ParaProps { margin_left: Some(36.0), ..composed },
            RunProps { font: major(), size: Some(28.0), italic: Some(true), ..Default::default() },
        ),
        style(
            "bigNumber",
            "Big Number",
            Paragraph,
            ParaProps { line_spacing: Some(Spacing::Pct(0.85)), ..Default::default() },
            RunProps { font: major(), size: Some(96.0), bold: Some(true), fill: color(SchemeSlot::Accent1), ..Default::default() },
        ),
        style("strong", "Strong", Character, ParaProps::default(), RunProps { bold: Some(true), ..Default::default() }),
        style("emphasis", "Emphasis", Character, ParaProps::default(), RunProps { italic: Some(true), ..Default::default() }),
        style("accent", "Accent", Character, ParaProps::default(), RunProps { fill: color(SchemeSlot::Accent1), ..Default::default() }),
    ]
}

/// Whether `id` names a built-in style.
pub fn is_builtin(id: &str) -> bool {
    builtin().iter().any(|s| s.id == id)
}

impl Presentation {
    /// The style `id` as defined (the deck's entry, else the built-in one), before `basedOn`.
    pub fn text_style(&self, id: &str) -> Option<TextStyle> {
        self.text_styles.iter().find(|s| s.id == id).cloned().or_else(|| builtin().into_iter().find(|s| s.id == id))
    }

    /// Every style the deck can use: the built-ins (as the deck changed them), then its own.
    pub fn all_text_styles(&self) -> Vec<TextStyle> {
        let mut out: Vec<TextStyle> = builtin().into_iter().map(|b| self.text_styles.iter().find(|s| s.id == b.id).cloned().unwrap_or(b)).collect();
        for s in &self.text_styles {
            if !out.iter().any(|o| o.id == s.id) {
                out.push(s.clone());
            }
        }
        out
    }

    /// The style `id` with its `basedOn` chain applied (at most [`MAX_BASED_ON`] deep; a cycle
    /// stops at the first repeat).
    pub fn resolved_text_style(&self, id: &str) -> Option<TextStyle> {
        let mut out = self.text_style(id)?;
        let mut seen = vec![out.id.clone()];
        let mut next = out.based_on.clone();
        while let Some(b) = next.take() {
            if seen.len() > MAX_BASED_ON || seen.contains(&b) {
                break;
            }
            let Some(parent) = self.text_style(&b) else { break };
            out.para.inherit(&parent.para);
            out.run.inherit(&parent.run);
            seen.push(b);
            next = parent.based_on;
        }
        Some(out)
    }

    /// Paragraph settings and run defaults a paragraph style gives (none for an unknown id or a
    /// character style).
    pub fn paragraph_style_props(&self, id: Option<&str>) -> Option<(ParaProps, RunProps)> {
        let s = self.resolved_text_style(id?)?;
        (s.kind == TextStyleKind::Paragraph).then_some((s.para, s.run))
    }

    /// Run settings a character style gives (none for an unknown id or a paragraph style).
    pub fn character_style_props(&self, id: Option<&str>) -> Option<RunProps> {
        let s = self.resolved_text_style(id?)?;
        (s.kind == TextStyleKind::Character).then_some(s.run)
    }
}

/// `own` without the settings it shares with `base`: what is left after removing a style that
/// was flattened into direct formatting. Settings `own` lacks stay unset.
pub fn strip_equal<T: Serialize + DeserializeOwned + Clone>(own: &T, base: &T) -> T {
    let (Ok(serde_json::Value::Object(mut a)), Ok(serde_json::Value::Object(b))) = (serde_json::to_value(own), serde_json::to_value(base)) else {
        return own.clone();
    };
    a.retain(|k, v| b.get(k) != Some(v));
    serde_json::from_value(serde_json::Value::Object(a)).unwrap_or_else(|_| own.clone())
}

/// Styles stored outside a `.deckcraft` file (the `.pptx` extension) are at most this long.
pub const MAX_JSON: usize = 1 << 20;

/// `styles` as JSON for storing in another format; `None` when empty or longer than [`MAX_JSON`].
pub fn to_json(styles: &[TextStyle]) -> Option<String> {
    if styles.is_empty() {
        return None;
    }
    serde_json::to_string(styles).ok().filter(|j| j.len() <= MAX_JSON)
}

/// Styles from [`to_json`]'s output, sanitized; empty when the text is too long or not styles.
pub fn from_json(json: &str) -> Vec<TextStyle> {
    if json.len() > MAX_JSON {
        return Vec::new();
    }
    serde_json::from_str::<Vec<TextStyle>>(json).map(sanitize).unwrap_or_default()
}

/// The run settings a styled run's style supplies, in resolution order: its character style,
/// the paragraph's `defRPr`, the paragraph style's text settings.
fn run_base(pres: &Presentation, para: &Paragraph, run: &RunProps, ps: Option<&RunProps>) -> RunProps {
    let mut base = pres.character_style_props(run.style.as_deref()).unwrap_or_default();
    base.inherit(&para.def_props);
    if let Some(pr) = ps {
        base.inherit(pr);
    }
    base.style = None;
    base
}

fn styled(para: &Paragraph) -> bool {
    para.props.style.is_some() || para.runs.iter().any(|r| r.props.style.is_some())
}

/// `para` with its styles written out as direct formatting, for a format without styles
/// (`.pptx`): what PowerPoint shows is what SlideCraft resolves. Style ids stay, so
/// [`unflatten_deck`] can take the formatting back out. `None` when nothing is styled.
/// `keep_fill` says whether a style's text colour reads on the shape (see
/// [`crate::resolve::run`]); a colour that does not is left to the shape, as resolution does.
pub fn flatten(pres: &Presentation, para: &Paragraph, keep_fill: &dyn Fn(&Fill) -> bool) -> Option<Paragraph> {
    if !styled(para) {
        return None;
    }
    let ps = pres.paragraph_style_props(para.props.style.as_deref());
    let mut out = para.clone();
    if let Some((pp, _)) = &ps {
        out.props.inherit(pp);
    }
    let pr = ps.as_ref().map(|(_, r)| r);
    let flat_run = |own: &RunProps| {
        let mut base = run_base(pres, para, own, pr);
        if own.fill.is_none() && base.fill.as_ref().is_some_and(|f| !keep_fill(f)) && para.def_props.fill.is_none() {
            base.fill = None;
        }
        let mut out = own.clone();
        out.inherit(&base);
        out
    };
    for r in &mut out.runs {
        r.props = flat_run(&r.props);
    }
    if pr.is_some() {
        out.end_props = flat_run(&para.end_props);
    }
    Some(out)
}

fn flatten_shapes(ctx: &crate::resolve::Ctx, shapes: &mut [Shape], depth: usize) {
    if depth > 64 {
        return;
    }
    for i in 0..shapes.len() {
        let Some(sh) = shapes.get(i).cloned() else { continue };
        let keep = |f: &Fill| !crate::resolve::unreadable_on(ctx, &sh, Some(f));
        let Some(slot) = shapes.get_mut(i) else { continue };
        if let Some(t) = slot.text.as_mut() {
            for p in &mut t.paragraphs {
                if let Some(f) = flatten(ctx.pres, p, &keep) {
                    *p = f;
                }
            }
        }
        match &mut slot.kind {
            ShapeKind::Group { children, .. } => flatten_shapes(ctx, children, depth + 1),
            ShapeKind::Table(t) => {
                if let Some(derived) = t.with_column_formats() {
                    *t = derived;
                }
                for c in t.rows.iter_mut().flat_map(|r| r.cells.iter_mut()) {
                    for p in &mut c.text.paragraphs {
                        if let Some(f) = flatten(ctx.pres, p, &|_| true) {
                            *p = f;
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// The deck with every styled paragraph on its slides, layouts and masters flattened
/// ([`flatten`]), for writing a format without styles; `None` when nothing is styled.
pub fn flatten_deck(pres: &Presentation) -> Option<Presentation> {
    fn any_styled(shapes: &[Shape], depth: usize) -> bool {
        depth <= 64
            && shapes.iter().any(|sh| {
                sh.text.as_ref().is_some_and(|t| t.paragraphs.iter().any(styled))
                    || match &sh.kind {
                        ShapeKind::Group { children, .. } => any_styled(children, depth + 1),
                        ShapeKind::Table(t) => {
                            t.has_column_formats() || t.rows.iter().flat_map(|r| &r.cells).any(|c| c.text.paragraphs.iter().any(styled))
                        }
                        _ => false,
                    }
            })
    }
    let styled_anywhere = pres.slides.iter().any(|s| any_styled(&s.shapes, 0))
        || pres.masters.iter().any(|m| any_styled(&m.shapes, 0) || m.layouts.iter().any(|l| any_styled(&l.shapes, 0)));
    if !styled_anywhere {
        return None;
    }
    let mut out = pres.clone();
    for (i, slide) in out.slides.iter_mut().enumerate() {
        let Some(orig) = pres.slides.get(i) else { continue };
        let Some(ctx) = crate::resolve::Ctx::for_slide(pres, orig) else { continue };
        flatten_shapes(&ctx, &mut std::sync::Arc::make_mut(slide).shapes, 0);
    }
    for (mi, master) in out.masters.iter_mut().enumerate() {
        let Some(orig) = pres.masters.get(mi) else { continue };
        let m = std::sync::Arc::make_mut(master);
        let ctx = crate::resolve::Ctx::for_master(pres, orig);
        flatten_shapes(&ctx, &mut m.shapes, 0);
        for (li, l) in m.layouts.iter_mut().enumerate() {
            let Some(ol) = orig.layouts.get(li) else { continue };
            let ctx = crate::resolve::Ctx::for_layout(pres, orig, ol);
            flatten_shapes(&ctx, &mut l.shapes, 0);
        }
    }
    Some(out)
}

/// Undoes [`flatten`] on a paragraph read back: settings equal to what its styles supply are
/// removed, so later changes to a style show again.
pub fn unflatten(pres: &Presentation, para: &mut Paragraph) {
    if !styled(para) {
        clear_column_derived(para);
        return;
    }
    let ps = pres.paragraph_style_props(para.props.style.as_deref());
    if let Some((pp, _)) = &ps {
        // `strip_equal` goes through serde, which skips the export-only marker: keep it.
        let derived = para.props.column_derived;
        para.props = strip_equal(&para.props, pp);
        para.props.column_derived = derived;
    }
    let pr = ps.as_ref().map(|(_, r)| r.clone());
    let snapshot = para.clone();
    for r in &mut para.runs {
        let base = run_base(pres, &snapshot, &r.props, pr.as_ref());
        r.props = strip_equal(&r.props, &base);
    }
    if pr.is_some() {
        let base = run_base(pres, &snapshot, &snapshot.end_props, pr.as_ref());
        para.end_props = strip_equal(&para.end_props, &base);
    }
    clear_column_derived(para);
}

/// Takes out what a table column format supplied ([`crate::Table::with_column_formats`]).
fn clear_column_derived(para: &mut Paragraph) {
    let d = std::mem::take(&mut para.props.column_derived);
    if d & crate::table::DERIVED_ALIGN != 0 {
        para.props.align = None;
    }
    if d & crate::table::DERIVED_STYLE != 0 {
        para.props.style = None;
    }
}

fn unflatten_shapes(pres: &Presentation, shapes: &mut [Shape], depth: usize) {
    if depth > 64 {
        return;
    }
    for sh in shapes {
        if let Some(t) = sh.text.as_mut() {
            t.paragraphs.iter_mut().for_each(|p| unflatten(pres, p));
        }
        match &mut sh.kind {
            ShapeKind::Group { children, .. } => unflatten_shapes(pres, children, depth + 1),
            ShapeKind::Table(t) => {
                for c in t.rows.iter_mut().flat_map(|r| r.cells.iter_mut()) {
                    c.text.paragraphs.iter_mut().for_each(|p| unflatten(pres, p));
                }
            }
            _ => {}
        }
    }
}

/// [`unflatten`] on every paragraph of the deck's slides, layouts and masters.
pub fn unflatten_deck(pres: &mut Presentation) {
    let styles = Presentation { text_styles: pres.text_styles.clone(), ..Default::default() };
    for slide in &mut pres.slides {
        let s = std::sync::Arc::make_mut(slide);
        unflatten_shapes(&styles, &mut s.shapes, 0);
    }
    for master in &mut pres.masters {
        let m = std::sync::Arc::make_mut(master);
        unflatten_shapes(&styles, &mut m.shapes, 0);
        for l in &mut m.layouts {
            unflatten_shapes(&styles, &mut l.shapes, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_have_unique_valid_ids_and_bind_to_the_theme() {
        let b = builtin();
        for (i, s) in b.iter().enumerate() {
            assert!(valid_id(&s.id), "{}", s.id);
            assert!(b.iter().skip(i + 1).all(|o| o.id != s.id), "duplicate {}", s.id);
            if let Some(f) = &s.run.font {
                assert!(f.starts_with('+'), "{} uses a theme font, not {f}", s.id);
            }
            if let Some(Fill::Solid { color }) = &s.run.fill {
                assert!(matches!(color.base, crate::ColorBase::Scheme { .. }), "{} uses a theme colour", s.id);
            }
        }
        for want in ["title", "subtitle", "heading", "body", "caption", "quote", "bigNumber"] {
            assert!(b.iter().any(|s| s.id == want && s.kind == TextStyleKind::Paragraph), "{want}");
        }
    }

    #[test]
    fn deck_entries_override_builtins_and_based_on_fills_gaps_without_looping() {
        let p = Presentation {
            text_styles: vec![
                TextStyle { id: "body".into(), name: "Body".into(), run: RunProps { size: Some(22.0), ..Default::default() }, ..Default::default() },
                TextStyle {
                    id: "a".into(),
                    name: "A".into(),
                    based_on: Some("b".into()),
                    run: RunProps { bold: Some(true), ..Default::default() },
                    ..Default::default()
                },
                TextStyle {
                    id: "b".into(),
                    name: "B".into(),
                    based_on: Some("a".into()),
                    run: RunProps { size: Some(9.0), ..Default::default() },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(p.text_style("body").and_then(|s| s.run.size), Some(22.0));
        assert_eq!(p.text_style("title").map(|s| s.name), Some("Title".into()));
        let a = p.resolved_text_style("a").expect("a");
        assert_eq!((a.run.bold, a.run.size), (Some(true), Some(9.0)), "b fills what a leaves unset; the cycle back to a stops");
        assert_eq!(p.all_text_styles().len(), builtin().len() + 2);
        assert!(p.resolved_text_style("missing").is_none());
    }

    #[test]
    fn hostile_style_lists_are_bounded() {
        let mut many: Vec<TextStyle> = (0..1000).map(|i| TextStyle { id: format!("s{i}"), name: "x".repeat(10_000), ..Default::default() }).collect();
        many.push(TextStyle { id: "bad id!".into(), ..Default::default() });
        many.insert(0, TextStyle { id: "s0".into(), based_on: Some("s0".into()), ..Default::default() });
        let kept = sanitize(many);
        assert_eq!(kept.len(), MAX_STYLES);
        assert!(kept.iter().all(|s| valid_id(&s.id) && s.name.chars().count() <= MAX_NAME));
        assert_eq!(kept[0].based_on, None, "a style is not based on itself");
        let json = serde_json::json!({"textStyles": [{"id": "x", "para": {"style": "x"}, "run": {"style": "y"}}]});
        let p: Presentation = serde_json::from_value(json).expect("presentation");
        assert_eq!((p.text_styles[0].para.style.clone(), p.text_styles[0].run.style.clone()), (None, None));
    }

    #[test]
    fn strip_equal_removes_what_the_style_supplied() {
        let style = RunProps { size: Some(20.0), bold: Some(true), ..Default::default() };
        let flat = RunProps { size: Some(20.0), bold: Some(true), italic: Some(true), ..Default::default() };
        assert_eq!(strip_equal(&flat, &style), RunProps { italic: Some(true), ..Default::default() });
        let changed = RunProps { size: Some(24.0), bold: Some(true), ..Default::default() };
        assert_eq!(strip_equal(&changed, &style), RunProps { size: Some(24.0), ..Default::default() });
    }

    #[test]
    fn flatten_then_unflatten_restores_the_paragraph() {
        use crate::text::{Paragraph, Run};
        let p = Presentation {
            text_styles: vec![TextStyle {
                id: "big".into(),
                name: "Big".into(),
                run: RunProps { size: Some(60.0), ..Default::default() },
                ..Default::default()
            }],
            ..Default::default()
        };
        let para = Paragraph {
            props: ParaProps { style: Some("body".into()), align: Some(crate::text::Align::Center), ..Default::default() },
            runs: vec![
                Run::with("plain ", RunProps::default()),
                Run::with("loud", RunProps { style: Some("strong".into()), italic: Some(true), ..Default::default() }),
            ],
            def_props: RunProps { size: Some(30.0), ..Default::default() },
            ..Default::default()
        };
        let flat = flatten(&p, &para, &|_| true).expect("styled");
        // What PowerPoint sees: the style's settings, with direct formatting and defRPr on top.
        assert_eq!(flat.props.composer, Some(Composer::Paragraph));
        assert_eq!(flat.props.align, Some(crate::text::Align::Center));
        assert_eq!(flat.runs[0].props.size, Some(30.0), "defRPr beats the paragraph style");
        assert_eq!(flat.runs[0].props.font.as_deref(), Some("+mn-lt"));
        assert_eq!((flat.runs[1].props.bold, flat.runs[1].props.italic), (Some(true), Some(true)));
        let mut back = flat.clone();
        unflatten(&p, &mut back);
        assert_eq!(back, para);
        assert!(flatten(&p, &Paragraph::default(), &|_| true).is_none());
    }

    #[test]
    fn a_coloured_style_gives_way_on_a_shape_its_colour_would_vanish_on() {
        use crate::text::{Paragraph, Run};
        let p = crate::defaults::new_presentation(None);
        let slide = p.slides[0].clone();
        let ctx = crate::resolve::Ctx::for_slide(&p, &slide).expect("ctx");
        let para = Paragraph {
            props: ParaProps { style: Some("bigNumber".into()), ..Default::default() },
            runs: vec![Run::with("42%", RunProps::default())],
            ..Default::default()
        };
        let accent = ctx.color(&ColorRef::scheme(SchemeSlot::Accent1), None);
        let text_colour = |shape: &Shape| crate::resolve::text_color(&ctx, &crate::resolve::run(&ctx, shape, &para, &para.runs[0].props));
        let mut plain = slide.shapes[0].clone();
        plain.fill = Some(Fill::None);
        assert_eq!(text_colour(&plain), accent, "on the slide the number is in the accent colour");
        let mut filled = plain.clone();
        filled.fill = Some(Fill::solid(ColorRef::scheme(SchemeSlot::Accent1)));
        let c = text_colour(&filled);
        assert_ne!(c, accent);
        assert!(c.contrast(accent) >= crate::resolve::MIN_STYLE_CONTRAST, "{c:?} reads on the accent fill");
        // A colour the run asks for itself is kept, even where it does not read.
        let mut own = para.clone();
        own.runs[0].props.fill = Some(Fill::solid(ColorRef::scheme(SchemeSlot::Accent1)));
        assert_eq!(crate::resolve::text_color(&ctx, &crate::resolve::run(&ctx, &filled, &own, &own.runs[0].props)), accent);
    }
}
