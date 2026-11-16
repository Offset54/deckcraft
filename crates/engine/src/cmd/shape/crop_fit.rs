//! Crop › Fill, Fit and Aspect Ratio, and the Format Picture pane's Crop fields.
//!
//! A picture's crop is the fraction cut off each side of the image (left, top, right, bottom;
//! negative pads), relative to its frame. All of these keep the image's own proportions in
//! mind: Fill and Fit work from its size in pixels, so a picture that was stretched is shown
//! unstretched again.
//!
//! - **Fill** (`picture.cropFill`): the picture covers the frame; what overflows is cropped evenly.
//! - **Fit** (`picture.cropFit`): the whole picture shows inside the frame, with empty bands.
//! - **Aspect Ratio** (`picture.cropAspect`): the frame becomes the largest one of that ratio
//!   that fits the whole picture, centred on it.
//! - **Crop position** (`picture.cropPosition` / `picture.cropInfo`): the picture's size and
//!   offset inside the frame, and the frame's size and place, in points.

use deckcraft_geom::{Point, Xfrm};
use deckcraft_model::{Presentation, ShapeId, ShapeKind};
use serde_json::{Value, json};

use crate::cmd::{bad, f64_param, id_param, targets, xfrm_of};
use crate::tools::crop::{crop_for, picture_box};
use crate::{Result, Session};

/// Least of the picture a crop may leave showing, per axis (the same floor `picture.crop` keeps).
const MIN_SHOWN: f64 = 0.01;
/// Most a side is padded, as `picture.crop` allows.
const MAX_PAD: f64 = 10.0;
/// Sizes and places typed into the pane, in points: within what a slide can hold.
const MAX_LENGTH: f64 = 1.0e5;
/// Aspect ratios taken (width ÷ height).
const RATIO_RANGE: std::ops::RangeInclusive<f64> = 0.01..=100.0;

/// The crop that makes an image of aspect `image` (width ÷ height) cover a frame of aspect
/// `frame`: the overflowing direction is cut evenly on both sides.
pub fn fill_crop(image: f64, frame: f64) -> [f64; 4] {
    if !(image.is_finite() && frame.is_finite() && image > 0.0 && frame > 0.0) {
        return [0.0; 4];
    }
    let cut = |shown: f64| ((1.0 - shown.clamp(MIN_SHOWN, 1.0)) / 2.0).max(0.0);
    if image > frame {
        let side = cut(frame / image);
        [side, 0.0, side, 0.0]
    } else {
        let side = cut(image / frame);
        [0.0, side, 0.0, side]
    }
}

/// The crop that shows the whole image of aspect `image` inside a frame of aspect `frame`: the
/// direction with room to spare is padded evenly (negative crop).
pub fn fit_crop(image: f64, frame: f64) -> [f64; 4] {
    if !(image.is_finite() && frame.is_finite() && image > 0.0 && frame > 0.0) {
        return [0.0; 4];
    }
    let pad = |shown: f64| ((1.0 - shown.max(1.0)) / 2.0).max(-MAX_PAD);
    if image > frame {
        let side = pad(image / frame);
        [0.0, side, 0.0, side]
    } else {
        let side = pad(frame / image);
        [side, 0.0, side, 0.0]
    }
}

/// The frame and crop of ratio `ratio` (width ÷ height) for the picture of frame `orig` cropped
/// by `crop`: the largest such frame inside the whole picture, centred on it. The picture stays
/// where it is on the slide, and the frame keeps its rotation and flips.
pub fn aspect_frame(orig: &Xfrm, crop: [f64; 4], ratio: f64) -> (Xfrm, [f64; 4]) {
    let (px, py, pw, ph) = picture_box(orig, crop);
    let (fw, fh) = if pw / ph > ratio { (ph * ratio, ph) } else { (pw, pw / ratio) };
    let (cx, cy) = (px + pw / 2.0, py + ph / 2.0);
    let frame = (cx - fw / 2.0, cy - fh / 2.0, cx + fw / 2.0, cy + fh / 2.0);
    let center = orig.affine() * Point::new(cx, cy);
    let mut x = *orig;
    (x.w, x.h) = (fw, fh);
    (x.x, x.y) = (center.x - fw / 2.0, center.y - fh / 2.0);
    (x, crop_for(frame, (px, py, pw, ph)))
}

