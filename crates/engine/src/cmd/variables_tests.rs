//! Variables with modes and aliases (P4.1).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use deckcraft_model::{SchemeSlot, ShapeId};
use serde_json::{Value, json};

use crate::Session;

fn id(v: &Value) -> u32 {
    v["id"].as_u64().unwrap() as u32
}

fn color_at(s: &mut Session, x: f64, y: f64) -> String {
    s.execute("slide.colorAt", &json!({"x": x, "y": y})).unwrap()["color"].as_str().unwrap().to_string()
}

/// A blank slide, a "Theme" collection with Light and Dark, a colour `surface` (white / near
/// black, driving lt1), a colour `ink` aliasing nothing, and a rectangle.
fn setup() -> (Session, u32, u32, u32, ShapeId) {
    let mut s = Session::with_new();
    s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
    let c = s.execute("variable.createCollection", &json!({"name": "Theme", "modes": ["Light", "Dark"]})).unwrap();
    let coll = id(&c);
    let surface = id(&s
        .execute(
            "variable.create",
            &json!({"collection": "Theme", "name": "color/surface", "type": "color", "values": {"Light": "#FFFFFF", "Dark": "#101820"}, "theme": "lt1"}),
        )
        .unwrap());
    let accent = id(&s
        .execute(
            "variable.create",
            &json!({"collection": coll, "name": "color/accent", "type": "color", "values": {"Light": "#2E6FD8", "Dark": "#FFB000"}}),
        )
        .unwrap());
    let r = ShapeId(id(&s.execute("shape.insert", &json!({"preset": "rect", "rect": [100, 100, 200, 100]})).unwrap()));
    (s, coll, surface, accent, r)
}

#[test]
fn a_mode_switch_restyles_bound_colours_without_copying_slides() {
    let (mut s, _coll, _surface, accent, r) = setup();
    s.execute("variable.bind", &json!({"ids": [r.0], "field": "fill", "variable": accent})).unwrap();
    assert_eq!(color_at(&mut s, 200.0, 150.0), "2E6FD8");
    // The background follows the theme's lt1, which the surface variable drives.
    assert_eq!(color_at(&mut s, 20.0, 20.0), "FFFFFF");
    let before = s.doc().unwrap().doc.clone();
    let steps = s.doc().unwrap().history.undo.len();
    s.execute("variable.mode", &json!({"collection": "Theme", "mode": "Dark"})).unwrap();
    assert_eq!(s.doc().unwrap().history.undo.len(), steps + 1, "one undo step");
    let after = s.doc().unwrap().doc.clone();
    let rewritten: Vec<usize> =
        after.slides.iter().zip(&before.slides).enumerate().filter(|(_, (a, b))| !Arc::ptr_eq(a, b)).map(|(i, _)| i).collect();
    assert!(
        rewritten.is_empty(),
        "no slide is rewritten: {rewritten:?} {:?}",
        rewritten.first().map(|i| (before.slides[*i].shapes.len(), after.slides[*i] == before.slides[*i]))
    );
    assert_eq!(after.masters[0].theme.colors.get(SchemeSlot::Lt1), deckcraft_color::Rgba::rgb(0x10, 0x18, 0x20));
    assert_eq!(color_at(&mut s, 200.0, 150.0), "FFB000");
    assert_eq!(color_at(&mut s, 20.0, 20.0), "101820");
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(color_at(&mut s, 200.0, 150.0), "2E6FD8");
    assert_eq!(color_at(&mut s, 20.0, 20.0), "FFFFFF");
}

#[test]
fn a_slide_can_use_another_mode() {
    let (mut s, _, _, accent, r) = setup();
    s.execute("variable.bind", &json!({"ids": [r.0], "field": "fill", "variable": accent})).unwrap();
    let cur = s.doc().unwrap().selection.slide;
    s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
    let other = s.doc().unwrap().selection.slide;
    s.execute("variable.mode", &json!({"collection": "Theme", "mode": "Dark", "slides": [cur]})).unwrap();
    s.execute("slide.go", &json!({"index": cur})).unwrap();
    assert_eq!(color_at(&mut s, 200.0, 150.0), "FFB000");
    assert_eq!(color_at(&mut s, 20.0, 20.0), "101820", "theme colours follow the slide's mode too");
    s.execute("slide.go", &json!({"index": other})).unwrap();
    assert_eq!(color_at(&mut s, 20.0, 20.0), "FFFFFF");
    s.execute("variable.mode", &json!({"collection": "Theme", "mode": null, "slides": "all"})).unwrap();
    assert!(s.doc().unwrap().doc.slides.iter().all(|sl| sl.modes.is_empty()));
}

