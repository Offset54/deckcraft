//! Components, variants, properties, instances and overrides (P4.3).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use deckcraft_model::{ShapeId, TextBody};
use serde_json::{Value, json};

use crate::Session;

fn id(v: &Value) -> u32 {
    v["id"].as_u64().unwrap() as u32
}

fn texts(s: &Session, inst: ShapeId) -> Vec<String> {
    let d = s.doc().unwrap();
    d.shape(inst).unwrap().children().iter().map(|c| c.text.as_ref().map(TextBody::text).unwrap_or_default()).collect()
}

/// A stat card (a frame with a value and a label) made a component: (session, component, main).
fn stat_card() -> (Session, u32, ShapeId) {
    let mut s = Session::with_new();
    s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
    let v = id(&s.execute("shape.insert", &json!({"preset": "rect", "rect": [40, 40, 160, 60], "text": "42"})).unwrap());
    s.execute("shape.rename", &json!({"id": v, "name": "Value"})).unwrap();
    let l = id(&s.execute("shape.insert", &json!({"preset": "rect", "rect": [40, 110, 160, 30], "text": "users"})).unwrap());
    s.execute("shape.rename", &json!({"id": l, "name": "Label"})).unwrap();
    let steps = s.doc().unwrap().history.undo.len();
    let r = s.execute("component.create", &json!({"ids": [v, l], "name": "Stat", "set": "Stat", "variant": {"Size": "Small"}})).unwrap();
    assert_eq!(s.doc().unwrap().history.undo.len(), steps + 1, "framing and making the component are one undo step");
    let main = ShapeId(r["main"].as_u64().unwrap() as u32);
    (s, id(&r), main)
}

fn layer_named(s: &Session, frame: ShapeId, name: &str) -> ShapeId {
    s.doc().unwrap().shape(frame).unwrap().children().iter().find(|c| c.name == name).map(|c| c.id).unwrap_or_else(|| panic!("no layer {name}"))
}

#[test]
fn editing_the_main_updates_every_instance_and_keeps_overrides() {
    let (mut s, comp, main) = stat_card();
    let a = ShapeId(id(&s.execute("component.insert", &json!({"component": "Stat", "at": [400, 40]})).unwrap()));
    let b = ShapeId(id(&s.execute("component.insert", &json!({"component": comp, "at": [400, 250]})).unwrap()));
    assert_eq!(texts(&s, a), vec!["42", "users"]);
    // Typing into an instance is an override.
    let label_b = layer_named(&s, b, "Label");
    s.execute("text.set", &json!({"id": label_b.0, "text": "customers"})).unwrap();
    assert_eq!(s.doc().unwrap().shape(b).unwrap().instance.as_ref().unwrap().overrides.len(), 1);
    // Edit the main: both instances follow, B keeps its label.
    let value_main = layer_named(&s, main, "Value");
    s.execute("text.set", &json!({"id": value_main.0, "text": "1,204"})).unwrap();
    s.execute("shape.fill", &json!({"ids": [value_main.0], "color": "#E4572E"})).unwrap();
    assert_eq!(texts(&s, a), vec!["1,204", "users"]);
    assert_eq!(texts(&s, b), vec!["1,204", "customers"]);
    let va = layer_named(&s, a, "Value");
    let fa = s.doc().unwrap().shape(va).unwrap().fill.clone();
    assert_eq!(fa, s.doc().unwrap().shape(value_main).unwrap().fill.clone());
    // Instance layers keep their ids across updates (selections and links stay valid).
    assert_eq!(layer_named(&s, b, "Label"), label_b);
    // Undo the main's fill: the instances go back too.
    s.execute("edit.undo", &json!({})).unwrap();
    assert_ne!(s.doc().unwrap().shape(layer_named(&s, a, "Value")).unwrap().fill, fa);
    // The instance moves as a whole and keeps its content.
    s.execute("shape.move", &json!({"ids": [a.0], "dx": 30})).unwrap();
    assert_eq!(texts(&s, a), vec!["1,204", "users"]);
    let l = s.execute("component.list", &json!({})).unwrap();
    assert_eq!(l["components"][0]["instances"], 2);
    assert_eq!(l["components"][0]["main"]["id"], main.0);
}

