//! SlideCraft Phase 4.1 in the GUI: the Variables pane switches modes and shows values.
//! `SLIDECRAFT_UI_SHOTS=<dir>` saves pictures to look at.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use egui_kittest::kittest::Queryable;
use serde_json::json;

use crate::{Services, SlideApp};
use deckcraft_engine::Session;

fn shot(h: &mut egui_kittest::Harness<'static, SlideApp>, name: &str) {
    if let (Ok(dir), Ok(img)) = (std::env::var("SLIDECRAFT_UI_SHOTS"), h.render()) {
        let _ = img.save(std::path::Path::new(&dir).join(name));
    }
}

#[test]
fn the_variables_pane_switches_the_deck_mode() {
    let _gpu = crate::gpu_lock::gpu_lock();
    let mut s = Session::with_new();
    s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
    s.execute("variable.createCollection", &json!({"name": "Brand", "modes": ["Light", "Dark"]})).unwrap();
    s.execute(
        "variable.create",
        &json!({"collection": "Brand", "name": "color/surface", "type": "color", "values": {"Light": "#FFFFFF", "Dark": "#14161C"}, "theme": "lt1"}),
    )
    .unwrap();
    s.execute(
        "variable.create",
        &json!({"collection": "Brand", "name": "color/accent", "type": "color", "values": {"Light": "#2E6FD8", "Dark": "#FFB000"}}),
    )
    .unwrap();
    s.execute("variable.create", &json!({"collection": "Brand", "name": "radius/card", "type": "number", "values": {"Light": 12, "Dark": 24}}))
        .unwrap();
    s.execute("variable.create", &json!({"collection": "Brand", "name": "deck/title", "type": "string", "value": "Quarterly review"})).unwrap();
    let r = s.execute("shape.insert", &json!({"preset": "rect", "rect": [100, 150, 300, 160]})).unwrap();
    s.execute("variable.bind", &json!({"ids": [r["id"]], "field": "fill", "variable": "color/accent"})).unwrap();
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 1000.0)).build_ui_state(
        |ui, app: &mut SlideApp| {
            app.logic(ui.ctx());
            app.ui(ui);
        },
        SlideApp::new(s, Services::default()),
    );
    h.state_mut().run("view.variables", json!({})).unwrap();
    h.run_steps(5);
    shot(&mut h, "variables-light.png");
    assert!(h.query_all_by_label("Add Variable").count() >= 1);
    h.query_all_by_label("Dark").next().unwrap().click();
    h.run_steps(6);
    shot(&mut h, "variables-dark.png");
    let st = h.state().session.active().unwrap();
    let ds = st.doc.design_system().unwrap();
    let c = ds.collections[0].clone();
    assert_eq!(ds.deck_mode(c.id), c.mode_named("Dark").map(|m| m.id), "the button switched the deck's mode");
}

#[test]
fn the_assets_pane_inserts_and_edits_instances() {
    let _gpu = crate::gpu_lock::gpu_lock();
    let mut s = Session::with_new();
    s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
    let v = s.execute("shape.insert", &json!({"preset": "roundRect", "rect": [60, 60, 200, 70], "text": "42%"})).unwrap()["id"].clone();
    s.execute("shape.rename", &json!({"id": v, "name": "Value"})).unwrap();
    let l = s.execute("shape.insert", &json!({"preset": "rect", "rect": [60, 140, 200, 36], "text": "growth"})).unwrap()["id"].clone();
    s.execute("shape.rename", &json!({"id": l, "name": "Label"})).unwrap();
    let c = s.execute("component.create", &json!({"ids": [v, l], "name": "Stat card"})).unwrap()["id"].clone();
    s.execute("component.addProperty", &json!({"component": c, "name": "Value", "type": "text", "layers": ["Value"]})).unwrap();
    s.execute("designStyle.create", &json!({"type": "paint", "name": "Brand", "color": "#1B7FA6"})).unwrap();
    s.execute("designStyle.create", &json!({"type": "grid", "name": "12 col", "columns": 12, "gutter": 16, "margin": 48})).unwrap();
    s.execute("designStyle.apply", &json!({"style": "12 col", "slides": "all"})).unwrap();
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 1000.0)).build_ui_state(
        |ui, app: &mut SlideApp| {
            app.logic(ui.ctx());
            app.ui(ui);
        },
        SlideApp::new(s, Services::default()),
    );
    h.state_mut().run("view.assets", json!({})).unwrap();
    h.run_steps(4);
    h.get_by_label("Place").click();
    h.run_steps(6);
    shot(&mut h, "assets-instance.png");
    let st = h.state().session.active().unwrap();
    let inst = st.selection.shapes.first().and_then(|i| st.shape(*i)).cloned().unwrap();
    assert!(inst.instance.is_some(), "Insert placed an instance and selected it");
    assert!(h.query_all_by_label("Detach").count() >= 1, "the instance's controls show");
}