#[test]
fn aliases_follow_their_target_and_cycles_are_refused() {
    let (mut s, _, surface, accent, r) = setup();
    let card = id(&s
        .execute("variable.create", &json!({"collection": "Theme", "name": "color/card", "type": "color", "value": {"alias": accent}}))
        .unwrap());
    s.execute("variable.bind", &json!({"ids": [r.0], "field": "fill", "variable": "color/card"})).unwrap();
    assert_eq!(color_at(&mut s, 200.0, 150.0), "2E6FD8");
    s.execute("variable.set", &json!({"variable": accent, "value": "#00AA00"})).unwrap();
    assert_eq!(color_at(&mut s, 200.0, 150.0), "00AA00");
    let e = s.execute("variable.set", &json!({"variable": accent, "value": {"alias": card}})).unwrap_err();
    assert!(e.to_string().contains("refer back"), "{e}");
    let e = s.execute("variable.set", &json!({"variable": accent, "value": {"alias": accent}})).unwrap_err();
    assert!(e.to_string().contains("refer back"), "{e}");
    // An alias must be of the same type.
    let n = id(&s.execute("variable.create", &json!({"collection": "Theme", "name": "n", "type": "number", "value": 4})).unwrap());
    assert!(s.execute("variable.set", &json!({"variable": n, "value": {"alias": surface}})).is_err());
    // Deleting the target leaves the alias with its value, and the shape keeps its colour.
    s.execute("variable.delete", &json!({"variable": accent})).unwrap();
    assert_eq!(color_at(&mut s, 200.0, 150.0), "00AA00");
}

#[test]
fn number_string_and_boolean_bindings_drive_layout_text_and_visibility() {
    let (mut s, _, _, _, r) = setup();
    let gap = id(&s
        .execute("variable.create", &json!({"collection": "Theme", "name": "space/gap", "type": "number", "values": {"Light": 10, "Dark": 40}}))
        .unwrap());
    let b = ShapeId(id(&s.execute("shape.insert", &json!({"preset": "rect", "rect": [400, 100, 100, 100]})).unwrap()));
    let f = ShapeId(s.execute("layout.set", &json!({"ids": [r.0, b.0], "flow": "horizontal", "gap": 0})).unwrap()["ids"][0].as_u64().unwrap() as u32);
    s.execute("variable.bind", &json!({"ids": [f.0], "field": "gap", "variable": gap})).unwrap();
    let gap_now = |s: &Session| {
        let d = s.doc().unwrap();
        let a = d.shape(r).unwrap().xfrm.unwrap();
        let c = d.shape(b).unwrap().xfrm.unwrap();
        c.x - (a.x + a.w)
    };
    assert!((gap_now(&s) - 10.0).abs() < 1e-6, "{}", gap_now(&s));
    s.execute("variable.mode", &json!({"collection": "Theme", "mode": "Dark"})).unwrap();
    assert!((gap_now(&s) - 40.0).abs() < 1e-6, "auto layout follows the variable: {}", gap_now(&s));
    let title = id(&s
        .execute("variable.create", &json!({"collection": "Theme", "name": "deck/title", "type": "string", "value": "Quarterly review"}))
        .unwrap());
    let show = id(&s
        .execute("variable.create", &json!({"collection": "Theme", "name": "show/logo", "type": "boolean", "values": {"Light": true, "Dark": false}}))
        .unwrap());
    let t = ShapeId(id(&s.execute("shape.insert", &json!({"preset": "rect", "rect": [100, 300, 300, 60], "text": "x"})).unwrap()));
    s.execute("variable.bind", &json!({"ids": [t.0], "field": "text", "variable": title})).unwrap();
    s.execute("variable.bind", &json!({"ids": [t.0], "field": "visible", "variable": show})).unwrap();
    let d = s.doc().unwrap();
    assert_eq!(d.shape(t).unwrap().text.as_ref().unwrap().text(), "Quarterly review");
    assert!(d.shape(t).unwrap().hidden, "dark mode hides it");
    s.execute("variable.set", &json!({"variable": title, "value": "Annual review"})).unwrap();
    assert_eq!(s.doc().unwrap().shape(t).unwrap().text.as_ref().unwrap().text(), "Annual review");
    s.execute("variable.unbind", &json!({"ids": [t.0], "field": "text"})).unwrap();
    s.execute("variable.set", &json!({"variable": title, "value": "Again"})).unwrap();
    assert_eq!(s.doc().unwrap().shape(t).unwrap().text.as_ref().unwrap().text(), "Annual review", "detached");
}

#[test]
fn theme_commands_and_variables_stay_in_step() {
    let (mut s, _, surface, _, _) = setup();
    s.execute("design.theme", &json!({"name": "Harbor"})).unwrap();
    // Harbor's lt1 is white: taken into the active (Light) mode.
    let l = s.execute("variable.list", &json!({})).unwrap();
    let v = l["variables"].as_array().unwrap().iter().find(|v| v["id"] == surface).unwrap().clone();
    assert_eq!(v["values"]["Light"], "#FFFFFF");
    assert_eq!(v["values"]["Dark"], "#101820");
    assert_eq!(v["theme"], "lt1");
    s.execute("variable.set", &json!({"variable": surface, "value": "#FAFAF0"})).unwrap();
    assert_eq!(s.doc().unwrap().doc.masters[0].theme.colors.get(SchemeSlot::Lt1), deckcraft_color::Rgba::rgb(0xFA, 0xFA, 0xF0));
}

