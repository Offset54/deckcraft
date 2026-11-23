//! SlideCraft's Frame & Layout section of the Format pane (Phase 3): make a frame, its clip and
//! corner radius, how a member follows its frame (constraints), and moving in and out of groups.
//! Every control runs a command (`frame.*`, `layout.*`, `edit.selectChildren`/`selectParent`).

use deckcraft_model::canvas::{Align, AutoLayout, Constraints, Flow, Pin, Sizing, Track};
use deckcraft_model::{Shape, ShapeKind};

use super::{SlideApp, Ui, json, run, theme, vec2, widgets};

const PINS: [(Pin, &str, &str); 5] = [
    (Pin::Start, "start", "Start"),
    (Pin::End, "end", "End"),
    (Pin::Both, "both", "Stretch"),
    (Pin::Center, "center", "Center"),
    (Pin::Scale, "scale", "Scale"),
];

fn pin_label(p: Pin) -> &'static str {
    PINS.iter().find(|(q, ..)| *q == p).map(|(_, _, l)| *l).unwrap_or("Start")
}

/// The section, for the single selected shape `sh`.
pub(super) fn frame_section(app: &mut SlideApp, ui: &mut Ui, sh: &Shape) {
    let Some(st) = app.session.active() else { return };
    let parent = deckcraft_model::canvas::ancestors(st.shapes(), sh.id).and_then(|c| c.last().copied()).and_then(|g| st.shape(g)).cloned();
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Frame & Layout").font(theme::bold(13.0)));
    match (&sh.kind, sh.frame) {
        (ShapeKind::Group { .. }, Some(fr)) => {
            let mut clip = fr.clip;
            if ui.checkbox(&mut clip, "Clip content").changed() {
                run(app, "frame.set", json!({"clip": clip}));
            }
            ui.horizontal(|ui| {
                ui.add_sized(vec2(90.0, 20.0), egui::Label::new("Corner radius"));
                if let Some(v) = widgets::spinner(ui, "frame-radius", "Corner radius", fr.radius, 1.0, " pt", 70.0) {
                    run(app, "frame.set", json!({"radius": v.clamp(0.0, deckcraft_model::canvas::Frame::MAX_RADIUS)}));
                }
            });
            match &sh.auto_layout {
                Some(al) => auto_layout_controls(app, ui, al),
                None => {
                    if ui.button("Add Auto Layout").on_hover_text("Line the members up with gaps and padding").clicked() {
                        run(app, "layout.set", json!({}));
                    }
                }
            }
            if ui.button("Turn into Group").clicked() {
                run(app, "frame.set", json!({"frame": false}));
            }
        }
        (ShapeKind::Group { .. }, None) => {
            if ui.button("Make Frame").on_hover_text("Give the group its own box, fill and clipping").clicked() {
                run(app, "frame.set", json!({}));
            }
        }
        _ if parent.is_none() && ui.button("Frame Selection").on_hover_text("Wrap the selection in a frame").clicked() => {
            run(app, "frame.create", json!({}));
        }
        _ => {}
    }
    if matches!(sh.kind, ShapeKind::Group { .. }) && ui.button("Select Members").clicked() {
        run(app, "edit.selectChildren", json!({}));
    }
    let Some(parent) = parent else { return };
    if ui.button("Select Parent").clicked() {
        run(app, "edit.selectParent", json!({}));
    }
    if parent.frame.is_none() {
        return;
    }
    if parent.auto_layout.is_some() {
        member_controls(app, ui, sh);
    }
    let k: Constraints = sh.constraints.unwrap_or_default();
    for (label, key, cur) in [("Horizontal", "horizontal", k.horizontal), ("Vertical", "vertical", k.vertical)] {
        ui.horizontal(|ui| {
            ui.add_sized(vec2(90.0, 20.0), egui::Label::new(label));
            let mut pick = None;
            let r = craft_ui::ComboBox::from_id_salt(("frame-pin", key)).selected_text(pin_label(cur)).show_ui(ui, |ui| {
                for (p, id, l) in PINS {
                    if ui.selectable_label(p == cur, l).clicked() {
                        pick = Some(id);
                    }
                }
            });
            r.response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, label));
            if let Some(id) = pick {
                run(app, "layout.constraints", json!({key: id}));
            }
        });
    }
}

