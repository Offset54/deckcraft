//! Draw › Ink Replay.

use std::sync::Arc;

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;

use super::harness;
use crate::{Services, SlideApp};
use deckcraft_engine::Session;
use deckcraft_model::{InkStroke, Shape, ShapeKind, Xfrm};

/// A blank slide, with one ink object of two 350 pt lines (one second of replay) when `ink`.
fn app(ink: bool) -> Harness<'static, SlideApp> {
    let mut s = Session::with_new();
    s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
    if ink {
        let line = |y: f64| InkStroke { points: vec![(0.0, y, 1.0), (350.0, y, 1.0)], width: 3.0, ..Default::default() };
        s.edit(|doc, sel| {
            let id = deckcraft_model::ShapeId(doc.alloc_id().unwrap());
            let shape = Shape {
                id,
                name: "Ink".into(),
                xfrm: Some(Xfrm::new(100.0, 100.0, 350.0, 40.0)),
                kind: ShapeKind::Ink { strokes: vec![line(0.0), line(40.0)] },
                ..Default::default()
            };
            Arc::make_mut(&mut doc.slides[sel.slide]).shapes.push(shape);
            Ok(())
        })
        .unwrap();
    }
    let mut app = SlideApp::new(s, Services::default());
    app.ui.tab = "Draw".into();
    let mut h = harness(app);
    h.run_steps(3);
    h
}

/// How many ink points the canvas would draw now, and how many the slide really has.
fn drawn_and_real(h: &Harness<'static, SlideApp>) -> (usize, usize) {
    let st = h.state().session.active().unwrap();
    let count = |doc: &deckcraft_model::Presentation| {
        let mut n = 0;
        deckcraft_model::walk(&doc.slides[st.selection.slide].shapes, &mut |sh, _| {
            if let ShapeKind::Ink { strokes } = &sh.kind {
                n += strokes.iter().map(|s| s.points.len()).sum::<usize>();
            }
        });
        n
    };
    let shown = crate::ink_replay::frame_doc(h.state().ink_replay.as_ref(), &st.doc, st.selection.slide)
        .map_or_else(|| count(&st.doc), |(doc, _)| count(&doc));
    (shown, count(&st.doc))
}

#[test]
fn the_button_replays_the_ink_and_leaves_the_slide_alone() {
    let mut h = app(true);
    let before = h.state().session.active().unwrap().doc.clone();
    h.get_by_role_and_label(Role::Button, "Ink Replay").click();
    h.run_steps(2);
    assert!(h.state().ink_replay.is_some());
    // Just started: little or nothing is drawn yet, though the slide has all its ink.
    let (shown, real) = drawn_and_real(&h);
    assert!(shown < real && real == 4, "{shown} of {real}");
    // Half a second on: the first line is whole, the second not begun or just begun.
    h.run_steps(30);
    let (time, duration) = h.state().ink_replay.as_ref().unwrap().progress();
    assert!((duration - 1.0).abs() < 1e-9 && time > 0.3 && time < 0.7, "{time} of {duration}");
    assert!(drawn_and_real(&h).0 >= 2);
    // It ends by itself, with every stroke back.
    h.run_steps(80);
    assert!(h.state().ink_replay.is_none());
    assert_eq!(drawn_and_real(&h), (4, 4));
    // Nothing was edited: the same presentation, and no undo step.
    assert!(Arc::ptr_eq(&before, &h.state().session.active().unwrap().doc));
}

#[test]
fn pause_rewind_and_stop_through_the_command() {
    let mut h = app(true);
    let r = h.state_mut().run("draw.inkReplay", json!({})).unwrap();
    assert_eq!((r["playing"].as_bool(), r["strokes"].as_u64()), (Some(true), Some(2)));
    h.run_steps(20);
    h.state_mut().run("draw.inkReplay", json!({"pause": true})).unwrap();
    h.run_steps(2);
    let held = h.state().ink_replay.as_ref().unwrap().progress().0;
    h.run_steps(30);
    let r = h.state().ink_replay.as_ref().unwrap();
    assert!(r.paused() && (r.progress().0 - held).abs() < 1e-9 && held > 0.1);
    h.state_mut().run("draw.inkReplay", json!({"rewind": true})).unwrap();
    h.run_steps(3);
    let r = h.state().ink_replay.as_ref().unwrap();
    assert!(!r.paused() && r.progress().0 < held);
    let r = h.state_mut().run("draw.inkReplay", json!({"on": false})).unwrap();
    assert_eq!(r["playing"], false);
    assert!(h.state().ink_replay.is_none());
    // Bad parameters are refused; pausing nothing is an error, not a crash.
    assert!(h.state_mut().run("draw.inkReplay", json!({"pause": "yes"})).is_err());
    assert!(h.state_mut().run("draw.inkReplay", json!({"pause": true})).is_err());
}

#[test]
fn an_edit_or_another_slide_ends_the_replay() {
    let mut h = app(true);
    h.state_mut().run("draw.inkReplay", json!({})).unwrap();
    h.run_steps(5);
    h.state_mut().run("shape.insert", json!({"preset": "rect", "rect": [10.0, 10.0, 50.0, 50.0]})).unwrap();
    h.run_steps(2);
    assert!(h.state().ink_replay.is_none());
    h.state_mut().run("draw.inkReplay", json!({})).unwrap();
    h.run_steps(5);
    h.state_mut().run("slide.go", json!({"index": 0})).unwrap();
    h.run_steps(2);
    assert!(h.state().ink_replay.is_none());
}

#[test]
fn a_slide_without_ink_has_nothing_to_replay() {
    let mut h = app(false);
    assert!(h.state_mut().run("draw.inkReplay", json!({})).is_err());
    assert!(h.state().ink_replay.is_none());
    assert!(!crate::ink_replay::available(h.state()));
}
