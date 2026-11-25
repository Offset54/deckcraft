//! SlideCraft design-system pass (Phase 4), run after every edit next to the canvas pass:
//!
//! 1. **Theme ↔ variables.** Colour and font variables mapped to a theme slot drive every
//!    master's theme in the deck's active modes. When an edit changed the variables (a value,
//!    a mode switch), the masters' themes are rewritten from them; when an edit changed a theme
//!    some other way (Design ▸ Themes, Colors, Fonts) and left the variables alone, the theme's
//!    new colours and fonts are taken into the variables' active mode instead, so the two never
//!    disagree.
//! 2. **Bindings.** Number, string and boolean bindings are written into their fields on the
//!    slides, layouts and masters that have bindings, when the variables changed or the slide
//!    itself did. Colours are not written: they resolve when drawn, so a Light/Dark switch copies
//!    no slide.
//!
//! The variables are compared by pointer (`Presentation::design` is an `Arc`), so an edit that
//! leaves them alone costs one comparison.

use std::sync::Arc;

use deckcraft_model::design_styles::{apply_styles, detach_edited, uses_styles};
use deckcraft_model::variables::{DesignSystem, apply_bindings, has_bindings};
use deckcraft_model::{Master, Presentation, Shape, ShapeKind};

fn same_design(a: &Presentation, b: &Presentation) -> bool {
    match (&a.design, &b.design) {
        (None, None) => true,
        (Some(x), Some(y)) => Arc::ptr_eq(x, y),
        _ => false,
    }
}

/// Settle `new` after an edit of `old`.
pub(crate) fn settle_changed(old: &Presentation, new: &mut Presentation) {
    let Some(ds) = new.design.clone() else { return };
    let design_changed = !same_design(old, new);
    if design_changed {
        apply_theme(&ds, new);
    } else {
        take_theme(old, new);
    }
    let ds = new.design.clone().unwrap_or(ds);
    if design_changed {
        bind_text_styles(&ds, new);
    }
    let styles_changed = design_changed && old.design.as_ref().is_none_or(|o| o.styles != ds.styles);
    let old_slides: std::collections::HashMap<_, _> = old.slides.iter().map(|s| (s.id, s)).collect();
    for slide in new.slides.iter_mut() {
        let untouched = old_slides.get(&slide.id).is_some_and(|b| Arc::ptr_eq(b, slide));
        if !untouched || styles_changed {
            style_slide(&ds, slide, !untouched && !styles_changed);
        }
        if (untouched && !design_changed) || !has_bindings(&slide.shapes) {
            continue;
        }
        bind_slide(&ds, slide);
    }
    for master in new.masters.iter_mut() {
        let untouched = old.masters.iter().any(|m| Arc::ptr_eq(m, master));
        if untouched && !design_changed {
            continue;
        }
        style_master(&ds, master, !untouched && !styles_changed);
        bind_master(&ds, master);
    }
    components_settle(old, new, design_changed);
}

/// Paragraph and character styles bound to variables take their values (deck modes).
fn bind_text_styles(ds: &DesignSystem, p: &mut Presentation) {
    for st in p.text_styles.iter_mut().filter(|s| !s.bindings.is_empty()) {
        for b in &st.bindings {
            let Some(v) = ds.resolve(b.variable, &[]) else { continue };
            match b.field {
                deckcraft_model::variables::BindField::FontSize => {
                    if let Some(n) = v.as_number() {
                        st.run.size = Some(n.clamp(1.0, 4000.0));
                    }
                }
                deckcraft_model::variables::BindField::FontFamily => {
                    if let Some(f) = v.as_str().filter(|f| !f.is_empty()) {
                        st.run.font = Some(f.to_string());
                    }
                }
                _ => {}
            }
        }
    }
}

/// Paint, effect and grid styles on one shape tree: with `edited`, links whose look was changed
/// some other way are dropped first; then every link's look is written. Whether it changed.
fn style_list(ds: &DesignSystem, list: &mut [Shape], edited: bool) -> bool {
    if !uses_styles(list) {
        return false;
    }
    let mut changed = false;
    if edited {
        changed |= detach_edited(&ds.styles, list);
    }
    changed | apply_styles(&ds.styles, list)
}

