//! Paragraph and character styles (`style.*`): list, apply, define and delete. The model is
//! `deckcraft_model::text_style`; built-in styles use theme fonts and colours.

use deckcraft_model::text::{ParaProps, RunProps};
use deckcraft_model::text_style::{self, MAX_NAME, MAX_STYLES, TextStyle, TextStyleKind};
use serde_json::{Value, json};

use super::text::{format_para, format_run};
use super::{CommandSpec, always, bad, cmd, has_doc, has_text_or_shapes, str_param};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "style.list",
            "Styles",
            [],
            None,
            "{} → {styles: [{id, name, kind: paragraph|character, basedOn, builtin, changed}], paragraph: id of the caret paragraph's style or null, character: id of the caret run's style or null}",
            always,
            list
        ),
        cmd!(
            "style.apply",
            "Apply Style",
            ["Home", "Styles"],
            None,
            "{id: style id (a paragraph style formats the selected paragraphs, a character style the selected text) or null with kind to remove one, kind?: paragraph|character (needed with null), keep?: bool (keep direct formatting the style also sets; default false: it is removed so the style shows)}",
            has_text_or_shapes,
            apply
        ),
        cmd!(
            "style.define",
            "Define Style",
            ["Home", "Styles"],
            None,
            "{id: 1–64 of A–Z a–z 0–9 - _, name?, kind?: paragraph|character (new styles; default paragraph), basedOn?: id or null, para?: paragraph settings (as `ParaProps` in JSON: align, marginLeft, lineSpacing, spaceAfter, composer, hyphenate, opticalMargin…), run?: text settings (as `RunProps`: font ('+mj-lt' / '+mn-lt' for the theme fonts), size, bold, italic, fill…)} — creates a style or changes one (settings given replace those keys; null clears one); changing a built-in style changes it for this deck → the style",
            has_doc,
            define
        ),
        cmd!(
            "style.matchSelection",
            "Update Style to Match Selection",
            ["Home", "Styles"],
            None,
            "{id?: paragraph style id (default: the caret paragraph's)} — the caret paragraph's own paragraph settings and its first run's text settings become the style's, and that paragraph's own formatting is cleared, so every paragraph with the style looks like it → the style",
            has_text_or_shapes,
            match_selection
        ),
        cmd!(
            "style.delete",
            "Delete Style",
            ["Home", "Styles"],
            None,
            "{id} — removes a deck style; a built-in style returns to its original settings. Paragraphs and text that use a removed style keep its id and their own formatting",
            has_doc,
            delete
        ),
    ]
}

fn kind_param(cmd: &str, p: &Value) -> Result<Option<TextStyleKind>> {
    match p.get("kind") {
        None | Some(Value::Null) => Ok(None),
        Some(v) => match v.as_str() {
            Some("paragraph") => Ok(Some(TextStyleKind::Paragraph)),
            Some("character") => Ok(Some(TextStyleKind::Character)),
            _ => Err(bad(cmd, "`kind` must be paragraph or character")),
        },
    }
}

fn describe(s: &TextStyle) -> Value {
    let builtin = text_style::builtin().into_iter().find(|b| b.id == s.id);
    json!({
        "id": s.id,
        "name": s.name,
        "kind": s.kind.as_str(),
        "basedOn": s.based_on,
        "builtin": builtin.is_some(),
        "changed": builtin.is_some_and(|b| b != *s),
    })
}

/// The paragraph and run the caret (or the first selected shape's first paragraph) is in.
fn current(s: &Session) -> (Option<String>, Option<String>) {
    let Ok(st) = s.doc() else { return (None, None) };
    let para = match &st.selection.text {
        Some(t) => super::text::body_of(st, t).and_then(|b| b.paragraphs.get(t.caret.0).cloned()),
        None => st.selected_shapes().first().and_then(|sh| sh.text.as_ref()).and_then(|t| t.paragraphs.first().cloned()),
    };
    let Some(para) = para else { return (None, None) };
    let run_style = match &st.selection.text {
        Some(t) => {
            let mut at = 0usize;
            let caret = t.caret.1;
            para.runs.iter().find(|r| {
                let len = r.text.chars().count();
                let hit = caret < at + len.max(1);
                at += len;
                hit
            })
        }
        None => para.runs.first(),
    }
    .and_then(|r| r.props.style.clone());
    (para.props.style.clone(), run_style)
}

