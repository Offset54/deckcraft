//! Slide Size › Number slides from: the number the first slide shows (`p:presentation/@firstSlideNum`).
//! `design.slideSize {firstNumber}` sets it, alone or together with a size.

use serde_json::{Value, json};

use super::bad;
use crate::{Result, Session};

/// Largest first slide number, the same cap the PPTX reader and writer apply.
pub(super) const MAX_FIRST_NUMBER: u32 = 9999;

/// `firstNumber` as given: a whole number 0..=9999. Anything else is refused rather than
/// clamped, so a typo does not silently renumber the deck.
pub(super) fn first_number_param(p: &Value) -> Result<Option<u32>> {
    match p.get("firstNumber") {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n <= MAX_FIRST_NUMBER)
            .map(Some)
            .ok_or_else(|| bad("design.slideSize", format!("`firstNumber` must be a whole number from 0 to {MAX_FIRST_NUMBER}"))),
    }
}

/// Is `firstNumber` the only thing asked for (no preset and no size)?
pub(super) fn only_first_number(p: &Value) -> bool {
    !["preset", "w", "h"].iter().any(|k| p.get(k).is_some_and(|v| !v.is_null()))
}

/// Renumber the slides from `first`, leaving the slide size alone. One undo step.
pub(super) fn set_first_number(s: &mut Session, first: u32) -> Result<Value> {
    s.edit(|doc, _| {
        doc.first_slide_number = first;
        Ok(())
    })?;
    let doc = &s.doc()?.doc;
    Ok(json!({"w": doc.slide_size.width, "h": doc.slide_size.height, "firstNumber": doc.first_slide_number}))
}

#[cfg(test)]
mod patch_tests {
    use serde_json::json;

    use crate::Session;

    fn first(s: &Session) -> u32 {
        s.doc().unwrap().doc.first_slide_number
    }

    #[test]
    fn first_number_alone_renumbers_and_keeps_the_size() {
        let mut s = Session::with_new();
        let size = s.doc().unwrap().doc.slide_size;
        let r = s.execute("design.slideSize", &json!({"firstNumber": 5})).unwrap();
        assert_eq!(first(&s), 5);
        assert_eq!(r["firstNumber"], 5);
        assert_eq!(s.doc().unwrap().doc.slide_size, size);
        assert_eq!(s.doc().unwrap().doc.slide_number(0), 5);
        assert_eq!(s.doc().unwrap().doc.slide_number(2), 7);
        // One undo step back to the default.
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(first(&s), 1);
    }

    #[test]
    fn first_number_rides_along_with_a_size_and_zero_is_allowed() {
        let mut s = Session::with_new();
        s.execute("design.slideSize", &json!({"preset": "standard", "firstNumber": 0})).unwrap();
        assert_eq!(first(&s), 0);
        assert_eq!(s.doc().unwrap().doc.slide_number(0), 0);
        // A size without firstNumber leaves the numbering as it is.
        s.execute("design.slideSize", &json!({"preset": "widescreen"})).unwrap();
        assert_eq!(first(&s), 0);
        s.execute("design.slideSize", &json!({"w": 720.0, "h": 540.0, "firstNumber": 9999})).unwrap();
        assert_eq!(first(&s), 9999);
    }

    #[test]
    fn a_bad_first_number_is_refused_and_changes_nothing() {
        let mut s = Session::with_new();
        for bad in [json!(-1), json!(10000), json!(2.5), json!("3"), json!(true), json!(1e300), json!([1])] {
            assert!(s.execute("design.slideSize", &json!({"firstNumber": bad})).is_err(), "{bad}");
            assert!(s.execute("design.slideSize", &json!({"preset": "standard", "firstNumber": bad})).is_err(), "{bad}");
        }
        assert_eq!(first(&s), 1);
        assert_eq!(s.doc().unwrap().doc.slide_size, deckcraft_model::defaults::WIDE);
        // Neither a size nor a number is still an error, as before.
        assert!(s.execute("design.slideSize", &json!({})).is_err());
    }
}
