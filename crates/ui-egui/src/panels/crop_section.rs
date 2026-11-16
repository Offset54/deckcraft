//! The Crop section of the Format Picture pane: the picture's size and offset inside its crop
//! frame, and the frame's own size and place. Each field runs `picture.cropPosition`; the values
//! shown come from `picture.cropInfo`. Also the Aspect Ratio list of the ribbon's Crop menu.

use egui::{Ui, vec2};
use serde_json::{Value, json};

use crate::theme;
use crate::{SlideApp, widgets};

/// Crop › Aspect Ratio, as grouped in the menu.
pub(crate) const ASPECT_RATIOS: &[(&str, &[&str])] =
    &[("Square", &["1:1"]), ("Portrait", &["2:3", "3:4", "3:5", "4:5"]), ("Landscape", &["3:2", "4:3", "5:3", "5:4", "16:9", "16:10"])];

/// The Aspect Ratio, Fill and Fit entries of the Crop menu.
pub(crate) fn crop_menu_items(app: &mut SlideApp, ui: &mut Ui) {
    ui.menu_button("Aspect Ratio", |ui| {
        for (group, ratios) in ASPECT_RATIOS {
            ui.label(egui::RichText::new(*group).font(theme::bold(12.0)));
            for ratio in *ratios {
                if ui.button(*ratio).clicked() {
                    let _ = app.run("picture.cropAspect", json!({"ratio": ratio}));
                    ui.close();
                }
            }
        }
    });
    if ui.button("Fill").on_hover_text("The picture covers the frame; what overflows is cropped").clicked() {
        let _ = app.run("picture.cropFill", json!({}));
        ui.close();
    }
    if ui.button("Fit").on_hover_text("The whole picture shows inside the frame").clicked() {
        let _ = app.run("picture.cropFit", json!({}));
        ui.close();
    }
}

/// The fields, in the pane's order: (heading, [(label, key)]).
const FIELDS: &[(&str, &[(&str, &str)])] = &[
    ("Picture position", &[("Width", "pictureWidth"), ("Height", "pictureHeight"), ("Offset X", "offsetX"), ("Offset Y", "offsetY")]),
    ("Crop position", &[("Width", "cropWidth"), ("Height", "cropHeight"), ("Left", "cropLeft"), ("Top", "cropTop")]),
];

/// Draw the Crop section for the selected picture `id`.
pub(super) fn crop_section(app: &mut SlideApp, ui: &mut Ui, id: u32) {
    let Ok(info) = app.session.execute("picture.cropInfo", &json!({"id": id})) else { return };
    let len = app.session.prefs.units;
    let (unit, suffix) = (len.points_per_unit(), len.suffix());
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Crop").font(theme::bold(13.0)));
    for (heading, fields) in FIELDS {
        ui.label(*heading);
        for (label, key) in *fields {
            let Some(points) = info.get(*key).and_then(Value::as_f64) else { continue };
            ui.horizontal(|ui| {
                ui.add_sized(vec2(90.0, 20.0), egui::Label::new(*label));
                // Named in full for screen readers and tests: "Width" alone appears twice.
                let name = format!("{heading} {label}");
                let shown = points / unit;
                // Leaving a field untouched commits its two-decimal rendering; only a real change
                // is sent, so tabbing through the fields does not nudge the picture.
                if let Some(typed) = widgets::spinner(ui, &format!("crop-{key}-{id}"), &name, shown, 0.1, suffix, 70.0)
                    && widgets::fmt_num(typed, "") != widgets::fmt_num(shown, "")
                {
                    let _ = app.run("picture.cropPosition", json!({"id": id, *key: typed * unit}));
                }
            });
        }
    }
    ui.horizontal(|ui| {
        if ui.button("Fill").clicked() {
            let _ = app.run("picture.cropFill", json!({"ids": [id]}));
        }
        if ui.button("Fit").clicked() {
            let _ = app.run("picture.cropFit", json!({"ids": [id]}));
        }
        if ui.button("Reset Crop").clicked() {
            let _ = app.run("picture.crop", json!({"left": 0.0, "top": 0.0, "right": 0.0, "bottom": 0.0, "ids": [id]}));
        }
    });
    if let Some([w, h]) = info.get("pixels").and_then(Value::as_array).map(Vec::as_slice)
        && let (Some(w), Some(h)) = (w.as_u64(), h.as_u64())
    {
        ui.label(egui::RichText::new(format!("{w} × {h} pixels")).color(theme::Tokens::get(ui.ctx()).text_dim));
    }
}
