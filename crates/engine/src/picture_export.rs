//! Picture file types for exports: slides (`file.export`) and Save as Picture
//! (`shape.saveAsPicture`). PNG and TIFF keep transparency; JPEG, GIF and BMP have none (GIF's
//! one transparent colour would leave ragged edges), so those are drawn on white.

use deckcraft_render::Image;

use crate::{EngineError, Result};

/// The picture types written, by the name commands take.
pub const PICTURE_FORMATS: &[&str] = &["png", "jpeg", "gif", "tiff", "bmp"];

/// `name` as one of [`PICTURE_FORMATS`] (`jpg` and `tif` are the short spellings).
pub fn picture_format(name: &str) -> Option<&'static str> {
    match name.to_ascii_lowercase().as_str() {
        "png" => Some("png"),
        "jpeg" | "jpg" => Some("jpeg"),
        "gif" => Some("gif"),
        "tiff" | "tif" => Some("tiff"),
        "bmp" => Some("bmp"),
        _ => None,
    }
}

/// The picture type a file name asks for by its extension.
pub fn picture_format_for_path(path: &str) -> Option<&'static str> {
    let ext = std::path::Path::new(path).extension()?.to_str()?;
    picture_format(ext)
}

/// Does `format` keep transparent pixels?
pub fn keeps_transparency(format: &str) -> bool {
    matches!(format, "png" | "tiff")
}

/// Straight RGBA with every pixel drawn over white, opaque.
fn on_white(straight: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(straight.len() / 4 * 3);
    for px in straight.as_chunks::<4>().0 {
        let a = u32::from(px[3]);
        for c in &px[..3] {
            out.push(((u32::from(*c) * a + 255 * (255 - a) + 127) / 255) as u8);
        }
    }
    out
}

/// `img` as a `format` file (one of [`PICTURE_FORMATS`]), drawn at `scale` pixels per point (kept
/// as the file's resolution where the type has one written here).
pub fn encode(img: &Image, format: &str, scale: f64) -> Result<Vec<u8>> {
    let failed = |what: &str| EngineError::Other(format!("could not write the {format} picture: {what}"));
    if img.width == 0 || img.height == 0 {
        return Err(failed("it has no pixels"));
    }
    let dpi = deckcraft_render::dpi_for_scale(scale);
    let straight = img.to_straight();
    let mut out = std::io::Cursor::new(Vec::new());
    match format {
        "png" => return Ok(img.to_png_at(dpi)),
        "tiff" => {
            let rgba = image::RgbaImage::from_raw(img.width, img.height, straight).ok_or_else(|| failed("its pixels are incomplete"))?;
            image::DynamicImage::ImageRgba8(rgba).write_to(&mut out, image::ImageFormat::Tiff).map_err(|e| failed(&e.to_string()))?;
        }
        "jpeg" | "bmp" | "gif" => {
            let rgb = image::RgbImage::from_raw(img.width, img.height, on_white(&straight)).ok_or_else(|| failed("its pixels are incomplete"))?;
            let rgb = image::DynamicImage::ImageRgb8(rgb);
            match format {
                "jpeg" => {
                    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 92);
                    rgb.write_with_encoder(enc).map_err(|e| failed(&e.to_string()))?;
                }
                "bmp" => rgb.write_to(&mut out, image::ImageFormat::Bmp).map_err(|e| failed(&e.to_string()))?,
                _ => rgb.write_to(&mut out, image::ImageFormat::Gif).map_err(|e| failed(&e.to_string()))?,
            }
        }
        other => return Err(EngineError::Other(format!("`{other}` is not a picture type ({})", PICTURE_FORMATS.join(", ")))),
    }
    Ok(out.into_inner())
}

#[cfg(test)]
mod patch_tests {
    use super::*;

    /// A 4 x 2 image: opaque red, then half-transparent blue, then clear (premultiplied).
    fn image() -> Image {
        let mut pixels = Vec::new();
        for px in [[255u8, 0, 0, 255], [0, 0, 128, 128], [0, 0, 0, 0], [0, 0, 0, 0]].iter().cycle().take(8) {
            pixels.extend_from_slice(px);
        }
        Image { width: 4, height: 2, pixels }
    }

    #[test]
    fn every_picture_type_is_written_and_reads_back() {
        for format in PICTURE_FORMATS {
            let bytes = encode(&image(), format, 2.0).unwrap();
            let back = image::load_from_memory(&bytes).unwrap_or_else(|e| panic!("{format}: {e}"));
            assert_eq!((back.width(), back.height()), (4, 2), "{format}");
            let want = match *format {
                "png" => image::ImageFormat::Png,
                "jpeg" => image::ImageFormat::Jpeg,
                "gif" => image::ImageFormat::Gif,
                "tiff" => image::ImageFormat::Tiff,
                _ => image::ImageFormat::Bmp,
            };
            assert_eq!(image::guess_format(&bytes).unwrap(), want);
        }
    }