fn list(s: &mut Session, _p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let styles: Vec<Value> = st.doc.all_text_styles().iter().map(describe).collect();
    let (paragraph, character) = current(s);
    Ok(json!({"styles": styles, "paragraph": paragraph, "character": character}))
}

/// Settings of `own` that `style` also sets are removed, so the style shows through.
fn without_keys<T: serde::Serialize + serde::de::DeserializeOwned + Clone + Default + PartialEq>(own: &T, style: &T) -> T {
    let (Ok(Value::Object(mut a)), Ok(Value::Object(b))) = (serde_json::to_value(own), serde_json::to_value(style)) else { return own.clone() };
    a.retain(|k, v| v.is_null() || !b.get(k).is_some_and(|x| !x.is_null()));
    serde_json::from_value(Value::Object(a)).unwrap_or_else(|_| own.clone())
}

fn apply(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "style.apply";
    let keep = p.get("keep").and_then(Value::as_bool).unwrap_or(false);
    let id = match p.get("id") {
        Some(Value::Null) => None,
        Some(v) => Some(v.as_str().ok_or_else(|| bad(CMD, "`id` must be a style id or null"))?.to_string()),
        None => return Err(bad(CMD, "missing `id`")),
    };
    let asked = kind_param(CMD, p)?;
    let (kind, style) = match &id {
        Some(id) => {
            let st = s.doc()?.doc.resolved_text_style(id).ok_or_else(|| bad(CMD, format!("no style `{id}` (see style.list)")))?;
            if asked.is_some_and(|k| k != st.kind) {
                return Err(bad(CMD, format!("`{id}` is a {} style", st.kind.as_str())));
            }
            (st.kind, Some(st))
        }
        None => (asked.ok_or_else(|| bad(CMD, "removing a style needs `kind`"))?, None),
    };
    match kind {
        TextStyleKind::Paragraph => {
            let (sp, sr) = style.map(|x| (x.para, x.run)).unwrap_or_default();
            format_para(s, &move |para| {
                para.props.style.clone_from(&id);
                if !keep && id.is_some() {
                    let kept = para.props.style.clone();
                    para.props = without_keys(&para.props, &sp);
                    para.props.style = kept;
                    for r in &mut para.runs {
                        r.props = without_keys(&r.props, &sr);
                    }
                    para.end_props = without_keys(&para.end_props, &sr);
                }
            })?;
        }
        TextStyleKind::Character => {
            let sr = style.map(|x| x.run).unwrap_or_default();
            format_run(s, &move |r: &mut RunProps| {
                r.style.clone_from(&id);
                if !keep && id.is_some() {
                    let kept = r.style.clone();
                    *r = without_keys(r, &sr);
                    r.style = kept;
                }
            })?;
        }
    }
    list(s, &json!({}))
}

/// `value` merged over `base`: keys given replace those of `base`, `null` clears one.
fn merged<T: serde::Serialize + serde::de::DeserializeOwned>(cmd: &str, key: &str, base: &T, value: Option<&Value>) -> Result<T> {
    let Some(v) = value else {
        return serde_json::to_value(base).and_then(serde_json::from_value).map_err(|e| bad(cmd, e.to_string()));
    };
    let Value::Object(patch) = v else { return Err(bad(cmd, format!("`{key}` must be an object"))) };
    let Ok(Value::Object(mut out)) = serde_json::to_value(base) else { return Err(bad(cmd, format!("`{key}`: settings could not be read"))) };
    for (k, v) in patch {
        if v.is_null() {
            out.remove(k);
        } else {
            out.insert(k.clone(), v.clone());
        }
    }
    serde_json::from_value(Value::Object(out)).map_err(|e| bad(cmd, format!("`{key}`: {e}")))
}