#[test]
fn properties_variants_swap_reset_and_detach() {
    let (mut s, comp, main) = stat_card();
    s.execute("component.addProperty", &json!({"component": comp, "name": "Value", "type": "text", "layers": ["Value"]})).unwrap();
    s.execute("component.addProperty", &json!({"component": comp, "name": "Show label", "type": "boolean", "layers": ["Label"]})).unwrap();
    // A large variant: duplicate the main's content into a second component of the set.
    let v2 = id(&s.execute("shape.insert", &json!({"preset": "rect", "rect": [40, 300, 240, 90], "text": "42"})).unwrap());
    s.execute("shape.rename", &json!({"id": v2, "name": "Value"})).unwrap();
    let l2 = id(&s.execute("shape.insert", &json!({"preset": "rect", "rect": [40, 400, 240, 40], "text": "users"})).unwrap());
    s.execute("shape.rename", &json!({"id": l2, "name": "Label"})).unwrap();
    let large =
        id(&s.execute("component.create", &json!({"ids": [v2, l2], "name": "Stat Large", "set": "Stat", "variant": {"Size": "Large"}})).unwrap());
    s.execute("component.addProperty", &json!({"component": large, "name": "Value", "type": "text", "layers": ["Value"]})).unwrap();
    let i = ShapeId(id(&s.execute("component.insert", &json!({"component": comp, "at": [500, 40], "props": {"Value": "98%"}})).unwrap()));
    assert_eq!(texts(&s, i), vec!["98%", "users"]);
    s.execute("instance.set", &json!({"ids": [i.0], "props": {"Show label": false}})).unwrap();
    assert!(s.doc().unwrap().shape(layer_named(&s, i, "Label")).unwrap().hidden);
    s.execute("instance.override", &json!({"ids": [i.0], "layer": "Label", "text": "uptime", "hidden": false})).unwrap();
    assert_eq!(texts(&s, i), vec!["98%", "uptime"]);
    // Switch the variant: the value and the label's override carry over by name.
    s.execute("instance.set", &json!({"ids": [i.0], "variant": {"Size": "Large"}})).unwrap();
    let d = s.doc().unwrap();
    assert_eq!(d.shape(i).unwrap().instance.as_ref().unwrap().component, large);
    assert_eq!(texts(&s, i), vec!["98%", "uptime"]);
    assert!(s.execute("instance.set", &json!({"ids": [i.0], "variant": {"Size": "Huge"}})).is_err());
    s.execute("instance.swap", &json!({"ids": [i.0], "component": comp})).unwrap();
    s.execute("instance.reset", &json!({"ids": [i.0]})).unwrap();
    assert_eq!(texts(&s, i), vec!["42", "users"]);
    s.execute("instance.detach", &json!({"ids": [i.0]})).unwrap();
    s.execute("text.set", &json!({"id": layer_named(&s, main, "Value").0, "text": "0"})).unwrap();
    assert_eq!(texts(&s, i), vec!["42", "users"], "a detached instance no longer follows");
    // Deleting a component detaches its instances and keeps the main as a frame.
    let j = ShapeId(id(&s.execute("component.insert", &json!({"component": comp})).unwrap()));
    let r = s.execute("component.delete", &json!({"component": comp})).unwrap();
    assert_eq!(r["detached"], 1);
    assert!(s.doc().unwrap().shape(j).unwrap().instance.is_none());
    assert!(s.doc().unwrap().shape(main).unwrap().main_of.is_none());
}

#[test]
fn a_main_is_placed_when_none_is_on_a_slide() {
    let (mut s, comp, main) = stat_card();
    s.execute("edit.delete", &json!({"ids": [main.0]})).unwrap();
    let i = ShapeId(id(&s.execute("component.insert", &json!({"component": comp, "at": [400, 40]})).unwrap()));
    s.execute("edit.select", &json!({"ids": [i.0]})).unwrap();
    let r = s.execute("component.editMain", &json!({})).unwrap();
    let m = ShapeId(r["main"].as_u64().unwrap() as u32);
    assert_eq!(s.doc().unwrap().shape(m).unwrap().main_of, Some(comp));
    s.execute("text.set", &json!({"id": layer_named(&s, m, "Label").0, "text": "members"})).unwrap();
    assert_eq!(texts(&s, i), vec!["42", "members"]);
}

#[test]
fn hostile_component_params() {
    let (mut s, comp, main) = stat_card();
    let i = id(&s.execute("component.insert", &json!({"component": comp})).unwrap());
    for (cmd, p) in [
        ("component.insert", json!({"component": 999_999})),
        ("component.insert", json!({"component": comp, "rect": [0, 0, 0, 10]})),
        ("component.insert", json!({"component": comp, "props": {"Nope": "x"}})),
        ("component.addProperty", json!({"component": comp, "name": "P", "type": "text", "layers": ["Nope"]})),
        ("component.addProperty", json!({"component": comp, "name": "P", "type": "swap", "layers": ["Value"]})),
        ("component.addProperty", json!({"component": comp, "name": "P", "type": "nope", "layers": ["Value"]})),
        ("component.create", json!({"ids": [i]})),
        ("component.create", json!({"ids": [main.0]})),
        ("instance.set", json!({"ids": [main.0], "props": {}})),
        ("instance.override", json!({"ids": [i], "layer": "Nope", "text": "x"})),
        ("instance.override", json!({"ids": [i], "layer": "Value", "fill": "not a colour"})),
        ("component.update", json!({"component": comp, "variant": ["x"]})),
    ] {
        let before = s.doc().unwrap().doc.clone();
        assert!(s.execute(cmd, &p).is_err(), "{cmd} {p}");
        assert!(Arc::ptr_eq(&before, &s.doc().unwrap().doc), "{cmd} left the deck alone");
    }
}

#[test]
fn components_survive_deckcraft_and_pptx() {
    let (mut s, comp, main) = stat_card();
    s.execute("component.addProperty", &json!({"component": comp, "name": "Value", "type": "text", "layers": ["Value"]})).unwrap();
    let i = ShapeId(id(&s.execute("component.insert", &json!({"component": comp, "at": [500, 40], "props": {"Value": "7"}})).unwrap()));
    s.execute("instance.override", &json!({"ids": [i.0], "layer": "Label", "text": "days"})).unwrap();
    let doc = s.doc().unwrap().doc.clone();
    for fmt in ["deckcraft", "pptx"] {
        let bytes = crate::cmd::file::save_bytes(&doc, fmt).unwrap();
        let (back, _) = crate::cmd::file::open_checked(&format!("x.{fmt}"), &bytes).unwrap();
        assert_eq!(back.design.as_ref().map(|d| d.components.len()), Some(1), "{fmt}");
        let sh = back.slides.iter().find_map(|sl| sl.shape(i)).unwrap();
        assert_eq!(sh.instance, doc.slides.iter().find_map(|sl| sl.shape(i)).unwrap().instance, "{fmt}");
        let t: Vec<String> = sh.children().iter().map(|c| c.text.as_ref().map(TextBody::text).unwrap_or_default()).collect();
        assert_eq!(t, vec!["7", "days"], "{fmt}");
        assert!(back.slides.iter().find_map(|sl| sl.shape(main)).unwrap().main_of.is_some(), "{fmt}");
    }
}
