#![allow(clippy::unwrap_used, clippy::expect_used)]

use deckcraft_model::{Geom, ShapeId};
use serde_json::json;

use crate::Session;

fn id(v: &serde_json::Value) -> ShapeId {
    ShapeId(v["id"].as_u64().unwrap() as u32)
}

fn deck() -> Session {
    let mut s = Session::with_new();
    s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
    s
}

/// A triangle with a spur from one corner: three segments meet at (200, 100).
fn spur() -> serde_json::Value {
    json!({
        "vertices": [[100, 100], [200, 100], [150, 180], [260, 60]],
        "segments": [[0, 1], [1, 2], [2, 0], {"a": 1, "b": 3, "ta": [20, 0], "tb": [0, 20]}],
        "regions": [{"loops": [[0, 1, 2]]}]
    })
}

#[test]
fn a_network_with_a_branch_is_one_shape_with_a_filled_region() {
    let mut s = deck();
    let v = id(&s.execute("vector.create", &spur()).unwrap());
    let st = s.doc().unwrap();
    let sh = st.shape(v).unwrap();
    let x = sh.xfrm.unwrap();
    // The box covers the points and the curve handles.
    assert_eq!((x.x, x.y), (100.0, 60.0));
    assert_eq!(x.x + x.w, 260.0);
    let Geom::Custom { paths } = &sh.geom else { panic!() };
    assert_eq!(paths.len(), 2, "one region and the strokes");
    // Points come back in slide points.
    let got = s.execute("vector.get", &json!({"id": v.0})).unwrap();
    assert_eq!(got["vertices"][3], json!([260.0, 60.0]));
    assert_eq!(got["segments"][3]["ta"], json!([20.0, 0.0]));
    // The filled triangle is drawn (default style fill), the space beside it is not.
    let inside = s.execute("slide.colorAt", &json!({"x": 150, "y": 120})).unwrap()["color"].clone();
    let outside = s.execute("slide.colorAt", &json!({"x": 600, "y": 400})).unwrap()["color"].clone();
    assert_ne!(inside, outside);
}

#[test]
fn moving_a_point_drags_every_segment_it_joins_and_the_box_follows() {
    let mut s = deck();
    let v = id(&s.execute("vector.create", &spur()).unwrap());
    let steps = s.doc().unwrap().history.undo.len();
    s.execute("vector.moveVertex", &json!({"index": 1, "x": 300, "y": 100})).unwrap();
    assert_eq!(s.doc().unwrap().history.undo.len(), steps + 1);
    let got = s.execute("vector.get", &json!({})).unwrap();
    assert_eq!(got["vertices"][1], json!([300.0, 100.0]));
    assert_eq!(got["vertices"][3], json!([260.0, 60.0]), "other points stay");
    let x = s.doc().unwrap().shape(v).unwrap().xfrm.unwrap();
    assert!(x.x + x.w >= 320.0 - 1e-6, "{x:?}");
    // A resized shape scales its network.
    s.execute("shape.resize", &json!({"w": x.w * 2.0})).unwrap();
    let got = s.execute("vector.get", &json!({})).unwrap();
    assert!(got["vertices"][1][0].as_f64().unwrap() > 300.0);
}

#[test]
fn pen_paths_and_presets_become_networks() {
    let mut s = deck();
    let r = id(&s.execute("shape.insert", &json!({"preset": "rect", "rect": [100, 100, 100, 50]})).unwrap());
    s.execute("vector.fromPath", &json!({})).unwrap();
    let got = s.execute("vector.get", &json!({"id": r.0})).unwrap();
    assert_eq!(got["vertices"].as_array().unwrap().len(), 4, "corners are shared: {got}");
    assert_eq!(got["segments"].as_array().unwrap().len(), 4);
    assert_eq!(got["regions"].as_array().unwrap().len(), 1);
    // A freeform drawn with the pen tool.
    let f = id(&s.execute("shape.freeform", &json!({"points": [[300, 300], [400, 320], [350, 400]], "smooth": true})).unwrap());
    s.execute("vector.fromPath", &json!({"ids": [f.0]})).unwrap();
    let got = s.execute("vector.get", &json!({"id": f.0})).unwrap();
    assert!(got["segments"].as_array().unwrap().len() >= 2, "{got}");
    assert!(got["regions"].as_array().unwrap().is_empty(), "an open pen path has no region");
}

#[test]
fn editing_the_outline_another_way_makes_it_a_plain_path() {
    let mut s = deck();
    let v = id(&s.execute("vector.create", &spur()).unwrap());
    s.execute("shape.change", &json!({"preset": "ellipse"})).unwrap();
    assert!(s.doc().unwrap().shape(v).unwrap().vector.is_none());
    assert!(s.execute("vector.get", &json!({"id": v.0})).is_err());
}

#[test]
fn vector_commands_refuse_hostile_input() {
    let mut s = deck();
    for p in [
        json!({}),
        json!({"vertices": []}),
        json!({"vertices": [[0, 0], [1]]}),
        json!({"vertices": [[0, 0], [1, 1]], "segments": [[0, 9]]}),
        json!({"vertices": [[0, 0], [1, 1]], "segments": [{"a": 0, "b": 1, "ta": ["x", 0]}]}),
        json!({"vertices": [[0, 0], [1, 1]], "segments": [[0, 1]], "regions": [{"loops": [[3]]}]}),
        json!({"vertices": [[0, 0], [1, 1]], "segments": [[0, 1]], "regions": [{}]}),
        json!({"vertices": "many"}),
    ] {
        assert!(s.execute("vector.create", &p).is_err(), "{p}");
    }
    let v = id(&s.execute("vector.create", &spur()).unwrap());
    assert!(s.execute("vector.moveVertex", &json!({"id": v.0, "index": 99, "x": 0, "y": 0})).is_err());
    assert!(s.execute("vector.moveVertex", &json!({"id": v.0, "index": 0, "x": "far", "y": 0})).is_err());
    assert!(s.execute("vector.set", &json!({"id": v.0, "segments": [[0, 77]]})).is_err());
    // A network survives saving and opening.
    let bytes = deckcraft_format::save(&s.doc().unwrap().doc).unwrap();
    let (p, _) = crate::cmd::file::open_checked("t.deckcraft", &bytes).unwrap();
    assert!(deckcraft_model::find_shape(&p.slides.last().unwrap().shapes, v).unwrap().vector.is_some());
}
