//! Paint, effect and grid styles, and text styles bound to variables (P4.2).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use deckcraft_model::ShapeId;
use serde_json::{Value, json};

use crate::Session;

fn id(v: &Value) -> u32 {
    v["id"].as_u64().unwrap() as u32
}

fn color_at(s: &mut Session, x: f64, y: f64) -> String {
    s.execute("slide.colorAt", &json!({"x": x, "y": y})).unwrap()["color"].as_str().unwrap().to_string()
}

fn setup() -> (Session, ShapeId, ShapeId) {
    let mut s = Session::with_new();
    s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
    let a = ShapeId(id(&s.execute("shape.insert", &json!({"preset": "rect", "rect": [100, 100, 100, 100]})).unwrap()));
    let b = ShapeId(id(&s.execute("shape.insert", &json!({"preset": "rect", "rect": [300, 100, 100, 100]})).unwrap()));
    (s, a, b)
}

#[test]
fn a_paint_style_restyles_every_shape_that_uses_it() {
    let (mut s, a, b) = setup();
    let st = id(&s.execute("designStyle.create", &json!({"type": "paint", "name": "Brand/Primary", "color": "#1B7FA6"})).unwrap());
    s.execute("designStyle.apply", &json!({"ids": [a.0, b.0], "style": st})).unwrap();
    assert_eq!(color_at(&mut s, 150.0, 150.0), "1B7FA6");
    assert_eq!(color_at(&mut s, 350.0, 150.0), "1B7FA6");
    let steps = s.doc().unwrap().history.undo.len();
    s.execute("designStyle.update", &json!({"style": "Brand/Primary", "color": "#E4572E"})).unwrap();
    assert_eq!(s.doc().unwrap().history.undo.len(), steps + 1);
    assert_eq!(color_at(&mut s, 150.0, 150.0), "E4572E");
    assert_eq!(color_at(&mut s, 350.0, 150.0), "E4572E");
    // Shape Fill on one of them detaches it; the other keeps following.
    s.execute("shape.fill", &json!({"ids": [a.0], "color": "#00AA00"})).unwrap();
    assert_eq!(s.doc().unwrap().shape(a).unwrap().styles, None);
    s.execute("designStyle.update", &json!({"style": st, "color": "#222222"})).unwrap();
    assert_eq!(color_at(&mut s, 150.0, 150.0), "00AA00");
    assert_eq!(color_at(&mut s, 350.0, 150.0), "222222");
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(color_at(&mut s, 350.0, 150.0), "E4572E");
    // Deleting the style keeps the look.
    s.execute("designStyle.delete", &json!({"style": st})).unwrap();
    assert_eq!(s.doc().unwrap().shape(b).unwrap().styles, None);
    assert_eq!(color_at(&mut s, 350.0, 150.0), "E4572E");
}

#[test]
fn a_paint_style_on_a_variable_follows_the_mode() {
    let (mut s, a, _) = setup();
    s.execute("variable.createCollection", &json!({"name": "Theme", "modes": ["Light", "Dark"]})).unwrap();
    let v = id(&s
        .execute(
            "variable.create",
            &json!({"collection": "Theme", "name": "brand", "type": "color", "values": {"Light": "#2E6FD8", "Dark": "#FFB000"}}),
        )
        .unwrap());
    let st = id(&s.execute("designStyle.create", &json!({"type": "paint", "name": "Brand", "color": {"variable": v}})).unwrap());
    s.execute("designStyle.apply", &json!({"ids": [a.0], "style": st, "as": "line"})).unwrap();
    s.execute("designStyle.apply", &json!({"ids": [a.0], "style": st})).unwrap();
    assert_eq!(color_at(&mut s, 150.0, 150.0), "2E6FD8");
    s.execute("variable.mode", &json!({"collection": "Theme", "mode": "Dark"})).unwrap();
    assert_eq!(color_at(&mut s, 150.0, 150.0), "FFB000");
    let r = s.doc().unwrap().shape(a).unwrap().styles.unwrap();
    assert_eq!((r.fill, r.line), (Some(st), Some(st)));
}

#[test]
fn effect_and_grid_styles() {
    let (mut s, a, b) = setup();
    s.execute("shape.effects", &json!({"ids": [a.0], "shadow": "outer"})).ok();
    s.execute("format.blur", &json!({"ids": [a.0], "layer": 4})).unwrap();
    let fx = id(&s.execute("designStyle.create", &json!({"type": "effect", "name": "Soft", "from": a.0})).unwrap());
    s.execute("designStyle.apply", &json!({"ids": [b.0], "style": "Soft"})).unwrap();
    let d = s.doc().unwrap();
    assert_eq!(d.shape(b).unwrap().effects, d.shape(a).unwrap().effects);
    assert_eq!(d.shape(b).unwrap().styles.unwrap().effect, Some(fx));
    let g = id(&s.execute("designStyle.create", &json!({"type": "grid", "name": "12 columns", "columns": 12, "gutter": 16, "margin": 48})).unwrap());
    s.execute("designStyle.apply", &json!({"style": g, "slides": "all"})).unwrap();
    assert!(s.doc().unwrap().doc.slides.iter().all(|sl| sl.grid_style == Some(g)));
    // A grid style on a shape that isn't a frame is refused.
    assert!(s.execute("designStyle.apply", &json!({"ids": [a.0], "style": g})).is_err());
    let f = s.execute("frame.create", &json!({"ids": [a.0, b.0]})).unwrap();
    s.execute("designStyle.apply", &json!({"ids": [f["id"]], "style": g})).unwrap();
    let l = s.execute("designStyle.list", &json!({})).unwrap();
    let grid = l["styles"].as_array().unwrap().iter().find(|x| x["id"] == g).unwrap().clone();
    assert_eq!(grid["users"], 3, "two slides and a frame");
    assert_eq!(grid["grid"]["columns"], 12);
}