/// A combo box of `(value, command word, label)` choices; runs `cmd` with `{key: word}` on a pick.
fn choice<T: PartialEq + Copy>(app: &mut SlideApp, ui: &mut Ui, label: &str, cmd: &str, key: &str, cur: T, items: &[(T, &str, &str)]) {
    ui.horizontal(|ui| {
        ui.add_sized(vec2(90.0, 20.0), egui::Label::new(label));
        let shown = items.iter().find(|(v, ..)| *v == cur).map(|(_, _, l)| *l).unwrap_or("");
        let mut pick = None;
        let r = craft_ui::ComboBox::from_id_salt(("layout-choice", key)).selected_text(shown).show_ui(ui, |ui| {
            for (v, word, l) in items {
                if ui.selectable_label(*v == cur, *l).clicked() {
                    pick = Some(*word);
                }
            }
        });
        r.response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, label));
        if let Some(word) = pick {
            run(app, cmd, json!({key: word}));
        }
    });
}

fn track_text(t: &Track) -> String {
    match t {
        Track::Fixed(v) => format!("{v}"),
        Track::Hug => "hug".into(),
        Track::Fr(f) => format!("{f}fr"),
    }
}

fn auto_layout_controls(app: &mut SlideApp, ui: &mut Ui, al: &AutoLayout) {
    choice(
        app,
        ui,
        "Direction",
        "layout.set",
        "flow",
        al.flow,
        &[(Flow::Horizontal, "horizontal", "Horizontal"), (Flow::Vertical, "vertical", "Vertical"), (Flow::Grid, "grid", "Grid")],
    );
    if al.flow != Flow::Grid {
        let mut wrap = al.wrap;
        if ui.checkbox(&mut wrap, "Wrap").changed() {
            run(app, "layout.set", json!({"wrap": wrap}));
        }
    }
    for (label, id, key, v) in
        [("Gap", "al-gap", "gap", al.gap), ("Line gap", "al-cross-gap", "crossGap", al.cross_gap), ("Padding", "al-pad", "padding", al.padding[0])]
    {
        ui.horizontal(|ui| {
            ui.add_sized(vec2(90.0, 20.0), egui::Label::new(label));
            if let Some(nv) = widgets::spinner(ui, id, label, v, 1.0, " pt", 70.0) {
                run(app, "layout.set", json!({key: nv.clamp(0.0, deckcraft_model::autolayout::MAX_SPACE)}));
            }
        });
    }
    choice(
        app,
        ui,
        "Align",
        "layout.set",
        "align",
        al.align,
        &[
            (Align::Start, "start", "Start"),
            (Align::Center, "center", "Center"),
            (Align::End, "end", "End"),
            (Align::SpaceBetween, "spaceBetween", "Space between"),
        ],
    );
    choice(
        app,
        ui,
        "Align across",
        "layout.set",
        "crossAlign",
        al.cross_align,
        &[(Align::Start, "start", "Start"), (Align::Center, "center", "Center"), (Align::End, "end", "End"), (Align::Stretch, "stretch", "Stretch")],
    );
    let sizes = [(Sizing::Fixed, "fixed", "Fixed"), (Sizing::Hug, "hug", "Hug")];
    choice(app, ui, "Width", "layout.set", "width", al.width, &sizes);
    choice(app, ui, "Height", "layout.set", "height", al.height, &sizes);
    if al.flow == Flow::Grid {
        // Columns as text: `1fr 2fr hug 120`.
        let id = ui.scope_id().with("al-columns");
        let shown = al.columns.iter().map(track_text).collect::<Vec<_>>().join(" ");
        let mut text: String = ui.data_mut(|d| d.get_temp(id).unwrap_or_else(|| shown.clone()));
        ui.horizontal(|ui| {
            ui.add_sized(vec2(90.0, 20.0), egui::Label::new("Columns"));
            let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(120.0).hint_text("1fr 2fr hug 120"));
            if r.lost_focus() && text != shown {
                let tracks: Vec<serde_json::Value> =
                    text.split_whitespace().map(|t| t.parse::<f64>().map(|n| json!(n)).unwrap_or_else(|_| json!(t))).collect();
                run(app, "layout.set", json!({"columns": tracks}));
                ui.data_mut(|d| d.remove::<String>(id));
            } else {
                ui.data_mut(|d| d.insert_temp(id, text.clone()));
            }
        });
    }
    if ui.button("Remove Auto Layout").clicked() {
        run(app, "layout.remove", json!({}));
    }
}

fn member_controls(app: &mut SlideApp, ui: &mut Ui, sh: &Shape) {
    let c = sh.layout_child.unwrap_or_default();
    let sizes = [(Sizing::Fixed, "fixed", "Fixed"), (Sizing::Fill, "fill", "Fill")];
    choice(app, ui, "Width", "layout.child", "width", c.width, &sizes);
    choice(app, ui, "Height", "layout.child", "height", c.height, &sizes);
    let mut abs = c.absolute;
    if ui.checkbox(&mut abs, "Ignore auto layout").changed() {
        run(app, "layout.child", json!({"absolute": abs}));
    }
}
