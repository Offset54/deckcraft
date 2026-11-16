//! Draw › Ink Replay on the editor canvas: the current slide's ink is drawn again in the order
//! it was drawn. The presentation is not touched: while a replay runs, the canvas draws
//! [`deckcraft_engine::ink_replay::frame`]'s copy of the slide. `draw.inkReplay` starts, pauses,
//! rewinds and stops it; an edit, another slide or the slide show ends it.

use std::sync::Arc;

use deckcraft_engine::Target;
use deckcraft_engine::ink_replay::{Plan, frame, plan};
use deckcraft_model::Presentation;
use serde_json::{Value, json};

use crate::SlideApp;

/// How long the finished drawing is held before the replay ends, in seconds.
const HOLD_SECS: f64 = 0.4;

/// A replay in progress.
#[derive(Debug)]
pub struct Replay {
    /// The document, slide and slide revision it was started on.
    uid: u64,
    slide: usize,
    slide_ptr: usize,
    plan: Plan,
    /// Seconds of ink drawn so far.
    played: f64,
    /// The clock at the last frame (`None` until the first one, and while paused).
    last: Option<f64>,
    paused: bool,
    /// Counts frames, so the canvas never mistakes one frame's picture for another's.
    tick: u64,
}

impl Replay {
    pub fn paused(&self) -> bool {
        self.paused
    }
    /// Seconds drawn and seconds in all.
    pub fn progress(&self) -> (f64, f64) {
        (self.played.min(self.plan.duration()), self.plan.duration())
    }
}

/// Does the current slide have ink to replay?
pub fn available(app: &SlideApp) -> bool {
    app.session.active().is_some_and(|d| d.selection.target == Target::Slides && d.current_slide().is_some_and(|sl| !plan(&sl.shapes).is_empty()))
}

fn state(app: &SlideApp) -> Value {
    match &app.ink_replay {
        Some(r) => json!({"playing": !r.paused, "paused": r.paused, "time": r.progress().0, "duration": r.progress().1, "strokes": r.plan.strokes()}),
        None => json!({"playing": false, "paused": false}),
    }
}

/// `draw.inkReplay {on?: bool, pause?: bool, rewind?: bool}`: with nothing, start a replay (or
/// stop the one running); `pause` holds or resumes it; `rewind` starts it over.
pub fn command(app: &mut SlideApp, p: &Value) -> Result<Value, String> {
    let flag = |key: &str| match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(format!("`{key}` must be true or false")),
    };
    let (on, pause, rewind) = (flag("on")?, flag("pause")?, flag("rewind")?);
    if let Some(pause) = pause {
        let r = app.ink_replay.as_mut().ok_or("no ink replay is running")?;
        r.paused = pause;
        r.last = None;
        return Ok(state(app));
    }
    if rewind == Some(true)
        && let Some(r) = app.ink_replay.as_mut()
    {
        r.played = 0.0;
        r.last = None;
        r.paused = false;
        return Ok(state(app));
    }
    let start = on.unwrap_or(app.ink_replay.is_none() || rewind == Some(true));
    if !start {
        app.ink_replay = None;
        return Ok(state(app));
    }
    let d = app.session.active().ok_or("no presentation is open")?;
    if d.selection.target != Target::Slides {
        return Err("ink replay plays a slide's ink: leave the master view first".into());
    }
    let slide = d.selection.slide;
    let sl = d.doc.slides.get(slide).ok_or("no slide")?;
    let plan = plan(&sl.shapes);
    if plan.is_empty() {
        return Err("this slide has no ink to replay".into());
    }
    let replay = Replay { uid: d.uid, slide, slide_ptr: Arc::as_ptr(sl) as usize, plan, played: 0.0, last: None, paused: false, tick: 0 };
    // The pen's own previews and the animation preview would fight over the canvas.
    app.stop_anim_preview();
    app.ink_replay = Some(replay);
    Ok(state(app))
}

/// Once a frame: move the replay on. `true` while another frame is wanted.
pub fn tick(app: &mut SlideApp, now: f64) -> bool {
    let Some(r) = app.ink_replay.as_ref() else { return false };
    let alive = app.show.is_none()
        && app.session.active().is_some_and(|d| {
            d.uid == r.uid
                && d.selection.target == Target::Slides
                && d.selection.slide == r.slide
                && d.doc.slides.get(r.slide).is_some_and(|s| Arc::as_ptr(s) as usize == r.slide_ptr)
        });
    if !alive {
        app.ink_replay = None;
        return false;
    }
    let Some(r) = app.ink_replay.as_mut() else { return false };
    if r.paused || !now.is_finite() {
        r.last = None;
        return false;
    }
    // A long gap between frames (a stalled window) moves the pen on by at most a little.
    let step = r.last.map_or(0.0, |last| (now - last).clamp(0.0, 0.25));
    r.last = Some(now);
    r.played += step;
    // A new picture only when the pen has moved: a paused replay keeps the one it has.
    r.tick = r.tick.wrapping_add(1);
    if r.played >= r.plan.duration() + HOLD_SECS {
        app.ink_replay = None;
        return false;
    }
    true
}

/// The presentation the canvas draws for slide `index` while a replay runs on it, and a number
/// that differs from frame to frame; `None` when there is nothing to replay there.
pub fn frame_doc(replay: Option<&Replay>, doc: &Arc<Presentation>, index: usize) -> Option<(Arc<Presentation>, u64)> {
    let r = replay.filter(|r| r.slide == index)?;
    let slide = doc.slides.get(index).filter(|s| Arc::as_ptr(s) as usize == r.slide_ptr)?;
    if r.played >= r.plan.duration() {
        return None;
    }
    let mut shown = (**doc).clone();
    let drawn = frame(slide, &r.plan, r.played);
    *shown.slides.get_mut(index)? = Arc::new(drawn);
    Some((Arc::new(shown), r.tick.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1))
}
