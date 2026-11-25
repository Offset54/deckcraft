//! SlideCraft's built-in design kit (Phase 4.4): every theme as variables with a Light and a Dark
//! mode, so the theme gallery, the layout library and a deck's own slides restyle together.
//!
//! The kit is one collection, `Theme`:
//! - the theme's 12 colours, each mapped to its slot (Light: the theme as designed; Dark: derived
//!   here so that text, accents and links keep WCAG AA contrast on the dark background);
//! - semantic colours the layouts use: `color/surface` (cards), `color/on-accent` (text on the
//!   first accent), `color/border`;
//! - a spacing scale (`space/xs`…`space/xl`), corner radii (`radius/sm`…`radius/lg`) and a type
//!   scale (`type/display`…`type/caption`, `type/big-number`), varied a little per theme;
//! - the heading and body fonts, mapped to the theme's font scheme.
//!
//! Our colours and numbers; nothing taken from another product's themes.

use deckcraft_color::{ColorScheme, Rgba, SchemeSlot};

use crate::Theme;
use crate::variables::{ThemeSlot, VarKind, VarValue};

/// Name of the kit's collection and its two modes.
pub const COLLECTION: &str = "Theme";
pub const LIGHT: &str = "Light";
pub const DARK: &str = "Dark";

/// One variable of the kit: name, kind, theme slot, Light value, Dark value.
#[derive(Clone, Debug, PartialEq)]
pub struct KitVar {
    pub name: &'static str,
    pub kind: VarKind,
    pub theme: Option<ThemeSlot>,
    pub light: VarValue,
    pub dark: VarValue,
}

/// WCAG contrast of two colours.
pub fn contrast(a: Rgba, b: Rgba) -> f64 {
    a.contrast(b)
}

/// `c` made lighter (or darker, with `darker`) in steps until it reaches `min` contrast against
/// `bg`; at most 20 steps, so it always ends.
pub fn push_contrast(c: Rgba, bg: Rgba, min: f64, darker: bool) -> Rgba {
    let (h, s, mut l) = c.to_hsl();
    let mut out = c;
    for _ in 0..20 {
        if contrast(out, bg) >= min {
            return out;
        }
        l = if darker { (l - 0.04).max(0.0) } else { (l + 0.04).min(1.0) };
        out = Rgba::from_hsl(h, s, l, c.a);
    }
    out
}

fn with_lightness(c: Rgba, l: f64, max_sat: f64) -> Rgba {
    let (h, s, _) = c.to_hsl();
    Rgba::from_hsl(h, s.min(max_sat), l, 255)
}

/// The theme's Dark mode: a deep background with the theme's own hue (from its second dark
/// colour), light text, and accents and links lifted until they read on it.
pub fn dark_scheme(light: &ColorScheme) -> ColorScheme {
    let mut d = light.clone();
    let bg = with_lightness(light.get(SchemeSlot::Dk2), 0.09, 0.35);
    let bg2 = with_lightness(light.get(SchemeSlot::Dk2), 0.16, 0.30);
    let text = if light.get(SchemeSlot::Lt1).luminance() > 0.85 { light.get(SchemeSlot::Lt1) } else { Rgba::rgb(0xF5, 0xF6, 0xF8) };
    let text2 = push_contrast(with_lightness(light.get(SchemeSlot::Lt2), 0.85, 0.25), bg2, 4.5, false);
    d.set(SchemeSlot::Lt1, bg);
    d.set(SchemeSlot::Lt2, bg2);
    d.set(SchemeSlot::Dk1, text);
    d.set(SchemeSlot::Dk2, text2);
    for s in [SchemeSlot::Accent1, SchemeSlot::Accent2, SchemeSlot::Accent3, SchemeSlot::Accent4, SchemeSlot::Accent5, SchemeSlot::Accent6] {
        d.set(s, push_contrast(light.get(s), bg2, 3.0, false));
    }
    d.set(SchemeSlot::Hlink, push_contrast(light.get(SchemeSlot::Hlink), bg, 4.5, false));
    d.set(SchemeSlot::FolHlink, push_contrast(light.get(SchemeSlot::FolHlink), bg, 4.5, false));
    d
}

