//! SlideCraft's Variables pane (Phase 4.1): the deck's variable collections, their modes (switch
//! the deck's or this slide's mode in one click), each variable's value per mode, and binding the
//! selection to a variable. Every control runs a `variable.*` command.

use deckcraft_model::variables::{BIND_FIELDS, DesignSystem, VarKind, VarValue};
use egui::{Color32, Sense, Stroke, Ui, vec2};
use serde_json::{Value, json};

use super::{SlideApp, run, theme, widgets};

/// Pane state kept in egui's memory (not saved).
#[derive(Clone, Default)]
struct PaneState {
    collection: Option<u32>,
    new_name: String,
    new_kind: usize,
    new_collection: String,
    new_mode: String,
}

const KINDS: [(VarKind, &str); 4] =
    [(VarKind::Color, "Color"), (VarKind::Number, "Number"), (VarKind::String, "String"), (VarKind::Boolean, "Boolean")];

fn rgb32(c: deckcraft_color::Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

pub(crate) fn variables(app: &mut SlideApp, ui: &mut Ui) {
    let id = egui::Id::unique("variables-pane");
    let mut st: PaneState = ui.data_mut(|d| d.get_temp(id)).unwrap_or_default();
    let Some(doc) = app.session.active() else { return };
    let ds: DesignSystem = doc.doc.design_system().cloned().unwrap_or_default();
    let slide_modes = doc.doc.slides.get(doc.selection.slide).map(|s| s.modes.clone()).unwrap_or_default();
    let slide_index = doc.selection.slide;
    let has_selection = !doc.selection.shapes.is_empty();

    // Collections.
    if st.collection.is_none_or(|c| ds.collection(c).is_none()) {
        st.collection = ds.collections.first().map(|c| c.id);
    }
    ui.horizontal(|ui| {
        ui.label("Collection");
        let current = st.collection.and_then(|c| ds.collection(c)).map(|c| c.name.clone()).unwrap_or_else(|| "None".into());
        craft_ui::ComboBox::from_id_salt("var-collection").selected_text(current).show_ui(ui, |ui| {
            for c in &ds.collections {
                ui.selectable_value(&mut st.collection, Some(c.id), &c.name);
            }
        });
    });
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut st.new_collection).hint_text("New collection").desired_width(140.0));
        if ui.button("Add Collection").clicked() && !st.new_collection.trim().is_empty() {
            if let Ok(r) = app.run("variable.createCollection", json!({"name": st.new_collection.trim(), "modes": ["Light", "Dark"]})) {
                st.collection = r["id"].as_u64().and_then(|v| u32::try_from(v).ok());
            }
            st.new_collection.clear();
        }
    });
    let Some(coll) = st.collection.and_then(|c| ds.collection(c)).cloned() else {
        ui.add_space(8.0);
        ui.label(egui::RichText::new("No variables yet. A collection holds variables with a value per mode, such as Light and Dark.").weak());
        if ui.button("Theme Variables").on_hover_text("Make the theme a Light and Dark collection the layout library uses").clicked() {
            run(app, "design.kit", json!({}));
        }
        ui.data_mut(|d| d.insert_temp(id, st));
        return;
    };

    // Modes: the deck's, then this slide's.
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Mode").font(theme::bold(13.0)));
    let deck_mode = ds.deck_mode(coll.id);
    ui.horizontal_wrapped(|ui| {
        for m in &coll.modes {
            if ui.selectable_label(deck_mode == Some(m.id), &m.name).on_hover_text("The deck's mode").clicked() && deck_mode != Some(m.id) {
                run(app, "variable.mode", json!({"collection": coll.id, "mode": m.id}));
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("This slide");
        let own = slide_modes.iter().find(|(c, _)| *c == coll.id).map(|(_, m)| *m);
        let text = own.and_then(|m| coll.mode(m)).map_or_else(|| "Follow deck".to_string(), |m| m.name.clone());
        craft_ui::ComboBox::from_id_salt("var-slide-mode").selected_text(text).show_ui(ui, |ui| {
            if ui.selectable_label(own.is_none(), "Follow deck").clicked() && own.is_some() {
                run(app, "variable.mode", json!({"collection": coll.id, "mode": null, "slides": [slide_index]}));
            }
            for m in &coll.modes {
                if ui.selectable_label(own == Some(m.id), &m.name).clicked() && own != Some(m.id) {
                    run(app, "variable.mode", json!({"collection": coll.id, "mode": m.id, "slides": [slide_index]}));
                }
            }
        });
    });
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut st.new_mode).hint_text("New mode").desired_width(140.0));
        if ui.button("Add Mode").clicked() && !st.new_mode.trim().is_empty() {
            run(app, "variable.addMode", json!({"collection": coll.id, "name": st.new_mode.trim()}));
            st.new_mode.clear();
        }
    });

    // Variables, one row each with a value per mode.
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Variables").font(theme::bold(13.0)));
    let vars: Vec<_> = ds.variables.iter().filter(|v| v.collection == coll.id).cloned().collect();
    egui::Grid::new("var-grid").num_columns(coll.modes.len() + 2).striped(true).show(ui, |ui| {
        ui.label(egui::RichText::new("Name").weak());
        for m in &coll.modes {
            ui.label(egui::RichText::new(&m.name).weak());
        }
        ui.label("");
        ui.end_row();
        for v in &vars {
            let label = match v.theme {
                Some(t) => format!("{} ({})", v.name, t.name()),
                None => v.name.clone(),
            };
            ui.label(label).on_hover_text(if v.description.is_empty() { v.kind.name() } else { &v.description });
            for m in &coll.modes {
                value_cell(app, ui, &ds, v.id, m.id, v.value_in(m.id).cloned());
            }
            ui.menu_button("⋯", |ui| {
                ui.add_enabled_ui(has_selection, |ui| bind_menu(app, ui, v.id, v.kind));
                ui.separator();
                if ui.button("Delete Variable").clicked() {
                    run(app, "variable.delete", json!({"variable": v.id}));
                    ui.close();
                }
            });
            ui.end_row();
        }
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut st.new_name).hint_text("color/brand").desired_width(110.0));
        craft_ui::ComboBox::from_id_salt("var-kind").width(80.0).selected_text(KINDS.get(st.new_kind).map_or("", |k| k.1)).show_ui(ui, |ui| {
            for (i, (_, label)) in KINDS.iter().enumerate() {
                ui.selectable_value(&mut st.new_kind, i, *label);
            }
        });
        if ui.button("Add Variable").clicked() && !st.new_name.trim().is_empty() {
            let kind = KINDS.get(st.new_kind).map_or(VarKind::Color, |k| k.0);
            let value = match kind {
                VarKind::Color => json!("#808080"),
                VarKind::Number => json!(0),
                VarKind::String => json!(""),
                VarKind::Boolean => json!(true),
            };
            run(app, "variable.create", json!({"collection": coll.id, "name": st.new_name.trim(), "type": kind.name(), "value": value}));
            st.new_name.clear();
        }
    });
    if has_selection {
        ui.add_space(4.0);
        if ui.button("Detach Variables").on_hover_text("The selection keeps its current look").clicked() {
            run(app, "variable.unbind", json!({}));
        }
    }
    ui.data_mut(|d| d.insert_temp(id, st));
}

