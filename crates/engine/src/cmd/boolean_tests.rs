#![allow(clippy::unwrap_used, clippy::expect_used)]

use deckcraft_model::{Geom, ShapeId, ShapeKind};
use serde_json::json;

use crate::Session;

fn id(v: &serde_json::Value) -> ShapeId {
    ShapeId(v["id"].as_u64().unwrap() as u32)
}

fn color_at(s: &mut Session, x: f64, y: f64) -> String {
    s.execute("slide.colorAt", &json!({"x": x, "y": y})).unwrap()["color"].as_str().unwrap().to_string()
}

/// Two overlapping red squares: (session, a at 100..200, b at 150..250).
fn two() -> (Session, ShapeId, ShapeId) {
    let mut s = Session::with_new();
    s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
    let a = id(&s.execute("shape.insert", &json!({"preset": "rect", "rect": [100, 100, 100, 100]})).unwrap());
    s.execute("shape.fill", &json!({"color": "#FF0000"})).unwrap();
    let b = id(&s.execute("shape.insert", &json!({"preset": "rect", "rect": [150, 150, 100, 100]})).unwrap());
    s.execute("shape.fill", &json!({"color": "#0000FF"})).unwrap();
    (s, a, b)
}

#[test]
fn subtract_cuts_the_second_out_of_the_first_and_stays_editable() {
    let (mut s, a, b) = two();
    let g = id(&s.execute("boolean.create", &json!({"op": "subtract", "ids": [a.0, b.0]})).unwrap());
    // Drawn as one shape with the first operand's look: red where only a was, nothing in the cut.
    assert_eq!(color_at(&mut s, 120.0, 120.0), "FF0000");
    let bg = color_at(&mut s, 600.0, 450.0);
    assert_eq!(color_at(&mut s, 180.0, 180.0), bg);
    assert_eq!(color_at(&mut s, 230.0, 230.0), bg, "b itself is not drawn");
    // Edit an operand in place: the cut follows.
    s.execute("shape.move", &json!({"ids": [b.0], "dx": 200})).unwrap();
    assert_eq!(color_at(&mut s, 180.0, 180.0), "FF0000");
    let st = s.doc().unwrap();
    let ShapeKind::Group { children, .. } = &st.shape(g).unwrap().kind else { panic!() };
    assert_eq!(children.len(), 2);
    // One undo step puts the cut back.
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(color_at(&mut s, 180.0, 180.0), bg);
}

#[test]
fn the_operation_can_change_and_be_flattened() {
    let (mut s, a, b) = two();
    let g = id(&s.execute("boolean.create", &json!({"op": "union", "ids": [a.0, b.0]})).unwrap());
    assert_eq!(color_at(&mut s, 230.0, 230.0), "FF0000");
    s.execute("boolean.setOp", &json!({"op": "intersect"})).unwrap();
    let bg = color_at(&mut s, 600.0, 450.0);
    assert_eq!(color_at(&mut s, 120.0, 120.0), bg);
    assert_eq!(color_at(&mut s, 180.0, 180.0), "FF0000");
    s.execute("boolean.setOp", &json!({"op": "exclude"})).unwrap();
    assert_eq!(color_at(&mut s, 180.0, 180.0), bg);
    assert_eq!(color_at(&mut s, 120.0, 120.0), "FF0000");
    s.execute("boolean.flatten", &json!({})).unwrap();
    let st = s.doc().unwrap();
    let sh = st.shape(g).unwrap();
    assert!(matches!(sh.kind, ShapeKind::Shape) && sh.boolean.is_none());
    assert!(matches!(sh.geom, Geom::Custom { .. }));
    assert!(st.shape(a).is_none());
    assert_eq!(color_at(&mut s, 120.0, 120.0), "FF0000");
}

#[test]
fn boolean_commands_refuse_hostile_input() {
    let (mut s, a, b) = two();
    for p in [
        json!({"ids": [a.0, b.0]}),
        json!({"op": "melt", "ids": [a.0, b.0]}),
        json!({"op": "union", "ids": [a.0]}),
        json!({"op": "union", "ids": [a.0, 99999]}),
    ] {
        assert!(s.execute("boolean.create", &p).is_err(), "{p}");
    }
    assert!(s.execute("boolean.setOp", &json!({"op": "union", "ids": [a.0]})).is_err());
    assert!(s.execute("boolean.flatten", &json!({"ids": [a.0]})).is_err());
    // A boolean shape survives saving and opening.
    let g = id(&s.execute("boolean.create", &json!({"op": "union", "ids": [a.0, b.0]})).unwrap());
    let bytes = deckcraft_format::save(&s.doc().unwrap().doc).unwrap();
    let (p, _) = crate::cmd::file::open_checked("t.deckcraft", &bytes).unwrap();
    let sh = deckcraft_model::find_shape(&p.slides.last().unwrap().shapes, g).unwrap();
    assert_eq!(sh.boolean, Some(deckcraft_model::canvas::BoolOp::Union));
}
