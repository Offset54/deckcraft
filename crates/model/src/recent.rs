//! Recent Colors: the custom colours last picked from More Colors or with the Eyedropper, kept
//! with the presentation (`p:clrMru` in a .pptx) and offered again in every colour menu.

use crate::{Presentation, Rgba};

/// How many recent colours a presentation keeps (one row of a colour menu).
pub const MAX_RECENT_COLORS: usize = 10;

impl Presentation {
    /// Put `color` at the front of the recent colours (opaque; a colour already there moves up
    /// rather than appearing twice) and drop the oldest beyond [`MAX_RECENT_COLORS`]. Returns
    /// whether the list changed.
    pub fn note_recent_color(&mut self, color: Rgba) -> bool {
        let color = color.with_alpha(255);
        if self.recent_colors.first() == Some(&color) && self.recent_colors.len() <= MAX_RECENT_COLORS {
            return false;
        }
        self.recent_colors.retain(|c| *c != color);
        self.recent_colors.insert(0, color);
        self.recent_colors.truncate(MAX_RECENT_COLORS);
        true
    }
}

#[cfg(test)]
mod patch_tests {
    use super::MAX_RECENT_COLORS;
    use crate::{Presentation, Rgba};

    #[test]
    fn newest_first_without_repeats_and_capped() {
        let mut p = Presentation::default();
        assert!(p.recent_colors.is_empty());
        assert!(p.note_recent_color(Rgba::rgb(1, 2, 3)));
        assert!(p.note_recent_color(Rgba::rgb(4, 5, 6)));
        // Picking the newest again changes nothing; an older one moves to the front.
        assert!(!p.note_recent_color(Rgba::rgb(4, 5, 6)));
        assert!(p.note_recent_color(Rgba::rgb(1, 2, 3).with_alpha(9)));
        assert_eq!(p.recent_colors, vec![Rgba::rgb(1, 2, 3), Rgba::rgb(4, 5, 6)]);
        for k in 0..30 {
            p.note_recent_color(Rgba::rgb(k, 0, 0));
        }
        assert_eq!(p.recent_colors.len(), MAX_RECENT_COLORS);
        assert_eq!(p.recent_colors.first(), Some(&Rgba::rgb(29, 0, 0)));
        assert_eq!(p.recent_colors.last(), Some(&Rgba::rgb(20, 0, 0)));
    }

    /// A presentation saved before there were recent colours opens with none, and one with none
    /// is saved without the field.
    #[test]
    fn the_field_is_optional_in_saved_files() {
        let mut p = Presentation::default();
        let json = serde_json::to_value(&p).unwrap();
        assert!(json.get("recentColors").is_none());
        let back: Presentation = serde_json::from_value(json).unwrap();
        assert!(back.recent_colors.is_empty());
        p.note_recent_color(Rgba::rgb(0x12, 0x34, 0x56));
        let back: Presentation = serde_json::from_value(serde_json::to_value(&p).unwrap()).unwrap();
        assert_eq!(back.recent_colors, vec![Rgba::rgb(0x12, 0x34, 0x56)]);
    }
}
