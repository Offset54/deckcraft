//! Selecting and editing inside groups (P3.2), measured distances, and group child space kept in
//! slide space.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use deckcraft_model::{ShapeId, ShapeKind};
use serde_json::json;

use crate::Session;
use crate::tools::{Mods, PointerEvent, PointerKind};

fn ev(kind: PointerKind, x: f64, y: f64) -> PointerEvent {
    PointerEvent { kind, x, y, mods: Mods::default(), tol: 3.0, pressure: None }
}

fn id(v: &serde_json::Value) -> ShapeId {
    ShapeId(v["id"].as_u64().unwrap() as u32)
}

/// A blank slide with two rectangles grouped: returns (group, left member, right member).
fn grouped() -> (Session, ShapeId, ShapeId, ShapeId) {
    let mut s = Session::with_new();
    s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
    let a = id(&s.execute("shape.insert", &json!({"preset": "rect", "rect": [100, 100, 100, 100]})).unwrap());
    let b = id(&s.execute("shape.insert", &json!({"preset": "rect", "rect": [300, 100, 100, 100]})).unwrap());
    let g = id(&s.execute("arrange.group", &json!({"ids": [a.0, b.0]})).unwrap());
    (s, g, a, b)
}

fn selected(s: &Session) -> Vec<ShapeId> {
    s.doc().unwrap().selection.shapes.clone()
}

#[test]
fn enter_selects_members_and_shift_enter_or_escape_the_group() {
    let (mut s, g, a, b) = grouped();
    s.execute("edit.select", &json!({"ids": [g.0]})).unwrap();
    let r = s.key("Enter", Mods::default()).unwrap();
    assert_eq!(r["command"], "edit.selectChildren");
    assert_eq!(selected(&s), vec![a, b]);
    // Editing a member in place: one undo step, the group stays, and its box follows.
    let steps = s.doc().unwrap().history.undo.len();
    s.execute("shape.move", &json!({"ids": [b.0], "dx": 100})).unwrap();
    assert_eq!(s.doc().unwrap().history.undo.len(), steps + 1);
    let gx = s.doc().unwrap().shape(g).unwrap().xfrm.unwrap();
    assert_eq!((gx.x, gx.w), (100.0, 400.0));
    let r = s.key("Enter", Mods { shift: true, ..Default::default() }).unwrap();
    assert_eq!(r["command"], "edit.selectParent");
    assert_eq!(selected(&s), vec![g]);
    s.execute("edit.selectChildren", &json!({})).unwrap();
    s.key("Escape", Mods::default()).unwrap();
    assert_eq!(selected(&s), vec![g]);
    // At the top level Esc deselects, as before.
    s.key("Escape", Mods::default()).unwrap();
    assert!(selected(&s).is_empty());
}

#[test]
fn clicks_pick_members_once_inside_a_group() {
    let (mut s, g, a, b) = grouped();
    s.execute("edit.deselect", &json!({})).unwrap();
    // A click on a member selects the group.
    s.pointer(ev(PointerKind::Down, 150.0, 150.0)).unwrap();
    s.pointer(ev(PointerKind::Up, 150.0, 150.0)).unwrap();
    assert_eq!(selected(&s), vec![g]);
    // A double-click goes in and selects the member under the pointer.
    s.pointer(ev(PointerKind::DoubleClick, 150.0, 150.0)).unwrap();
    assert_eq!(selected(&s), vec![a]);
    // Inside: a click on another member selects that one.
    s.pointer(ev(PointerKind::Down, 350.0, 150.0)).unwrap();
    s.pointer(ev(PointerKind::Up, 350.0, 150.0)).unwrap();
    assert_eq!(selected(&s), vec![b]);
    // Dragging it moves just the member.
    s.pointer(ev(PointerKind::Down, 350.0, 150.0)).unwrap();
    s.pointer(ev(PointerKind::Drag, 350.0, 250.0)).unwrap();
    s.pointer(ev(PointerKind::Up, 350.0, 250.0)).unwrap();
    let st = s.doc().unwrap();
    assert!(st.shape(b).unwrap().xfrm.unwrap().y > 150.0);
    assert_eq!(st.shape(a).unwrap().xfrm.unwrap().y, 100.0);
    assert!(matches!(st.current_slide().unwrap().shapes.last().unwrap().kind, ShapeKind::Group { .. }));
    // A click outside the group leaves it.
    s.pointer(ev(PointerKind::Down, 600.0, 400.0)).unwrap();
    s.pointer(ev(PointerKind::Up, 600.0, 400.0)).unwrap();
    assert!(selected(&s).is_empty());
}

