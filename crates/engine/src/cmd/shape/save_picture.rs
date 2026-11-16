//! Save as Picture… (an object's right-click menu): `shape.saveAsPicture` draws the selected
//! objects together, as they look on the slide, into a picture file.

use serde_json::{Value, json};

use crate::cmd::{bad, base64_encode, f64_or, str_param, targets};
use crate::picture_export::{PICTURE_FORMATS, encode, picture_format, picture_format_for_path};
use crate::{EngineError, Result, Session};

const CMD: &str = "shape.saveAsPicture";

pub(super) fn save_as_picture(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_param(p, "path").map(String::from);
    // The type asked for, else the one the file name ends in, else PNG.
    let format = match str_param(p, "format") {
        Some(f) => picture_format(f).ok_or_else(|| bad(CMD, format!("`format` must be one of {}", PICTURE_FORMATS.join(", "))))?,
        None => path.as_deref().and_then(picture_format_for_path).unwrap_or("png"),
    };
    let scale = f64_or(p, "scale", 2.0);
    if !(scale.is_finite() && (0.01..=16.0).contains(&scale)) {
        return Err(bad(CMD, "`scale` must be from 0.01 to 16 pixels per point"));
    }
    let ids = targets(s, p)?;
    let st = s.doc()?;
    let slide = st.current_slide().ok_or_else(|| bad(CMD, "no slide"))?;
    let img =
        deckcraft_render::render_shapes(&st.doc, slide, &ids, scale).ok_or_else(|| bad(CMD, "nothing to draw: select an object on the slide"))?;
    // The drawing may come out smaller than asked for (a long side is capped); say what it is.
    let (width, height) = (img.width, img.height);
    let bytes = encode(&img, format, scale)?;
    let mut out = json!({"format": format, "width": width, "height": height, "bytes": bytes.len()});
    match path {
        Some(path) => {
            crate::io::write_file(&path, &bytes).map_err(EngineError::Other)?;
            out["path"] = json!(path);
        }
        None => out["data"] = json!(base64_encode(&bytes)),
    }
    Ok(out)
}

#[cfg(test)]
mod patch_tests {
    use serde_json::json;

    use crate::Session;
    use crate::cmd::base64_decode;

    fn session() -> (Session, u64) {
        let mut s = Session::with_new();
        s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
        let id = s.execute("shape.insert", &json!({"preset": "ellipse", "rect": [100.0, 100.0, 200.0, 100.0]})).unwrap()["id"].as_u64().unwrap();
        s.execute("shape.fill", &json!({"color": "#FF0000", "ids": [id]})).unwrap();
        (s, id)
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("deckcraft-savepic-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_object_is_drawn_as_it_looks_with_a_clear_surround() {
        let (mut s, id) = session();
        let r = s.execute("shape.saveAsPicture", &json!({"ids": [id]})).unwrap();
        assert_eq!(r["format"], "png");
        let img = image::load_from_memory(&base64_decode(r["data"].as_str().unwrap()).unwrap()).unwrap().to_rgba8();
        assert_eq!((u64::from(img.width()), u64::from(img.height())), (r["width"].as_u64().unwrap(), r["height"].as_u64().unwrap()));
        // 200 x 100 pt and a little room around it, at 2 px per pt.
        assert!((400..=440).contains(&img.width()) && (200..=240).contains(&img.height()), "{}x{}", img.width(), img.height());
        // Red in the middle of the ellipse, nothing in the corner beside it.
        let mid = img.get_pixel(img.width() / 2, img.height() / 2).0;
        assert!(mid[0] > 200 && mid[1] < 60 && mid[3] == 255, "{mid:?}");
        assert_eq!(img.get_pixel(1, 1).0[3], 0);
        // Saving a picture is not an edit.
        assert!(!s.doc().unwrap().is_dirty() || s.doc().unwrap().history.undo.last().is_none_or(|h| h.label != "Save as Picture…"));
    }

    #[test]
    fn the_file_name_chooses_the_type_and_format_overrides_it() {
        let (mut s, id) = session();
        let dir = scratch("types");
        for (name, want) in [
            ("a.png", image::ImageFormat::Png),
            ("b.jpg", image::ImageFormat::Jpeg),
            ("c.tif", image::ImageFormat::Tiff),
            ("d.bmp", image::ImageFormat::Bmp),
            ("e.gif", image::ImageFormat::Gif),
        ] {
            let path = dir.join(name);
            let r = s.execute("shape.saveAsPicture", &json!({"path": path.to_string_lossy(), "ids": [id], "scale": 1})).unwrap();
            assert!(r.get("data").is_none());
            let bytes = std::fs::read(&path).unwrap();
            assert_eq!(bytes.len() as u64, r["bytes"].as_u64().unwrap());
            assert_eq!(image::guess_format(&bytes).unwrap(), want, "{name}");
        }
        // A name without a known ending is a PNG; `format` wins over the ending.
        let plain = dir.join("picture");
        s.execute("shape.saveAsPicture", &json!({"path": plain.to_string_lossy(), "ids": [id]})).unwrap();
        assert_eq!(image::guess_format(&std::fs::read(&plain).unwrap()).unwrap(), image::ImageFormat::Png);
        let odd = dir.join("odd.png");
        s.execute("shape.saveAsPicture", &json!({"path": odd.to_string_lossy(), "format": "bmp", "ids": [id]})).unwrap();
        assert_eq!(image::guess_format(&std::fs::read(&odd).unwrap()).unwrap(), image::ImageFormat::Bmp);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn several_objects_are_saved_together_and_bad_requests_are_refused() {
        let (mut s, id) = session();
        let other = s.execute("shape.insert", &json!({"preset": "rect", "rect": [500.0, 100.0, 100.0, 100.0]})).unwrap()["id"].as_u64().unwrap();
        let one = s.execute("shape.saveAsPicture", &json!({"ids": [id], "scale": 1})).unwrap()["width"].as_u64().unwrap();
        let both = s.execute("shape.saveAsPicture", &json!({"ids": [id, other], "scale": 1})).unwrap()["width"].as_u64().unwrap();
        assert!(both > one + 250, "{both} vs {one}");
        for bad in
            [json!({"ids": [id], "format": "svg"}), json!({"ids": [id], "scale": 0}), json!({"ids": [id], "scale": 1e9}), json!({"ids": [987_654]})]
        {
            assert!(s.execute("shape.saveAsPicture", &bad).is_err(), "{bad}");
        }
    }
}