/// The Light mode: the theme as designed, except that text colours and links that would not
/// reach AA on the light backgrounds are darkened.
pub fn light_scheme(theme: &ColorScheme) -> ColorScheme {
    let mut l = theme.clone();
    let (bg, bg2) = (theme.get(SchemeSlot::Lt1), theme.get(SchemeSlot::Lt2));
    // A theme whose "light" background is dark (Nocturne) has its text pushed lighter instead.
    let d = is_light(bg);
    l.set(SchemeSlot::Dk1, push_contrast(theme.get(SchemeSlot::Dk1), bg2, 7.0, d));
    l.set(SchemeSlot::Dk2, push_contrast(theme.get(SchemeSlot::Dk2), bg2, 4.5, d));
    l.set(SchemeSlot::Hlink, push_contrast(theme.get(SchemeSlot::Hlink), bg, 4.5, d));
    l.set(SchemeSlot::FolHlink, push_contrast(theme.get(SchemeSlot::FolHlink), bg, 4.5, d));
    l
}

/// Whether text on `bg` should be darker than it (a light background), else lighter.
fn is_light(bg: Rgba) -> bool {
    bg.luminance() > 0.18
}

/// Black or white text, whichever reads better on `c` (white on a tie).
pub fn on(c: Rgba, dark: Rgba, light: Rgba) -> Rgba {
    if contrast(light, c) >= contrast(dark, c) { light } else { dark }
}

/// An accent that reads as body text (4.5:1, WCAG AA) on both backgrounds: the layouts use it for
/// small kickers as well as big numbers.
fn accent_text(accent: Rgba, bg: Rgba, bg2: Rgba) -> Rgba {
    let darker = is_light(bg);
    let a = push_contrast(accent, bg, 4.5, darker);
    push_contrast(a, bg2, 4.5, darker)
}

/// Per-theme spacing, radius and type personality: (radius md, type scale factor, space factor).
fn personality(theme_name: &str) -> (f64, f64, f64) {
    match theme_name {
        "Paper" => (2.0, 1.0, 1.0),
        "Harbor" | "Meadow" => (10.0, 1.0, 1.0),
        "Ember" => (6.0, 1.05, 1.0),
        "Nocturne" => (18.0, 1.0, 1.1),
        "Coral Reef" => (20.0, 1.0, 1.0),
        "Slate" => (8.0, 1.0, 1.0),
        _ => (12.0, 1.0, 1.0),
    }
}

const SLOT_NAMES: [(SchemeSlot, &str); 12] = [
    (SchemeSlot::Dk1, "color/text"),
    (SchemeSlot::Lt1, "color/background"),
    (SchemeSlot::Dk2, "color/text-2"),
    (SchemeSlot::Lt2, "color/background-2"),
    (SchemeSlot::Accent1, "color/accent-1"),
    (SchemeSlot::Accent2, "color/accent-2"),
    (SchemeSlot::Accent3, "color/accent-3"),
    (SchemeSlot::Accent4, "color/accent-4"),
    (SchemeSlot::Accent5, "color/accent-5"),
    (SchemeSlot::Accent6, "color/accent-6"),
    (SchemeSlot::Hlink, "color/link"),
    (SchemeSlot::FolHlink, "color/link-visited"),
];

