//! Save as Picture… on an object's right-click menu: ask where to save, then run
//! `shape.saveAsPicture`. The file name's ending chooses the picture type (PNG when it has none
//! of .png, .jpg, .gif, .tif, .bmp). Without a save dialog (the web app) the picture is handed
//! over as a PNG download.

use serde_json::{Value, json};

use crate::SlideApp;

/// A file name for the selection: the object's name without characters a file system refuses.
pub(crate) fn suggested_name(app: &SlideApp) -> String {
    let name = app
        .session
        .active()
        .and_then(|st| match st.selected_shapes().as_slice() {
            [one] => Some(one.name.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let clean: String = name.chars().filter(|c| !c.is_control()).map(|c| if r#"\/:*?"<>|"#.contains(c) { '_' } else { c }).take(80).collect();
    let clean = clean.trim().trim_matches('.');
    format!("{}.png", if clean.is_empty() { "Picture" } else { clean })
}

/// Ask for a place and save the selected objects there as one picture.
pub(crate) fn save_as_picture(app: &mut SlideApp) {
    let suggested = suggested_name(app);
    if let Some(pick) = app.services.pick_save.as_mut() {
        let Some(path) = pick(&suggested) else { return };
        if app.run("shape.saveAsPicture", json!({"path": path})).is_ok() {
            app.set_status(format!("Saved {path}"));
        }
        return;
    }
    // No save dialog: hand the PNG to the host as a download.
    let picture = app.run("shape.saveAsPicture", json!({"format": "png"}));
    let bytes = picture.ok().and_then(|v| v.get("data").and_then(Value::as_str).and_then(deckcraft_engine::cmd::base64_decode));
    match (bytes, app.services.download.as_mut()) {
        (Some(bytes), Some(download)) => download(&suggested, &bytes),
        (Some(_), None) => app.set_status("Saving a picture needs a file picker, which this window doesn't have."),
        (None, _) => {}
    }
}
