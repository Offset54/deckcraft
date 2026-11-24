//! Display P3 colours (SlideCraft, Phase 3). `format.p3` gives a shape a wide-gamut fill or
//! outline colour. The deck keeps the Display P3 original (`Shape::p3`) and the shape's fill or
//! outline becomes the nearest sRGB colour, which is what this app draws (its renderer and
//! window are sRGB) and what PowerPoint gets. The conversion is `craft-color-transform`'s
//! (Display P3 primaries to sRGB, both D65), with out-of-gamut colours clipped per channel.

use deckcraft_color::Rgba;
use deckcraft_model::canvas::P3Colors;
use deckcraft_model::style::{Fill, Line};
use deckcraft_model::{ColorRef, Shape, ShapeKind};
use serde_json::{Value, json};

use super::{CommandSpec, bad, cmd, has_selection, ids_param};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "format.p3",
        "Display P3 Colour",
        ["Shape Format", "Shape Styles"],
        None,
        "{ids?, fill?: [r,g,b]|\"color(display-p3 r g b)\"|null (components 0–1), line?: (the same)} → {ids, fill?: {srgb: hex, inGamut}, line?: {srgb: hex, inGamut}}: a wide-gamut colour, kept in the deck; drawn and saved to .pptx as the nearest sRGB colour; null forgets the P3 original",
        has_selection,
        set_p3
    )]
}

/// Encoded Display P3 to encoded sRGB, and whether it was inside sRGB's gamut.
pub fn p3_to_srgb(c: [f64; 3]) -> ([f64; 3], bool) {
    use craft_color_transform::{DISPLAY_P3, SRGB};
    let lin = c.map(|v| f64::from(craft_color::srgb_to_linear(v.clamp(0.0, 1.0) as f32)));
    let out = DISPLAY_P3.to_space(&SRGB).apply(lin);
    let inside = out.iter().all(|v| (-1e-4..=1.0 + 1e-4).contains(v));
    (out.map(|v| f64::from(craft_color::linear_to_srgb(if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 } as f32))), inside)
}

/// The sRGB colour a P3 colour is drawn with.
pub fn p3_rgba(c: [f64; 3]) -> Rgba {
    let (s, _) = p3_to_srgb(c);
    let b = |v: f64| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgba::rgb(b(s[0]), b(s[1]), b(s[2]))
}

fn parse(v: &Value) -> Option<[f64; 3]> {
    let ok = |n: f64| (n.is_finite() && (0.0..=1.0).contains(&n)).then_some(n);
    match v {
        Value::Array(a) if a.len() == 3 => {
            let n: Vec<f64> = a.iter().filter_map(Value::as_f64).filter_map(ok).collect();
            <[f64; 3]>::try_from(n).ok()
        }
        Value::String(s) => {
            let inner = s.trim().strip_prefix("color(")?.strip_suffix(')')?.trim();
            let rest = inner.strip_prefix("display-p3")?;
            let n: Vec<f64> = rest.split_whitespace().map(|t| t.parse::<f64>().ok().and_then(ok)).collect::<Option<Vec<_>>>()?;
            <[f64; 3]>::try_from(n).ok()
        }
        _ => None,
    }
}

/// `Some(None)` forgets, `Some(Some(c))` sets, `None` leaves alone.
fn param(p: &Value, key: &str) -> Result<Option<Option<[f64; 3]>>> {
    match p.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(v) => parse(v)
            .map(|c| Some(Some(c)))
            .ok_or_else(|| bad("format.p3", format!("`{key}` must be [r, g, b] from 0 to 1 or \"color(display-p3 r g b)\""))),
    }
}

fn solid(c: Rgba) -> Fill {
    Fill::Solid { color: ColorRef::rgb(c) }
}

fn set_p3(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "format.p3";
    let (fill, line) = (param(p, "fill")?, param(p, "line")?);
    if fill.is_none() && line.is_none() {
        return Err(bad(CMD, "give `fill` or `line`"));
    }
    let st = s.doc()?;
    let ids = ids_param(p, "ids").unwrap_or_else(|| st.selection.shapes.clone());
    if ids.is_empty() {
        return Err(bad(CMD, "select a shape"));
    }
    for id in &ids {
        let sh = st.shape(*id).ok_or_else(|| bad(CMD, format!("no object {id}")))?;
        if matches!(sh.kind, ShapeKind::Group { .. }) && sh.frame.is_none() && sh.boolean.is_none() {
            return Err(bad(CMD, format!("object {id} is a group: select the shapes in it")));
        }
    }
    let mut answer = json!({"ids": ids});
    for (key, c) in [("fill", fill), ("line", line)] {
        if let Some(Some(c)) = c {
            let (_, inside) = p3_to_srgb(c);
            answer[key] = json!({"srgb": p3_rgba(c).hex(), "inGamut": inside});
        }
    }
    s.edit(|doc, sel| {
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad(CMD, "no slide"))?;
        for id in &ids {
            let Some(sh) = deckcraft_model::find_shape_mut(list, *id) else { continue };
            let mut k = sh.p3.unwrap_or_default();
            if let Some(c) = fill {
                k.fill = c;
                if let Some(c) = c {
                    sh.fill = Some(solid(p3_rgba(c)));
                }
            }
            if let Some(c) = line {
                k.line = c;
                if let Some(c) = c {
                    let mut l = sh.line.clone().unwrap_or_default();
                    l.fill = Some(solid(p3_rgba(c)));
                    sh.line = Some(l);
                }
            }
            sh.p3 = (k != P3Colors::default()).then_some(k);
        }
        Ok(answer.clone())
    })
}

