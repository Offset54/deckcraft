//! SlideCraft Grid view (decision 003): one keystroke switches between Grid view and the
//! single-slide view; the view is window state, so switching never touches the deck.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use egui::{Key, Modifiers};
use serde_json::json;

use crate::{Services, SlideApp, ViewMode};
use deckcraft_engine::Session;

fn harness() -> egui_kittest::Harness<'static, SlideApp> {
    let mut s = Session::with_new();
    for _ in 0..3 {
        s.execute("slide.new", &json!({})).unwrap();
    }
    egui_kittest::Harness::builder().with_size(egui::vec2(1280.0, 800.0)).build_ui_state(
        |ui, app: &mut SlideApp| {
            app.logic(ui.ctx());
            app.ui(ui);
        },
        SlideApp::new(s, Services::default()),
    )
}

#[test]
fn one_keystroke_switches_between_grid_and_single_slide_view() {
    let mut h = harness();
    h.run_steps(2);
    assert_eq!(h.state().ui.view, ViewMode::Normal);
    let doc = h.state().session.active().unwrap().doc.clone();
    let steps = h.state().session.active().unwrap().history.undo.len();
    let chord = Modifiers::COMMAND | Modifiers::ALT;
    h.key_press_modifiers(chord, Key::B);
    h.run_steps(2);
    assert_eq!(h.state().ui.view, ViewMode::Sorter);
    h.key_press_modifiers(chord, Key::B);
    h.run_steps(2);
    assert_eq!(h.state().ui.view, ViewMode::Normal);
    // Window state: the deck is the same object, with no undo step added.
    let st = h.state().session.active().unwrap();
    assert!(std::sync::Arc::ptr_eq(&doc, &st.doc));
    assert_eq!(st.history.undo.len(), steps);
}

#[test]
fn view_grid_takes_an_explicit_state_and_reports_it() {
    let mut h = harness();
    h.run_steps(1);
    assert_eq!(h.state_mut().run("view.grid", json!({"on": true})).unwrap(), json!({"view": "grid"}));
    assert_eq!(h.state_mut().run("view.grid", json!({"on": true})).unwrap(), json!({"view": "grid"}));
    h.run_steps(2);
    assert_eq!(h.state().ui.view, ViewMode::Sorter);
    assert_eq!(h.state_mut().run("view.grid", json!({"on": false})).unwrap(), json!({"view": "single"}));
    assert_eq!(h.state().ui.view, ViewMode::Normal);
    // From any other view the toggle goes to the grid.
    h.state_mut().run("view.notesPage", json!({})).unwrap();
    assert_eq!(h.state_mut().run("view.grid", json!({})).unwrap(), json!({"view": "grid"}));
}

#[test]
fn slides_selected_in_the_grid_stay_selected_in_single_slide_view() {
    let mut h = harness();
    h.run_steps(1);
    h.state_mut().run("view.grid", json!({})).unwrap();
    h.state_mut().run("slide.selectSlides", json!({"indices": [1, 2]})).unwrap();
    h.run_steps(2);
    let before = h.state().session.active().unwrap().selection.slides.clone();
    assert_eq!(before.len(), 2);
    h.state_mut().run("view.grid", json!({})).unwrap();
    h.run_steps(2);
    assert_eq!(h.state().session.active().unwrap().selection.slides, before);
}