/// The kit's variables for `theme`.
pub fn kit(theme: &Theme) -> Vec<KitVar> {
    let light = light_scheme(&theme.colors);
    let dark = dark_scheme(&light);
    let mut v: Vec<KitVar> = SLOT_NAMES
        .iter()
        .map(|(slot, name)| KitVar {
            name,
            kind: VarKind::Color,
            theme: Some(ThemeSlot::Color(*slot)),
            light: VarValue::Color(light.get(*slot)),
            dark: VarValue::Color(dark.get(*slot)),
        })
        .collect();
    let col = |s: &ColorScheme, slot| s.get(slot);
    let both = |f: &dyn Fn(&ColorScheme) -> Rgba| (VarValue::Color(f(&light)), VarValue::Color(f(&dark)));
    let (sl, sd) = both(&|s| col(s, SchemeSlot::Lt2));
    v.push(KitVar { name: "color/surface", kind: VarKind::Color, theme: None, light: sl, dark: sd });
    let (ol, od) = both(&|s| on(col(s, SchemeSlot::Accent1), col(s, SchemeSlot::Dk1).min_lum(col(s, SchemeSlot::Lt1)), Rgba::WHITE));
    v.push(KitVar { name: "color/on-accent", kind: VarKind::Color, theme: None, light: ol, dark: od });
    let (bl, bd) = both(&|s| col(s, SchemeSlot::Dk1).lerp(col(s, SchemeSlot::Lt1), 0.82));
    v.push(KitVar { name: "color/border", kind: VarKind::Color, theme: None, light: bl, dark: bd });
    let (al, ad) = (
        VarValue::Color(accent_text(col(&light, SchemeSlot::Accent1), col(&light, SchemeSlot::Lt1), col(&light, SchemeSlot::Lt2))),
        VarValue::Color(accent_text(col(&dark, SchemeSlot::Accent1), col(&dark, SchemeSlot::Lt1), col(&dark, SchemeSlot::Lt2))),
    );
    v.push(KitVar { name: "color/accent-text", kind: VarKind::Color, theme: None, light: al, dark: ad });
    let (radius, type_k, space_k) = personality(&theme.name);
    let num = |name: &'static str, n: f64| KitVar { name, kind: VarKind::Number, theme: None, light: VarValue::Number(n), dark: VarValue::Number(n) };
    for (name, n) in [("space/xs", 8.0), ("space/sm", 16.0), ("space/md", 24.0), ("space/lg", 40.0), ("space/xl", 64.0)] {
        v.push(num(name, (n * space_k).round()));
    }
    for (name, n) in [("radius/sm", radius / 2.0), ("radius/md", radius), ("radius/lg", radius * 2.0)] {
        v.push(num(name, n));
    }
    for (name, n) in
        [("type/display", 54.0), ("type/title", 36.0), ("type/heading", 24.0), ("type/body", 18.0), ("type/caption", 13.0), ("type/big-number", 96.0)]
    {
        v.push(num(name, (n * type_k).round()));
    }
    let font = |name: &'static str, slot: ThemeSlot, f: &str| KitVar {
        name,
        kind: VarKind::String,
        theme: Some(slot),
        light: VarValue::String(f.to_string()),
        dark: VarValue::String(f.to_string()),
    };
    v.push(font("font/heading", ThemeSlot::MajorFont, &theme.fonts.major.latin));
    v.push(font("font/body", ThemeSlot::MinorFont, &theme.fonts.minor.latin));
    v
}

trait MinLum {
    fn min_lum(self, other: Rgba) -> Rgba;
}
impl MinLum for Rgba {
    /// The darker of two colours.
    fn min_lum(self, other: Rgba) -> Rgba {
        if self.luminance() <= other.luminance() { self } else { other }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::builtin_themes;

    #[test]
    fn every_built_in_theme_reads_in_both_modes() {
        for t in builtin_themes() {
            let light = light_scheme(&t.colors);
            let dark = dark_scheme(&light);
            for (name, s) in [("light", &light), ("dark", &dark)] {
                let (bg, bg2) = (s.get(SchemeSlot::Lt1), s.get(SchemeSlot::Lt2));
                assert!(contrast(s.get(SchemeSlot::Dk1), bg) >= 7.0, "{} {name}: text", t.name);
                assert!(contrast(s.get(SchemeSlot::Dk1), bg2) >= 4.5, "{} {name}: text on surface", t.name);
                assert!(contrast(s.get(SchemeSlot::Dk2), bg2) >= 4.5, "{} {name}: second text", t.name);
                assert!(contrast(s.get(SchemeSlot::Hlink), bg) >= 4.5, "{} {name}: link", t.name);
            }
            for k in kit(&t) {
                if k.name == "color/on-accent" {
                    for (mode, s, v) in [("light", &light, &k.light), ("dark", &dark, &k.dark)] {
                        let VarValue::Color(c) = v else { panic!("colour") };
                        assert!(contrast(*c, s.get(SchemeSlot::Accent1)) >= 3.0, "{} {mode}: text on accent", t.name);
                    }
                }
            }
        }
    }

    #[test]
    fn the_kit_has_every_slot_once() {
        let k = kit(&Theme::default());
        let slots: Vec<_> = k.iter().filter_map(|v| v.theme).collect();
        assert_eq!(slots.len(), 14);
        let mut names: Vec<_> = k.iter().map(|v| v.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), k.len());
    }
}
