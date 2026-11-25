//! Fit auto-sizing text boxes (Resize shape to fit text) in a shape tree that is not, or not yet,
//! on a slide: the layout library's new slides and components, and component instances rebuilt
//! by the design pass. Each box's height becomes what its text needs at its width, as
//! `cmd::text::layout::fit_text_box` does for a box being typed into; auto layout then reflows.

use deckcraft_model::resolve::Ctx;
use deckcraft_model::text::AutoFit;
use deckcraft_model::{Presentation, Shape, ShapeKind, Slide};

/// Fit every auto-sizing text box in `list` (and its groups), measured with `doc`'s theme and
/// text styles. Whether anything changed.
pub(crate) fn fit_tree(doc: &Presentation, list: &mut [Shape]) -> bool {
    let layout = doc.masters.first().and_then(|m| m.layouts.first()).map(|l| l.id);
    let Some(layout) = layout else { return false };
    let probe = Slide { layout, ..Default::default() };
    let Some(ctx) = Ctx::for_slide(doc, &probe) else { return false };
    fit_list(&ctx, list, 0)
}

fn fit_list(ctx: &Ctx, list: &mut [Shape], depth: usize) -> bool {
    if depth > deckcraft_model::canvas::MAX_DEPTH {
        return false;
    }
    let mut changed = false;
    for sh in list.iter_mut() {
        if let ShapeKind::Group { children, .. } = &mut sh.kind {
            changed |= fit_list(ctx, children, depth + 1);
            continue;
        }
        changed |= fit_one(ctx, sh);
    }
    changed
}

fn fit_one(ctx: &Ctx, sh: &mut Shape) -> bool {
    let (Some(body), Some(x)) = (sh.text.as_ref(), sh.xfrm) else { return false };
    if body.body.autofit != Some(AutoFit::Shape) || sh.ph.is_some() || !(x.w.is_finite() && x.w > 0.0) {
        return false;
    }
    let geo = deckcraft_render::shape_geometry(sh, x.w, x.h);
    let need = deckcraft_text::fit_height(ctx, sh, body, geo.text_rect, &deckcraft_text::NoFields);
    let extra = (x.h - geo.text_rect.height()).max(0.0);
    let h = (need + extra).max(8.0);
    if !h.is_finite() || (h - x.h).abs() < 0.01 {
        return false;
    }
    sh.xfrm = Some(deckcraft_geom::Xfrm { h, ..x });
    true
}
