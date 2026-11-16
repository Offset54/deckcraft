//! Save as Picture… on an object's right-click menu.

use std::cell::RefCell;
use std::rc::Rc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;

use crate::{Services, SlideApp};
use deckcraft_engine::Session;

/// A named red rectangle covering the middle of a blank slide, selected.
fn harness(services: Services) -> Harness<'static, SlideApp> {
    let mut s = Session::with_new();
    s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
    let id = s.execute("shape.insert", &json!({"preset": "rect", "rect": [80.0, 60.0, 800.0, 420.0]})).unwrap()["id"].as_u64().unwrap();
    s.execute("shape.rename", &json!({"id": id, "name": "Sales / Q3: chart?"})).unwrap();
    s.execute("selection.select", &json!({"ids": [id]})).unwrap();
    let mut h = super::harness(SlideApp::new(s, services));
    h.run_steps(3);
    h
}

/// Right-click the middle of the slide and choose Save as Picture….
fn choose_save_as_picture(h: &mut Harness<'static, SlideApp>) {
    let pos = h.state().slide_rect.unwrap().center();
    h.hover_at(pos);
    h.run_steps(1);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run_steps(3);
    h.get_by_label("Save as Picture…").click();
    h.run_steps(3);
}

#[test]
fn the_menu_saves_the_object_where_the_dialog_says() {
    let dir = std::env::temp_dir().join(format!("deckcraft-ui-savepic-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("out.bmp");
    let asked: Rc<RefCell<Vec<String>>> = Rc::default();
    let (seen, chosen) = (asked.clone(), path.to_string_lossy().to_string());
    let services = Services {
        pick_save: Some(Box::new(move |suggested: &str| {
            seen.borrow_mut().push(suggested.to_string());
            Some(chosen.clone())
        })),
        ..Services::default()
    };
    let mut h = harness(services);
    let undo = h.state().session.active().unwrap().history.undo.len();
    choose_save_as_picture(&mut h);
    // The dialog was offered the object's name, made safe for a file system, as a PNG.
    assert_eq!(asked.borrow().as_slice(), ["Sales _ Q3_ chart_.png"]);
    // The name typed ended in .bmp, so a BMP was written; nothing was added to Undo.
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[..2], b"BM");
    assert_eq!(h.state().session.active().unwrap().history.undo.len(), undo);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cancelling_the_dialog_saves_nothing() {
    let services = Services { pick_save: Some(Box::new(|_: &str| None)), ..Services::default() };
    let mut h = harness(services);
    choose_save_as_picture(&mut h);
    assert!(h.state().session.active().is_some());
}

#[test]
fn without_a_save_dialog_the_picture_is_a_png_download() {
    let got: Rc<RefCell<Vec<(String, Vec<u8>)>>> = Rc::default();
    let sink = got.clone();
    let services = Services {
        download: Some(Box::new(move |name: &str, bytes: &[u8]| sink.borrow_mut().push((name.to_string(), bytes.to_vec())))),
        ..Services::default()
    };
    let mut h = harness(services);
    choose_save_as_picture(&mut h);
    let got = got.borrow();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].0, "Sales _ Q3_ chart_.png");
    assert_eq!(&got[0].1[..8], b"\x89PNG\r\n\x1a\n");
}