/// The picture's size in pixels, read from its header (nothing is decoded). `None` for a
/// picture without one a raster reader knows (a metafile, a broken file).
pub fn pixel_size(doc: &Presentation, media: deckcraft_model::MediaId) -> Option<(u32, u32)> {
    let item = doc.media(media)?;
    let reader = image::ImageReader::new(std::io::Cursor::new(item.data.as_slice())).with_guessed_format().ok()?;
    reader.into_dimensions().ok().filter(|(w, h)| *w > 0 && *h > 0)
}

/// A ratio as given: `"16:9"`, `"3/2"`, or a number (width ÷ height).
fn ratio_param(p: &Value) -> Result<f64> {
    let refuse = || bad("picture.cropAspect", "`ratio` must be like \"16:9\", or a number (width ÷ height) from 0.01 to 100");
    let ratio = match p.get("ratio") {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => {
            let mut parts = s.split([':', '/']).map(|t| t.trim().parse::<f64>().ok());
            match (parts.next().flatten(), parts.next(), parts.next()) {
                (Some(w), Some(Some(h)), None) if h > 0.0 => Some(w / h),
                (Some(r), None, None) => Some(r),
                _ => None,
            }
        }
        _ => None,
    };
    ratio.filter(|r| r.is_finite() && RATIO_RANGE.contains(r)).ok_or_else(refuse)
}

#[derive(Clone, Copy)]
enum Refit {
    Fill,
    Fit,
    Aspect(f64),
}

/// A frame worth working on: finite, with some size.
fn usable(x: &Xfrm) -> bool {
    [x.x, x.y, x.w, x.h].iter().all(|v| v.is_finite()) && x.w > 0.0 && x.h > 0.0
}

/// Store new frames and crops, all in one edit.
fn apply(s: &mut Session, cmd: &'static str, plan: &[(ShapeId, Option<Xfrm>, [f64; 4])]) -> Result<()> {
    s.edit(|doc, sel| {
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad(cmd, "no slide"))?;
        for (id, frame, crop) in plan {
            if let Some(sh) = deckcraft_model::find_shape_mut(list, *id)
                && let ShapeKind::Picture { fill } = &mut sh.kind
            {
                fill.crop = *crop;
                if let Some(x) = frame {
                    sh.xfrm = Some(*x);
                }
            }
        }
        Ok(())
    })
}

fn refit(s: &mut Session, p: &Value, cmd: &'static str, how: Refit) -> Result<Value> {
    let ids = targets(s, p)?;
    let st = s.doc()?;
    let mut plan: Vec<(ShapeId, Option<Xfrm>, [f64; 4])> = Vec::new();
    for id in ids {
        let Some(sh) = st.shape(id).filter(|sh| !sh.locked) else { continue };
        let ShapeKind::Picture { fill } = &sh.kind else { continue };
        let x = xfrm_of(&st.doc, &st.selection, sh);
        if !usable(&x) {
            continue;
        }
        match how {
            Refit::Aspect(ratio) => {
                let (frame, crop) = aspect_frame(&x, fill.crop, ratio);
                if usable(&frame) {
                    plan.push((id, Some(frame), crop));
                }
            }
            Refit::Fill | Refit::Fit => {
                // The image's own proportions; one whose size can't be read keeps those it is shown with.
                let (_, _, pw, ph) = picture_box(&x, fill.crop);
                let image = pixel_size(&st.doc, fill.media).map_or(pw / ph, |(w, h)| f64::from(w) / f64::from(h));
                let frame = x.w / x.h;
                plan.push((id, None, if matches!(how, Refit::Fill) { fill_crop(image, frame) } else { fit_crop(image, frame) }));
            }
        }
    }
    if plan.is_empty() {
        return Err(bad(cmd, "select a picture that is not locked"));
    }
    apply(s, cmd, &plan)?;
    Ok(json!({"changed": plan.len(), "crop": plan.first().map(|(_, _, c)| c.to_vec())}))
}

pub(super) fn crop_fill(s: &mut Session, p: &Value) -> Result<Value> {
    refit(s, p, "picture.cropFill", Refit::Fill)
}