#[test]
fn a_rotated_group_is_not_entered() {
    let (mut s, g, ..) = grouped();
    s.execute("edit.select", &json!({"ids": [g.0]})).unwrap();
    s.execute("shape.rotate", &json!({"deg": 30})).unwrap();
    let e = s.execute("edit.selectChildren", &json!({})).unwrap_err().to_string();
    assert!(e.contains("ungroup"), "{e}");
    assert_eq!(selected(&s), vec![g]);
    // Not a group at all.
    let (mut s, _, a, _) = grouped();
    let e = s.execute("edit.selectChildren", &json!({"ids": [a.0]})).unwrap_err().to_string();
    assert!(e.contains("select a group"), "{e}");
    assert!(s.execute("edit.selectChildren", &json!({"ids": [99999]})).is_err());
}

#[test]
fn groups_keep_their_members_in_slide_space_after_any_edit() {
    let (mut s, g, a, _) = grouped();
    s.execute("edit.select", &json!({"ids": [g.0]})).unwrap();
    s.execute("shape.move", &json!({"dx": 50, "dy": 20})).unwrap();
    s.execute("shape.resize", &json!({"w": 600})).unwrap();
    let st = s.doc().unwrap();
    let gs = st.shape(g).unwrap();
    let ShapeKind::Group { child, .. } = &gs.kind else { panic!() };
    let gx = gs.xfrm.unwrap();
    assert!((child.x - gx.x).abs() < 1e-6 && (child.w - gx.w).abs() < 1e-6, "{child:?} {gx:?}");
    // The member is drawn where its stored box says.
    let ax = st.shape(a).unwrap().xfrm.unwrap();
    assert!((ax.x - gx.x).abs() < 1e-6, "{ax:?} {gx:?}");
    assert!(deckcraft_model::canvas::editable_in_place(st.shapes(), a));
}

#[test]
fn a_file_with_a_drifted_group_opens_with_its_members_in_slide_space() {
    let (mut s, g, a, _) = grouped();
    {
        let st = s.doc_mut().unwrap();
        let doc = std::sync::Arc::make_mut(&mut st.doc);
        let slide = std::sync::Arc::make_mut(doc.slides.last_mut().unwrap());
        let gs = deckcraft_model::find_shape_mut(&mut slide.shapes, g).unwrap();
        // As PowerPoint leaves it after a move: the box moved, the child space did not.
        if let Some(x) = gs.xfrm.as_mut() {
            x.x += 200.0;
        }
    }
    let bytes = deckcraft_format::save(&s.doc().unwrap().doc).unwrap();
    let (p, _) = crate::cmd::file::open_checked("t.deckcraft", &bytes).unwrap();
    let slide = p.slides.last().unwrap();
    let ax = deckcraft_model::find_shape(&slide.shapes, a).unwrap().xfrm.unwrap();
    assert_eq!(ax.x, 300.0);
    assert!(deckcraft_model::canvas::editable_in_place(&slide.shapes, a));
}

#[test]
fn measure_reports_gaps_and_alt_hover_shows_them() {
    let (mut s, _, a, b) = grouped();
    let r = s.execute("arrange.measure", &json!({"ids": [a.0], "to": b.0})).unwrap();
    assert_eq!(r["gaps"].as_array().unwrap().len(), 1);
    assert_eq!(r["gaps"][0]["value"], 100.0);
    assert_eq!(r["gaps"][0]["axis"], "x");
    // To the slide: four distances, nothing changed, no undo step.
    let steps = s.doc().unwrap().history.undo.len();
    let r = s.execute("arrange.measure", &json!({"ids": [a.0]})).unwrap();
    assert_eq!(r["gaps"].as_array().unwrap().len(), 4);
    assert_eq!(r["gaps"][0]["value"], 100.0);
    assert_eq!(s.doc().unwrap().history.undo.len(), steps);
    assert!(s.execute("arrange.measure", &json!({"ids": [a.0], "to": 424242})).is_err());
    s.execute("edit.deselect", &json!({})).unwrap();
    assert!(s.execute("arrange.measure", &json!({})).is_err());
    // Alt over the canvas.
    s.execute("edit.select", &json!({"ids": [a.0]})).unwrap();
    let alt = PointerEvent { mods: Mods { alt: true, ..Default::default() }, ..ev(PointerKind::Move, 350.0, 150.0) };
    s.pointer(alt).unwrap();
    assert_eq!(s.tool.measures.len(), 1);
    assert_eq!(s.tool.measures[0].value, 100.0);
    s.pointer(ev(PointerKind::Move, 350.0, 150.0)).unwrap();
    assert!(s.tool.measures.is_empty());
}

