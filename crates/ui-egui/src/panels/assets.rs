//! SlideCraft's Assets pane (Phase 4.2–4.3): the deck's components, grouped by component set,
//! with Insert and Go to Main; the selected instance's properties, variant, swap, reset and
//! detach; and the paint, effect and grid styles. Every control runs a command
//! (`component.*`, `instance.*`, `designStyle.*`).

use deckcraft_model::component::{Component, PropValue};
use deckcraft_model::design_styles::StyleKind;
use deckcraft_model::{Fill, ShapeId};
use egui::{Color32, Sense, Stroke, Ui, vec2};
use serde_json::json;

use super::{SlideApp, run, theme};

#[derive(Clone, Default)]
struct PaneState {
    name: String,
    filter: String,
}

pub(crate) fn assets(app: &mut SlideApp, ui: &mut Ui) {
    let id = egui::Id::unique("assets-pane");
    let mut st: PaneState = ui.data_mut(|d| d.get_temp(id)).unwrap_or_default();
    let Some(doc) = app.session.active() else { return };
    let lib: Vec<Component> = doc.doc.design_system().map(|d| d.components.clone()).unwrap_or_default();
    let styles = doc.doc.design_system().map(|d| d.styles.clone()).unwrap_or_default();
    let scheme = super::scheme(app);
    let selection: Vec<ShapeId> = doc.selection.shapes.clone();
    let instance = selection.first().and_then(|i| doc.shape(*i)).and_then(|s| s.instance.as_deref().cloned().map(|n| (s.id, n)));
    let slide = doc.selection.slide;

    ui.add(egui::TextEdit::singleline(&mut st.filter).hint_text("Search assets").desired_width(f32::INFINITY));
    let wanted = |name: &str| st.filter.trim().is_empty() || name.to_lowercase().contains(&st.filter.trim().to_lowercase());

    // The selected instance.
    if let Some((iid, inst)) = &instance
        && let Some(comp) = lib.iter().find(|c| c.id == inst.component)
    {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(format!("Instance of {}", comp.display_name())).font(theme::bold(13.0)));
        // Variant axes of its set.
        if !comp.set.is_empty() {
            for (axis, cur) in &comp.variant {
                let mut values: Vec<String> = lib
                    .iter()
                    .filter(|c| c.set == comp.set)
                    .filter_map(|c| c.variant.iter().find(|(k, _)| k == axis).map(|(_, v)| v.clone()))
                    .collect();
                values.sort();
                values.dedup();
                ui.horizontal(|ui| {
                    ui.label(axis);
                    craft_ui::ComboBox::from_id_salt(("asset-variant", axis.as_str())).selected_text(cur).show_ui(ui, |ui| {
                        for v in &values {
                            if ui.selectable_label(v == cur, v).clicked() && v != cur {
                                run(app, "instance.set", json!({"ids": [iid.0], "variant": {axis.clone(): v}}));
                            }
                        }
                    });
                });
            }
        }
        for def in &comp.props {
            let cur = inst.prop(&def.name).cloned().unwrap_or_else(|| def.default.clone());
            ui.horizontal(|ui| {
                ui.label(&def.name);
                match cur {
                    PropValue::Text(t) => {
                        let key = egui::Id::unique(("asset-prop", iid.0, def.name.as_str()));
                        let mut text: String = ui.data_mut(|d| d.get_temp(key)).unwrap_or_else(|| t.clone());
                        let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(140.0));
                        if r.lost_focus() && text != t {
                            run(app, "instance.set", json!({"ids": [iid.0], "props": {def.name.clone(): text}}));
                        }
                        if r.has_focus() {
                            ui.data_mut(|d| d.insert_temp(key, text));
                        } else {
                            ui.data_mut(|d| d.remove::<String>(key));
                        }
                    }
                    PropValue::Boolean(b) => {
                        let mut on = b;
                        if ui.checkbox(&mut on, "").changed() {
                            run(app, "instance.set", json!({"ids": [iid.0], "props": {def.name.clone(): on}}));
                        }
                    }
                    PropValue::Swap(c) => {
                        let shown = lib.iter().find(|x| x.id == c).map_or_else(|| "?".into(), Component::display_name);
                        craft_ui::ComboBox::from_id_salt(("asset-swap", def.name.as_str())).selected_text(shown).show_ui(ui, |ui| {
                            for x in &lib {
                                if ui.selectable_label(x.id == c, x.display_name()).clicked() && x.id != c {
                                    run(app, "instance.set", json!({"ids": [iid.0], "props": {def.name.clone(): x.id}}));
                                }
                            }
                        });
                    }
                }
            });
        }
        ui.horizontal_wrapped(|ui| {
            craft_ui::ComboBox::from_id_salt("asset-swap-instance").selected_text("Swap…").show_ui(ui, |ui| {
                for x in &lib {
                    if ui.selectable_label(x.id == comp.id, x.display_name()).clicked() && x.id != comp.id {
                        run(app, "instance.swap", json!({"ids": [iid.0], "component": x.id}));
                    }
                }
            });
            if ui.button("Reset").on_hover_text("Drop overrides and property values").clicked() {
                run(app, "instance.reset", json!({"ids": [iid.0]}));
            }
            if ui.button("Detach").on_hover_text("Make it a plain frame").clicked() {
                run(app, "instance.detach", json!({"ids": [iid.0]}));
            }
            if ui.button("Go to Main").clicked() {
                run(app, "component.editMain", json!({"component": comp.id}));
            }
        });
        ui.separator();
    }

    // Components.
    ui.label(egui::RichText::new("Components").font(theme::bold(13.0)));
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut st.name).hint_text("Name").desired_width(120.0));
        let can = !selection.is_empty() && instance.is_none();
        if ui.add_enabled(can, egui::Button::new("Create Component")).on_hover_text("The selection becomes a main component").clicked() {
            let name = st.name.trim().to_string();
            let p = if name.is_empty() { json!({}) } else { json!({"name": name}) };
            run(app, "component.create", p);
            st.name.clear();
        }
    });
    if lib.is_empty() {
        ui.label(egui::RichText::new("No components yet. Select a frame or some objects and create one.").weak());
    }
    let mut sets: Vec<String> = lib.iter().map(|c| c.set.clone()).collect();
    sets.sort();
    sets.dedup();
    for set in sets {
        let members: Vec<&Component> = lib.iter().filter(|c| c.set == set && wanted(&c.display_name())).collect();
        if members.is_empty() {
            continue;
        }
        if !set.is_empty() {
            ui.label(egui::RichText::new(&set).strong());
        }
        for c in members {
            ui.horizontal(|ui| {
                let label =
                    if set.is_empty() { c.name.clone() } else { c.variant.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(", ") };
                ui.label(if label.is_empty() { c.name.clone() } else { label }).on_hover_text(&c.description);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Main").on_hover_text("Go to the main component").clicked() {
                        run(app, "component.editMain", json!({"component": c.id}));
                    }
                    if ui.small_button("Place").on_hover_text("Insert an instance on this slide").clicked() {
                        run(app, "component.insert", json!({"component": c.id}));
                    }
                });
            });
        }
    }

    // Styles.
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Styles").font(theme::bold(13.0)));
    ui.horizontal_wrapped(|ui| {
        let one = selection.len() == 1;
        if ui.add_enabled(one, egui::Button::new("Paint from Selection")).clicked() {
            let n = format!("Paint {}", styles.len() + 1);
            run(app, "designStyle.create", json!({"type": "paint", "name": n}));
        }
        if ui.add_enabled(one, egui::Button::new("Effect from Selection")).clicked() {
            let n = format!("Effect {}", styles.len() + 1);
            run(app, "designStyle.create", json!({"type": "effect", "name": n}));
        }
        if ui.button("New Grid").clicked() {
            let n = format!("Grid {}", styles.len() + 1);
            run(app, "designStyle.create", json!({"type": "grid", "name": n, "columns": 12, "gutter": 16, "margin": 48}));
        }
    });
    let has_sel = !selection.is_empty();
    for s in styles.iter().filter(|s| wanted(&s.name)) {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
            match &s.kind {
                StyleKind::Paint { fill: Fill::Solid { color } } => {
                    let c = color.resolve(&scheme, None);
                    ui.painter().rect(
                        r,
                        3.0,
                        Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a),
                        Stroke::new(1.0, Color32::GRAY),
                        egui::StrokeKind::Inside,
                    );
                }
                StyleKind::Paint { .. } => {
                    ui.painter().rect(r, 3.0, Color32::LIGHT_GRAY, Stroke::new(1.0, Color32::GRAY), egui::StrokeKind::Inside);
                }
                StyleKind::Effect { .. } => {
                    ui.painter().circle(r.center(), 7.0, Color32::WHITE, Stroke::new(2.0, Color32::DARK_GRAY));
                }
                StyleKind::Grid { .. } => {
                    for i in 0..3 {
                        let x = r.min.x + 2.0 + i as f32 * 5.0;
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(egui::pos2(x, r.min.y + 2.0), vec2(3.0, 14.0)),
                            0.0,
                            Color32::from_rgb(240, 120, 120),
                        );
                    }
                }
            }
            ui.label(&s.name);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button("⋯", |ui| {
                    match &s.kind {
                        StyleKind::Paint { .. } => {
                            if ui.add_enabled(has_sel, egui::Button::new("Apply as Fill")).clicked() {
                                run(app, "designStyle.apply", json!({"style": s.id}));
                                ui.close();
                            }
                            if ui.add_enabled(has_sel, egui::Button::new("Apply as Outline")).clicked() {
                                run(app, "designStyle.apply", json!({"style": s.id, "as": "line"}));
                                ui.close();
                            }
                            if ui.add_enabled(selection.len() == 1, egui::Button::new("Update from Selection")).clicked() {
                                run(app, "designStyle.update", json!({"style": s.id, "from": selection.first().map(|i| i.0)}));
                                ui.close();
                            }
                        }
                        StyleKind::Effect { .. } => {
                            if ui.add_enabled(has_sel, egui::Button::new("Apply")).clicked() {
                                run(app, "designStyle.apply", json!({"style": s.id}));
                                ui.close();
                            }
                            if ui.add_enabled(selection.len() == 1, egui::Button::new("Update from Selection")).clicked() {
                                run(app, "designStyle.update", json!({"style": s.id, "from": selection.first().map(|i| i.0)}));
                                ui.close();
                            }
                        }
                        StyleKind::Grid { .. } => {
                            if ui.button("Show on This Slide").clicked() {
                                run(app, "designStyle.apply", json!({"style": s.id, "slides": [slide]}));
                                ui.close();
                            }
                            if ui.button("Show on Every Slide").clicked() {
                                run(app, "designStyle.apply", json!({"style": s.id, "slides": "all"}));
                                ui.close();
                            }
                            if ui.add_enabled(has_sel, egui::Button::new("Apply to Frame")).clicked() {
                                run(app, "designStyle.apply", json!({"style": s.id}));
                                ui.close();
                            }
                        }
                    }
                    ui.separator();
                    if ui.button("Delete Style").clicked() {
                        run(app, "designStyle.delete", json!({"style": s.id}));
                        ui.close();
                    }
                });
                if let StyleKind::Paint { .. } | StyleKind::Effect { .. } = s.kind
                    && ui.add_enabled(has_sel, egui::Button::new("Apply").small()).clicked()
                {
                    run(app, "designStyle.apply", json!({"style": s.id}));
                }
            });
        });
    }
    ui.data_mut(|d| d.insert_temp(id, st));
}