pub(super) fn crop_fit(s: &mut Session, p: &Value) -> Result<Value> {
    refit(s, p, "picture.cropFit", Refit::Fit)
}

pub(super) fn crop_aspect(s: &mut Session, p: &Value) -> Result<Value> {
    let ratio = ratio_param(p)?;
    refit(s, p, "picture.cropAspect", Refit::Aspect(ratio))
}

/// The one picture a crop-position command is about: `id`, else the single selected shape.
fn one_picture(s: &Session, p: &Value, cmd: &'static str) -> Result<(ShapeId, Xfrm, [f64; 4], Option<(u32, u32)>)> {
    let st = s.doc()?;
    let id = match id_param(p, "id", cmd)? {
        Some(id) => id,
        None => match st.selection.shapes.as_slice() {
            [id] => *id,
            _ => return Err(bad(cmd, "select one picture")),
        },
    };
    let sh = st.shape(id).ok_or_else(|| bad(cmd, "no such shape"))?;
    let ShapeKind::Picture { fill } = &sh.kind else { return Err(bad(cmd, "not a picture")) };
    let x = xfrm_of(&st.doc, &st.selection, sh);
    if !usable(&x) {
        return Err(bad(cmd, "the picture has no size"));
    }
    Ok((id, x, fill.crop, pixel_size(&st.doc, fill.media)))
}

/// The Crop fields of the Format Picture pane for frame `x` cropped by `crop`, in points.
fn position_json(id: ShapeId, x: &Xfrm, crop: [f64; 4], pixels: Option<(u32, u32)>) -> Value {
    let (px, py, pw, ph) = picture_box(x, crop);
    json!({
        "id": id.0,
        "pictureWidth": pw,
        "pictureHeight": ph,
        "offsetX": px + pw / 2.0 - x.w / 2.0,
        "offsetY": py + ph / 2.0 - x.h / 2.0,
        "cropWidth": x.w,
        "cropHeight": x.h,
        "cropLeft": x.x,
        "cropTop": x.y,
        "crop": crop.to_vec(),
        "pixels": pixels.map(|(w, h)| vec![w, h]),
    })
}

pub(super) fn crop_info(s: &mut Session, p: &Value) -> Result<Value> {
    let (id, x, crop, pixels) = one_picture(s, p, "picture.cropInfo")?;
    Ok(position_json(id, &x, crop, pixels))
}

pub(super) fn crop_position(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "picture.cropPosition";
    let (id, x, crop, pixels) = one_picture(s, p, CMD)?;
    if s.doc()?.shape(id).is_some_and(|sh| sh.locked) {
        return Err(bad(CMD, "the picture is locked"));
    }
    // A length (above zero) or a place, in points; anything else is refused, not clamped away.
    let number = |key: &str, length: bool| -> Result<Option<f64>> {
        match p.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(_) => f64_param(p, key)
                .filter(|v| v.is_finite() && v.abs() <= MAX_LENGTH && (!length || *v >= 1.0))
                .map(Some)
                .ok_or_else(|| bad(CMD, format!("`{key}` must be a number of points{}", if length { ", 1 or more" } else { "" }))),
        }
    };
    let (px, py, pw, ph) = picture_box(&x, crop);
    // Where the picture's centre is on the slide: it stays there while the frame changes.
    let held = x.affine() * Point::new(px + pw / 2.0, py + ph / 2.0);
    let (pw, ph) = (number("pictureWidth", true)?.unwrap_or(pw), number("pictureHeight", true)?.unwrap_or(ph));
    let mut frame = x;
    frame.w = number("cropWidth", true)?.unwrap_or(x.w);
    frame.h = number("cropHeight", true)?.unwrap_or(x.h);
    frame.x = number("cropLeft", false)?.unwrap_or(x.x);
    frame.y = number("cropTop", false)?.unwrap_or(x.y);
    // The picture's centre in the new frame: where it was on the slide, unless an offset from
    // the frame's centre is given.
    let local = frame.affine().inverse() * held;
    let cx = number("offsetX", false)?.map_or(local.x, |o| frame.w / 2.0 + o);
    let cy = number("offsetY", false)?.map_or(local.y, |o| frame.h / 2.0 + o);
    // Some of the picture must still be inside the frame, measured before the crop is brought
    // into range (a picture far outside would otherwise read as a heavily padded one).
    let (left, top) = (cx - pw / 2.0, cy - ph / 2.0);
    let inside = ((left + pw).min(frame.w) - left.max(0.0), (top + ph).min(frame.h) - top.max(0.0));
    let new_crop = crop_for((0.0, 0.0, frame.w, frame.h), (left, top, pw, ph));
    let shown = (1.0 - new_crop[0] - new_crop[2], 1.0 - new_crop[1] - new_crop[3]);
    if inside.0 < pw * MIN_SHOWN || inside.1 < ph * MIN_SHOWN || shown.0 < MIN_SHOWN || shown.1 < MIN_SHOWN {
        return Err(bad(CMD, "that would leave nothing of the picture in the frame"));
    }
    let moved = frame != x;
    apply(s, CMD, &[(id, moved.then_some(frame), new_crop)])?;
    Ok(position_json(id, &frame, new_crop, pixels))
}

