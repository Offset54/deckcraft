//! Recent Colors: a colour typed into More Colors in one menu is offered in every colour menu.

use deckcraft_model::{ColorBase, ColorRef, Fill, Rgba, ShapeId};
use egui::accesskit::Role;
use egui::{Event, Key, Modifiers, PointerButton};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;

use super::harness;
use crate::{Services, SlideApp};
use deckcraft_engine::Session;

/// One selected rectangle on the Home tab.
fn app() -> (Harness<'static, SlideApp>, ShapeId) {
    let mut s = Session::with_new();
    s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
    let id = s.execute("shape.insert", &json!({"preset": "rect", "rect": [100.0, 100.0, 80.0, 60.0]})).unwrap()["id"].as_u64().unwrap();
    s.execute("selection.select", &json!({"ids": [id]})).unwrap();
    let mut app = SlideApp::new(s, Services::default());
    app.ui.tab = "Home".into();
    let mut h = harness(app);
    h.run_steps(3);
    (h, ShapeId(id as u32))
}

fn click_at(h: &mut Harness<'static, SlideApp>, pos: egui::Pos2) {
    h.input_mut().events.push(Event::PointerMoved(pos));
    h.run_steps(2);
    for pressed in [true, false] {
        h.input_mut().events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        h.run_steps(2);
    }
    h.run_steps(2);
}

/// Open the colour menu on the arrow of the split button `button`.
fn open_menu(h: &mut Harness<'static, SlideApp>, button: &str) {
    let arrow = h.get_by_role_and_label(Role::Button, &format!("{button} options")).rect().center();
    click_at(h, arrow);
}

/// Type `hex` into the open menu's More Colors field and press Enter.
fn type_hex(h: &mut Harness<'static, SlideApp>, hex: &str) {
    let label = h.get_by_label("More Colors… #").rect();
    let field = h
        .get_all_by_role(Role::TextInput)
        .filter(|n| n.rect().min.x >= label.max.x - 1.0 && (n.rect().center().y - label.center().y).abs() < 6.0)
        .map(|n| n.rect().center())
        .next()
        .expect("the More Colors hex field");
    click_at(h, field);
    h.input_mut().events.push(Event::Text(hex.into()));
    h.run_steps(2);
    for pressed in [true, false] {
        h.input_mut().events.push(Event::Key { key: Key::Enter, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE });
        h.run_steps(2);
    }
    h.run_steps(3);
}

fn recent(h: &Harness<'static, SlideApp>) -> Vec<Rgba> {
    h.state().session.active().unwrap().doc.recent_colors.clone()
}

fn line_rgb(h: &Harness<'static, SlideApp>, id: ShapeId) -> Option<String> {
    let sh = h.state().session.active().unwrap().shape(id).unwrap();
    match sh.line.as_ref()?.fill.as_ref()? {
        Fill::Solid { color: ColorRef { base: ColorBase::Rgb { rgb }, .. } } => Some(rgb.hex()),
        _ => None,
    }
}

#[test]
fn a_colour_from_more_colors_is_offered_in_another_menu() {
    let (mut h, id) = app();
    // A new presentation has no Recent Colors row.
    open_menu(&mut h, "Shape Fill");
    assert!(h.query_by_label("Recent Colors").is_none());
    type_hex(&mut h, "00AA55");
    assert_eq!(recent(&h), vec![Rgba::rgb(0x00, 0xAA, 0x55)]);
    // Remembering it added no undo step of its own: the last one is the fill.
    assert_eq!(h.state().session.active().unwrap().history.undo.last().unwrap().label, "Shape Fill");

    // The outline menu now offers it, and the swatch applies it.
    open_menu(&mut h, "Shape Outline");
    assert!(h.query_by_label("Recent Colors").is_some());
    let swatch = h.get_by_role_and_label(Role::Button, "Recent color #00AA55").rect().center();
    click_at(&mut h, swatch);
    assert_eq!(line_rgb(&h, id).as_deref(), Some("00AA55"));
    // Picking a recent colour does not shuffle the list.
    assert_eq!(recent(&h), vec![Rgba::rgb(0x00, 0xAA, 0x55)]);
}

#[test]
fn the_eyedroppers_colour_is_remembered_too() {
    let (mut h, _) = app();
    // The Eyedropper hands its colour to the menu that asked; the grid notes it on the way.
    open_menu(&mut h, "Shape Fill");
    let menu = crate::eyedropper::open_menu(&h.ctx).expect("the open menu noted its button");
    crate::eyedropper::store_result(&h.ctx, menu, "123456");
    h.run_steps(4);
    assert_eq!(recent(&h), vec![Rgba::rgb(0x12, 0x34, 0x56)]);
}