fn define(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "style.define";
    let id = str_param(p, "id").ok_or_else(|| bad(CMD, "missing `id`"))?.to_string();
    if !text_style::valid_id(&id) {
        return Err(bad(CMD, "`id` must be 1–64 of A–Z a–z 0–9 - _"));
    }
    let doc = &s.doc()?.doc;
    let existing = doc.text_style(&id);
    let kind = kind_param(CMD, p)?;
    if let (Some(e), Some(k)) = (&existing, kind)
        && e.kind != k
    {
        return Err(bad(CMD, format!("`{id}` is a {} style; its kind cannot change", e.kind.as_str())));
    }
    if existing.as_ref().is_none_or(|_| !doc.text_styles.iter().any(|x| x.id == id)) && doc.text_styles.len() >= MAX_STYLES {
        return Err(bad(CMD, format!("a deck holds at most {MAX_STYLES} styles")));
    }
    let mut style = existing.unwrap_or_else(|| TextStyle { id: id.clone(), name: id.clone(), kind: kind.unwrap_or_default(), ..Default::default() });
    if let Some(name) = p.get("name") {
        let n = name.as_str().ok_or_else(|| bad(CMD, "`name` must be text"))?.trim();
        if n.is_empty() {
            return Err(bad(CMD, "`name` must not be empty"));
        }
        style.name = n.chars().take(MAX_NAME).collect();
    }
    match p.get("basedOn") {
        None => {}
        Some(Value::Null) => style.based_on = None,
        Some(v) => {
            let b = v.as_str().ok_or_else(|| bad(CMD, "`basedOn` must be a style id or null"))?;
            let parent = doc.text_style(b).ok_or_else(|| bad(CMD, format!("no style `{b}` to base on")))?;
            if b == id || parent.kind != style.kind {
                return Err(bad(CMD, format!("`{id}` cannot be based on `{b}`")));
            }
            style.based_on = Some(b.to_string());
        }
    }
    if style.kind == TextStyleKind::Character && p.get("para").is_some_and(|v| !v.is_null()) {
        return Err(bad(CMD, "a character style has no paragraph settings"));
    }
    let para: ParaProps = merged(CMD, "para", &style.para, p.get("para"))?;
    let run: RunProps = merged(CMD, "run", &style.run, p.get("run"))?;
    style.para = ParaProps { style: None, ..para };
    style.run = RunProps { style: None, ..run };
    let saved = style.clone();
    s.edit(move |doc, _| {
        match doc.text_styles.iter_mut().find(|x| x.id == saved.id) {
            Some(x) => *x = saved.clone(),
            None => doc.text_styles.push(saved.clone()),
        }
        doc.text_styles = text_style::sanitize(std::mem::take(&mut doc.text_styles));
        Ok(())
    })?;
    Ok(describe(&style))
}

fn match_selection(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "style.matchSelection";
    let para = {
        let st = s.doc()?;
        match &st.selection.text {
            Some(t) => super::text::body_of(st, t).and_then(|b| b.paragraphs.get(t.caret.0).cloned()),
            None => st.selected_shapes().first().and_then(|sh| sh.text.as_ref()).and_then(|t| t.paragraphs.first().cloned()),
        }
    }
    .ok_or_else(|| bad(CMD, "no paragraph is selected"))?;
    let id = match str_param(p, "id") {
        Some(id) => id.to_string(),
        None => para.props.style.clone().ok_or_else(|| bad(CMD, "the paragraph has no style; give `id`"))?,
    };
    let style = s.doc()?.doc.text_style(&id).ok_or_else(|| bad(CMD, format!("no style `{id}`")))?;
    if style.kind != TextStyleKind::Paragraph {
        return Err(bad(CMD, format!("`{id}` is a character style")));
    }
    let own_para = serde_json::to_value(ParaProps { style: None, ..para.props.clone() }).map_err(|e| bad(CMD, e.to_string()))?;
    let own_run =
        serde_json::to_value(RunProps { style: None, link: None, hover: None, ..para.runs.first().map(|r| r.props.clone()).unwrap_or_default() })
            .map_err(|e| bad(CMD, e.to_string()))?;
    let drop_nulls = |v: Value| match v {
        Value::Object(m) => Value::Object(m.into_iter().filter(|(_, v)| !v.is_null()).collect()),
        v => v,
    };
    let out = define(s, &json!({"id": id, "para": drop_nulls(own_para), "run": drop_nulls(own_run)}))?;
    // The paragraph now shows its style rather than its own formatting.
    apply(s, &json!({"id": id}))?;
    Ok(out)
}