#[cfg(test)]
mod patch_tests {
    use serde_json::{Value, json};

    use super::{aspect_frame, fill_crop, fit_crop};
    use crate::Session;
    use crate::cmd::base64_encode;

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    fn crop_near(got: [f64; 4], want: [f64; 4]) {
        assert!(got.iter().zip(&want).all(|(a, b)| near(*a, *b)), "{got:?} vs {want:?}");
    }

    #[test]
    fn fill_cuts_the_overflow_and_fit_pads_the_spare_room() {
        // A 2:1 image in a square frame: fill cuts a quarter off each side, fit pads top and bottom.
        crop_near(fill_crop(2.0, 1.0), [0.25, 0.0, 0.25, 0.0]);
        crop_near(fit_crop(2.0, 1.0), [0.0, -0.5, 0.0, -0.5]);
        // A 1:2 image in a square frame: the other way round.
        crop_near(fill_crop(0.5, 1.0), [0.0, 0.25, 0.0, 0.25]);
        crop_near(fit_crop(0.5, 1.0), [-0.5, 0.0, -0.5, 0.0]);
        // The same proportions: nothing to do.
        crop_near(fill_crop(1.5, 1.5), [0.0; 4]);
        crop_near(fit_crop(1.5, 1.5), [0.0; 4]);
    }

    #[test]
    fn extreme_and_broken_proportions_stay_in_range() {
        for (image, frame) in [(1e9, 1e-9), (1e-9, 1e9), (f64::NAN, 1.0), (1.0, f64::INFINITY), (0.0, 1.0), (-1.0, 1.0), (1.0, 0.0)] {
            for c in [fill_crop(image, frame), fit_crop(image, frame)] {
                assert!(c.iter().all(|v| v.is_finite() && (-10.0..=0.5).contains(v)), "{image} in {frame}: {c:?}");
                assert!(1.0 - c[0] - c[2] > 0.0 && 1.0 - c[1] - c[3] > 0.0, "{c:?}");
            }
        }
    }

    #[test]
    fn an_aspect_frame_is_centred_on_the_whole_picture() {
        // A 400 x 200 picture, uncropped: a square frame is its middle 200 x 200.
        let orig = deckcraft_geom::Xfrm::new(100.0, 50.0, 400.0, 200.0);
        let (x, crop) = aspect_frame(&orig, [0.0; 4], 1.0);
        assert!(near(x.w, 200.0) && near(x.h, 200.0) && near(x.x, 200.0) && near(x.y, 50.0), "{x:?}");
        crop_near(crop, [0.25, 0.0, 0.25, 0.0]);
        // From an existing crop the whole picture still counts: the left half shown, then 16:9.
        let half = deckcraft_geom::Xfrm::new(100.0, 50.0, 200.0, 200.0);
        let (x, crop) = aspect_frame(&half, [0.0, 0.0, 0.5, 0.0], 2.0);
        assert!(near(x.w, 400.0) && near(x.h, 200.0) && near(x.x, 100.0), "{x:?}");
        crop_near(crop, [0.0; 4]);
        // A rotated frame keeps its rotation and its centre on the picture's.
        let mut turned = orig;
        turned.rot = 90.0;
        let (x, _) = aspect_frame(&turned, [0.0; 4], 1.0);
        assert!(near(x.rot, 90.0) && near(x.x + x.w / 2.0, 300.0) && near(x.y + x.h / 2.0, 150.0), "{x:?}");
    }

