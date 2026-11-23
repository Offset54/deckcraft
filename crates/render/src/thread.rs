//! Threaded text: text boxes linked with `Shape::thread_next`. The first box of a thread holds
//! the whole text; [`flow_slide`] gives each box the part that falls in it, laying the text out
//! box after box and cutting it at the first line a box cannot hold. Drawing, export and the
//! `.pptx` writer all work from that flowed slide, so every surface shows the same text.
//!
//! Limits: boxes on one slide, at most [`MAX_FRAMES`] per thread, top-level shapes only (not in
//! groups); numbered lists restart in each box; the last box shows whatever is left, overflowing
//! as any box does.

use std::collections::{HashMap, HashSet};

use deckcraft_model::edit::{self, Pos};
use deckcraft_model::resolve::{self, Ctx};
use deckcraft_model::text::{Anchor, TextBody};
use deckcraft_model::{Shape, ShapeId, Slide};
use deckcraft_text::{NoFields, Opts, layout};

use crate::shape_geometry;

/// Most boxes one thread flows through.
pub const MAX_FRAMES: usize = 16;

/// The threads on a slide, each as shape indices from its first box. A link to a box that is
/// missing, already in a thread, or back into the thread ends it there.
pub fn chains(shapes: &[Shape]) -> Vec<Vec<usize>> {
    let at: HashMap<ShapeId, usize> = shapes.iter().enumerate().map(|(i, s)| (s.id, i)).collect();
    let targets: HashSet<ShapeId> = shapes.iter().filter_map(|s| s.thread_next).filter(|t| at.contains_key(t)).collect();
    let mut used: HashSet<usize> = HashSet::new();
    let mut out = Vec::new();
    for (i, s) in shapes.iter().enumerate() {
        if s.thread_next.is_none() || targets.contains(&s.id) || s.text.is_none() {
            continue;
        }
        let mut chain = vec![i];
        used.insert(i);
        let mut next = s.thread_next;
        while let Some(id) = next {
            let Some(&j) = at.get(&id) else { break };
            if chain.len() >= MAX_FRAMES || !used.insert(j) {
                break;
            }
            chain.push(j);
            next = shapes.get(j).and_then(|x| x.thread_next);
        }
        if chain.len() > 1 {
            out.push(chain);
        }
    }
    out
}

/// Where `body` stops fitting in `frame`: the position of the first line below the box, or
/// `None` when all of it fits (or the text runs vertically, where boxes are not split).
fn split_point(ctx: &Ctx, frame: &Shape, body: &TextBody) -> Option<Pos> {
    let x = resolve::xfrm(ctx, frame);
    let rect = shape_geometry(frame, x.w, x.h).text_rect;
    // Measured from the top: a box anchored in the middle or at the bottom would centre the
    // overflowing text and start it above the box. Each box anchors its own part when drawn.
    // `layout` reads the body settings from the shape, so the box is measured holding the text.
    let mut top = body.clone();
    top.body.anchor = Some(Anchor::Top);
    let measured = Shape { text: Some(top), ..frame.clone() };
    let text = measured.text.as_ref()?;
    let l = layout(ctx, &measured, text, &Opts { rect, fields: &NoFields, prompt_color: None, no_shrink: true });
    if l.rotation.abs() > 1e-9 {
        return None;
    }
    let limit = l.inner.y1 + 0.5;
    l.lines.iter().find(|li| li.bottom > limit).map(|li| (li.para, li.start))
}

/// `frame`'s own text settings (insets, anchor, columns, list style) with `paragraphs`.
fn in_frame(frame: &Shape, from: &TextBody) -> TextBody {
    let own = frame.text.clone().unwrap_or_default();
    TextBody { paragraphs: from.paragraphs.clone(), ..own }
}

/// A slide with its threads flowed ([`flow`]).
pub struct Flowed {
    pub slide: Slide,
    /// Boxes whose first paragraph goes on from the previous box's last one (the cut fell
    /// inside a paragraph), so a reader can join them back.
    pub joined: Vec<ShapeId>,
}

/// The slide with each thread's text flowed through its boxes; `None` when it has no thread.
/// `keep_whole`: a first box being edited keeps its whole text, so carets reach all of it.
pub fn flow_slide(ctx: &Ctx, slide: &Slide, keep_whole: Option<ShapeId>) -> Option<Slide> {
    flow(ctx, slide, keep_whole).map(|f| f.slide)
}

/// [`flow_slide`], also saying which boxes start inside a paragraph.
pub fn flow(ctx: &Ctx, slide: &Slide, keep_whole: Option<ShapeId>) -> Option<Flowed> {
    let chains = chains(&slide.shapes);
    if chains.is_empty() {
        return None;
    }
    let mut out = slide.clone();
    let mut joined = Vec::new();
    for chain in chains {
        let Some(story) = chain.first().and_then(|&i| slide.shapes.get(i)).and_then(|s| s.text.clone()) else { continue };
        let mut rest = story.clone();
        for (k, &si) in chain.iter().enumerate() {
            let Some(frame) = slide.shapes.get(si) else { continue };
            let body = if k == 0 { story.clone() } else { in_frame(frame, &rest) };
            let last = k + 1 == chain.len();
            let (here, there) = match (last, split_point(ctx, frame, &body)) {
                (false, Some(at)) => {
                    if at.1 > 0
                        && let Some(&next) = chain.get(k + 1)
                        && let Some(n) = slide.shapes.get(next)
                    {
                        joined.push(n.id);
                    }
                    (edit::slice(&body, (0, 0), at), Some(edit::slice(&body, at, edit::end(&body))))
                }
                _ => (body, None),
            };
            if let Some(slot) = out.shapes.get_mut(si) {
                let whole = k == 0 && keep_whole == Some(frame.id);
                slot.text = Some(if whole { story.clone() } else { here });
            }
            match there {
                Some(t) => rest = t,
                None => rest = TextBody { paragraphs: vec![], ..rest },
            }
        }
    }
    Some(Flowed { slide: out, joined })
}

