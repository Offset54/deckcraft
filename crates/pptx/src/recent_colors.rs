//! Recent Colors in a .pptx: `p:clrMru` in `ppt/presProps.xml` (ECMA-376 Part 1, §19.2.1.4), the
//! custom colours last picked, newest first. Only literal `a:srgbClr` entries are kept.

use deckcraft_model::recent::MAX_RECENT_COLORS;
use deckcraft_model::{Presentation, Rgba};

use crate::xml::{El, W};

/// The recent colours `p:presentationPr` lists, at most [`MAX_RECENT_COLORS`].
pub(crate) fn read(props: &El) -> Vec<Rgba> {
    let Some(mru) = props.child("clrMru") else { return Vec::new() };
    let mut out: Vec<Rgba> = Vec::new();
    // A hostile file may list any number of entries; look at a bounded few.
    for el in mru.children_named("srgbClr").take(MAX_RECENT_COLORS * 8) {
        let Some(c) = el.attr("val").filter(|v| v.len() == 6).and_then(Rgba::from_hex) else { continue };
        let c = c.with_alpha(255);
        if !out.contains(&c) {
            out.push(c);
        }
        if out.len() >= MAX_RECENT_COLORS {
            break;
        }
    }
    out
}

/// Write `p:clrMru` (after `p:showPr`, where the schema puts it); nothing when there are none.
pub(crate) fn write(w: &mut W, p: &Presentation) {
    if p.recent_colors.is_empty() {
        return;
    }
    w.open0("p:clrMru");
    for c in p.recent_colors.iter().take(MAX_RECENT_COLORS) {
        w.val("a:srgbClr", c.hex());
    }
    w.close("p:clrMru");
}

#[cfg(test)]
mod patch_tests {
    use deckcraft_model::{Presentation, Rgba};

    fn props_xml(p: &Presentation) -> String {
        let bytes = crate::export(p).expect("export");
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("zip");
        let mut x = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("ppt/presProps.xml").expect("presProps"), &mut x).expect("read");
        x
    }

    #[test]
    fn recent_colours_survive_save_and_reopen_in_order() {
        let mut p = Presentation::default();
        // None: nothing is written.
        assert!(!props_xml(&p).contains("clrMru"));
        for c in [Rgba::rgb(0x11, 0x22, 0x33), Rgba::rgb(0xAB, 0xCD, 0xEF), Rgba::rgb(0, 0, 0)] {
            p.note_recent_color(c);
        }
        let xml = props_xml(&p);
        // After p:showPr, as the schema orders the children of p:presentationPr.
        let (show, mru) = (xml.find("</p:showPr>").expect("showPr"), xml.find("<p:clrMru>").expect("clrMru"));
        assert!(show < mru, "{xml}");
        let back = crate::import(&crate::export(&p).expect("export")).expect("import");
        assert_eq!(back.recent_colors, vec![Rgba::rgb(0, 0, 0), Rgba::rgb(0xAB, 0xCD, 0xEF), Rgba::rgb(0x11, 0x22, 0x33)]);
    }

    #[test]
    fn a_hostile_list_is_read_bounded_and_clean() {
        let mut xml = String::from(
            r#"<p:presentationPr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:clrMru>"#,
        );
        xml.push_str(r#"<a:srgbClr val="zzzzzz"/><a:srgbClr val="12345"/><a:srgbClr/><a:schemeClr val="accent1"/><a:srgbClr val="FF0000"/><a:srgbClr val="ff0000"/>"#);
        for k in 0..5000 {
            xml.push_str(&format!(r#"<a:srgbClr val="{k:06X}"/>"#));
        }
        xml.push_str("</p:clrMru></p:presentationPr>");
        let doc = crate::xml::parse(xml.as_bytes()).expect("xml");
        let got = super::read(&doc.root);
        assert_eq!(got.len(), deckcraft_model::recent::MAX_RECENT_COLORS);
        assert_eq!(got.first(), Some(&Rgba::rgb(0xFF, 0, 0)));
        // The repeated red was taken once.
        assert_eq!(got.iter().filter(|c| **c == Rgba::rgb(0xFF, 0, 0)).count(), 1);
    }
}
