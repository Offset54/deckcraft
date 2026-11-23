//! Threaded text (`text.threadTo`, `text.unthread`): link text boxes on a slide so text one box
//! cannot hold continues in the next. The flow itself is `deckcraft_render::thread`.

use std::sync::Arc;

use deckcraft_model::text::TextBody;
use deckcraft_model::{ShapeId, ShapeKind, Slide, TextWrap};
use deckcraft_render::thread::{MAX_FRAMES, chains};
use serde_json::{Value, json};

use super::{CommandSpec, bad, cmd, edit_shapes, has_selection, has_slide, id_param};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "text.threadTo",
            "Link Text Box",
            ["Shape Format", "Text"],
            None,
            "{to: id of a text box on the current slide, from?: id (default: the selected shape)}, or {} with two boxes selected (the first continues in the second) — text `from`'s thread cannot hold continues in `to`; `to`'s own text moves to the end of the thread → {thread: [ids from the first box]}",
            has_slide,
            thread_to
        ),
        cmd!(
            "arrange.textWrap",
            "Wrap Text",
            ["Shape Format", "Arrange"],
            None,
            "{mode: none|around (text in other boxes on the slide keeps clear of the shape's bounding box), margin?: pt 0–200 (default 8), ids?: [id] (default: the selection)}",
            has_selection,
            text_wrap
        ),
        cmd!(
            "text.unthread",
            "Unlink Text Box",
            ["Shape Format", "Text"],
            None,
            "{id?: id (default: the selected shape)} — ends the thread after this box; the text stays in the thread's first box",
            has_slide,
            unthread
        ),
    ]
}

/// The one shape a command acts on: `key`, else the single selected shape.
fn target(s: &Session, p: &Value, key: &str, cmd: &str) -> Result<ShapeId> {
    if let Some(id) = id_param(p, key, cmd)? {
        return Ok(id);
    }
    match s.doc()?.selection.shapes.as_slice() {
        [one] => Ok(*one),
        _ => Err(bad(cmd, format!("give `{key}` or select one shape"))),
    }
}

fn holds_text(slide: &Slide, id: ShapeId) -> bool {
    slide.shapes.iter().any(|s| s.id == id && matches!(s.kind, ShapeKind::Shape))
}

/// The thread `id` is in, as shape ids from its first box.
fn thread_of(slide: &Slide, id: ShapeId) -> Vec<ShapeId> {
    chains(&slide.shapes)
        .into_iter()
        .map(|c| c.into_iter().filter_map(|i| slide.shapes.get(i).map(|s| s.id)).collect::<Vec<_>>())
        .find(|c| c.contains(&id))
        .unwrap_or_else(|| vec![id])
}

fn thread_to(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "text.threadTo";
    // With two shapes selected and neither id given: the first continues in the second.
    let (from, to) = match (id_param(p, "from", CMD)?, id_param(p, "to", CMD)?, s.doc()?.selection.shapes.as_slice()) {
        (None, None, [a, b]) => (*a, *b),
        (_, None, _) => return Err(bad(CMD, "missing `to` (or select two boxes)")),
        (_, Some(to), _) => (target(s, p, "from", CMD)?, to),
    };
    let st = s.doc()?;
    let slide = st.current_slide().ok_or_else(|| bad(CMD, "no slide"))?;
    if from == to {
        return Err(bad(CMD, "a box cannot continue in itself"));
    }
    if !holds_text(slide, from) || !holds_text(slide, to) {
        return Err(bad(CMD, "both must be text boxes or shapes on the current slide (not in a group)"));
    }
    if slide.shapes.iter().any(|x| x.thread_next == Some(to) && x.id != from) {
        return Err(bad(CMD, "`to` already continues another box's text"));
    }
    let thread = thread_of(slide, from);
    if thread.contains(&to) {
        return Err(bad(CMD, "`to` is already in this thread"));
    }
    let after_to = thread_of(slide, to);
    if after_to.first() != Some(&to) {
        return Err(bad(CMD, "`to` already continues another box's text"));
    }
    let from_at = thread.iter().position(|x| *x == from).unwrap_or(0);
    if from_at + 1 + after_to.len() > MAX_FRAMES {
        return Err(bad(CMD, format!("a thread has at most {MAX_FRAMES} boxes")));
    }
    if slide.shapes.iter().find(|x| x.id == from).is_some_and(|x| x.thread_next.is_some()) {
        return Err(bad(CMD, "`from` already continues in another box; unlink it first (text.unthread)"));
    }
    let head = thread.first().copied().unwrap_or(from);
    let si = st.selection.slide;
    s.edit(move |doc, _| {
        let Some(slide) = doc.slides.get_mut(si) else { return Err(bad(CMD, "no slide")) };
        let slide = Arc::make_mut(slide);
        // `to`'s own text (and what flows on from it) joins the end of the thread's text.
        let moved = slide.shapes.iter_mut().find(|x| x.id == to).and_then(|x| {
            let own = x.text.get_or_insert_with(TextBody::default);
            let paras = std::mem::take(&mut own.paragraphs);
            (!paras.is_empty() && paras.iter().any(|p| !p.is_empty())).then_some(paras)
        });
        if let Some(h) = slide.shapes.iter_mut().find(|x| x.id == head) {
            let story = h.text.get_or_insert_with(TextBody::default);
            if let Some(paras) = moved {
                story.paragraphs.retain(|p| !p.is_empty() || p.runs.iter().any(|r| !r.text.is_empty()));
                story.paragraphs.extend(paras);
            }
        }
        if let Some(f) = slide.shapes.iter_mut().find(|x| x.id == from) {
            f.thread_next = Some(to);
        }
        Ok(())
    })?;
    let slide = s.doc()?.current_slide().cloned().ok_or_else(|| bad(CMD, "no slide"))?;
    Ok(json!({"thread": thread_of(&slide, from).iter().map(|i| i.0).collect::<Vec<_>>()}))
}

