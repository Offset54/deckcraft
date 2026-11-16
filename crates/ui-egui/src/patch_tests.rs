//! Tests of this fork's patches (see CUSTOM-BUILD.md), one module per patch. The custom build
//! runs every test whose path contains `patch_tests` before it packages anything.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use egui_kittest::Harness;

use crate::{Services, SlideApp};
use deckcraft_engine::Session;

mod crop_fit;
mod kerning;
mod number_from;
mod reading_order;
mod recent_colors;
mod reverse_path;

/// The whole app in a test window.
fn harness(app: SlideApp) -> Harness<'static, SlideApp> {
    Harness::builder().with_size(egui::vec2(1440.0, 900.0)).with_step_dt(1.0 / 60.0).build_ui_state(
        |ui, app: &mut SlideApp| {
            app.logic(ui.ctx());
            app.ui(ui);
        },
        app,
    )
}

/// The app on a new presentation.
fn new_app() -> SlideApp {
    SlideApp::new(Session::with_new(), Services::default())
}