    /// A PNG of `w` x `h` pixels.
    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([200, 30, 30, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    /// A blank slide with a 200 x 100 px picture in a `w` x `h` pt frame at (100, 100), selected.
    fn session(w: f64, h: f64) -> (Session, u64) {
        let mut s = Session::with_new();
        s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
        let r =
            s.execute("insert.picture", &json!({"name": "wide.png", "data": base64_encode(&png(200, 100)), "rect": [100.0, 100.0, w, h]})).unwrap();
        let id = r["id"].as_u64().unwrap();
        s.execute("selection.select", &json!({"ids": [id]})).unwrap();
        (s, id)
    }

    fn crop_of(s: &Session, id: u64) -> [f64; 4] {
        match &s.doc().unwrap().shape(deckcraft_model::ShapeId(id as u32)).unwrap().kind {
            deckcraft_model::ShapeKind::Picture { fill } => fill.crop,
            _ => panic!("a picture"),
        }
    }

    fn frame_of(s: &Session, id: u64) -> deckcraft_geom::Xfrm {
        s.doc().unwrap().shape(deckcraft_model::ShapeId(id as u32)).unwrap().xfrm.unwrap()
    }

    #[test]
    fn fill_and_fit_use_the_pictures_pixels_and_keep_the_frame() {
        // The 2:1 picture stretched into a square frame.
        let (mut s, id) = session(200.0, 200.0);
        let frame = frame_of(&s, id);
        s.execute("picture.cropFill", &json!({})).unwrap();
        crop_near(crop_of(&s, id), [0.25, 0.0, 0.25, 0.0]);
        assert_eq!(frame_of(&s, id), frame);
        s.execute("picture.cropFit", &json!({})).unwrap();
        crop_near(crop_of(&s, id), [0.0, -0.5, 0.0, -0.5]);
        assert_eq!(frame_of(&s, id), frame);
        // One undo step each.
        s.execute("edit.undo", &json!({})).unwrap();
        crop_near(crop_of(&s, id), [0.25, 0.0, 0.25, 0.0]);
        // An earlier crop does not count: Fill starts from the whole picture again.
        s.execute("picture.crop", &json!({"left": 0.4, "top": 0.3})).unwrap();
        s.execute("picture.cropFill", &json!({})).unwrap();
        crop_near(crop_of(&s, id), [0.25, 0.0, 0.25, 0.0]);
    }

    #[test]
    fn aspect_ratio_reshapes_the_frame_over_the_picture() {
        let (mut s, id) = session(400.0, 200.0);
        let r = s.execute("picture.cropAspect", &json!({"ratio": "1:1"})).unwrap();
        assert_eq!(r["changed"], 1);
        let x = frame_of(&s, id);
        assert!(near(x.w, 200.0) && near(x.h, 200.0) && near(x.x, 200.0) && near(x.y, 100.0), "{x:?}");
        crop_near(crop_of(&s, id), [0.25, 0.0, 0.25, 0.0]);
        // Another ratio starts from the whole picture, not from the square.
        s.execute("picture.cropAspect", &json!({"ratio": 4.0})).unwrap();
        let x = frame_of(&s, id);
        assert!(near(x.w, 400.0) && near(x.h, 100.0) && near(x.x, 100.0) && near(x.y, 150.0), "{x:?}");
        s.execute("edit.undo", &json!({})).unwrap();
        assert!(near(frame_of(&s, id).w, 200.0));
        for bad in [json!("0:1"), json!("1:0"), json!("wide"), json!(0), json!(-2), json!(1e9), json!("1:2:3"), json!(null), json!([16, 9])] {
            assert!(s.execute("picture.cropAspect", &json!({"ratio": bad})).is_err(), "{bad}");
        }
        assert!(s.execute("picture.cropAspect", &json!({})).is_err());
    }

    #[test]
    fn only_unlocked_pictures_are_refitted() {
        let (mut s, id) = session(200.0, 200.0);
        let rect = s.execute("shape.insert", &json!({"preset": "rect", "rect": [10.0, 10.0, 50.0, 50.0]})).unwrap()["id"].as_u64().unwrap();
        assert!(s.execute("picture.cropFill", &json!({"ids": [rect]})).is_err());
        assert_eq!(s.execute("picture.cropFill", &json!({"ids": [rect, id]})).unwrap()["changed"], 1);
        s.execute("shape.lock", &json!({"locked": true, "ids": [id]})).unwrap();
        assert!(s.execute("picture.cropFit", &json!({"ids": [id]})).is_err());
        assert!(s.execute("picture.cropPosition", &json!({"id": id, "offsetX": 5.0})).is_err());
    }

    fn info(s: &mut Session) -> Value {
        s.execute("picture.cropInfo", &json!({})).unwrap()
    }

    fn field(v: &Value, key: &str) -> f64 {
        v[key].as_f64().unwrap_or_else(|| panic!("{key} in {v}"))
    }

    #[test]
    fn the_crop_fields_read_back_what_they_set() {
        let (mut s, id) = session(400.0, 200.0);
        let i = info(&mut s);
        assert_eq!(i["pixels"], json!([200, 100]));
        for (key, want) in
            [("pictureWidth", 400.0), ("pictureHeight", 200.0), ("offsetX", 0.0), ("cropWidth", 400.0), ("cropLeft", 100.0), ("cropTop", 100.0)]
        {
            assert!(near(field(&i, key), want), "{key}: {i}");
        }
        // A narrower crop frame: the picture stays where it was, so the right part is cut off.
        let r = s.execute("picture.cropPosition", &json!({"cropWidth": 300.0})).unwrap();
        crop_near(crop_of(&s, id), [0.0, 0.0, 0.25, 0.0]);
        assert!(near(field(&r, "offsetX"), 50.0) && near(field(&r, "pictureWidth"), 400.0), "{r}");
        assert!(near(frame_of(&s, id).w, 300.0) && near(frame_of(&s, id).x, 100.0));
        // Centre the picture in the frame: an eighth off each side.
        s.execute("picture.cropPosition", &json!({"offsetX": 0.0})).unwrap();
        crop_near(crop_of(&s, id), [0.125, 0.0, 0.125, 0.0]);
        // Move the frame over the picture: the picture does not move on the slide.
        s.execute("picture.cropPosition", &json!({"cropLeft": 150.0})).unwrap();
        crop_near(crop_of(&s, id), [0.25, 0.0, 0.0, 0.0]);
        // A bigger picture in the same frame, about its own centre.
        let before = info(&mut s);
        let r = s.execute("picture.cropPosition", &json!({"pictureWidth": 800.0, "pictureHeight": 400.0})).unwrap();
        assert!(near(field(&r, "offsetX"), field(&before, "offsetX")) && near(field(&r, "pictureHeight"), 400.0), "{r}");
        let now = info(&mut s);
        for key in ["pictureWidth", "pictureHeight", "offsetX", "offsetY", "cropWidth", "cropHeight", "cropLeft", "cropTop"] {
            assert!(near(field(&now, key), field(&r, key)), "{key}: {now} vs {r}");
        }
    }

    #[test]
    fn crop_fields_refuse_what_cannot_be() {
        let (mut s, id) = session(400.0, 200.0);
        let before = (crop_of(&s, id), frame_of(&s, id));
        for bad in [
            json!({"cropWidth": 0.0}),
            json!({"cropHeight": -5.0}),
            json!({"pictureWidth": "wide"}),
            json!({"offsetX": 1e300}),
            json!({"cropLeft": true}),
            // The frame moved right off the picture: nothing would show.
            json!({"offsetX": 5000.0}),
        ] {
            assert!(s.execute("picture.cropPosition", &bad).is_err(), "{bad}");
        }
        assert_eq!((crop_of(&s, id), frame_of(&s, id)), before);
        // Nothing selected, or not a picture.
        let rect = s.execute("shape.insert", &json!({"preset": "rect", "rect": [10.0, 10.0, 50.0, 50.0]})).unwrap()["id"].as_u64().unwrap();
        assert!(s.execute("picture.cropInfo", &json!({"id": rect})).is_err());
        assert!(s.execute("picture.cropInfo", &json!({"id": 999_999})).is_err());
    }
}
