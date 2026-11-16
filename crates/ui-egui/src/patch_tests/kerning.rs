//! Font dialog › Kerning for fonts.

use egui_kittest::kittest::Queryable;
use serde_json::json;

use super::harness;
use crate::{Services, SlideApp};
use deckcraft_engine::Session;

fn font_dialog() -> egui_kittest::Harness<'static, SlideApp> {
    let mut s = Session::with_new();
    s.execute("shape.insert", &json!({"preset": "rect", "rect": [80.0, 80.0, 320.0, 80.0], "text": "AVATAR"})).unwrap();
    let _ = s.execute("text.exit", &json!({}));
    let mut h = harness(SlideApp::new(s, Services::default()));
    h.run_steps(3);
    h.state_mut().run("format.fontDialog", json!({})).unwrap();
    h.run_steps(3);
    assert_eq!(h.state().dialog.as_ref().map(|d| d.id.as_str()), Some("font"));
    h
}

fn kern(h: &egui_kittest::Harness<'_, SlideApp>) -> Option<f64> {
    let st = h.state().session.active().unwrap();
    st.selected_shapes()
        .first()
        .and_then(|sh| sh.text.as_ref())
        .and_then(|t| t.paragraphs.first())
        .and_then(|p| p.runs.first())
        .and_then(|r| r.props.kern)
}

#[test]
fn the_font_dialog_turns_kerning_on_from_a_size() {
    let mut h = font_dialog();
    assert!(h.query_by_label("Kerning for fonts").is_some());
    let d = h.state_mut().dialog.as_mut().unwrap();
    d.fields.insert("kernOn".into(), "1".into());
    d.fields.insert("kern".into(), "14".into());
    h.run_steps(1);
    h.get_by_label("OK").click();
    h.run_steps(2);
    assert!(h.state().dialog.is_none(), "status {:?}", h.state().status);
    assert_eq!(kern(&h), Some(14.0));

    // Reopened, it shows what the text has; unticking turns kerning off.
    h.state_mut().run("format.fontDialog", json!({})).unwrap();
    h.run_steps(3);
    let d = h.state_mut().dialog.as_mut().unwrap();
    assert_eq!((d.fields.get("kernOn").map(String::as_str), d.fields.get("kern").map(String::as_str)), (Some("1"), Some("14")));
    d.fields.insert("kernOn".into(), "0".into());
    h.run_steps(1);
    h.get_by_label("OK").click();
    h.run_steps(2);
    assert_eq!(kern(&h), Some(0.0));
}

#[test]
fn ok_without_touching_kerning_leaves_it_alone() {
    let mut h = font_dialog();
    h.get_by_label("OK").click();
    h.run_steps(2);
    assert!(h.state().dialog.is_none(), "status {:?}", h.state().status);
    assert_eq!(kern(&h), None);
}

#[test]
fn a_kerning_size_that_is_not_a_number_keeps_the_dialog_open() {
    let mut h = font_dialog();
    let d = h.state_mut().dialog.as_mut().unwrap();
    d.fields.insert("kernOn".into(), "1".into());
    d.fields.insert("kern".into(), "big".into());
    h.run_steps(1);
    h.get_by_label("OK").click();
    h.run_steps(2);
    assert!(h.state().dialog.is_some());
    assert_eq!(kern(&h), None);
}