fn delete(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "style.delete";
    let id = str_param(p, "id").ok_or_else(|| bad(CMD, "missing `id`"))?.to_string();
    if !s.doc()?.doc.text_styles.iter().any(|x| x.id == id) {
        let why = if text_style::is_builtin(&id) { "has its original settings already" } else { "is not a style of this deck" };
        return Err(bad(CMD, format!("`{id}` {why}")));
    }
    s.edit(move |doc, _| {
        doc.text_styles.retain(|x| x.id != id);
        Ok(())
    })?;
    list(s, &json!({}))
}

#[cfg(test)]
mod tests {
    use deckcraft_model::text::Composer;
    use serde_json::json;

    use crate::Session;

    fn shape() -> Session {
        let mut s = Session::with_new();
        s.execute("shape.insert", &json!({"preset": "rect", "rect": [40.0, 40.0, 400.0, 200.0], "text": "42%"})).expect("insert");
        s
    }

    fn para(s: &Session) -> deckcraft_model::text::Paragraph {
        s.doc()
            .expect("doc")
            .selected_shapes()
            .first()
            .and_then(|sh| sh.text.as_ref())
            .and_then(|t| t.paragraphs.first().cloned())
            .expect("paragraph")
    }

    #[test]
    fn lists_the_builtin_styles() {
        let mut s = shape();
        let r = s.execute("style.list", &json!({})).expect("list");
        let ids: Vec<&str> = r["styles"].as_array().expect("styles").iter().filter_map(|x| x["id"].as_str()).collect();
        for id in ["title", "subtitle", "heading", "body", "caption", "quote", "bigNumber", "strong", "emphasis", "accent"] {
            assert!(ids.contains(&id), "{id}");
        }
        assert_eq!(r["paragraph"], json!(null));
    }

    #[test]
    fn applying_a_paragraph_style_restyles_the_text_and_undoes_in_one_step() {
        let mut s = shape();
        s.execute("format.size", &json!({"size": 12})).expect("size");
        assert!(para(&s).runs.iter().all(|r| r.props.size == Some(12.0)), "a direct size to begin with");
        let before = s.doc().expect("doc").history.undo.len();
        let r = s.execute("style.apply", &json!({"id": "bigNumber"})).expect("apply");
        assert_eq!(r["paragraph"], json!("bigNumber"));
        assert_eq!(s.doc().expect("doc").history.undo.len(), before + 1);
        let p = para(&s);
        assert_eq!(p.props.style.as_deref(), Some("bigNumber"));
        assert!(p.runs.iter().all(|r| r.props.size.is_none()), "direct sizes give way to the style");
        // Resolution gives the style's size and theme colour.
        let st = s.doc().expect("doc");
        let slide = st.doc.slides.first().expect("slide");
        let sh = st.selected_shapes().first().map(|x| (*x).clone()).expect("shape");
        let ctx = deckcraft_model::resolve::Ctx::for_slide(&st.doc, slide).expect("ctx");
        let rp = deckcraft_model::resolve::run(&ctx, &sh, &p, &p.runs[0].props);
        assert_eq!(rp.size, Some(96.0));
        assert_eq!(rp.bold, Some(true));
        s.execute("edit.undo", &json!({})).expect("undo");
        assert_eq!(para(&s).props.style, None);
    }