fn text_wrap(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "arrange.textWrap";
    let wrap = match p.get("mode").and_then(Value::as_str) {
        Some("none") => None,
        Some("around") => {
            let margin = match p.get("margin") {
                None | Some(Value::Null) => 8.0,
                Some(v) => v
                    .as_f64()
                    .filter(|m| m.is_finite() && (0.0..=TextWrap::MAX_MARGIN).contains(m))
                    .ok_or_else(|| bad(CMD, format!("`margin` must be 0–{} pt", TextWrap::MAX_MARGIN)))?,
            };
            Some(TextWrap { margin })
        }
        _ => return Err(bad(CMD, "`mode` must be none or around")),
    };
    edit_shapes(s, p, CMD, move |sh| {
        sh.text_wrap = wrap;
        Ok(())
    })?;
    Ok(json!({}))
}

fn unthread(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "text.unthread";
    let id = target(s, p, "id", CMD)?;
    let st = s.doc()?;
    let slide = st.current_slide().ok_or_else(|| bad(CMD, "no slide"))?;
    if !slide.shapes.iter().any(|x| x.id == id && x.thread_next.is_some()) {
        return Err(bad(CMD, "this box does not continue in another"));
    }
    let si = st.selection.slide;
    s.edit(move |doc, _| {
        let Some(slide) = doc.slides.get_mut(si) else { return Err(bad(CMD, "no slide")) };
        if let Some(x) = Arc::make_mut(slide).shapes.iter_mut().find(|x| x.id == id) {
            x.thread_next = None;
        }
        Ok(())
    })?;
    Ok(json!({}))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    fn two_boxes() -> (Session, u64, u64) {
        let mut s = Session::with_new();
        s.execute("slide.new", &json!({"layout": "blank"})).expect("slide");
        let a = s.execute("shape.insert", &json!({"preset": "rect", "rect": [40.0, 40.0, 200.0, 60.0], "text": "First box text."})).expect("a")["id"]
            .as_u64()
            .expect("id");
        let b = s.execute("shape.insert", &json!({"preset": "rect", "rect": [300.0, 40.0, 200.0, 60.0], "text": "Second."})).expect("b")["id"]
            .as_u64()
            .expect("id");
        (s, a, b)
    }

    fn text(s: &Session, id: u64) -> String {
        let st = s.doc().expect("doc");
        let sh = st.current_slide().and_then(|sl| sl.shapes.iter().find(|x| u64::from(x.id.0) == id)).cloned().expect("shape");
        sh.text.map(|t| t.paragraphs.iter().map(|p| p.text()).collect::<Vec<_>>().join("\n")).unwrap_or_default()
    }

    #[test]
    fn linking_moves_the_second_boxs_text_into_the_thread_and_undoes() {
        let (mut s, a, b) = two_boxes();
        let r = s.execute("text.threadTo", &json!({"from": a, "to": b})).expect("link");
        assert_eq!(r["thread"], json!([a, b]));
        assert_eq!(text(&s, a), "First box text.\nSecond.");
        assert_eq!(text(&s, b), "");
        s.execute("edit.undo", &json!({})).expect("undo");
        assert_eq!(text(&s, b), "Second.");
        s.execute("edit.redo", &json!({})).expect("redo");
        s.execute("text.unthread", &json!({"id": a})).expect("unlink");
        assert_eq!(text(&s, a), "First box text.\nSecond.", "the text stays in the first box");
    }

    #[test]
    fn refuses_links_that_would_loop_or_branch() {
        let (mut s, a, b) = two_boxes();
        let c = s.execute("shape.insert", &json!({"preset": "rect", "rect": [40.0, 200.0, 200.0, 60.0]})).expect("c")["id"].as_u64().expect("id");
        s.execute("text.threadTo", &json!({"from": a, "to": b})).expect("link");
        let before = s.doc().expect("doc").doc.clone();
        for p in [
            json!({"from": b, "to": a}),
            json!({"from": a, "to": a}),
            json!({"from": c, "to": b}),
            json!({"from": a, "to": c}),
            json!({"from": a}),
            json!({}),
            json!({"from": a, "to": 9999}),
        ] {
            assert!(s.execute("text.threadTo", &p).is_err(), "{p}");
        }
        assert!(s.execute("text.unthread", &json!({"id": c})).is_err());
        assert_eq!(s.doc().expect("doc").doc, before);
        // b → c extends the thread.
        assert_eq!(s.execute("text.threadTo", &json!({"from": b, "to": c})).expect("extend")["thread"], json!([a, b, c]));
    }

    #[test]
    fn editing_a_continuation_box_edits_the_threads_first_box() {
        let (mut s, a, b) = two_boxes();
        s.execute("text.threadTo", &json!({"from": a, "to": b})).expect("link");
        let r = s.execute("text.edit", &json!({"id": b, "at": [0, 0]})).expect("edit");
        assert_eq!(r["id"], json!(a));
        s.execute("text.insert", &json!({"text": " More."})).expect("type");
        assert_eq!(text(&s, a), "First box text.\nSecond. More.");
        assert_eq!(text(&s, b), "", "the continuation box keeps no text of its own");
    }

    #[test]
    fn a_thread_saved_as_pptx_shows_each_boxs_part_and_reopens_as_one_text() {
        let (mut s, a, b) = two_boxes();
        let long = "Words that do not fit in one small box carry on into the next box on the slide. ".repeat(4);
        s.execute("text.edit", &json!({"id": a})).expect("edit");
        s.execute("text.insert", &json!({"text": long})).expect("type");
        s.execute("text.exit", &json!({})).ok();
        s.execute("text.threadTo", &json!({"from": a, "to": b})).expect("link");
        let story = text(&s, a);
        let bytes = super::super::file::save_bytes(&s.doc().expect("doc").doc, "pptx").expect("pptx");
        // PowerPoint sees the text split between the boxes: the second box's part is in the file.
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes.clone())).expect("zip");
        let name = (0..z.len())
            .filter_map(|i| z.by_index(i).ok().map(|f| f.name().to_string()))
            .filter(|n| n.starts_with("ppt/slides/slide"))
            .max()
            .expect("a slide");
        let mut xml = String::new();
        std::io::Read::read_to_string(&mut z.by_name(&name).expect("slide"), &mut xml).expect("xml");
        assert_eq!(xml.matches("<sc:thread").count(), 2, "both boxes are marked: {xml}");
        let second = xml.rfind("<p:sp>").map(|i| &xml[i..]).expect("second box");
        assert!(second.contains("<a:t>"), "the second box carries its part of the text");
        let back = deckcraft_pptx::import(&bytes).expect("import");
        let slide = back.slides.last().expect("slide");
        let by = |id: u64| slide.shapes.iter().find(|x| u64::from(x.id.0) == id).cloned().expect("shape");
        let joined = |sh: &deckcraft_model::Shape| {
            sh.text.as_ref().map(|t| t.paragraphs.iter().map(|p| p.text()).collect::<Vec<_>>().join("\n")).unwrap_or_default()
        };
        assert_eq!(joined(&by(a)), story, "the first box holds the whole text again");
        assert_eq!(joined(&by(b)), "");
        assert_eq!(by(a).thread_next.map(|x| u64::from(x.0)), Some(b));
    }

    #[test]
    fn two_selected_boxes_link_first_to_second() {
        let (mut s, a, b) = two_boxes();
        s.execute("edit.select", &json!({"ids": [a, b]})).expect("select");
        assert_eq!(s.execute("text.threadTo", &json!({})).expect("link")["thread"], json!([a, b]));
    }

    #[test]
    fn wrap_text_sets_and_clears_the_shapes_wrap() {
        let (mut s, a, _) = two_boxes();
        s.execute("edit.select", &json!({"ids": [a]})).expect("select");
        s.execute("arrange.textWrap", &json!({"mode": "around", "margin": 12})).expect("wrap");
        let wrap = |s: &Session| s.doc().expect("doc").selected_shapes().first().and_then(|x| x.text_wrap);
        assert_eq!(wrap(&s), Some(deckcraft_model::TextWrap { margin: 12.0 }));
        for bad in [json!({"mode": "tight"}), json!({"mode": "around", "margin": -1}), json!({"mode": "around", "margin": 1000})] {
            assert!(s.execute("arrange.textWrap", &bad).is_err(), "{bad}");
        }
        s.execute("arrange.textWrap", &json!({"mode": "none"})).expect("off");
        assert_eq!(wrap(&s), None);
        s.execute("edit.undo", &json!({})).expect("undo");
        assert_eq!(wrap(&s), Some(deckcraft_model::TextWrap { margin: 12.0 }));
    }
}