// ---------- Frames and constraints (P3.3) ----------

fn color_at(s: &mut Session, x: f64, y: f64) -> String {
    s.execute("slide.colorAt", &json!({"x": x, "y": y})).unwrap()["color"].as_str().unwrap().to_string()
}

fn xf(s: &Session, id: ShapeId) -> deckcraft_geom::Xfrm {
    s.doc().unwrap().shape(id).unwrap().xfrm.unwrap()
}

/// A blank slide with two rectangles in a frame with 20 pt padding: (frame, a, b).
fn framed() -> (Session, ShapeId, ShapeId, ShapeId) {
    let mut s = Session::with_new();
    s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
    let a = id(&s.execute("shape.insert", &json!({"preset": "rect", "rect": [100, 100, 100, 100]})).unwrap());
    let b = id(&s.execute("shape.insert", &json!({"preset": "rect", "rect": [300, 100, 100, 100]})).unwrap());
    let f = id(&s.execute("frame.create", &json!({"ids": [a.0, b.0], "padding": 20, "fill": "#00FF00"})).unwrap());
    (s, f, a, b)
}

#[test]
fn a_frame_wraps_objects_with_padding_and_keeps_its_box_when_they_move() {
    let (mut s, f, a, b) = framed();
    assert_eq!(selected(&s), vec![f]);
    assert_eq!(xf(&s, f), deckcraft_geom::Xfrm::new(80.0, 80.0, 340.0, 140.0));
    assert_eq!(xf(&s, a), deckcraft_geom::Xfrm::new(100.0, 100.0, 100.0, 100.0));
    // One undo step puts the two shapes back.
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(s.doc().unwrap().current_slide().unwrap().shapes.iter().all(|x| x.frame.is_none()));
    s.execute("edit.redo", &json!({})).unwrap();
    // A member moved inside the frame doesn't resize it (a plain group would refit).
    s.execute("shape.move", &json!({"ids": [b.0], "dx": -50})).unwrap();
    assert_eq!(xf(&s, f), deckcraft_geom::Xfrm::new(80.0, 80.0, 340.0, 140.0));
    // The frame draws its fill where it has no member.
    assert_eq!(color_at(&mut s, 90.0, 90.0), "00FF00");
    assert_ne!(color_at(&mut s, 70.0, 70.0), "00FF00");
    // And is hit there.
    s.execute("edit.deselect", &json!({})).unwrap();
    s.pointer(ev(PointerKind::Down, 90.0, 90.0)).unwrap();
    s.pointer(ev(PointerKind::Up, 90.0, 90.0)).unwrap();
    assert_eq!(selected(&s), vec![f]);
}

#[test]
fn resizing_a_frame_moves_members_by_their_constraints() {
    let (mut s, f, a, b) = framed();
    // a stretches across; b keeps its distance from the right edge.
    s.execute("layout.constraints", &json!({"ids": [a.0], "horizontal": "both"})).unwrap();
    s.execute("layout.constraints", &json!({"ids": [b.0], "horizontal": "end", "vertical": "center"})).unwrap();
    s.execute("shape.resize", &json!({"ids": [f.0], "w": 440, "h": 240})).unwrap();
    let fx = xf(&s, f);
    // shape.resize keeps the centre.
    assert_eq!((fx.w, fx.h), (440.0, 240.0));
    let (ax, bx) = (xf(&s, a), xf(&s, b));
    assert!((ax.x - (fx.x + 20.0)).abs() < 1e-6 && (ax.w - 200.0).abs() < 1e-6, "{ax:?} {fx:?}");
    assert!((ax.h - 100.0).abs() < 1e-6, "start vertically keeps the height: {ax:?}");
    assert!(((fx.x + fx.w) - (bx.x + bx.w) - 20.0).abs() < 1e-6 && (bx.w - 100.0).abs() < 1e-6, "{bx:?} {fx:?}");
    assert!(((bx.y + bx.h / 2.0) - (fx.y + fx.h / 2.0)).abs() < 1e-6, "centred: {bx:?} {fx:?}");
    // Scale follows the frame's proportions.
    s.execute("layout.constraints", &json!({"ids": [a.0], "horizontal": "scale", "vertical": "scale"})).unwrap();
    let before = xf(&s, a);
    s.execute("shape.resize", &json!({"ids": [f.0], "w": 880, "h": 480})).unwrap();
    let after = xf(&s, a);
    assert!((after.w - before.w * 2.0).abs() < 1e-6 && (after.h - before.h * 2.0).abs() < 1e-6, "{before:?} {after:?}");
}

