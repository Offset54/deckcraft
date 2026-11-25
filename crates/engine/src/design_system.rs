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

use deckcraft_model::variables::{DesignSystem, apply_bindings, has_bindings};
use deckcraft_model::{Master, Presentation};

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
    let old_slides: std::collections::HashMap<_, _> = old.slides.iter().map(|s| (s.id, s)).collect();
    for slide in new.slides.iter_mut() {
        let untouched = old_slides.get(&slide.id).is_some_and(|b| Arc::ptr_eq(b, slide));
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
        bind_master(&ds, master);
    }
    components_settle(old, new, design_changed);
}

/// Components are settled by P4.3.
fn components_settle(_old: &Presentation, _new: &mut Presentation, _design_changed: bool) {}

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
    for slide in p.slides.iter_mut() {
        if has_bindings(&slide.shapes) {
            bind_slide(&ds, slide);
        }
    }
    for m in p.masters.iter_mut() {
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