#[test]
fn hostile_and_wrong_params_are_refused() {
    let (mut s, coll, surface, _, r) = setup();
    for (cmd, p) in [
        ("variable.create", json!({"collection": coll, "name": "x", "type": "colour?", "value": "#000000"})),
        ("variable.create", json!({"collection": coll, "name": "x", "type": "number", "value": "12"})),
        ("variable.create", json!({"collection": coll, "name": "x", "type": "number", "value": null})),
        ("variable.create", json!({"collection": 999, "name": "x", "type": "number", "value": 1})),
        ("variable.create", json!({"collection": coll, "name": "", "type": "number", "value": 1})),
        ("variable.create", json!({"collection": coll, "name": "color/surface", "type": "color", "value": "#000000"})),
        ("variable.create", json!({"collection": coll, "name": "y", "type": "number", "value": 1, "theme": "accent1"})),
        ("variable.create", json!({"collection": coll, "name": "y", "type": "color", "value": "#000000", "theme": "bg1"})),
        ("variable.set", json!({"variable": surface, "mode": "Sepia", "value": "#000000"})),
        ("variable.mode", json!({"collection": coll, "mode": "Sepia"})),
        ("variable.mode", json!({"collection": coll, "mode": "Dark", "slides": [99]})),
        ("variable.mode", json!({"collection": coll, "mode": null})),
        ("variable.bind", json!({"ids": [r.0], "field": "gap", "variable": surface})),
        ("variable.bind", json!({"ids": [r.0], "field": "fontSize", "variable": surface})),
        ("variable.bind", json!({"ids": [r.0], "field": "nope", "variable": surface})),
        ("variable.createCollection", json!({"name": "Theme"})),
        ("variable.createCollection", json!({"name": "Big", "modes": (0..100).map(|i| format!("m{i}")).collect::<Vec<_>>()})),
    ] {
        let before = s.doc().unwrap().doc.clone();
        assert!(s.execute(cmd, &p).is_err(), "{cmd} {p}");
        assert!(Arc::ptr_eq(&before, &s.doc().unwrap().doc), "{cmd} left the deck alone");
    }
    // A number bound to a non-auto-layout shape's gap is refused, not ignored.
    let n = id(&s.execute("variable.create", &json!({"collection": coll, "name": "n", "type": "number", "value": 3})).unwrap());
    assert!(s.execute("variable.bind", &json!({"ids": [r.0], "field": "gap", "variable": n})).is_err());
    s.execute("variable.deleteMode", &json!({"collection": coll, "mode": "Dark"})).unwrap();
    s.execute("variable.deleteCollection", &json!({"collection": coll})).unwrap();
    assert!(s.doc().unwrap().doc.design.is_none(), "nothing left: the deck is as it was");
}

#[test]
fn variables_survive_deckcraft_and_pptx() {
    let (mut s, _, _, accent, r) = setup();
    s.execute("variable.bind", &json!({"ids": [r.0], "field": "fill", "variable": accent})).unwrap();
    let pad = id(&s
        .execute("variable.create", &json!({"collection": "Theme", "name": "w", "type": "number", "values": {"Light": 150, "Dark": 250}}))
        .unwrap());
    s.execute("variable.bind", &json!({"ids": [r.0], "field": "width", "variable": pad})).unwrap();
    s.execute("variable.mode", &json!({"collection": "Theme", "mode": "Dark"})).unwrap();
    let doc = s.doc().unwrap().doc.clone();
    for fmt in ["deckcraft", "pptx"] {
        let bytes = crate::cmd::file::save_bytes(&doc, fmt).unwrap();
        let (back, _) = crate::cmd::file::open_checked(&format!("x.{fmt}"), &bytes).unwrap();
        assert_eq!(back.design, doc.design, "{fmt}: the design system");
        let sh = back.slides.iter().find_map(|sl| sl.shape(r)).unwrap();
        assert!(
            matches!(&sh.fill, Some(deckcraft_model::Fill::Solid { color }) if matches!(color.base, deckcraft_model::ColorBase::Var { id, .. } if id == accent)),
            "{fmt}: {:?}",
            sh.fill
        );
        assert_eq!(sh.bindings, doc.slides.iter().find_map(|sl| sl.shape(r)).unwrap().bindings, "{fmt}");
        assert_eq!(sh.xfrm.unwrap().w, 250.0, "{fmt}");
        assert_eq!(back.masters[0].theme.colors.get(SchemeSlot::Lt1), deckcraft_color::Rgba::rgb(0x10, 0x18, 0x20), "{fmt}: dark theme");
    }
    // PowerPoint sees the dark accent as a plain colour.
    let bytes = crate::cmd::file::save_bytes(&doc, "pptx").unwrap();
    let mut plain = deckcraft_pptx::import(&bytes).unwrap();
    plain.design = None;
    let sh = plain.slides.iter().find_map(|sl| sl.shape(r)).unwrap();
    let deckcraft_model::Fill::Solid { color } = sh.fill.as_ref().unwrap() else { panic!() };
    assert_eq!(color.resolve(&plain.masters[0].theme.colors, None), deckcraft_color::Rgba::rgb(0xFF, 0xB0, 0x00));
}