#[test]
fn frame_options_clip_and_turn_back_into_a_group() {
    let (mut s, f, _, b) = framed();
    // Push b half out of the frame, to the right.
    s.execute("shape.move", &json!({"ids": [b.0], "dx": 80})).unwrap();
    assert_eq!(color_at(&mut s, 470.0, 150.0), color_at(&mut s, 450.0, 150.0));
    let outside = color_at(&mut s, 470.0, 150.0);
    s.execute("frame.set", &json!({"clip": true, "radius": 12})).unwrap();
    assert_ne!(color_at(&mut s, 470.0, 150.0), outside, "clipped away");
    let st = s.doc().unwrap();
    assert_eq!(st.shape(f).unwrap().frame, Some(deckcraft_model::canvas::Frame { clip: true, radius: 12.0 }));
    s.execute("frame.set", &json!({"frame": false})).unwrap();
    assert!(s.doc().unwrap().shape(f).unwrap().frame.is_none());
    assert!(s.execute("layout.constraints", &json!({"ids": [b.0], "horizontal": "end"})).is_err());
}

#[test]
fn frame_commands_refuse_hostile_input() {
    let (mut s, f, a, _) = framed();
    for p in [json!({"ids": [a.0], "horizontal": "sideways"}), json!({"ids": [a.0], "vertical": 7}), json!({"ids": [123456], "horizontal": "end"})] {
        assert!(s.execute("layout.constraints", &p).is_err(), "{p}");
    }
    for p in [json!({"ids": [f.0], "radius": -1}), json!({"ids": [f.0], "radius": 1e300}), json!({"ids": [a.0], "clip": true})] {
        assert!(s.execute("frame.set", &p).is_err(), "{p}");
    }
    for p in [json!({"padding": "x", "ids": [a.0]}), json!({"rect": [0, 0, 10, 10], "fill": "not a colour"}), json!({"ids": []})] {
        assert!(s.execute("frame.create", &p).is_err(), "{p}");
    }
    // A member can't be framed apart from its frame.
    assert!(s.execute("frame.create", &json!({"ids": [a.0]})).is_err());
    // An empty frame from a rectangle.
    let e = id(&s.execute("frame.create", &json!({"rect": [500, 300, 100, 50]})).unwrap());
    assert_eq!(xf(&s, e), deckcraft_geom::Xfrm::new(500.0, 300.0, 100.0, 50.0));
    // A frame survives saving and opening.
    let bytes = deckcraft_format::save(&s.doc().unwrap().doc).unwrap();
    let (p, _) = crate::cmd::file::open_checked("t.deckcraft", &bytes).unwrap();
    let sh = deckcraft_model::find_shape(&p.slides.last().unwrap().shapes, a).unwrap();
    assert_eq!(sh.constraints, None);
    assert!(deckcraft_model::find_shape(&p.slides.last().unwrap().shapes, f).unwrap().frame.is_some());
}

// ---------- Auto layout (P3.4) ----------

/// Three 100×50 rectangles on a blank slide: (session, ids).
fn three() -> (Session, Vec<ShapeId>) {
    let mut s = Session::with_new();
    s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
    let ids = [[50, 300], [400, 100], [200, 200]]
        .iter()
        .map(|[x, y]| id(&s.execute("shape.insert", &json!({"preset": "rect", "rect": [x, y, 100, 50]})).unwrap()))
        .collect();
    (s, ids)
}

