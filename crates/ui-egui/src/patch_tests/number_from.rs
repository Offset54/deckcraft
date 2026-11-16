//! Slide Size › Number slides from.

use egui_kittest::kittest::Queryable;

use super::{harness, new_app};

fn first(h: &egui_kittest::Harness<'_, crate::SlideApp>) -> u32 {
    h.state().session.active().unwrap().doc.first_slide_number
}

#[test]
fn the_slide_size_dialog_sets_the_first_slide_number() {
    let mut app = new_app();
    app.dialog = Some(crate::dialogs::Dialog::new("slideSize"));
    let mut h = harness(app);
    h.run_steps(3);
    assert!(h.query_by_label("Number slides from:").is_some());
    assert_eq!(h.state().dialog.as_ref().unwrap().fields.get("first").map(String::as_str), Some("1"));
    h.state_mut().dialog.as_mut().unwrap().fields.insert("first".into(), "0".into());
    h.run_steps(1);
    h.get_by_label("OK").click();
    h.run_steps(3);
    assert!(h.state().dialog.is_none(), "status {:?}", h.state().status);
    assert_eq!(first(&h), 0);
    // The size was left alone.
    let size = h.state().session.active().unwrap().doc.slide_size;
    assert_eq!((size.width, size.height), (960.0, 540.0));
}

#[test]
fn a_bad_first_slide_number_keeps_the_dialog_open() {
    let mut app = new_app();
    app.dialog = Some(crate::dialogs::Dialog::new("slideSize"));
    let mut h = harness(app);
    h.run_steps(3);
    h.state_mut().dialog.as_mut().unwrap().fields.insert("first".into(), "10000".into());
    h.run_steps(1);
    h.get_by_label("OK").click();
    h.run_steps(3);
    assert!(h.state().dialog.is_some());
    assert_eq!(first(&h), 1);
}
