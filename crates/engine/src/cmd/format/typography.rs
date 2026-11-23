//! `format.typography`: SlideCraft's paragraph composer, hyphenation and optical margins.

use deckcraft_model::text::{Composer, ParaProps};

use super::{Result, Session, Value, bad, current_para, format_para, json};

const CMD: &str = "format.typography";

/// A key that sets a value, clears it back to inherited (`null`), or is absent.
fn tri<T>(p: &Value, key: &str, parse: impl Fn(&Value) -> Option<T>, what: &str) -> Result<Option<Option<T>>> {
    match p.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(v) => parse(v).map(|x| Some(Some(x))).ok_or_else(|| bad(CMD, format!("`{key}` must be {what} or null"))),
    }
}

fn state(pp: &ParaProps) -> Value {
    json!({
        "composer": pp.composer.unwrap_or(Composer::SingleLine).as_str(),
        "hyphenate": pp.hyphenate.unwrap_or(false),
        "opticalMargin": pp.optical_margin.unwrap_or(false),
        "snapToGrid": pp.snap_to_grid.unwrap_or(false),
    })
}

pub(super) fn typography(s: &mut Session, p: &Value) -> Result<Value> {
    let composer = tri(p, "composer", |v| v.as_str().and_then(Composer::parse), "singleLine or paragraph")?;
    let hyphenate = tri(p, "hyphenate", Value::as_bool, "a bool")?;
    let optical = tri(p, "opticalMargin", Value::as_bool, "a bool")?;
    let snap = tri(p, "snapToGrid", Value::as_bool, "a bool")?;
    if composer.is_none() && hyphenate.is_none() && optical.is_none() && snap.is_none() {
        // Nothing to change: report the paragraph's effective settings.
        return Ok(state(&current_para(s)));
    }
    format_para(s, &move |para| {
        if let Some(c) = composer {
            para.props.composer = c;
        }
        if let Some(h) = hyphenate {
            para.props.hyphenate = h;
        }
        if let Some(o) = optical {
            para.props.optical_margin = o;
        }
        if let Some(g) = snap {
            para.props.snap_to_grid = g;
        }
    })?;
    Ok(state(&current_para(s)))
}

#[cfg(test)]
mod tests {
    use deckcraft_model::text::Composer;
    use serde_json::json;

    use crate::Session;

    fn session_with_body_selected() -> Session {
        let mut s = Session::with_new();
        s.execute("shape.insert", &json!({"preset": "rect", "rect": [40.0, 40.0, 240.0, 80.0], "text": "Some words to compose"})).expect("insert");
        s
    }

    fn props(s: &Session) -> deckcraft_model::text::ParaProps {
        let st = s.doc().expect("doc");
        st.selected_shapes().first().and_then(|sh| sh.text.as_ref()).and_then(|t| t.paragraphs.first()).map(|p| p.props.clone()).expect("a paragraph")
    }

    #[test]
    fn sets_reports_clears_and_undoes_typography() {
        let mut s = session_with_body_selected();
        let r = s.execute("format.typography", &json!({"composer": "paragraph", "hyphenate": true, "opticalMargin": true})).expect("set");
        assert_eq!(r, json!({"composer": "paragraph", "hyphenate": true, "opticalMargin": true, "snapToGrid": false}));
        let pp = props(&s);
        assert_eq!((pp.composer, pp.hyphenate, pp.optical_margin), (Some(Composer::Paragraph), Some(true), Some(true)));
        // With no keys it is a query.
        assert_eq!(s.execute("format.typography", &json!({})).expect("query"), r);
        // null returns a setting to what the paragraph inherits.
        s.execute("format.typography", &json!({"hyphenate": null})).expect("clear");
        assert_eq!(props(&s).hyphenate, None);
        s.execute("edit.undo", &json!({})).expect("undo");
        assert_eq!(props(&s).hyphenate, Some(true));
    }

    #[test]
    fn rejects_bad_values_without_changing_the_deck() {
        let mut s = session_with_body_selected();
        let before = props(&s);
        for bad in [json!({"composer": "fancy"}), json!({"hyphenate": "yes"}), json!({"opticalMargin": 1}), json!({"composer": 3})] {
            assert!(s.execute("format.typography", &bad).is_err(), "{bad}");
        }
        assert_eq!(props(&s), before);
    }
}
