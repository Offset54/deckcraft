//! Reading Order pane (Review › Check Accessibility): the slide's objects in the order a screen
//! reader reads them, first read at the top. Tick boxes say which are read (unticked = marked as
//! decorative), Move Up / Move Down and dragging change the order, and a click selects the
//! object. The order is the shape tree's, so a move here is the same `arrange.reorder` the
//! Selection pane makes; nothing is changed except through engine commands.

use deckcraft_engine::cmd::review::reading_order::{ReadingItem, reading_order};
use egui::{Align2, CornerRadius, Id, Rect, Role, Sense, Stroke, Ui, WidgetInfo, pos2, vec2};
use serde_json::json;

use crate::SlideApp;
use crate::icons::{self, Icon};
use crate::theme::{self, Tokens};

const ROW_H: f32 = 38.0;
const INDENT: f32 = 14.0;

fn run(app: &mut SlideApp, id: &str, p: serde_json::Value) {
    let _ = app.run(id, p);
}

/// The second line of a row: what a screen reader has to say about the object.
pub(crate) fn describe(item: &ReadingItem) -> String {
    if item.decorative {
        "Decorative: not read".into()
    } else if !item.alt_text.trim().is_empty() {
        format!("Alt text: {}", item.alt_text.trim())
    } else if item.needs_alt_text {
        "Missing alt text".into()
    } else if !item.text.is_empty() {
        format!("“{}”", item.text)
    } else {
        item.kind.to_string()
    }
}

/// Where a dragged row would land among the rows sharing its parent. `blocks`: each of those
/// siblings with the rectangle of its row and its members' rows, top to bottom. Returns the index
/// for `arrange.reorder` (0 = read first) and the y of the insertion line.
pub(crate) fn drop_index(dragged: u32, blocks: &[(u32, Rect)], y: f32) -> Option<(usize, f32)> {
    let others: Vec<&(u32, Rect)> = blocks.iter().filter(|(id, _)| *id != dragged).collect();
    let first = others.first()?;
    let mut gaps: Vec<f32> = vec![first.1.top()];
    gaps.extend(others.iter().map(|(_, r)| r.bottom()));
    let (g, gy) = gaps.iter().enumerate().min_by(|a, b| (a.1 - y).abs().total_cmp(&(b.1 - y).abs()))?;
    // `g` siblings are read before the line; the dragged object comes right after them.
    Some((g, *gy))
}

/// Each object sharing `parent`, with the rectangle of its row and of the rows inside it.
fn sibling_blocks(items: &[ReadingItem], rects: &[Rect], parent: Option<u32>) -> Vec<(u32, Rect)> {
    let mut out: Vec<(u32, Rect)> = Vec::new();
    let mut open: Option<(usize, u32, Rect)> = None;
    for (item, rect) in items.iter().zip(rects) {
        match open.as_mut() {
            Some((depth, _, r)) if item.depth > *depth => *r = r.union(*rect),
            _ => {
                if let Some((_, id, r)) = open.take() {
                    out.push((id, r));
                }
                if item.parent == parent {
                    open = Some((item.depth, item.id, *rect));
                }
            }
        }
    }
    if let Some((_, id, r)) = open {
        out.push((id, r));
    }
    out
}

fn small_icon_button(ui: &mut Ui, icon: Icon, label: &str, enabled: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 22.0), if enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, label));
    if enabled && resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(4), t.hover);
    }
    ui.painter().rect_stroke(rect, CornerRadius::same(4), Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    icons::paint(ui.painter(), rect.shrink(4.0), icon, t.text, !enabled);
    resp.on_hover_text(label)
}

