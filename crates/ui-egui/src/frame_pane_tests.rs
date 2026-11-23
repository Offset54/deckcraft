//! SlideCraft Phase 3 in the GUI: the Format pane's Frame & Layout section, and distances shown
//! while Alt is held. `SLIDECRAFT_UI_SHOTS=<dir>` saves pictures to look at.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use egui_kittest::kittest::Queryable;
use serde_json::json;

use crate::{Services, SlideApp};
use deckcraft_engine::Session;

fn harness() -> (egui_kittest::Harness<'static, SlideApp>, Vec<u64>) {
    let mut s = Session::with_new();
    s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
    let mut ids = vec![];
    for (x, color) in [(100, "#E4572E"), (300, "#17BEBB"), (500, "#FFC914")] {
        let r = s.execute("shape.insert", &json!({"preset": "roundRect", "rect": [x, 150, 140, 90]})).unwrap();
        s.execute("shape.fill", &json!({"color": color})).unwrap();
        ids.push(r["id"].as_u64().unwrap());
    }
    let h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 1300.0)).build_ui_state(
        |ui, app: &mut SlideApp| {
            app.logic(ui.ctx());
            app.ui(ui);
        },
        SlideApp::new(s, Services::default()),
    );
    (h, ids)
}

fn shot(h: &mut egui_kittest::Harness<'static, SlideApp>, name: &str) {
    if let (Ok(dir), Ok(img)) = (std::env::var("SLIDECRAFT_UI_SHOTS"), h.render()) {
        let _ = img.save(std::path::Path::new(&dir).join(name));
    }
}

#[test]
fn the_format_pane_lays_out_a_frame() {
    let _gpu = crate::gpu_lock::gpu_lock();
    let (mut h, ids) = harness();
    let f = h.state_mut().run("layout.set", json!({"ids": ids, "gap": 16, "padding": 24, "width": "hug", "height": "hug"})).unwrap();
    let f = f["ids"][0].as_u64().unwrap();
    h.state_mut().run("shape.fill", json!({"ids": [f], "color": "#F2F2F2"})).unwrap();
    h.state_mut().run("frame.set", json!({"radius": 16})).unwrap();
    h.state_mut().run("view.pane", json!({"pane": "format"})).unwrap();
    h.state_mut().ui.format_tab = "size".into();
    h.run_steps(4);
    shot(&mut h, "frame-pane.png");
    h.state_mut().run("edit.deselect", json!({})).unwrap();
    h.run_steps(3);
    shot(&mut h, "frame-deselected.png");
    h.state_mut().run("edit.select", json!({"ids": [f]})).unwrap();
    h.run_steps(3);
    // The section's controls are there and run commands.
    assert!(h.query_all_by_label("Align across").count() >= 1);
    h.get_by_label("Remove Auto Layout").click();
    h.run_steps(2);
    let st = h.state().session.active().unwrap();
    let frame = st.shape(deckcraft_model::ShapeId(f as u32)).unwrap();
    assert!(frame.auto_layout.is_none() && frame.frame.is_some());
    h.get_by_label("Add Auto Layout").click();
    h.run_steps(2);
    assert!(h.state().session.active().unwrap().shape(deckcraft_model::ShapeId(f as u32)).unwrap().auto_layout.is_some());
}

#[test]
fn a_member_shows_its_layout_and_alt_shows_distances() {
    let _gpu = crate::gpu_lock::gpu_lock();
    let (mut h, ids) = harness();
    h.state_mut().run("layout.set", json!({"ids": ids, "gap": 16, "padding": 24})).unwrap();
    h.state_mut().run("edit.selectChildren", json!({})).unwrap();
    h.state_mut().run("edit.select", json!({"ids": [ids[1]]})).unwrap();
    h.state_mut().run("view.pane", json!({"pane": "format"})).unwrap();
    h.state_mut().ui.format_tab = "size".into();
    h.run_steps(3);
    h.get_by_label("Ignore auto layout");
    h.get_by_label("Select Parent");
    // Alt over the third member: the distance between them.
    let ev = deckcraft_engine::tools::PointerEvent {
        kind: deckcraft_engine::tools::PointerKind::Move,
        // The third member, after the layout: 24 padding, 140 wide, 16 apart.
        x: 100.0 + 24.0 + 2.0 * (140.0 + 16.0) + 70.0,
        y: 200.0,
        mods: deckcraft_engine::tools::Mods { alt: true, ..Default::default() },
        tol: 3.0,
        pressure: None,
    };
    h.state_mut().session.pointer(ev).unwrap();
    assert_eq!(h.state().session.tool.measures.len(), 1);
    assert_eq!(h.state().session.tool.measures[0].value, 16.0);
    h.run_steps(2);
    shot(&mut h, "measure.png");
}
