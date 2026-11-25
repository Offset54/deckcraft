//! The theme kit, the layout library and Tidy up (P4.4).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use deckcraft_model::{SchemeSlot, ShapeId, ShapeKind};
use serde_json::{Value, json};

use crate::Session;

fn id(v: &Value) -> u32 {
    v["id"].as_u64().unwrap() as u32
}

#[test]
fn every_library_layout_makes_a_slide_in_one_undo_step() {
    let mut s = Session::with_new();
    let layouts = s.execute("slide.library", &json!({})).unwrap()["layouts"].as_array().unwrap().clone();
    assert!(layouts.len() >= 30, "the brief asks for 30 or more: {}", layouts.len());
    for l in &layouts {
        let n = s.doc().unwrap().doc.slides.len();
        let steps = s.doc().unwrap().history.undo.len();
        let r = s.execute("slide.fromLibrary", &json!({"layout": l["id"]})).unwrap();
        let d = s.doc().unwrap();
        assert_eq!(d.doc.slides.len(), n + 1, "{l}");
        assert_eq!(d.history.undo.len(), steps + 1, "{l}");
        let slide = &d.doc.slides[r["index"].as_u64().unwrap() as usize];
        assert!(!slide.shapes.is_empty(), "{l}");
        assert_eq!(d.doc.validate(), Vec::<String>::new(), "{l}");
    }
    // The kit and the components the layouts use are in the deck once each.
    let ds = s.doc().unwrap().doc.design_system().unwrap().clone();
    assert_eq!(ds.collections.len(), 1);
    let mut names: Vec<String> = ds.components.iter().map(|c| c.display_name()).collect();
    let n = names.len();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), n, "{names:?}");
    assert!(n >= 9, "{names:?}");
    assert!(s.execute("slide.fromLibrary", &json!({"layout": "nope"})).is_err());
}

#[test]
fn library_slides_restyle_with_the_mode_and_the_type_scale() {
    let mut s = Session::with_new();
    let r = s.execute("slide.fromLibrary", &json!({"layout": "stats"})).unwrap();
    let i = r["index"].as_u64().unwrap() as usize;
    let light = s.doc().unwrap().doc.masters[0].theme.colors.get(SchemeSlot::Lt1);
    let before = s.doc().unwrap().doc.clone();
    s.execute("variable.mode", &json!({"collection": "Theme", "mode": "Dark"})).unwrap();
    let after = s.doc().unwrap().doc.clone();
    for (a, b) in after.slides.iter().zip(&before.slides) {
        if !Arc::ptr_eq(a, b) {
            let mut fa = vec![];
            let mut fb = vec![];
            deckcraft_model::walk(&a.shapes, &mut |s, _| fa.push(s.clone()));
            deckcraft_model::walk(&b.shapes, &mut |s, _| fb.push(s.clone()));
            for (x, y) in fa.iter().zip(&fb) {
                if x != y {
                    let mut x2 = x.clone();
                    let mut y2 = y.clone();
                    if let ShapeKind::Group { children, .. } = &mut x2.kind {
                        children.clear();
                    }
                    if let ShapeKind::Group { children, .. } = &mut y2.kind {
                        children.clear();
                    }
                    panic!("a mode switch copies no slide; {} differs:\n{:?}\n{:?}", x.name, x2, y2);
                }
            }
        }
    }
    let dark = after.masters[0].theme.colors.get(SchemeSlot::Lt1);
    assert!(dark.luminance() < 0.05 && light.luminance() > 0.8, "{light:?} → {dark:?}");
    // The title is bound to type/title: changing the variable resizes it.
    let title = after.slides[i].shapes.iter().find(|sh| sh.name == "Title").unwrap().id;
    s.execute("variable.set", &json!({"variable": "type/title", "value": 44})).unwrap();
    let t = s.doc().unwrap().doc.slides[i].shape(title).unwrap().clone();
    assert!(t.text.unwrap().paragraphs[0].runs.iter().all(|r| r.props.size == Some(44.0)));
}