/// Render the Reading Order pane.
pub fn show(app: &mut SlideApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let Some(st) = app.session.active() else { return };
    let items = reading_order(st.shapes());
    let selected: Vec<u32> = st.selection.shapes.iter().map(|i| i.0).collect();
    let uid = st.uid;

    ui.label(egui::RichText::new("Screen readers read the objects from top to bottom. Untick one to mark it as decorative.").color(t.text_dim));
    ui.add_space(4.0);
    // Move Up / Move Down act on the one selected object, among the objects sharing its group.
    let one = match selected.as_slice() {
        [id] => items.iter().find(|i| i.id == *id).cloned(),
        _ => None,
    };
    ui.horizontal(|ui| {
        let up = one.as_ref().is_some_and(|i| i.index > 0);
        let down = one.as_ref().is_some_and(|i| i.index + 1 < i.siblings);
        if small_icon_button(ui, Icon::ChevronUp, "Move Up", up).clicked()
            && let Some(i) = &one
        {
            run(app, "arrange.reorder", json!({"id": i.id, "index": i.index.saturating_sub(1)}));
        }
        if small_icon_button(ui, Icon::ChevronDown, "Move Down", down).clicked()
            && let Some(i) = &one
        {
            run(app, "arrange.reorder", json!({"id": i.id, "index": i.index + 1}));
        }
        let missing = items.iter().filter(|i| i.needs_alt_text).count();
        if missing > 0 {
            ui.label(egui::RichText::new(format!("{missing} missing alt text")).color(t.text_dim));
        }
    });
    ui.add_space(6.0);
    if items.is_empty() {
        ui.label(egui::RichText::new("This slide has no objects.").color(t.text_faint));
        return;
    }

    // The row being dragged, kept beside the pane and dropped when another document comes up.
    let drag_id = Id::unique(("deckcraft.readingOrder.drag", uid));
    let mut drag: Option<u32> = ui.data(|d| d.get_temp::<Option<u32>>(drag_id)).flatten();
    let mut rects: Vec<Rect> = Vec::with_capacity(items.len());
    let width = ui.available_width();
    for (k, item) in items.iter().enumerate() {
        let (r, _) = ui.allocate_exact_size(vec2(width, ROW_H), Sense::hover());
        let resp = ui.interact(r, Id::unique(("deckcraft.readingOrder.row", item.id)), Sense::click_and_drag());
        rects.push(r);
        let sel = selected.contains(&item.id);
        let spoken = format!("{}. {}, {}", k + 1, item.name, describe(item));
        resp.widget_info(|| WidgetInfo::selected(Role::Button, true, sel, &spoken));
        let painter = ui.painter_at(r);
        if sel {
            painter.rect_filled(r, CornerRadius::same(3), t.selected);
            painter.rect_stroke(r.shrink(0.5), CornerRadius::same(3), Stroke::new(1.0, t.text_faint), egui::StrokeKind::Inside);
        } else if resp.hovered() {
            painter.rect_filled(r, CornerRadius::same(3), t.hover);
        }
        let x0 = r.min.x + 4.0 + INDENT * item.depth.min(12) as f32;
        // Tick box: read (ticked) or decorative.
        let tick = Rect::from_center_size(pos2(x0 + 8.0, r.center().y), vec2(14.0, 14.0));
        let tick_resp = ui.interact(tick.expand(3.0), Id::unique(("deckcraft.readingOrder.tick", item.id)), Sense::click());
        let read = !item.decorative;
        let tick_label = format!("Read {}", item.name);
        tick_resp.widget_info(|| WidgetInfo::selected(Role::CheckBox, true, read, &tick_label));
        painter.rect_stroke(
            tick,
            CornerRadius::same(2),
            Stroke::new(1.0, if tick_resp.hovered() { t.accent } else { t.border }),
            egui::StrokeKind::Inside,
        );
        if read {
            icons::paint(&painter, tick.shrink(1.0), Icon::Check, t.text, false);
        }
        let ink = if item.decorative || item.hidden { t.text_faint } else { t.text };
        let text_x = x0 + 22.0;
        painter.text(pos2(text_x, r.min.y + 11.0), Align2::LEFT_CENTER, format!("{}. {}", k + 1, item.name), theme::font(12.5), ink);
        let note_ink = if item.needs_alt_text { t.accent } else { t.text_dim };
        painter.text(pos2(text_x, r.max.y - 11.0), Align2::LEFT_CENTER, describe(item), theme::font(11.0), note_ink);

        if tick_resp.clicked() {
            run(app, "shape.altText", json!({"text": item.alt_text, "decorative": read, "ids": [item.id]}));
        } else if resp.clicked() {
            run(app, "selection.select", json!({"id": item.id}));
        }
        if resp.drag_started() {
            if !sel {
                run(app, "selection.select", json!({"id": item.id}));
            }
            drag = Some(item.id);
        }
    }

    // Dragging: an insertion line among the dragged row's siblings, and the move on release.
    if let Some(dragged) = drag {
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let released = ui.input(|i| i.pointer.any_released() || !i.pointer.any_down());
        let target = pointer.and_then(|p| {
            let parent = items.iter().find(|i| i.id == dragged).map(|i| i.parent)?;
            drop_index(dragged, &sibling_blocks(&items, &rects, parent), p.y)
        });
        if let Some((_, y)) = target {
            let (x0, x1) = (ui.min_rect().left(), ui.min_rect().left() + width);
            let s = Stroke::new(2.0, t.accent);
            ui.painter().line_segment([pos2(x0, y), pos2(x1, y)], s);
        }
        if released {
            drag = None;
            if let Some((index, _)) = target {
                run(app, "arrange.reorder", json!({"id": dragged, "index": index}));
            }
        }
    }
    ui.data_mut(|d| d.insert_temp(drag_id, drag));
}