/// A P3 colour whose fill or outline was since changed some other way is forgotten, so the
/// deck never keeps a P3 original for a colour it no longer has (the canvas pass).
pub(crate) fn settle(list: &mut [Shape], depth: usize) {
    if depth > deckcraft_model::canvas::MAX_DEPTH {
        return;
    }
    for sh in list.iter_mut() {
        if let ShapeKind::Group { children, .. } = &mut sh.kind {
            settle(children, depth + 1);
        }
        let Some(mut k) = sh.p3 else { continue };
        if k.fill.is_some_and(|c| sh.fill.as_ref() != Some(&solid(p3_rgba(c)))) {
            k.fill = None;
        }
        if k.line.is_some_and(|c| sh.line.as_ref().and_then(|l: &Line| l.fill.as_ref()) != Some(&solid(p3_rgba(c)))) {
            k.line = None;
        }
        sh.p3 = (k != P3Colors::default()).then_some(k);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::Session;
    use deckcraft_model::ShapeId;

    #[test]
    fn p3_red_is_outside_srgb_and_maps_to_srgb_red() {
        let (s, inside) = p3_to_srgb([1.0, 0.0, 0.0]);
        assert!(!inside);
        assert!(s[0] > 0.99 && s[1] < 0.01 && s[2] < 0.01, "{s:?}");
        // White and grey are the same in both.
        let (w, inside) = p3_to_srgb([1.0, 1.0, 1.0]);
        assert!(inside && w.iter().all(|v| (v - 1.0).abs() < 1e-3), "{w:?}");
        let (g, _) = p3_to_srgb([0.5, 0.5, 0.5]);
        assert!(g.iter().all(|v| (v - 0.5).abs() < 2e-3), "{g:?}");
        // A muted P3 colour is inside sRGB and comes out more saturated in sRGB numbers.
        let (c, inside) = p3_to_srgb([0.6, 0.4, 0.3]);
        assert!(inside && c[0] > 0.6, "{c:?}");
    }

    #[test]
    fn format_p3_keeps_the_original_and_draws_the_nearest_srgb() {
        let mut s = Session::with_new();
        s.execute("slide.new", &serde_json::json!({"layout": "blank"})).unwrap();
        let r = s.execute("shape.insert", &serde_json::json!({"preset": "rect", "rect": [100, 100, 100, 100]})).unwrap();
        let id = ShapeId(r["id"].as_u64().unwrap() as u32);
        let a = s.execute("format.p3", &serde_json::json!({"fill": "color(display-p3 0 1 0)", "line": [0.2, 0.3, 0.9]})).unwrap();
        assert_eq!(a["fill"]["inGamut"], false);
        let st = s.doc().unwrap();
        let sh = st.shape(id).unwrap();
        assert_eq!(sh.p3.unwrap().fill, Some([0.0, 1.0, 0.0]));
        let hex = a["fill"]["srgb"].as_str().unwrap().to_string();
        let drawn = s.execute("slide.colorAt", &serde_json::json!({"x": 150, "y": 150})).unwrap()["color"].as_str().unwrap().to_string();
        assert_eq!(drawn.to_ascii_uppercase(), hex.to_ascii_uppercase());
        // Changing the fill another way forgets the P3 fill but keeps the outline's.
        s.execute("shape.fill", &serde_json::json!({"color": "#123456"})).unwrap();
        let k = s.doc().unwrap().shape(id).unwrap().p3.unwrap();
        assert_eq!((k.fill, k.line), (None, Some([0.2, 0.3, 0.9])));
        // Saved and opened.
        let bytes = deckcraft_format::save(&s.doc().unwrap().doc).unwrap();
        let (p, _) = crate::cmd::file::open_checked("t.deckcraft", &bytes).unwrap();
        assert_eq!(deckcraft_model::find_shape(&p.slides.last().unwrap().shapes, id).unwrap().p3, Some(k));
        for bad in [
            serde_json::json!({"fill": [1, 2, 0]}),
            serde_json::json!({"fill": "color(srgb 1 0 0)"}),
            serde_json::json!({"fill": "color(display-p3 1 0)"}),
            serde_json::json!({"line": [f64::MAX, 0, 0]}),
            serde_json::json!({}),
        ] {
            assert!(s.execute("format.p3", &bad).is_err(), "{bad}");
        }
    }
}
