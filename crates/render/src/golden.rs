//! Comparing a render with a stored picture of it (golden-image tests).
//!
//! Two renders of the same slide are not always the same bytes: `vello_cpu` picks a SIMD level
//! for the machine it runs on, and anti-aliased edges can come out a level or two apart. So a
//! comparison counts the pixels whose colour is further from the stored one than a channel
//! tolerance, and passes while that count stays under a share of the picture.
//! [`compare`] does the counting, [`Tolerance`] holds the two limits and [`diff_image`] draws
//! where two pictures differ, for a person to look at when a test fails.

use super::Image;

/// How far a render may be from its stored picture and still match.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tolerance {
    /// A pixel differs when any of its straight RGBA channels is further than this from the
    /// stored one.
    pub channel: u8,
    /// The share of the picture's pixels (0 to 1) that may differ.
    pub pixels: f64,
}

impl Tolerance {
    /// Nothing may differ.
    pub const EXACT: Tolerance = Tolerance { channel: 0, pixels: 0.0 };
    /// What slide renders are held to: an edge may be a few levels off on another machine, but
    /// a moved glyph, a changed colour or a missing shape is well past this.
    pub const SLIDE: Tolerance = Tolerance { channel: 24, pixels: 0.002 };
}

/// What [`compare`] found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Diff {
    /// Pixels compared.
    pub pixels: usize,
    /// Pixels with a channel past the tolerance's `channel`.
    pub differing: usize,
    /// The largest difference in any channel of any pixel.
    pub max_delta: u8,
}

impl Diff {
    /// The share of the picture that differs (0 to 1; 0 for an empty picture).
    pub fn share(&self) -> f64 {
        if self.pixels == 0 { 0.0 } else { self.differing as f64 / self.pixels as f64 }
    }
    pub fn within(&self, t: Tolerance) -> bool {
        self.share() <= t.pixels.clamp(0.0, 1.0)
    }
}

/// Why two pictures can't be compared pixel by pixel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mismatch {
    /// The pictures are different sizes: (width, height) of the render, then of the stored one.
    Size((u32, u32), (u32, u32)),
    /// A picture's pixel buffer is shorter than its width and height say.
    Truncated,
}

impl std::fmt::Display for Mismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mismatch::Size(a, b) => write!(f, "the render is {}×{} and the stored picture is {}×{}", a.0, a.1, b.0, b.1),
            Mismatch::Truncated => write!(f, "a picture has fewer pixels than its size says"),
        }
    }
}

/// Bytes a `width` × `height` RGBA8 picture needs; `None` when that overflows.
fn byte_len(width: u32, height: u32) -> Option<usize> {
    (width as usize).checked_mul(height as usize)?.checked_mul(4)
}

/// The two pictures as straight RGBA, checked to be the same size and whole.
fn straight_pair(render: &Image, stored: &Image) -> Result<(Vec<u8>, Vec<u8>), Mismatch> {
    if (render.width, render.height) != (stored.width, stored.height) {
        return Err(Mismatch::Size((render.width, render.height), (stored.width, stored.height)));
    }
    let len = byte_len(render.width, render.height).ok_or(Mismatch::Truncated)?;
    let (a, b) = (render.to_straight(), stored.to_straight());
    if a.len() < len || b.len() < len {
        return Err(Mismatch::Truncated);
    }
    Ok((a, b))
}

/// The largest channel difference between two RGBA pixels.
fn pixel_delta(a: &[u8], b: &[u8]) -> u8 {
    a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0)
}

/// Count the pixels of `render` that are further than `channel` from `stored` in any straight
/// RGBA channel. Both are compared unpremultiplied, the way a PNG stores them.
pub fn compare(render: &Image, stored: &Image, channel: u8) -> Result<Diff, Mismatch> {
    let (a, b) = straight_pair(render, stored)?;
    let mut diff = Diff { pixels: 0, differing: 0, max_delta: 0 };
    for (pa, pb) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let d = pixel_delta(pa, pb);
        diff.pixels += 1;
        diff.max_delta = diff.max_delta.max(d);
        if d > channel {
            diff.differing += 1;
        }
    }
    Ok(diff)
}

/// A picture of where `render` and `stored` differ: the stored picture faded toward white, with
/// every pixel past `channel` in red. `None` when they can't be compared.
pub fn diff_image(render: &Image, stored: &Image, channel: u8) -> Option<Image> {
    let (a, b) = straight_pair(render, stored).ok()?;
    let mut pixels = Vec::with_capacity(a.len());
    for (pa, pb) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        if pixel_delta(pa, pb) > channel {
            pixels.extend_from_slice(&[230, 0, 0, 255]);
        } else {
            // Opaque, so premultiplied and straight are the same bytes.
            pixels.extend(pb.iter().take(3).map(|c| 191 + c / 4));
            pixels.push(255);
        }
    }
    Some(Image { width: stored.width, height: stored.height, pixels })
}

