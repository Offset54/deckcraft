//! Check Accessibility › Reading Order: the objects of a slide in the order a screen reader
//! reads them. That is the order of the shape tree, back to front, a group's members right
//! after their group, so `arrange.reorder` (index 0 = read first) is also what reorders it.

use deckcraft_model::{Shape, ShapeKind};
use serde::Serialize;
use serde_json::{Value, json};

use crate::cmd::{bad, usize_param};
use crate::{Result, Session};

/// Most text of an object quoted in its row.
const MAX_TEXT: usize = 80;

/// One object in reading order.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingItem {
    pub id: u32,
    pub name: String,
    /// `picture`, `textBox`, `group`… as `shape.inspect` names kinds.
    pub kind: &'static str,
    /// How many groups it is inside.
    pub depth: usize,
    pub parent: Option<u32>,
    /// Its place among the objects of the same parent, 0 = read first.
    pub index: usize,
    /// How many objects share that parent.
    pub siblings: usize,
    pub alt_text: String,
    /// Marked as decorative: a screen reader skips it.
    pub decorative: bool,
    pub hidden: bool,
    /// A picture, chart, media object or group with no alt text that is not decorative.
    pub needs_alt_text: bool,
    /// The start of its own text, for telling objects apart.
    pub text: String,
    /// The slide's title placeholder.
    pub title: bool,
}

/// `shapes` (a slide's tree) in reading order.
pub fn reading_order(shapes: &[Shape]) -> Vec<ReadingItem> {
    fn rec(v: &[Shape], depth: usize, parent: Option<u32>, out: &mut Vec<ReadingItem>) {
        // The same bounds as the other walks over a shape tree.
        if depth > 64 {
            return;
        }
        for (index, sh) in v.iter().enumerate() {
            let needs_alt = matches!(sh.kind, ShapeKind::Picture { .. } | ShapeKind::Chart(_) | ShapeKind::Media(_) | ShapeKind::Group { .. });
            let text: String = sh.text.as_ref().map(|t| t.text()).unwrap_or_default();
            let mut quoted: String = text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_TEXT).collect();
            if text.chars().count() > MAX_TEXT {
                quoted.push('…');
            }
            out.push(ReadingItem {
                id: sh.id.0,
                name: sh.name.clone(),
                kind: sh.kind_name(),
                depth,
                parent,
                index,
                siblings: v.len(),
                alt_text: sh.descr.clone(),
                decorative: sh.decorative,
                hidden: sh.hidden,
                needs_alt_text: needs_alt && sh.descr.trim().is_empty() && !sh.decorative,
                text: quoted,
                title: matches!(sh.ph_type(), Some(deckcraft_model::PhType::Title | deckcraft_model::PhType::CtrTitle)),
            });
            rec(sh.children(), depth + 1, Some(sh.id.0), out);
        }
    }
    let mut out = Vec::new();
    rec(shapes, 0, None, &mut out);
    out
}

pub(super) fn reading_order_query(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let slide = usize_param(p, "slide").unwrap_or(st.selection.slide);
    let sl = st.doc.slides.get(slide).ok_or_else(|| bad("review.readingOrder", "no such slide"))?;
    Ok(json!({"slide": slide, "items": reading_order(&sl.shapes)}))
}

#[cfg(test)]
mod patch_tests {
    use serde_json::{Value, json};

    use crate::Session;

    /// A blank slide with a rectangle "A", a text box "B" and a group of two rectangles.
    fn session() -> (Session, Vec<u64>) {
        let mut s = Session::with_new();
        s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
        let mut ids = vec![];
        for (k, x) in [100.0, 250.0, 400.0, 550.0].iter().enumerate() {
            let r = s.execute("shape.insert", &json!({"preset": "rect", "rect": [x, 100.0, 80.0, 60.0], "text": format!("Box {k}")})).unwrap();
            ids.push(r["id"].as_u64().unwrap());
        }
        let g = s.execute("arrange.group", &json!({"ids": [ids[2], ids[3]]})).unwrap();
        ids.push(g["id"].as_u64().unwrap());
        (s, ids)
    }

    fn items(s: &mut Session) -> Vec<Value> {
        s.execute("review.readingOrder", &json!({})).unwrap()["items"].as_array().unwrap().clone()
    }

    fn order(s: &mut Session) -> Vec<u64> {
        items(s).iter().map(|i| i["id"].as_u64().unwrap()).collect()
    }

    #[test]
    fn objects_are_listed_back_to_front_with_group_members_after_their_group() {
        let (mut s, ids) = session();
        assert_eq!(order(&mut s), vec![ids[0], ids[1], ids[4], ids[2], ids[3]]);
        let all = items(&mut s);
        assert_eq!(all[0]["text"], "Box 0");
        assert_eq!((all[0]["index"].as_u64(), all[0]["siblings"].as_u64(), all[0]["depth"].as_u64()), (Some(0), Some(3), Some(0)));
        // The group has no alt text yet; its members sit one level in.
        assert_eq!(all[2]["kind"], "group");
        assert_eq!(all[2]["needsAltText"], true);
        assert_eq!((all[3]["depth"].as_u64(), all[3]["parent"].as_u64(), all[3]["index"].as_u64()), (Some(1), Some(ids[4]), Some(0)));
    }

    #[test]
    fn reordering_and_alt_text_show_in_the_reading_order() {
        let (mut s, ids) = session();
        // Read the second box first: the same move as sending it to the back.
        s.execute("arrange.reorder", &json!({"id": ids[1], "index": 0})).unwrap();
        assert_eq!(order(&mut s), vec![ids[1], ids[0], ids[4], ids[2], ids[3]]);
        // A member moves inside its group only.
        s.execute("arrange.reorder", &json!({"id": ids[3], "index": 0})).unwrap();
        assert_eq!(order(&mut s), vec![ids[1], ids[0], ids[4], ids[3], ids[2]]);
        s.execute("shape.altText", &json!({"text": "Two boxes", "ids": [ids[4]]})).unwrap();
        s.execute("shape.altText", &json!({"text": "", "decorative": true, "ids": [ids[0]]})).unwrap();
        let all = items(&mut s);
        assert_eq!((&all[2]["altText"], &all[2]["needsAltText"]), (&json!("Two boxes"), &json!(false)));
        assert_eq!(all[1]["decorative"], true);
    }

    #[test]
    fn another_slide_can_be_asked_for_and_a_missing_one_is_refused() {
        let (mut s, _) = session();
        let first = s.execute("review.readingOrder", &json!({"slide": 0})).unwrap();
        assert_eq!(first["slide"], 0);
        assert!(first["items"].as_array().unwrap().iter().any(|i| i["title"] == true));
        assert!(s.execute("review.readingOrder", &json!({"slide": 99})).is_err());
    }

    #[test]
    fn long_text_is_quoted_short() {
        let mut s = Session::with_new();
        s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
        s.execute("shape.insert", &json!({"preset": "rect", "rect": [0.0, 0.0, 80.0, 60.0], "text": "word ".repeat(500)})).unwrap();
        let text = items(&mut s)[0]["text"].as_str().unwrap().to_string();
        assert!(text.chars().count() <= 81 && text.ends_with('…'), "{text}");
    }
}