#[cfg(test)]
mod tests {
    use deckcraft_model::text::{Bullet, TextBody};
    use deckcraft_model::{LayoutType, Presentation, Xfrm, defaults};

    use super::*;

    const STORY: &str = "Threaded text runs from one box into the next. It starts in the first box, and what that box cannot hold continues in the second, then the third, so a long passage can wind around pictures and across columns of a layout without being cut by hand.";

    fn slide_with(boxes: &[(f64, f64, f64, f64)]) -> (Presentation, Slide) {
        let mut p = Presentation::default();
        let lay = defaults::layout_of_kind(&p, LayoutType::Blank).expect("layout");
        let mut s = defaults::new_slide(&mut p, lay).expect("slide");
        s.shapes.clear();
        for (i, &(x, y, w, h)) in boxes.iter().enumerate() {
            let mut sh =
                Shape { id: ShapeId(100 + i as u32), xfrm: Some(Xfrm::new(x, y, w, h)), text: Some(TextBody::default()), ..Default::default() };
            if i + 1 < boxes.len() {
                sh.thread_next = Some(ShapeId(101 + i as u32));
            }
            s.shapes.push(sh);
        }
        let mut body = TextBody::from_text(STORY);
        for para in &mut body.paragraphs {
            para.props.bullet = Some(Bullet::None);
            for r in &mut para.runs {
                r.props.size = Some(20.0);
            }
        }
        s.shapes[0].text = Some(body);
        p.slides = vec![std::sync::Arc::new(s.clone())];
        (p, s)
    }

    fn text_of(s: &Slide, i: usize) -> String {
        s.shapes[i].text.as_ref().map(|t| t.paragraphs.iter().map(|p| p.text()).collect::<Vec<_>>().join("\n")).unwrap_or_default()
    }

    #[test]
    fn text_flows_through_the_boxes_without_loss_or_repeats() {
        let (p, s) = slide_with(&[(20.0, 20.0, 260.0, 90.0), (300.0, 20.0, 260.0, 90.0), (580.0, 20.0, 260.0, 400.0)]);
        let ctx = Ctx::for_slide(&p, &s).expect("ctx");
        let f = flow_slide(&ctx, &s, None).expect("a thread");
        let parts: Vec<String> = (0..3).map(|i| text_of(&f, i)).collect();
        assert!(parts.iter().all(|t| !t.is_empty()), "every box gets some: {parts:?}");
        assert_eq!(parts.concat(), STORY, "the parts are the story, in order");
        // The first two hold what fits: nothing overflows them.
        for i in 0..2 {
            let sh = &f.shapes[i];
            let x = resolve::xfrm(&ctx, sh);
            let rect = shape_geometry(sh, x.w, x.h).text_rect;
            let l = layout(&ctx, sh, sh.text.as_ref().expect("text"), &Opts { rect, fields: &NoFields, prompt_color: None, no_shrink: true });
            assert!(!l.overflow, "box {i} holds its part");
        }
        // A single paragraph cut twice: the second and third boxes go on inside it.
        assert_eq!(flow(&ctx, &s, None).expect("a thread").joined, vec![ShapeId(101), ShapeId(102)]);
        // The first box being edited keeps the whole story.
        let editing = flow_slide(&ctx, &s, Some(ShapeId(100))).expect("a thread");
        assert_eq!(text_of(&editing, 0), STORY);
        assert_eq!(text_of(&editing, 1), parts[1]);
    }

    #[test]
    fn broken_and_looping_links_end_the_thread() {
        let (_, mut s) = slide_with(&[(0.0, 0.0, 100.0, 50.0), (0.0, 60.0, 100.0, 50.0)]);
        s.shapes[1].thread_next = Some(ShapeId(100));
        assert!(chains(&s.shapes).is_empty(), "a loop has no first box");
        s.shapes[1].thread_next = Some(ShapeId(999));
        assert_eq!(chains(&s.shapes), vec![vec![0, 1]], "a link to a missing box ends the thread");
        let many: Vec<_> = (0..40).map(|i| (0.0, i as f64 * 5.0, 100.0, 4.0)).collect();
        let (_, long) = slide_with(&many);
        assert_eq!(chains(&long.shapes).first().map(Vec::len), Some(MAX_FRAMES));
    }

    #[test]
    fn a_slide_without_threads_is_left_alone() {
        let (p, mut s) = slide_with(&[(0.0, 0.0, 100.0, 50.0)]);
        s.shapes[0].thread_next = None;
        let ctx = Ctx::for_slide(&p, &s).expect("ctx");
        assert!(flow_slide(&ctx, &s, None).is_none());
    }
}