/// A stored PNG as an [`Image`] (premultiplied, like a render). `None` when it doesn't decode.
pub fn decode_png(bytes: &[u8]) -> Option<Image> {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?.into_rgba8();
    let (width, height) = (img.width(), img.height());
    let mut pixels = img.into_raw();
    for px in pixels.chunks_exact_mut(4) {
        let a = px.get(3).copied().unwrap_or(255) as u32;
        if a != 255 {
            for c in px.iter_mut().take(3) {
                *c = ((*c as u32 * a + 127) / 255) as u8;
            }
        }
    }
    Some(Image { width, height, pixels })
}

#[cfg(test)]
mod tests {
    use super::{Diff, Mismatch, Tolerance, compare, decode_png, diff_image};
    use crate::Image;

    fn solid(w: u32, h: u32, px: [u8; 4]) -> Image {
        Image { width: w, height: h, pixels: px.repeat((w * h) as usize) }
    }

    #[test]
    fn the_same_picture_matches_exactly() {
        let a = solid(8, 4, [10, 20, 30, 255]);
        let d = compare(&a, &a.clone(), 0).unwrap();
        assert_eq!(d, Diff { pixels: 32, differing: 0, max_delta: 0 });
        assert!(d.within(Tolerance::EXACT));
    }

    #[test]
    fn a_small_edge_difference_is_within_tolerance_and_a_changed_area_is_not() {
        let stored = solid(100, 100, [200, 200, 200, 255]);
        // Ten pixels a few levels off: an anti-aliased edge on another machine.
        let mut edge = stored.clone();
        for px in edge.pixels.chunks_exact_mut(4).take(10) {
            px[0] = 190;
        }
        let d = compare(&edge, &stored, Tolerance::SLIDE.channel).unwrap();
        assert_eq!((d.differing, d.max_delta), (0, 10));
        assert!(d.within(Tolerance::SLIDE));
        assert!(!compare(&edge, &stored, 0).unwrap().within(Tolerance::EXACT), "exact comparison sees them");
        // A hundred pixels of another colour: a shape moved or changed.
        let mut moved = stored.clone();
        for px in moved.pixels.chunks_exact_mut(4).take(100) {
            px[1] = 20;
        }
        let d = compare(&moved, &stored, Tolerance::SLIDE.channel).unwrap();
        assert_eq!((d.differing, d.max_delta), (100, 180));
        assert!(!d.within(Tolerance::SLIDE), "1% of the picture changed");
    }

    #[test]
    fn pictures_of_different_sizes_or_short_buffers_are_refused() {
        let a = solid(4, 4, [0, 0, 0, 255]);
        assert_eq!(compare(&a, &solid(4, 5, [0, 0, 0, 255]), 0), Err(Mismatch::Size((4, 4), (4, 5))));
        let short = Image { width: 4, height: 4, pixels: vec![0; 8] };
        assert_eq!(compare(&a, &short, 0), Err(Mismatch::Truncated));
        assert!(diff_image(&a, &short, 0).is_none());
        // A hostile size must not overflow the length check.
        let huge = Image { width: u32::MAX, height: u32::MAX, pixels: vec![] };
        assert_eq!(compare(&huge, &huge.clone(), 0), Err(Mismatch::Truncated));
    }

    #[test]
    fn the_diff_picture_marks_only_the_changed_pixels() {
        let stored = solid(2, 1, [0, 0, 0, 255]);
        let mut render = stored.clone();
        render.pixels[4] = 255;
        let d = diff_image(&render, &stored, 16).unwrap();
        assert_eq!(d.pixels, [191, 191, 191, 255, 230, 0, 0, 255]);
    }

    #[test]
    fn a_png_round_trip_compares_equal_even_with_transparency() {
        // Premultiplied half-transparent red, an opaque pixel and a clear one.
        let img = Image { width: 3, height: 1, pixels: vec![128, 0, 0, 128, 10, 20, 30, 255, 0, 0, 0, 0] };
        let back = decode_png(&img.to_png()).unwrap();
        assert_eq!((back.width, back.height), (3, 1));
        assert_eq!(compare(&img, &back, 0).unwrap().differing, 0);
        assert!(decode_png(b"not a png").is_none());
    }
}