fn style_slide(ds: &DesignSystem, slide: &mut Arc<deckcraft_model::Slide>, edited: bool) {
    if !uses_styles(&slide.shapes) {
        return;
    }
    let mut shapes = slide.shapes.clone();
    if style_list(ds, &mut shapes, edited) {
        Arc::make_mut(slide).shapes = shapes;
    }
}

fn style_master(ds: &DesignSystem, master: &mut Arc<Master>, edited: bool) {
    if !uses_styles(&master.shapes) && !master.layouts.iter().any(|l| uses_styles(&l.shapes)) {
        return;
    }
    let mut m = (**master).clone();
    let mut changed = style_list(ds, &mut m.shapes, edited);
    for l in &mut m.layouts {
        changed |= style_list(ds, &mut l.shapes, edited);
    }
    if changed {
        *master = Arc::new(m);
    }
}

/// Fresh ids for instance layers while the deck's slides are borrowed: counts on from
/// `Presentation::next_id`, which is written back afterwards.
struct Counter(u32);

impl deckcraft_model::component::IdSource for Counter {
    fn next_id(&mut self) -> Option<u32> {
        self.0 = self.0.max(1).checked_add(1)?;
        Some(self.0)
    }
}

/// The library as the deck has it (empty without a design system).
fn library(p: &Presentation) -> Vec<deckcraft_model::component::Component> {
    p.design.as_ref().map(|d| d.components.clone()).unwrap_or_default()
}

/// Components whose definition differs between `old` and `new` (added, changed or removed).
fn changed_components(old: &Presentation, new: &Presentation) -> Vec<u32> {
    let a = library(old);
    let b = library(new);
    let mut out: Vec<u32> = b.iter().filter(|c| a.iter().find(|o| o.id == c.id) != Some(*c)).map(|c| c.id).collect();
    out.extend(a.iter().filter(|o| !b.iter().any(|c| c.id == o.id)).map(|o| o.id));
    out
}

/// A main component edited on a slide goes back into the library.
fn take_mains(list: &[Shape], lib: &mut [deckcraft_model::component::Component]) -> bool {
    let mut changed = false;
    deckcraft_model::walk(list, &mut |sh, _| {
        let Some(cid) = sh.main_of else { return };
        let Some(c) = lib.iter_mut().find(|c| c.id == cid) else { return };
        let mut root = sh.clone();
        root.main_of = None;
        root.instance = None;
        if c.root != root {
            c.root = root;
            changed = true;
        }
    });
    changed
}

/// Whether an instance (or anything nested in it) shows one of `comps`.
fn shows(sh: &Shape, comps: &[u32], lib: &[deckcraft_model::component::Component]) -> bool {
    let Some(inst) = &sh.instance else { return false };
    if comps.contains(&inst.component) {
        return true;
    }
    // A nested instance inside the component, or a swapped one.
    let mut nested = false;
    if let Some(c) = lib.iter().find(|c| c.id == inst.component) {
        deckcraft_model::walk(c.root.children(), &mut |s, _| nested |= s.instance.as_ref().is_some_and(|n| comps.contains(&n.component)));
    }
    nested || inst.props.iter().any(|(_, v)| matches!(v, deckcraft_model::component::PropValue::Swap(id) if comps.contains(id)))
}

/// Rebuild the instances in `list` that `pick` selects (top-level instances; nested ones are
/// rebuilt by their parent). With `capture`, edits made inside an instance become overrides
/// first. Whether anything changed.
fn sync_list(
    ctx_doc: &Presentation,
    list: &mut [Shape],
    lib: &[deckcraft_model::component::Component],
    ids: &mut Counter,
    capture: &dyn Fn(&Shape) -> bool,
    pick: &dyn Fn(&Shape) -> bool,
    depth: usize,
) -> bool {
    if depth > deckcraft_model::canvas::MAX_DEPTH {
        return false;
    }
    let mut changed = false;
    for sh in list.iter_mut() {
        if sh.instance.is_some() {
            if !pick(sh) {
                continue;
            }
            let Some(mut inst) = sh.instance.as_deref().cloned() else { continue };
            let Some(comp) = lib.iter().find(|c| c.id == inst.component) else {
                // Its component is gone: it stays as a plain frame.
                sh.instance = None;
                changed = true;
                continue;
            };
            if capture(sh) {
                deckcraft_model::component::capture_overrides(lib, comp, sh, &mut inst);
            }
            if let Some(built) = deckcraft_model::component::materialize(lib, comp, sh, &inst, ids) {
                let mut built = built;
                crate::canvas::settle_list(std::slice::from_mut(&mut built));
                // Its text reflows: auto-sizing boxes take the height their text needs (the
                // component's main was sized for the component's own text).
                if crate::library_fit::fit_tree(ctx_doc, std::slice::from_mut(&mut built)) {
                    crate::canvas::settle_list(std::slice::from_mut(&mut built));
                }
                if built != *sh {
                    *sh = built;
                    changed = true;
                }
            }
            continue;
        }
        if let ShapeKind::Group { children, .. } = &mut sh.kind {
            changed |= sync_list(ctx_doc, children, lib, ids, capture, pick, depth + 1);
        }
    }
    changed
}

