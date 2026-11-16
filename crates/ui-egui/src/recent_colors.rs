//! Recent Colors in every colour menu. `widgets::color_grid` has no access to the app, so the two
//! meet in the egui context: once a frame [`tick`] publishes the presentation's recent colours
//! for the grid to draw, and hands the engine any custom colour a grid noted (More Colors or the
//! Eyedropper) with `format.recentColor`.

use deckcraft_model::Rgba;
use egui::{Context, Id};
use serde_json::json;

use crate::SlideApp;

const SHOWN: &str = "deckcraft.recentColors.shown";
const NOTED: &str = "deckcraft.recentColors.noted";

/// The recent colours of the presentation in front, newest first (empty outside the app).
pub fn shown(ctx: &Context) -> Vec<Rgba> {
    ctx.data(|d| d.get_temp::<Vec<Rgba>>(Id::unique(SHOWN))).unwrap_or_default()
}

/// A custom colour was picked in a colour menu: remember it at the next [`tick`].
pub fn note(ctx: &Context, color: Rgba) {
    ctx.data_mut(|d| {
        let noted = d.get_temp_mut_or_default::<Vec<Rgba>>(Id::unique(NOTED));
        // One frame can't pick more than a colour or two; the cap only bounds a stuck queue.
        if noted.len() < 16 {
            noted.push(color);
        }
    });
}

/// Once a frame: store the colours noted since the last one, then publish the list.
pub fn tick(app: &mut SlideApp, ctx: &Context) {
    let noted = ctx.data_mut(|d| d.remove_temp::<Vec<Rgba>>(Id::unique(NOTED))).unwrap_or_default();
    for c in noted {
        // Quietly: without a presentation there is nowhere to remember it, and that is fine.
        let _ = app.session.execute("format.recentColor", &json!({"color": format!("#{}", c.hex())}));
    }
    let now = app.session.active().map(|d| d.doc.recent_colors.clone()).unwrap_or_default();
    ctx.data_mut(|d| d.insert_temp(Id::unique(SHOWN), now));
}
