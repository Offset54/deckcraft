//! `table.columnFormat` and `table.headerStyle`.

use serde_json::json;

use crate::Session;

fn table() -> (Session, u64) {
    let mut s = Session::with_new();
    s.execute("slide.new", &json!({"layout": "blank"})).expect("slide");
    let id = s.execute("insert.table", &json!({"rows": 4, "cols": 3})).expect("table")["id"].as_u64().expect("id");
    s.execute("edit.select", &json!({"ids": [id]})).expect("select");
    (s, id)
}

fn tbl(s: &Session, id: u64) -> deckcraft_model::Table {
    let st = s.doc().expect("doc");
    let sh = st.current_slide().and_then(|sl| sl.shapes.iter().find(|x| u64::from(x.id.0) == id)).cloned().expect("shape");
    match sh.kind {
        deckcraft_model::ShapeKind::Table(t) => t,
        _ => panic!("not a table"),
    }
}

#[test]
fn column_format_sets_alignment_decimal_and_style_in_one_undo_step() {
    let (mut s, id) = table();
    let before = s.doc().expect("doc").history.undo.len();
    let r = s.execute("table.columnFormat", &json!({"col": 2, "decimal": true, "style": "caption"})).expect("format");
    assert_eq!(r, json!({"align": null, "decimal": true, "style": "caption"}));
    assert_eq!(s.doc().expect("doc").history.undo.len(), before + 1);
    let t = tbl(&s, id);
    assert_eq!(t.col_formats.len(), 3);
    assert!(t.col_formats[2].decimal);
    s.execute("table.columnFormat", &json!({"col": 2, "decimal": false, "style": null})).expect("clear");
    assert!(tbl(&s, id).col_formats.is_empty(), "an empty format is not kept");
    s.execute("table.headerStyle", &json!({"style": "heading"})).expect("header");
    assert_eq!(tbl(&s, id).header_style.as_deref(), Some("heading"));
}

#[test]
fn column_format_refuses_bad_requests() {
    let (mut s, id) = table();
    for p in [
        json!({"col": 9, "decimal": true}),
        json!({"col": 0}),
        json!({"col": 0, "align": "middle"}),
        json!({"col": 0, "style": "accent"}),
        json!({"col": 0, "style": "nope"}),
        json!({"col": 0, "decimal": "yes"}),
    ] {
        assert!(s.execute("table.columnFormat", &p).is_err(), "{p}");
    }
    assert!(s.execute("table.headerStyle", &json!({})).is_err());
    assert!(tbl(&s, id).col_formats.is_empty());
}