fn has_instances(list: &[Shape]) -> bool {
    let mut found = false;
    deckcraft_model::walk(list, &mut |s, _| found |= s.instance.is_some() || s.main_of.is_some());
    found
}

/// The component step of the design pass.
fn components_settle(old: &Presentation, new: &mut Presentation, _design_changed: bool) {
    if new.design.as_ref().is_none_or(|d| d.components.is_empty()) {
        // Instances of a library that is gone become plain frames.
        if library(old).is_empty() {
            return;
        }
    }
    let old_slides: std::collections::HashMap<_, _> = old.slides.iter().map(|s| (s.id, s)).collect();
    let touched: Vec<bool> = new.slides.iter().map(|s| !old_slides.get(&s.id).is_some_and(|b| Arc::ptr_eq(b, s))).collect();
    // 1. Mains edited on slides go into the library.
    let mut lib = library(new);
    let mut from_mains = false;
    for (slide, t) in new.slides.iter().zip(&touched) {
        if *t {
            from_mains |= take_mains(&slide.shapes, &mut lib);
        }
    }
    if from_mains && let Some(d) = new.design.as_mut() {
        Arc::make_mut(d).components = lib.clone();
    }
    let changed = changed_components(old, new);
    if changed.is_empty() && !touched.iter().any(|t| *t) {
        return;
    }
    let mut ids = Counter(new.next_id);
    // Text in rebuilt instances is measured with the deck's theme as it now is.
    let ctx_doc = new.clone();
    // 2. Every instance of a changed component, and every instance on an edited slide.
    for (slide, t) in new.slides.iter_mut().zip(&touched) {
        if !has_instances(&slide.shapes) {
            continue;
        }
        let edited = *t;
        let pick = |sh: &Shape| edited || shows(sh, &changed, &lib);
        // Edits inside an instance are captured only where its component didn't change in the
        // same step (a difference then is the component's change, not an edit).
        let capture = |sh: &Shape| edited && !shows(sh, &changed, &lib);
        let mut shapes = slide.shapes.clone();
        if sync_list(&ctx_doc, &mut shapes, &lib, &mut ids, &capture, &pick, 0) {
            let s = Arc::make_mut(slide);
            s.shapes = shapes;
            crate::canvas::settle_list(&mut s.shapes);
        }
    }
    for m in new.masters.iter_mut() {
        let any = has_instances(&m.shapes) || m.layouts.iter().any(|l| has_instances(&l.shapes));
        if !any {
            continue;
        }
        let untouched = old.masters.iter().any(|o| Arc::ptr_eq(o, m));
        if untouched && changed.is_empty() {
            continue;
        }
        let pick = |sh: &Shape| !untouched || shows(sh, &changed, &lib);
        let capture = |sh: &Shape| !untouched && !shows(sh, &changed, &lib);
        let mut mm = (**m).clone();
        let mut ch = sync_list(&ctx_doc, &mut mm.shapes, &lib, &mut ids, &capture, &pick, 0);
        for l in &mut mm.layouts {
            ch |= sync_list(&ctx_doc, &mut l.shapes, &lib, &mut ids, &capture, &pick, 0);
        }
        if ch {
            crate::canvas::settle_list(&mut mm.shapes);
            for l in &mut mm.layouts {
                crate::canvas::settle_list(&mut l.shapes);
            }
            *m = Arc::new(mm);
        }
    }
    new.next_id = new.next_id.max(ids.0);
}

