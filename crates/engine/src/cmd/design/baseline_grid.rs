//! `design.baselineGrid`: the slide or master baseline grid that paragraphs with `snapToGrid`
//! (`format.typography`) align their lines to.

use std::sync::Arc;

use deckcraft_model::BaselineGrid;
use serde_json::{Value, json};

use super::bad;
use crate::{Result, Session};

const CMD: &str = "design.baselineGrid";

fn describe(g: Option<BaselineGrid>) -> Value {
    match g.and_then(|g| g.active()) {
        Some((increment, offset)) => json!({"on": true, "increment": increment, "offset": offset}),
        None => json!({"on": false}),
    }
}

/// The current slide's own, layout's and master's grids, and the one in force.
fn report(s: &Session) -> Result<Value> {
    let st = s.doc()?;
    let slide = st.doc.slides.get(st.selection.slide).ok_or_else(|| bad(CMD, "no slide"))?;
    let (master, layout) = st.doc.master_for(slide).ok_or_else(|| bad(CMD, "no master"))?;
    let effective = slide.baseline_grid.or(layout.and_then(|l| l.baseline_grid)).or(master.baseline_grid);
    Ok(json!({
        "slide": describe(slide.baseline_grid),
        "slideSet": slide.baseline_grid.is_some(),
        "master": describe(master.baseline_grid),
        "effective": describe(effective),
    }))
}

pub(super) fn baseline_grid(s: &mut Session, p: &Value) -> Result<Value> {
    let scope = match p.get("scope").and_then(Value::as_str) {
        None | Some("slide") => "slide",
        Some("master") => "master",
        Some(_) => return Err(bad(CMD, "`scope` must be slide or master")),
    };
    let clear = p.get("clear").and_then(Value::as_bool).unwrap_or(false);
    let num = |k: &str| -> Result<Option<f64>> {
        match p.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v.as_f64().filter(|x| x.is_finite()).map(Some).ok_or_else(|| bad(CMD, format!("`{k}` must be a number of points"))),
        }
    };
    let (increment, offset) = (num("increment")?, num("offset")?);
    if let Some(i) = increment
        && i != 0.0
        && !(BaselineGrid::MIN..=BaselineGrid::MAX).contains(&i)
    {
        return Err(bad(CMD, format!("`increment` must be 0 (off) or {}–{} pt", BaselineGrid::MIN, BaselineGrid::MAX)));
    }
    if let Some(o) = offset
        && !(0.0..=10_000.0).contains(&o)
    {
        return Err(bad(CMD, "`offset` must be 0–10000 pt"));
    }
    if !clear && increment.is_none() && offset.is_none() {
        return report(s);
    }
    let si = s.doc()?.selection.slide;
    s.edit(move |doc, _| {
        let Some(slide) = doc.slides.get(si).cloned() else { return Err(bad(CMD, "no slide")) };
        let change = |g: &mut Option<BaselineGrid>| {
            if clear {
                *g = None;
                return;
            }
            let mut v = g.unwrap_or(BaselineGrid { increment: 0.0, offset: 0.0 });
            if let Some(i) = increment {
                v.increment = i;
            }
            if let Some(o) = offset {
                v.offset = o;
            }
            *g = Some(v);
        };
        if scope == "slide" {
            if let Some(sl) = doc.slides.get_mut(si) {
                change(&mut Arc::make_mut(sl).baseline_grid);
            }
        } else {
            let mid = doc.master_for(&slide).map(|(m, _)| m.id).ok_or_else(|| bad(CMD, "no master"))?;
            if let Some(m) = doc.masters.iter_mut().find(|m| m.id == mid) {
                change(&mut Arc::make_mut(m).baseline_grid);
            }
        }
        Ok(())
    })?;
    report(s)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    #[test]
    fn sets_inherits_overrides_and_clears_the_grid() {
        let mut s = Session::with_new();
        assert_eq!(s.execute("design.baselineGrid", &json!({})).expect("query")["effective"], json!({"on": false}));
        s.execute("design.baselineGrid", &json!({"scope": "master", "increment": 24, "offset": 60})).expect("master");
        let r = s.execute("design.baselineGrid", &json!({})).expect("query");
        assert_eq!(r["effective"], json!({"on": true, "increment": 24.0, "offset": 12.0}), "the offset is kept within one increment");
        // A slide's own grid wins; increment 0 turns it off for that slide only.
        let r = s.execute("design.baselineGrid", &json!({"increment": 0})).expect("slide off");
        assert_eq!((r["effective"].clone(), r["master"]["on"].clone()), (json!({"on": false}), json!(true)));
        let r = s.execute("design.baselineGrid", &json!({"clear": true})).expect("clear");
        assert_eq!(r["effective"]["increment"], json!(24.0), "cleared, the slide follows its master again");
        s.execute("edit.undo", &json!({})).expect("undo");
        assert_eq!(s.execute("design.baselineGrid", &json!({})).expect("query")["effective"], json!({"on": false}));
    }

    #[test]
    fn refuses_out_of_range_values() {
        let mut s = Session::with_new();
        for bad in [
            json!({"increment": 1}),
            json!({"increment": 5000}),
            json!({"increment": "x"}),
            json!({"offset": -1}),
            json!({"scope": "deck", "increment": 20}),
        ] {
            assert!(s.execute("design.baselineGrid", &bad).is_err(), "{bad}");
        }
    }
}