#[test]
fn text_styles_follow_variables() {
    let (mut s, _, _) = setup();
    s.execute("variable.createCollection", &json!({"name": "Type", "modes": ["Compact", "Roomy"]})).unwrap();
    let size = id(&s
        .execute("variable.create", &json!({"collection": "Type", "name": "size/title", "type": "number", "values": {"Compact": 36, "Roomy": 54}}))
        .unwrap());
    let font =
        id(&s.execute("variable.create", &json!({"collection": "Type", "name": "font/display", "type": "string", "value": "Merriweather"})).unwrap());
    let ink = id(&s
        .execute(
            "variable.create",
            &json!({"collection": "Type", "name": "ink", "type": "color", "values": {"Compact": "#111111", "Roomy": "#333333"}}),
        )
        .unwrap());
    s.execute("style.bind", &json!({"id": "title", "field": "fontSize", "variable": size})).unwrap();
    s.execute("style.bind", &json!({"id": "title", "field": "fontFamily", "variable": font})).unwrap();
    s.execute("style.bind", &json!({"id": "title", "field": "color", "variable": ink})).unwrap();
    let title = |s: &Session| s.doc().unwrap().doc.text_style("title").unwrap();
    assert_eq!(title(&s).run.size, Some(36.0));
    assert_eq!(title(&s).run.font.as_deref(), Some("Merriweather"));
    s.execute("variable.mode", &json!({"collection": "Type", "mode": "Roomy"})).unwrap();
    assert_eq!(title(&s).run.size, Some(54.0), "the style follows the mode");
    assert!(s.execute("style.bind", &json!({"id": "title", "field": "fontSize", "variable": font})).is_err(), "wrong type");
    s.execute("style.bind", &json!({"id": "title", "field": "fontSize", "variable": null})).unwrap();
    s.execute("variable.mode", &json!({"collection": "Type", "mode": "Compact"})).unwrap();
    assert_eq!(title(&s).run.size, Some(54.0), "detached");
}

#[test]
fn hostile_style_params() {
    let (mut s, a, _) = setup();
    for p in [
        json!({"type": "paint", "name": "x"}),
        json!({"type": "nope", "name": "x", "color": "#000000"}),
        json!({"type": "paint", "name": "", "color": "#000000"}),
        json!({"type": "effect", "name": "x", "color": "#000000"}),
        json!({"type": "effect", "name": "x", "from": a.0}),
        json!({"type": "paint", "name": "x", "from": 99999}),
    ] {
        let before = s.doc().unwrap().doc.clone();
        assert!(s.execute("designStyle.create", &p).is_err(), "{p}");
        assert!(Arc::ptr_eq(&before, &s.doc().unwrap().doc));
    }
    let g = id(&s.execute("designStyle.create", &json!({"type": "grid", "name": "g", "columns": 1_000_000, "gutter": -4, "margin": 1e300})).unwrap());
    let l = s.execute("designStyle.list", &json!({})).unwrap();
    let grid = &l["styles"][0]["grid"];
    assert_eq!((grid["columns"].as_u64(), grid["gutter"].as_f64(), grid["margin"].as_f64()), (Some(48), Some(0.0), Some(10_000.0)));
    assert!(s.execute("designStyle.apply", &json!({"style": g, "slides": [99]})).is_err());
    assert!(s.execute("designStyle.apply", &json!({"ids": [a.0], "style": 424242})).is_err());
}

#[test]
fn styles_survive_deckcraft_and_pptx() {
    let (mut s, a, _) = setup();
    let st = id(&s.execute("designStyle.create", &json!({"type": "paint", "name": "P", "color": "#1B7FA6"})).unwrap());
    s.execute("designStyle.apply", &json!({"ids": [a.0], "style": st})).unwrap();
    let g = id(&s.execute("designStyle.create", &json!({"type": "grid", "name": "g"})).unwrap());
    let cur = s.doc().unwrap().selection.slide;
    s.execute("designStyle.apply", &json!({"style": g, "slides": [cur]})).unwrap();
    let doc = s.doc().unwrap().doc.clone();
    for fmt in ["deckcraft", "pptx"] {
        let bytes = crate::cmd::file::save_bytes(&doc, fmt).unwrap();
        let (back, _) = crate::cmd::file::open_checked(&format!("x.{fmt}"), &bytes).unwrap();
        assert_eq!(back.design, doc.design, "{fmt}");
        let sh = back.slides.iter().find_map(|sl| sl.shape(a)).unwrap();
        assert_eq!(sh.styles.and_then(|r| r.fill), Some(st), "{fmt}");
        assert_eq!(back.slides[cur].grid_style, Some(g), "{fmt}");
    }
}