    #[test]
    fn defining_a_style_changes_every_paragraph_that_uses_it() {
        let mut s = shape();
        s.execute("style.apply", &json!({"id": "body"})).expect("apply");
        let r = s.execute("style.define", &json!({"id": "body", "run": {"size": 26.0}, "para": {"composer": "singleLine"}})).expect("define");
        assert_eq!(r["changed"], json!(true));
        let doc = &s.doc().expect("doc").doc;
        let body = doc.resolved_text_style("body").expect("body");
        assert_eq!(body.run.size, Some(26.0));
        assert_eq!(body.para.composer, Some(Composer::SingleLine));
        assert_eq!(body.run.font.as_deref(), Some("+mn-lt"), "unchanged settings stay");
        // A new style based on another.
        s.execute("style.define", &json!({"id": "lead", "name": "Lead", "basedOn": "body", "run": {"italic": true}})).expect("new");
        let lead = s.doc().expect("doc").doc.resolved_text_style("lead").expect("lead");
        assert_eq!((lead.run.size, lead.run.italic), (Some(26.0), Some(true)));
        // Delete returns the built-in and removes the custom one.
        s.execute("style.delete", &json!({"id": "body"})).expect("reset body");
        assert_eq!(s.doc().expect("doc").doc.resolved_text_style("body").and_then(|b| b.run.size), Some(20.0));
        s.execute("style.delete", &json!({"id": "lead"})).expect("delete lead");
        assert!(s.doc().expect("doc").doc.resolved_text_style("lead").is_none());
    }

    #[test]
    fn character_styles_format_the_selected_text() {
        let mut s = shape();
        s.execute("style.apply", &json!({"id": "accent"})).expect("apply");
        assert!(para(&s).runs.iter().all(|r| r.props.style.as_deref() == Some("accent")));
        s.execute("style.apply", &json!({"id": null, "kind": "character"})).expect("clear");
        assert!(para(&s).runs.iter().all(|r| r.props.style.is_none()));
    }

    #[test]
    fn bad_requests_change_nothing() {
        let mut s = shape();
        let before = s.doc().expect("doc").doc.clone();
        for (cmd, p) in [
            ("style.apply", json!({"id": "nope"})),
            ("style.apply", json!({"id": "accent", "kind": "paragraph"})),
            ("style.apply", json!({"id": null})),
            ("style.apply", json!({})),
            ("style.define", json!({"id": "bad id"})),
            ("style.define", json!({"id": "title", "kind": "character"})),
            ("style.define", json!({"id": "x", "basedOn": "x"})),
            ("style.define", json!({"id": "x", "basedOn": "accent"})),
            ("style.define", json!({"id": "x", "run": {"size": "big"}})),
            ("style.define", json!({"id": "x", "run": 3})),
            ("style.define", json!({"id": "strong", "para": {"align": "center"}})),
            ("style.define", json!({"id": "x", "name": ""})),
            ("style.delete", json!({"id": "title"})),
            ("style.delete", json!({"id": "missing"})),
        ] {
            assert!(s.execute(cmd, &p).is_err(), "{cmd} {p}");
        }
        assert_eq!(s.doc().expect("doc").doc, before);
    }

    #[test]
    fn a_style_takes_the_selected_paragraphs_formatting() {
        let mut s = shape();
        s.execute("style.apply", &json!({"id": "heading"})).expect("apply");
        s.execute("format.size", &json!({"size": 40})).expect("size");
        s.execute("format.alignCenter", &json!({})).expect("centre");
        let before = s.doc().expect("doc").history.undo.len();
        let r = s.execute("style.matchSelection", &json!({})).expect("match");
        assert_eq!(s.doc().expect("doc").history.undo.len(), before + 1, "one undo step");
        assert_eq!(r["id"], json!("heading"));
        let doc = &s.doc().expect("doc").doc;
        let h = doc.resolved_text_style("heading").expect("heading");
        assert_eq!((h.run.size, h.para.align), (Some(40.0), Some(deckcraft_model::text::Align::Center)));
        let p = para(&s);
        assert!(p.runs.iter().all(|r| r.props.size.is_none()) && p.props.align.is_none(), "the paragraph's own formatting moved into the style");
        assert_eq!(p.props.style.as_deref(), Some("heading"));
    }
}
