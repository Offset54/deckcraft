//! Animations › Effect Options › Reverse Path Direction.

use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use serde_json::json;

use super::harness;
use crate::{Services, SlideApp};
use deckcraft_engine::Session;

/// A rectangle with `effect` on it, selected, on the Animations tab.
fn animated(effect: serde_json::Value) -> egui_kittest::Harness<'static, SlideApp> {
    let mut s = Session::with_new();
    s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
    let id = s.execute("shape.insert", &json!({"preset": "rect", "rect": [100.0, 100.0, 80.0, 60.0]})).unwrap()["id"].as_u64().unwrap();
    s.execute("selection.select", &json!({"ids": [id]})).unwrap();
    s.execute("animation.add", &effect).unwrap();
    let mut app = SlideApp::new(s, Services::default());
    app.ui.tab = "Animations".into();
    let mut h = harness(app);
    h.run_steps(3);
    h
}

fn path(h: &egui_kittest::Harness<'_, SlideApp>) -> Option<String> {
    h.state().session.active().unwrap().current_slide().unwrap().animations.first().and_then(|a| a.path.clone())
}

#[test]
fn effect_options_reverses_a_motion_path() {
    let mut h = animated(json!({"effect": "lines", "class": "path", "path": "M 0 0 L 0.25 0 E"}));
    h.get_by_role_and_label(Role::Button, "Effect Options").click();
    h.run_steps(3);
    h.get_by_label("Reverse Path Direction").click();
    h.run_steps(3);
    assert_eq!(path(&h).as_deref(), Some("M 0.25 0 L 0 0 E"));
    assert_eq!(h.state().session.active().unwrap().history.undo.last().unwrap().label, "Reverse Path Direction");
}

#[test]
fn other_effects_are_not_offered_a_path_to_reverse() {
    let mut h = animated(json!({"effect": "fly"}));
    h.get_by_role_and_label(Role::Button, "Effect Options").click();
    h.run_steps(3);
    assert!(h.query_by_label("Reverse Path Direction").is_none());
}