    #[test]
    fn png_and_tiff_keep_transparency_and_the_others_draw_on_white() {
        for format in ["png", "tiff"] {
            let back = image::load_from_memory(&encode(&image(), format, 1.0).unwrap()).unwrap().to_rgba8();
            assert_eq!(back.get_pixel(0, 0).0, [255, 0, 0, 255], "{format}");
            assert_eq!(back.get_pixel(2, 0).0[3], 0, "{format}: clear stays clear");
            assert_eq!(back.get_pixel(1, 0).0[3], 128, "{format}");
            assert!(keeps_transparency(format));
        }
        // BMP is exact: clear is white, half-transparent blue is blue over white.
        let back = image::load_from_memory(&encode(&image(), "bmp", 1.0).unwrap()).unwrap().to_rgba8();
        assert_eq!(back.get_pixel(2, 0).0, [255, 255, 255, 255]);
        assert_eq!(back.get_pixel(0, 0).0, [255, 0, 0, 255]);
        let half = back.get_pixel(1, 0).0;
        assert!(half[0] > 120 && half[0] < 135 && half[2] == 255, "{half:?}");
        // JPEG is lossy, but its clear corner is light, not black.
        let back = image::load_from_memory(&encode(&image(), "jpeg", 1.0).unwrap()).unwrap().to_rgba8();
        assert!(back.get_pixel(3, 1).0[..3].iter().all(|c| *c > 150), "{:?}", back.get_pixel(3, 1));
    }

    #[test]
    fn names_and_extensions_are_recognised() {
        assert_eq!(picture_format("JPG"), Some("jpeg"));
        assert_eq!(picture_format("tif"), Some("tiff"));
        assert_eq!(picture_format("svg"), None);
        assert_eq!(picture_format_for_path("/tmp/a.b/Slide.TIFF"), Some("tiff"));
        assert_eq!(picture_format_for_path("out.bmp"), Some("bmp"));
        assert_eq!(picture_format_for_path("out"), None);
        assert_eq!(picture_format_for_path("out.pptx"), None);
    }

    #[test]
    fn an_unknown_type_or_an_empty_image_is_an_error() {
        assert!(encode(&image(), "svg", 1.0).is_err());
        assert!(encode(&Image { width: 0, height: 0, pixels: vec![] }, "png", 1.0).is_err());
        assert!(encode(&Image { width: 4, height: 2, pixels: vec![0; 3] }, "bmp", 1.0).is_err());
    }
}

#[cfg(test)]
mod slide_export_patch_tests {
    use serde_json::json;

    use crate::Session;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("deckcraft-picexport-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn slides_export_as_tiff_and_bmp_by_name_or_format() {
        let mut s = Session::with_new();
        s.execute("slide.new", &json!({})).unwrap();
        let dir = scratch("slides");
        for (name, want) in [("one.tif", image::ImageFormat::Tiff), ("one.tiff", image::ImageFormat::Tiff), ("one.bmp", image::ImageFormat::Bmp)] {
            let path = dir.join(name);
            let r = s.execute("file.export", &json!({"path": path.to_string_lossy(), "slide": 0, "scale": 0.25})).unwrap();
            assert_eq!(r["files"].as_array().unwrap().len(), 1);
            let bytes = std::fs::read(&path).unwrap();
            assert_eq!(image::guess_format(&bytes).unwrap(), want, "{name}");
            let img = image::load_from_memory(&bytes).unwrap();
            assert_eq!((img.width(), img.height()), (240, 135), "{name}");
        }
        // Every slide, numbered like PNG export; and `format` given outright.
        let all = dir.join("deck.bmp");
        let r = s.execute("file.export", &json!({"path": all.to_string_lossy(), "all": true, "scale": 0.1})).unwrap();
        let files: Vec<String> = r["files"].as_array().unwrap().iter().map(|f| f.as_str().unwrap().to_string()).collect();
        assert_eq!(files.len(), 2);
        assert!(files[0].ends_with("deck1.bmp") && files[1].ends_with("deck2.bmp"), "{files:?}");
        let named = dir.join("named.dat");
        s.execute("file.export", &json!({"path": named.to_string_lossy(), "format": "tiff", "slide": 0, "scale": 0.1})).unwrap();
        assert_eq!(image::guess_format(&std::fs::read(&named).unwrap()).unwrap(), image::ImageFormat::Tiff);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_transparent_background_is_for_png_and_tiff_only() {
        let mut s = Session::with_new();
        let dir = scratch("clear");
        let tif = dir.join("clear.tif");
        s.execute("file.export", &json!({"path": tif.to_string_lossy(), "slide": 0, "scale": 0.1, "transparentBackground": true})).unwrap();
        let img = image::load_from_memory(&std::fs::read(&tif).unwrap()).unwrap().to_rgba8();
        assert!(img.pixels().any(|p| p.0[3] == 0), "the slide background was left out");
        let bmp = dir.join("clear.bmp");
        assert!(s.execute("file.export", &json!({"path": bmp.to_string_lossy(), "slide": 0, "transparentBackground": true})).is_err());
        assert!(!bmp.exists());
        // Saving the presentation under a picture name is still not a picture export.
        assert_eq!(crate::cmd::file::writable_format_for_path("a.bmp"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
