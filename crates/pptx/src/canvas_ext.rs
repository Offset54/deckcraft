//! SlideCraft's Phase 3 canvas data in `.pptx` (`sc:canvas`, one extension on a shape's
//! `p:cNvPr`): frames, auto layout, constraints, editable booleans, vector networks, Display P3
//! colours and the effects DrawingML has no markup for. PowerPoint ignores the extension and
//! shows the nearest static result, which the writer arranges:
//!
//! - members of frames and auto layout frames are written at their resolved boxes;
//! - a frame gets a background rectangle (its fill, outline, corner radius) as its first member,
//!   marked `frameBackground`, which the reader drops;
//! - an editable boolean is written as one shape with its result outline (`custGeom`); its
//!   operands ride in the extension;
//! - a vector network's geometry is already a `custGeom`;
//! - a P3 colour's nearest sRGB colour is the shape's fill or outline.
//!
//! Clipping, layer and background blur, and noise have no PowerPoint equivalent: PowerPoint
//! draws those shapes unclipped and unblurred.

use deckcraft_model::canvas::{AutoLayout, BoolOp, ChildLayout, Constraints, Frame, P3Colors};
use deckcraft_model::vector::VectorNetwork;
use deckcraft_model::{Effects, Shape};
use serde::{Deserialize, Serialize};

/// Largest extension the reader takes (bytes of JSON).
pub const MAX_LEN: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CanvasExt {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<Frame>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_layout: Option<AutoLayout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraints: Option<Constraints>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout_child: Option<ChildLayout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boolean: Option<BoolOp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector: Option<VectorNetwork>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p3: Option<P3Colors>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer_blur: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_blur: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub noise: Option<f64>,
    /// An editable boolean's operands (the shape itself is written as its result).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub operands: Vec<Shape>,
    /// The rectangle written for a frame's own box: dropped on reading.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub frame_background: bool,
    /// Number, string and boolean bindings (Phase 4; the bound fields are written resolved).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub bindings: Vec<deckcraft_model::variables::Binding>,
    /// Colours that name a variable, by path (`fill`, `line`, `text/<p>/<r>`), written as their
    /// value on the slide (Phase 4).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub color_vars: Vec<(String, u32)>,
}

impl CanvasExt {
    /// What `s` keeps in the extension; `None` when it has nothing to keep.
    pub fn of(s: &Shape, operands: Option<&[Shape]>) -> Option<CanvasExt> {
        let fx = s.effects.as_ref();
        let e = CanvasExt {
            frame: s.frame,
            auto_layout: s.auto_layout.clone(),
            constraints: s.constraints,
            layout_child: s.layout_child,
            boolean: s.boolean,
            vector: s.vector.clone(),
            p3: s.p3,
            layer_blur: fx.and_then(|f| f.layer_blur),
            background_blur: fx.and_then(|f| f.background_blur),
            noise: fx.and_then(|f| f.noise),
            operands: operands.map(<[Shape]>::to_vec).unwrap_or_default(),
            frame_background: false,
            bindings: s.bindings.clone(),
            color_vars: deckcraft_model::variables::color_bindings(s),
        };
        (e != CanvasExt::default()).then_some(e)
    }

    pub fn to_json(&self) -> Option<String> {
        serde_json::to_string(self).ok()
    }

    /// Read back; anything malformed or too large is ignored (the shape opens without it).
    pub fn parse(json: &str) -> Option<CanvasExt> {
        if json.len() > MAX_LEN {
            return None;
        }
        let e: CanvasExt = serde_json::from_str(json).ok()?;
        // A network that doesn't check out is dropped; the geometry it drew stays.
        let vector = e.vector.filter(|v| v.check().is_ok());
        Some(CanvasExt { vector, ..e })
    }

    /// Put the kept fields back on `s` (operands and the background mark are the reader's).
    pub fn apply(&self, s: &mut Shape) {
        s.frame = self.frame;
        s.auto_layout = self.auto_layout.clone();
        s.constraints = self.constraints;
        s.layout_child = self.layout_child;
        s.boolean = self.boolean;
        s.vector = self.vector.clone();
        // The network is what the outline is drawn from: rebuilt exactly, not from the custGeom
        // PowerPoint sees (whose numbers are rounded).
        if let Some(v) = &self.vector {
            s.geom = v.to_geom();
        }
        s.p3 = self.p3;
        s.bindings = self.bindings.iter().copied().take(deckcraft_model::variables::MAX_BINDINGS).collect();
        if !self.color_vars.is_empty() {
            let vars = &self.color_vars;
            deckcraft_model::variables::visit_bound_colors(s, &mut |path, c| {
                if let Some((_, id)) = vars.iter().find(|(p, _)| p == path)
                    && let deckcraft_model::ColorBase::Rgb { rgb } = c.base
                {
                    c.base = deckcraft_model::ColorBase::Var { id: *id, last: rgb };
                }
            });
        }
        let clean = |v: Option<f64>, max: f64| v.filter(|r| r.is_finite() && *r > 0.0).map(|r| r.min(max));
        let (lb, bb, n) = (
            clean(self.layer_blur, deckcraft_model::style::MAX_BLUR),
            clean(self.background_blur, deckcraft_model::style::MAX_BLUR),
            clean(self.noise, 1.0),
        );
        if lb.is_some() || bb.is_some() || n.is_some() {
            let e = s.effects.get_or_insert_with(Effects::default);
            e.layer_blur = lb;
            e.background_blur = bb;
            e.noise = n;
        }
    }
}
