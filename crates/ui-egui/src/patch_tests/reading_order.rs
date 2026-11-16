//! Review › Reading Order pane.

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use serde_json::json;

use super::harness;
use crate::{Services, SlideApp};
use deckcraft_engine::Session;

/// Three named rectangles on a blank slide, with the pane open.
fn pane() -> (Harness<'static, SlideApp>, [u64; 3]) {
    let mut s = Session::with_new();
    s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
    let mut ids = [0u64; 3];
    for (k, name) in ["Alpha", "Beta", "Gamma"].iter().enumerate() {
        let r = s.execute("shape.insert", &json!({"preset": "rect", "rect": [100.0 + 150.0 * k as f64, 100.0, 80.0, 60.0]})).unwrap();
        ids[k] = r["id"].as_u64().unwrap();
        s.execute("shape.rename", &json!({"id": ids[k], "name": name})).unwrap();
    }
    let mut app = SlideApp::new(s, Services::default());
    app.ui.tab = "Review".into();
    let mut h = harness(app);
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "Reading Order").click();
    h.run_steps(3);
    assert_eq!(h.state().ui.pane.as_deref(), Some("reading"));
    (h, ids)
}

/// The slide's objects, back to front: the order they are read in.
fn order(h: &Harness<'static, SlideApp>) -> Vec<u64> {
    h.state().session.active().unwrap().shapes().iter().map(|s| u64::from(s.id.0)).collect()
}

/// The row whose spoken name starts with `prefix`.
fn row<'a>(h: &'a Harness<'static, SlideApp>, prefix: &str) -> egui_kittest::Node<'a> {
    h.get_all_by_role(Role::Button)
        .find(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with(prefix)))
        .unwrap_or_else(|| panic!("a row {prefix}"))
}

#[test]
fn the_pane_lists_objects_in_reading_order_and_selects_on_click() {
    let (mut h, ids) = pane();
    // First drawn is read first, at the top.
    let (a, b, c) = (row(&h, "1. Alpha").rect(), row(&h, "2. Beta").rect(), row(&h, "3. Gamma").rect());
    assert!(a.top() < b.top() && b.top() < c.top());
    row(&h, "2. Beta").click();
    h.run_steps(3);
    let sel: Vec<u64> = h.state().session.active().unwrap().selection.shapes.iter().map(|i| u64::from(i.0)).collect();
    assert_eq!(sel, vec![ids[1]]);
}

#[test]
fn move_up_and_down_change_the_reading_order() {
    let (mut h, ids) = pane();
    row(&h, "2. Beta").click();
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "Move Up").click();
    h.run_steps(3);
    assert_eq!(order(&h), vec![ids[1], ids[0], ids[2]]);
    assert!(h.get_all_by_role(Role::Button).any(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with("1. Beta"))));
    for _ in 0..2 {
        h.get_by_role_and_label(Role::Button, "Move Down").click();
        h.run_steps(3);
    }
    assert_eq!(order(&h), vec![ids[0], ids[2], ids[1]]);
    // One undo step per move.
    assert_eq!(h.state().session.active().unwrap().history.undo.last().unwrap().label, "Reorder Objects");
}

#[test]
fn unticking_an_object_marks_it_as_decorative() {
    let (mut h, ids) = pane();
    let deco = |h: &Harness<'static, SlideApp>| {
        let st = h.state().session.active().unwrap();
        st.shape(deckcraft_model::ShapeId(ids[2] as u32)).unwrap().decorative
    };
    assert!(!deco(&h));
    h.get_by_role_and_label(Role::CheckBox, "Read Gamma").click();
    h.run_steps(3);
    assert!(deco(&h));
    assert!(row(&h, "3. Gamma").accesskit_node().label().unwrap().contains("Decorative"));
    h.get_by_role_and_label(Role::CheckBox, "Read Gamma").click();
    h.run_steps(3);
    assert!(!deco(&h));
}