fn bind_slide(ds: &DesignSystem, slide: &mut Arc<deckcraft_model::Slide>) {
    // Resolve on a copy first: a slide whose bound fields already hold their values is not
    // copied (a mode switch that only changes colours copies nothing).
    let mut shapes = slide.shapes.clone();
    if apply_bindings(ds, &mut shapes, &slide.modes) {
        let s = Arc::make_mut(slide);
        s.shapes = shapes;
        crate::canvas::settle_list(&mut s.shapes);
    }
}

fn bind_master(ds: &DesignSystem, master: &mut Arc<Master>) {
    let any = has_bindings(&master.shapes) || master.layouts.iter().any(|l| has_bindings(&l.shapes));
    if !any {
        return;
    }
    let mut m = (**master).clone();
    let mut changed = apply_bindings(ds, &mut m.shapes, &[]);
    for l in &mut m.layouts {
        changed |= apply_bindings(ds, &mut l.shapes, &[]);
    }
    if changed {
        crate::canvas::settle_list(&mut m.shapes);
        for l in &mut m.layouts {
            crate::canvas::settle_list(&mut l.shapes);
        }
        *master = Arc::new(m);
    }
}

/// Rewrite every master's theme from the variables (only the masters that differ are copied).
fn apply_theme(ds: &DesignSystem, p: &mut Presentation) {
    for m in p.masters.iter_mut() {
        let mut theme = m.theme.clone();
        if ds.apply_to_theme(&mut theme) {
            Arc::make_mut(m).theme = theme;
        }
    }
}

/// A theme changed without the variables: take the first changed master's theme into them.
fn take_theme(old: &Presentation, new: &mut Presentation) {
    let changed = new.masters.iter().find(|m| !old.masters.iter().any(|o| o.id == m.id && o.theme == m.theme)).map(|m| m.theme.clone());
    let Some(theme) = changed else { return };
    let Some(ds) = new.design.as_mut() else { return };
    if !ds.variables.iter().any(|v| v.theme.is_some()) {
        return;
    }
    let mut d = (**ds).clone();
    if d.take_from_theme(&theme) {
        *ds = Arc::new(d);
        // The other masters follow the one that was changed.
        if let Some(ds) = new.design.clone() {
            apply_theme(&ds, new);
        }
    }
}

/// Settle a file just opened: clean the design system, then apply it everywhere.
pub(crate) fn settle_all(p: &mut Presentation) {
    let Some(ds) = p.design.as_mut() else { return };
    let mut d = (**ds).clone();
    d.sanitize();
    if d.is_empty() {
        p.design = None;
        return;
    }
    if d != **ds {
        *ds = Arc::new(d);
    }
    let Some(ds) = p.design.clone() else { return };
    apply_theme(&ds, p);
    bind_text_styles(&ds, p);
    for slide in p.slides.iter_mut() {
        style_slide(&ds, slide, false);
        if has_bindings(&slide.shapes) {
            bind_slide(&ds, slide);
        }
    }
    for m in p.masters.iter_mut() {
        style_master(&ds, m, false);
        bind_master(&ds, m);
    }
    p.fix_next_id();
}

/// The deck as `.pptx` should get it: every variable colour's `last` value is its value on that
/// slide (writers without the variables use it). `None` when nothing changes.
pub(crate) fn resolved_for_export(doc: &Presentation) -> Option<Presentation> {
    let ds = doc.design.as_ref()?;
    let mut out: Option<Presentation> = None;
    for (i, slide) in doc.slides.iter().enumerate() {
        let mut shapes = slide.shapes.clone();
        if deckcraft_model::variables::refresh_last(ds, &mut shapes, &slide.modes) {
            let deck = out.get_or_insert_with(|| doc.clone());
            if let Some(s) = deck.slides.get_mut(i) {
                Arc::make_mut(s).shapes = shapes;
            }
        }
    }
    for (i, m) in doc.masters.iter().enumerate() {
        let mut mm = (**m).clone();
        let mut changed = deckcraft_model::variables::refresh_last(ds, &mut mm.shapes, &[]);
        for l in &mut mm.layouts {
            changed |= deckcraft_model::variables::refresh_last(ds, &mut l.shapes, &[]);
        }
        if changed {
            let deck = out.get_or_insert_with(|| doc.clone());
            if let Some(slot) = deck.masters.get_mut(i) {
                *slot = Arc::new(mm);
            }
        }
    }
    out
}
