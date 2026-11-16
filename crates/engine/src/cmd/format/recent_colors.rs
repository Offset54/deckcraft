//! Recent Colors: `format.recentColor` remembers a custom colour with the presentation and
//! `format.recentColors` lists them, newest first. Colour menus show the list as a row.

use serde_json::{Value, json};

use crate::cmd::bad;
use crate::{Result, Session};

/// The colour to remember: `#RRGGBB` (or `RRGGBB`). Theme and named colours are not recent
/// colours: they are already in every menu.
fn literal(p: &Value) -> Result<deckcraft_model::Rgba> {
    let refuse = || bad("format.recentColor", "`color` must be #RRGGBB");
    let text = p.get("color").and_then(Value::as_str).ok_or_else(refuse)?;
    let hex = text.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return Err(refuse());
    }
    deckcraft_model::Rgba::from_hex(hex).ok_or_else(refuse)
}

pub(super) fn recent_color(s: &mut Session, p: &Value) -> Result<Value> {
    let color = literal(p)?;
    // Already the newest: the presentation is left untouched (and so not marked as changed).
    if s.doc()?.doc.recent_colors.first() != Some(&color) {
        s.edit(|doc, _| {
            doc.note_recent_color(color);
            Ok(())
        })?;
    }
    recent_colors(s, p)
}

pub(super) fn recent_colors(s: &mut Session, _p: &Value) -> Result<Value> {
    let colors: Vec<String> = s.doc()?.doc.recent_colors.iter().map(|c| format!("#{}", c.hex())).collect();
    Ok(json!({"colors": colors}))
}

#[cfg(test)]
mod patch_tests {
    use serde_json::json;

    use crate::Session;

    fn listed(s: &mut Session) -> Vec<String> {
        let r = s.execute("format.recentColors", &json!({})).unwrap();
        r["colors"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect()
    }

    #[test]
    fn recent_colours_are_listed_newest_first_and_capped_at_ten() {
        let mut s = Session::with_new();
        assert!(listed(&mut s).is_empty());
        s.execute("format.recentColor", &json!({"color": "#112233"})).unwrap();
        let r = s.execute("format.recentColor", &json!({"color": "abcdef"})).unwrap();
        assert_eq!(r["colors"], json!(["#ABCDEF", "#112233"]));
        s.execute("format.recentColor", &json!({"color": "#112233"})).unwrap();
        assert_eq!(listed(&mut s), ["#112233", "#ABCDEF"]);
        for k in 0..25 {
            s.execute("format.recentColor", &json!({"color": format!("#0000{k:02X}")})).unwrap();
        }
        let all = listed(&mut s);
        assert_eq!(all.len(), 10);
        assert_eq!(all.first().map(String::as_str), Some("#000018"));
    }

    /// Remembering a colour is not something to undo: Undo after a fill takes back the fill.
    #[test]
    fn remembering_a_colour_adds_no_undo_step() {
        let mut s = Session::with_new();
        let steps = s.doc().unwrap().history.undo.len();
        s.execute("format.recentColor", &json!({"color": "#112233"})).unwrap();
        assert_eq!(s.doc().unwrap().history.undo.len(), steps);
        // The newest again leaves the presentation exactly as it was.
        let rev = s.doc().unwrap().revision;
        s.execute("format.recentColor", &json!({"color": "#112233"})).unwrap();
        assert_eq!(s.doc().unwrap().revision, rev);
    }

    #[test]
    fn only_literal_colours_are_taken() {
        let mut s = Session::with_new();
        for bad in [json!("accent1"), json!("red"), json!("#12345"), json!("#1234567"), json!("#12345G"), json!(""), json!(7), json!(null)] {
            assert!(s.execute("format.recentColor", &json!({"color": bad})).is_err(), "{bad}");
        }
        assert!(s.execute("format.recentColor", &json!({})).is_err());
        assert!(listed(&mut s).is_empty());
    }

    #[test]
    fn recent_colours_are_saved_with_the_presentation() {
        let mut s = Session::with_new();
        s.execute("format.recentColor", &json!({"color": "#112233"})).unwrap();
        s.execute("format.recentColor", &json!({"color": "#445566"})).unwrap();
        let doc = &s.doc().unwrap().doc;
        let back = deckcraft_pptx::import(&deckcraft_pptx::export(doc).unwrap()).unwrap();
        assert_eq!(back.recent_colors, doc.recent_colors);
        assert_eq!(back.recent_colors.len(), 2);
    }
}
