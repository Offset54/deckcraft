//! Font dialog › Character Spacing › Kerning for fonts: `format.fontDialog {kern}`. The run's
//! `kern` is the smallest size (points) whose letter pairs are kerned, and 0 turns kerning off
//! (`a:rPr/@kern`, hundredths of a point in the file).

use serde_json::Value;

use crate::Result;
use crate::cmd::bad;

/// Largest kerning threshold, the largest font size a run can have.
pub(super) const MAX_KERN: f64 = 4000.0;

/// `kern` as given: a finite number of points, kept within 0..=4000. Not a number is refused.
pub(super) fn kern_param(p: &Value) -> Result<Option<f64>> {
    match p.get("kern") {
        None => Ok(None),
        Some(v) => {
            let n = v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad("format.fontDialog", "`kern` must be a finite number of points"))?;
            Ok(Some(n.clamp(0.0, MAX_KERN)))
        }
    }
}

#[cfg(test)]
mod patch_tests {
    use serde_json::json;

    use crate::Session;

    fn session() -> Session {
        let mut s = Session::with_new();
        s.execute("shape.insert", &json!({"preset": "rect", "rect": [80.0, 80.0, 320.0, 80.0], "text": "AVATAR"})).unwrap();
        let _ = s.execute("text.exit", &json!({}));
        s
    }

    fn kerns(doc: &deckcraft_model::Presentation) -> Vec<Option<f64>> {
        doc.slides
            .iter()
            .flat_map(|sl| sl.shapes.iter())
            .filter(|sh| sh.text.as_ref().is_some_and(|t| t.text() == "AVATAR"))
            .flat_map(|sh| sh.text.iter())
            .flat_map(|t| t.paragraphs.iter())
            .flat_map(|p| p.runs.iter())
            .map(|r| r.props.kern)
            .collect()
    }

    #[test]
    fn kern_sets_the_threshold_in_one_undo_step() {
        let mut s = session();
        s.execute("format.fontDialog", &json!({"kern": 14})).unwrap();
        assert_eq!(kerns(&s.doc().unwrap().doc), vec![Some(14.0)]);
        // Off is 0, not "unset": the run then stops inheriting a threshold.
        s.execute("format.fontDialog", &json!({"kern": 0})).unwrap();
        assert_eq!(kerns(&s.doc().unwrap().doc), vec![Some(0.0)]);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(kerns(&s.doc().unwrap().doc), vec![Some(14.0)]);
        // Out of range is brought into range; with other fields it is still one step.
        s.execute("format.fontDialog", &json!({"kern": 1e9, "bold": true})).unwrap();
        assert_eq!(kerns(&s.doc().unwrap().doc), vec![Some(4000.0)]);
        s.execute("format.fontDialog", &json!({"kern": -3})).unwrap();
        assert_eq!(kerns(&s.doc().unwrap().doc), vec![Some(0.0)]);
    }

    #[test]
    fn a_kern_that_is_not_a_number_is_refused() {
        let mut s = session();
        for bad in [json!("12"), json!(true), json!(null), json!([12])] {
            assert!(s.execute("format.fontDialog", &json!({"kern": bad})).is_err(), "{bad}");
        }
        assert_eq!(kerns(&s.doc().unwrap().doc), vec![None]);
    }

    #[test]
    fn kern_survives_a_pptx_round_trip() {
        let mut s = session();
        s.execute("format.fontDialog", &json!({"kern": 13.5})).unwrap();
        let back = deckcraft_pptx::import(&deckcraft_pptx::export(&s.doc().unwrap().doc).unwrap()).unwrap();
        assert_eq!(kerns(&back), vec![Some(13.5)]);
        s.execute("format.fontDialog", &json!({"kern": 0})).unwrap();
        let back = deckcraft_pptx::import(&deckcraft_pptx::export(&s.doc().unwrap().doc).unwrap()).unwrap();
        assert_eq!(kerns(&back), vec![Some(0.0)]);
    }

    /// The Font dialog opens showing the kerning the text has.
    #[test]
    fn the_font_dialog_is_seeded_with_the_kern() {
        let mut s = session();
        s.execute("format.fontDialog", &json!({"kern": 16})).unwrap();
        s.ui_requests.clear();
        s.execute("format.fontDialog", &json!({})).unwrap();
        let shown = s.ui_requests.iter().find_map(|r| match r {
            crate::UiRequest::Dialog { id, params } if id == "font" => Some(params.clone()),
            _ => None,
        });
        assert_eq!(shown.unwrap()["kern"], 16.0);
    }
}