#[cfg(test)]
mod patch_tests {
    use super::*;

    fn r(id: u32, top: f32) -> (u32, Rect) {
        (id, Rect::from_min_size(pos2(0.0, top), vec2(100.0, ROW_H)))
    }

    #[test]
    fn a_dropped_row_is_read_after_the_rows_above_the_line() {
        // Read first to last: 1, 2, 3, 4; dragging 1.
        let rows = [r(1, 0.0), r(2, 38.0), r(3, 76.0), r(4, 114.0)];
        assert_eq!(drop_index(1, &rows, 30.0), Some((0, 38.0)));
        assert_eq!(drop_index(1, &rows, 80.0), Some((1, 76.0)));
        assert_eq!(drop_index(1, &rows, 900.0), Some((3, 152.0)));
        // Dragging the last one to the top.
        assert_eq!(drop_index(4, &rows, -20.0), Some((0, 0.0)));
        // Nothing else to drop among.
        assert_eq!(drop_index(1, &[r(1, 0.0)], 10.0), None);
    }

    fn item(id: u32, depth: usize, parent: Option<u32>) -> ReadingItem {
        ReadingItem {
            id,
            name: format!("Object {id}"),
            kind: "shape",
            depth,
            parent,
            index: 0,
            siblings: 1,
            alt_text: String::new(),
            decorative: false,
            hidden: false,
            needs_alt_text: false,
            text: String::new(),
            title: false,
        }
    }

    #[test]
    fn a_groups_block_covers_its_members() {
        // 1, group 2 {3, 4}, 5.
        let items = [item(1, 0, None), item(2, 0, None), item(3, 1, Some(2)), item(4, 1, Some(2)), item(5, 0, None)];
        let rects: Vec<Rect> = (0..5).map(|k| Rect::from_min_size(pos2(0.0, k as f32 * ROW_H), vec2(100.0, ROW_H))).collect();
        let top = sibling_blocks(&items, &rects, None);
        assert_eq!(top.iter().map(|b| b.0).collect::<Vec<_>>(), vec![1, 2, 5]);
        assert_eq!((top[1].1.top(), top[1].1.bottom()), (ROW_H, 4.0 * ROW_H));
        let inside = sibling_blocks(&items, &rects, Some(2));
        assert_eq!(inside.iter().map(|b| b.0).collect::<Vec<_>>(), vec![3, 4]);
    }

    #[test]
    fn rows_say_what_a_screen_reader_would() {
        let mut i = item(1, 0, None);
        assert_eq!(describe(&i), "shape");
        i.text = "Hello".into();
        assert_eq!(describe(&i), "“Hello”");
        i.needs_alt_text = true;
        i.text.clear();
        assert_eq!(describe(&i), "Missing alt text");
        i.alt_text = " A chart of sales ".into();
        assert_eq!(describe(&i), "Alt text: A chart of sales");
        i.decorative = true;
        assert_eq!(describe(&i), "Decorative: not read");
    }
}