#[test]
fn auto_layout_on_loose_objects_frames_them_and_lines_them_up() {
    let (mut s, ids) = three();
    let raw: Vec<u32> = ids.iter().map(|i| i.0).collect();
    let steps = s.doc().unwrap().history.undo.len();
    let r = s.execute("layout.set", &json!({"ids": raw, "gap": 10, "padding": 20, "width": "hug", "height": "hug"})).unwrap();
    // Framing and laying out are one undo step.
    assert_eq!(s.doc().unwrap().history.undo.len(), steps + 1);
    let f = ShapeId(r["ids"][0].as_u64().unwrap() as u32);
    let fx = xf(&s, f);
    assert_eq!((fx.w, fx.h), (20.0 + 300.0 + 20.0 + 20.0, 20.0 + 50.0 + 20.0));
    // In order, left to right, 10 apart, inside the padding.
    let xs: Vec<f64> = ids.iter().map(|i| xf(&s, *i).x - fx.x).collect();
    assert_eq!(xs, vec![20.0, 130.0, 240.0]);
    assert!(ids.iter().all(|i| (xf(&s, *i).y - fx.y - 20.0).abs() < 1e-9));
    // Changing the flow restacks them; the frame hugs again.
    s.execute("layout.set", &json!({"ids": [f.0], "flow": "vertical"})).unwrap();
    let fx = xf(&s, f);
    assert_eq!((fx.w, fx.h), (140.0, 20.0 + 150.0 + 20.0 + 20.0));
    // A member made wider pushes the frame out; one made to fill takes the width.
    s.execute("shape.resize", &json!({"ids": [ids[1].0], "w": 200})).unwrap();
    assert_eq!(xf(&s, f).w, 240.0);
    s.execute("layout.child", &json!({"ids": [ids[0].0], "width": "fill"})).unwrap();
    assert_eq!(xf(&s, ids[0]).w, 200.0);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(xf(&s, ids[0]).w, 100.0);
}

#[test]
fn a_member_moved_by_hand_goes_back_unless_it_is_absolute() {
    let (mut s, ids) = three();
    let raw: Vec<u32> = ids.iter().map(|i| i.0).collect();
    s.execute("layout.set", &json!({"ids": raw})).unwrap();
    let before = xf(&s, ids[0]);
    s.execute("shape.move", &json!({"ids": [ids[0].0], "dx": 33, "dy": 44})).unwrap();
    assert_eq!(xf(&s, ids[0]), before);
    s.execute("layout.child", &json!({"ids": [ids[0].0], "absolute": true})).unwrap();
    s.execute("shape.move", &json!({"ids": [ids[0].0], "dx": 33, "dy": 44})).unwrap();
    assert_eq!(xf(&s, ids[0]).x, before.x + 33.0);
    // The others close the gap.
    assert_eq!(xf(&s, ids[1]).x, before.x);
}

#[test]
fn a_grid_with_fr_tracks_and_a_spanning_member() {
    let (mut s, ids) = three();
    let raw: Vec<u32> = ids.iter().map(|i| i.0).collect();
    let r = s.execute(
        "layout.set",
        &json!({"ids": raw, "flow": "grid", "columns": ["1fr", "2fr"], "gap": 0, "crossGap": 0, "padding": 0, "align": "stretch"}),
    );
    // `align` doesn't take stretch for flows… but it's refused for any flow, consistently.
    assert!(r.is_err());
    let r = s.execute("layout.set", &json!({"ids": raw, "flow": "grid", "columns": ["1fr", "2fr"], "gap": 0, "crossGap": 0, "padding": 0})).unwrap();
    let f = ShapeId(r["ids"][0].as_u64().unwrap() as u32);
    s.execute("shape.resize", &json!({"ids": [f.0], "w": 300, "h": 200})).unwrap();
    s.execute("layout.child", &json!({"ids": [ids[2].0], "columnSpan": 2, "width": "fill"})).unwrap();
    let fx = xf(&s, f);
    assert_eq!(xf(&s, ids[0]).x - fx.x, 0.0);
    assert_eq!(xf(&s, ids[1]).x - fx.x, 100.0);
    let spanning = xf(&s, ids[2]);
    assert_eq!((spanning.x - fx.x, spanning.y - fx.y, spanning.w), (0.0, 50.0, 300.0));
}

#[test]
fn nested_auto_layout_frames_hug_from_the_inside_out() {
    let (mut s, ids) = three();
    let inner = s.execute("layout.set", &json!({"ids": [ids[0].0, ids[1].0], "gap": 0, "padding": 0, "width": "hug", "height": "hug"})).unwrap();
    let inner = ShapeId(inner["ids"][0].as_u64().unwrap() as u32);
    assert_eq!(xf(&s, inner).w, 200.0);
    let outer = s
        .execute("layout.set", &json!({"ids": [inner.0, ids[2].0], "flow": "vertical", "gap": 0, "padding": 10, "width": "hug", "height": "hug"}))
        .unwrap();
    let outer = ShapeId(outer["ids"][0].as_u64().unwrap() as u32);
    assert_eq!((xf(&s, outer).w, xf(&s, outer).h), (220.0, 120.0));
    // Growing a leaf grows both frames.
    s.execute("shape.resize", &json!({"ids": [ids[1].0], "w": 150})).unwrap();
    assert_eq!(xf(&s, inner).w, 250.0);
    assert_eq!(xf(&s, outer).w, 270.0);
    // The inner frame's members moved with it, in slide space.
    assert_eq!(xf(&s, ids[0]).x, xf(&s, outer).x + 10.0);
}

