//! Crop › Aspect Ratio, Fill, Fit, and the Format Picture pane's Crop fields.

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;

use super::harness;
use crate::{Services, SlideApp};
use deckcraft_engine::Session;
use deckcraft_geom::Unit;

/// A 1 x 1 pixel PNG.
fn png() -> Vec<u8> {
    const HEX: &str =
        "89504e470d0a1a0a0000000d4948445200000001000000010802000000907753de0000000c49444154789c63f8cfc0000003010100c9fe92ef0000000049454e44ae426082";
    (0..HEX.len()).step_by(2).map(|i| u8::from_str_radix(&HEX[i..i + 2], 16).expect("hex")).collect()
}

/// A square picture stretched into a 400 x 200 pt frame at (100, 100), selected.
fn app() -> (SlideApp, u32) {
    let mut s = Session::with_new();
    s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
    let data = deckcraft_engine::cmd::base64_encode(&png());
    let id =
        s.execute("insert.picture", &json!({"name": "p.png", "data": data, "rect": [100.0, 100.0, 400.0, 200.0]})).unwrap()["id"].as_u64().unwrap();
    s.execute("selection.select", &json!({"ids": [id]})).unwrap();
    s.prefs.units = Unit::Points;
    (SlideApp::new(s, Services::default()), id as u32)
}

fn crop(h: &Harness<'static, SlideApp>, id: u32) -> [f64; 4] {
    match &h.state().session.active().unwrap().shape(deckcraft_model::ShapeId(id)).unwrap().kind {
        deckcraft_model::ShapeKind::Picture { fill } => fill.crop,
        _ => panic!("a picture"),
    }
}

fn frame(h: &Harness<'static, SlideApp>, id: u32) -> deckcraft_geom::Xfrm {
    h.state().session.active().unwrap().shape(deckcraft_model::ShapeId(id)).unwrap().xfrm.unwrap()
}

fn near(got: [f64; 4], want: [f64; 4]) {
    assert!(got.iter().zip(&want).all(|(a, b)| (a - b).abs() < 1e-6), "{got:?} vs {want:?}");
}

fn open_crop_menu(h: &mut Harness<'static, SlideApp>) {
    h.get_by_role_and_label(Role::Button, "Crop options").click();
    h.run_steps(3);
}

#[test]
fn the_crop_menu_fills_and_fits() {
    let (mut app, id) = app();
    app.ui.tab = "Picture Format".into();
    let mut h = harness(app);
    h.run_steps(3);
    open_crop_menu(&mut h);
    h.get_by_label("Fill").click();
    h.run_steps(3);
    // The square picture covers the 2:1 frame: a quarter cut off the top and the bottom.
    near(crop(&h, id), [0.0, 0.25, 0.0, 0.25]);
    assert_eq!(h.state().session.active().unwrap().history.undo.last().unwrap().label, "Fill");
    open_crop_menu(&mut h);
    h.get_by_label("Fit").click();
    h.run_steps(3);
    near(crop(&h, id), [-0.5, 0.0, -0.5, 0.0]);
}

#[test]
fn the_crop_menu_offers_aspect_ratios() {
    let (mut app, id) = app();
    app.ui.tab = "Picture Format".into();
    let mut h = harness(app);
    h.run_steps(3);
    open_crop_menu(&mut h);
    h.get_by_label("Aspect Ratio").click();
    h.run_steps(3);
    assert!(h.query_by_label("16:9").is_some() && h.query_by_label("Portrait").is_some());
    h.get_by_label("1:1").click();
    h.run_steps(3);
    // The largest square inside the 400 x 200 picture, centred.
    let x = frame(&h, id);
    assert!((x.w - 200.0).abs() < 1e-6 && (x.h - 200.0).abs() < 1e-6 && (x.x - 200.0).abs() < 1e-6, "{x:?}");
    near(crop(&h, id), [0.25, 0.0, 0.25, 0.0]);
}

/// The pane's field named `name`, as (centre, shown text).
fn field(h: &Harness<'static, SlideApp>, name: &str) -> (egui::Pos2, String) {
    let label = format!("{name} value");
    let n = h.get_by_role_and_label(Role::TextInput, &label);
    (n.rect().center(), n.value().unwrap_or_default())
}

fn type_into(h: &mut Harness<'static, SlideApp>, name: &str, text: &str) {
    h.get_by_role_and_label(Role::TextInput, &format!("{name} value")).focus();
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run_steps(1);
    h.get_by_role_and_label(Role::TextInput, &format!("{name} value")).type_text(text);
    h.run_steps(1);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
}

#[test]
fn the_format_pane_shows_and_sets_the_crop_position() {
    let (mut app, id) = app();
    app.ui.pane = Some("format".into());
    app.ui.format_tab = "picture".into();
    let mut h = harness(app);
    h.run_steps(4);
    assert_eq!(field(&h, "Picture position Width").1, "400 pt");
    assert_eq!(field(&h, "Crop position Left").1, "100 pt");
    assert_eq!(field(&h, "Picture position Offset X").1, "0 pt");
    // A narrower frame over the same picture: the right quarter is cropped away.
    type_into(&mut h, "Crop position Width", "300");
    near(crop(&h, id), [0.0, 0.0, 0.25, 0.0]);
    assert!((frame(&h, id).w - 300.0).abs() < 1e-6);
    assert_eq!(field(&h, "Picture position Offset X").1, "50 pt");
    // Centre the picture in it.
    type_into(&mut h, "Picture position Offset X", "0");
    near(crop(&h, id), [0.125, 0.0, 0.125, 0.0]);
    // Leaving a field without changing it changes nothing.
    let before = (crop(&h, id), frame(&h, id), h.state().session.active().unwrap().history.undo.len());
    type_into(&mut h, "Crop position Top", "100");
    assert_eq!((crop(&h, id), frame(&h, id), h.state().session.active().unwrap().history.undo.len()), before);
}