#[test]
fn editing_a_library_component_updates_every_slide_that_uses_it() {
    let mut s = Session::with_new();
    s.execute("slide.fromLibrary", &json!({"layout": "threeUp"})).unwrap();
    s.execute("slide.fromLibrary", &json!({"layout": "contact"})).unwrap();
    let comp = s.execute("component.list", &json!({})).unwrap()["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "Feature card")
        .unwrap()
        .clone();
    assert_eq!(comp["instances"], 6);
    let r = s.execute("component.editMain", &json!({"component": comp["id"]})).unwrap();
    let main = ShapeId(r["main"].as_u64().unwrap() as u32);
    let icon = s.doc().unwrap().shape(main).unwrap().children().iter().find(|c| c.name == "Icon").unwrap().id;
    s.execute("shape.fill", &json!({"ids": [icon.0], "color": "#E4572E"})).unwrap();
    let d = s.doc().unwrap();
    let mut seen = 0;
    for sl in &d.doc.slides {
        deckcraft_model::walk(&sl.shapes, &mut |sh, _| {
            if sh.name == "Icon" && sh.id != icon {
                seen += 1;
                assert_eq!(sh.fill, d.shape(icon).unwrap().fill);
            }
        });
    }
    assert_eq!(seen, 6);
}

#[test]
fn the_kit_follows_a_built_in_theme_in_both_modes() {
    let mut s = Session::with_new();
    s.execute("design.kit", &json!({"theme": "Harbor", "mode": "Dark"})).unwrap();
    let d = s.doc().unwrap().doc.clone();
    assert_eq!(d.masters[0].theme.fonts.major.latin, "Merriweather");
    let bg = d.masters[0].theme.colors.get(SchemeSlot::Lt1);
    let text = d.masters[0].theme.colors.get(SchemeSlot::Dk1);
    assert!(bg.luminance() < 0.05, "dark background {bg:?}");
    assert!(text.contrast(bg) >= 7.0);
    // Changing the theme afterwards refreshes both modes of the kit.
    s.execute("design.theme", &json!({"name": "Ember"})).unwrap();
    let d = s.doc().unwrap().doc.clone();
    assert_eq!(d.masters[0].theme.fonts.major.latin, "Montserrat");
    assert!(d.masters[0].theme.colors.get(SchemeSlot::Lt1).luminance() < 0.05, "still dark");
    s.execute("variable.mode", &json!({"collection": "Theme", "mode": "Light"})).unwrap();
    let light = s.doc().unwrap().doc.masters[0].theme.colors.get(SchemeSlot::Lt1);
    assert_eq!(light, deckcraft_color::Rgba::from_hex("FFFCF7").unwrap(), "Ember's own background");
    assert!(s.execute("design.kit", &json!({"theme": "Nope"})).is_err());
    assert!(s.execute("design.kit", &json!({"mode": "Sepia"})).is_err());
}

#[test]
fn tidy_up_lines_a_messy_selection_up_in_reading_order() {
    let mut s = Session::with_new();
    s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
    // Two rough rows of three, out of order and unevenly spaced.
    let spots = [(420.0, 95.0), (100.0, 110.0), (250.0, 100.0), (110.0, 300.0), (430.0, 280.0), (260.0, 310.0)];
    let mut ids = vec![];
    for (x, y) in spots {
        ids.push(id(&s.execute("shape.insert", &json!({"preset": "rect", "rect": [x, y, 100, 80]})).unwrap()));
    }
    let steps = s.doc().unwrap().history.undo.len();
    let r = s.execute("arrange.tidy", &json!({"ids": ids})).unwrap();
    assert_eq!(s.doc().unwrap().history.undo.len(), steps + 1, "one undo step");
    assert_eq!((r["columns"].as_u64(), r["rows"].as_u64()), (Some(3), Some(2)));
    let gap = r["gap"].as_f64().unwrap();
    let f = ShapeId(id(&r));
    let d = s.doc().unwrap();
    let frame = d.shape(f).unwrap();
    assert!(frame.auto_layout.is_some());
    let order: Vec<u32> = frame.children().iter().map(|c| c.id.0).collect();
    assert_eq!(order, vec![ids[1], ids[2], ids[0], ids[3], ids[5], ids[4]], "reading order");
    let b: Vec<_> = frame.children().iter().map(|c| c.xfrm.unwrap()).collect();
    assert!((b[1].x - (b[0].x + b[0].w) - gap).abs() < 1e-6 && (b[2].x - (b[1].x + b[1].w) - gap).abs() < 1e-6, "even gaps");
    assert!((b[3].y - (b[0].y + b[0].h) - gap).abs() < 1e-6, "rows {gap}");
    assert!((b[0].y - b[2].y).abs() < 1e-6, "aligned rows");
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(matches!(s.doc().unwrap().doc.slides[s.doc().unwrap().selection.slide].shapes.len(), 6));
    // One object, or objects in different groups, are refused.
    assert!(s.execute("arrange.tidy", &json!({"ids": [ids[0]]})).is_err());
    assert!(s.execute("arrange.tidy", &json!({"ids": ids, "columns": 0})).is_err());
    assert!(s.execute("arrange.tidy", &json!({"ids": ids, "gap": -1})).is_err());
    let g = id(&s.execute("arrange.group", &json!({"ids": [ids[0], ids[1]]})).unwrap());
    assert!(s.execute("arrange.tidy", &json!({"ids": [ids[0], ids[2]]})).is_err());
    let _ = g;
    let st = s.doc().unwrap();
    assert!(st.doc.slides[st.selection.slide].shapes.iter().any(|sh| matches!(sh.kind, ShapeKind::Group { .. })));
}

#[test]
fn a_decks_own_theme_collection_is_never_taken_for_the_kit() {
    // Regression: the kit was found by the name "Theme", so Design > Themes overwrote a user's
    // collection of that name.
    let mut s = Session::with_new();
    s.execute("variable.createCollection", &json!({"name": "Theme", "modes": ["Light", "Dark"]})).unwrap();
    let mine = id(&s
        .execute(
            "variable.create",
            &json!({"collection": "Theme", "name": "color/surface", "type": "color", "values": {"Light": "#123456", "Dark": "#654321"}}),
        )
        .unwrap());
    s.execute("design.theme", &json!({"name": "Harbor"})).unwrap();
    s.execute("design.kit", &json!({})).unwrap();
    s.execute("design.theme", &json!({"name": "Ember"})).unwrap();
    let ds = s.doc().unwrap().doc.design_system().unwrap().clone();
    assert_eq!(ds.collections.len(), 2);
    assert_eq!(ds.kit().map(|c| c.name.as_str()), Some("Theme kit"));
    let v = ds.variable(mine).unwrap();
    let light = ds.collection_named("Theme").and_then(|c| c.mode_named("Light")).unwrap().id;
    assert_eq!(v.value_in(light), Some(&deckcraft_model::variables::VarValue::Color(deckcraft_color::Rgba::from_hex("123456").unwrap())));
    assert_eq!(v.values.len(), 2, "untouched");
}