#[test]
fn layout_commands_refuse_hostile_input() {
    let (mut s, ids) = three();
    let raw: Vec<u32> = ids.iter().map(|i| i.0).collect();
    for p in [
        json!({"ids": raw, "flow": "diagonal"}),
        json!({"ids": raw, "gap": -1}),
        json!({"ids": raw, "gap": 1e9}),
        json!({"ids": raw, "padding": [1, 2, 3]}),
        json!({"ids": raw, "padding": "lots"}),
        json!({"ids": raw, "columns": []}),
        json!({"ids": raw, "columns": ["-1fr"]}),
        json!({"ids": raw, "columns": ["wide"]}),
        json!({"ids": raw, "width": "fill"}),
        json!({"ids": raw, "crossAlign": "spaceBetween"}),
        json!({"ids": [999999]}),
    ] {
        assert!(s.execute("layout.set", &p).is_err(), "{p}");
    }
    assert!(s.doc().unwrap().current_slide().unwrap().shapes.iter().all(|x| x.frame.is_none()), "a refused command changes nothing");
    // Not in an auto layout frame.
    assert!(s.execute("layout.child", &json!({"ids": [raw[0]], "width": "fill"})).is_err());
    assert!(s.execute("layout.remove", &json!({"ids": [raw[0]]})).is_err());
    let r = s.execute("layout.set", &json!({"ids": raw})).unwrap();
    let f = r["ids"][0].as_u64().unwrap();
    for p in [
        json!({"ids": [raw[0]], "width": "hug"}),
        json!({"ids": [raw[0]], "column": -1}),
        json!({"ids": [raw[0]], "row": 100000}),
        json!({"ids": [raw[0]], "columnSpan": 0}),
    ] {
        assert!(s.execute("layout.child", &p).is_err(), "{p}");
    }
    // Removing keeps the members where they are.
    let before = xf(&s, ids[1]);
    s.execute("layout.remove", &json!({"ids": [f]})).unwrap();
    assert_eq!(xf(&s, ids[1]), before);
    assert!(s.doc().unwrap().shape(ShapeId(f as u32)).unwrap().frame.is_some());
}

#[test]
fn a_frame_with_a_corner_radius_draws_rounded_corners() {
    let (mut s, f, ..) = framed();
    let fx = xf(&s, f);
    assert_eq!(color_at(&mut s, fx.x + 2.0, fx.y + 2.0), "00FF00");
    s.execute("frame.set", &json!({"ids": [f.0], "radius": 30})).unwrap();
    assert_ne!(color_at(&mut s, fx.x + 2.0, fx.y + 2.0), "00FF00", "the corner is cut");
    assert_eq!(color_at(&mut s, fx.x + 30.0, fx.y + 2.0), "00FF00");
}

#[test]
fn a_frame_filled_afterwards_keeps_rounded_corners() {
    let (mut s, ids) = three();
    let raw: Vec<u32> = ids.iter().map(|i| i.0).collect();
    let r = s.execute("layout.set", &json!({"ids": raw, "gap": 16, "padding": 24, "width": "hug", "height": "hug"})).unwrap();
    let f = ShapeId(r["ids"][0].as_u64().unwrap() as u32);
    s.execute("shape.fill", &json!({"ids": [f.0], "color": "#00FF00"})).unwrap();
    s.execute("frame.set", &json!({"radius": 16})).unwrap();
    let fx = xf(&s, f);
    eprintln!("{:?} {:?}", s.doc().unwrap().shape(f).unwrap().fill, s.doc().unwrap().shape(f).unwrap().frame);
    assert_eq!(color_at(&mut s, fx.x + 20.0, fx.y + 8.0), "00FF00");
    assert_ne!(color_at(&mut s, fx.x + 1.0, fx.y + 1.0), "00FF00", "the corner is cut");
}