fn bind_menu(app: &mut SlideApp, ui: &mut Ui, var: u32, kind: VarKind) {
    let mut fields: Vec<(&str, &str)> = vec![];
    if kind == VarKind::Color {
        fields.extend([("fill", "Bind Fill"), ("line", "Bind Outline"), ("textColor", "Bind Text Color")]);
    }
    let labels: Vec<(String, &str)> = BIND_FIELDS.iter().filter(|f| f.kind() == kind).map(|f| (format!("Bind {}", f.name()), f.name())).collect();
    for (field, label) in fields {
        if ui.button(label).clicked() {
            run(app, "variable.bind", json!({"field": field, "variable": var}));
            ui.close();
        }
    }
    for (label, field) in labels {
        if ui.button(label).clicked() {
            run(app, "variable.bind", json!({"field": field, "variable": var}));
            ui.close();
        }
    }
}

fn set(app: &mut SlideApp, var: u32, mode: u32, value: Value) {
    run(app, "variable.set", json!({"variable": var, "mode": mode, "value": value}));
}

/// One value: a swatch with a colour grid, a number spinner, a text field or a checkbox; an alias
/// shows the variable it follows.
fn value_cell(app: &mut SlideApp, ui: &mut Ui, ds: &DesignSystem, var: u32, mode: u32, v: Option<VarValue>) {
    let key = format!("var-{var}-{mode}");
    match v {
        Some(VarValue::Alias(a)) => {
            let name = ds.variable(a).map_or_else(|| "?".to_string(), |t| t.name.clone());
            ui.label(egui::RichText::new(format!("→ {name}")).italics());
        }
        Some(VarValue::Color(c)) => {
            let (r, resp) = ui.allocate_exact_size(vec2(36.0, 18.0), Sense::click());
            ui.painter().rect(r, 3.0, rgb32(c), Stroke::new(1.0, Color32::GRAY), egui::StrokeKind::Inside);
            let resp = resp.on_hover_text(format!("#{}", c.hex()));
            let sc = super::scheme(app);
            widgets::color_popup(&resp).show(|ui| {
                if let Some(Some(cr)) = widgets::color_grid(ui, &sc, None) {
                    let rgb = cr.resolve(&sc, None);
                    set(app, var, mode, json!(format!("#{}", rgb.hex())));
                }
            });
        }
        Some(VarValue::Number(n)) => {
            if let Some(v) = widgets::spinner(ui, &key, "Value", n, 1.0, "", 60.0) {
                set(app, var, mode, json!(v));
            }
        }
        Some(VarValue::String(s)) => {
            let id = egui::Id::unique(&key);
            let mut text: String = ui.data_mut(|d| d.get_temp(id)).unwrap_or_else(|| s.clone());
            let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(90.0));
            if r.lost_focus() && text != s {
                set(app, var, mode, json!(text));
            }
            if r.has_focus() {
                ui.data_mut(|d| d.insert_temp(id, text));
            } else {
                ui.data_mut(|d| d.remove::<String>(id));
            }
        }
        Some(VarValue::Boolean(b)) => {
            let mut on = b;
            if ui.checkbox(&mut on, "").changed() {
                set(app, var, mode, json!(on));
            }
        }
        None => {
            ui.label("—");
        }
    }
}
