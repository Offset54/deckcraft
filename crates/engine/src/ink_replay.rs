//! Draw › Ink Replay: the ink on a slide drawn again, stroke by stroke, in the order it was
//! drawn. Ink keeps that order (objects back to front, and the strokes inside each) but not the
//! time each stroke took, so the replay moves the pen at a steady speed along every stroke.
//!
//! Nothing here changes the presentation: [`frame`] answers a copy of the slide as it looks a
//! given number of seconds into the replay, for a view to draw.

use deckcraft_model::{InkStroke, Shape, ShapeId, ShapeKind, Slide};

/// Pen speed, in points a second.
const SPEED: f64 = 700.0;
/// The least time one stroke takes (a dot or a tick is still seen being made).
const MIN_STROKE_SECS: f64 = 0.12;
/// The longest a replay runs: a slide full of ink is drawn faster instead.
pub const MAX_SECS: f64 = 30.0;
/// Most strokes replayed one by one; any beyond are simply shown.
const MAX_STROKES: usize = 20_000;

/// One stroke's turn in the replay.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Turn {
    shape: ShapeId,
    /// Index among its shape's strokes.
    stroke: usize,
    /// Length of the stroke, in the shape's own units.
    length: f64,
    /// When the pen starts and stops, in seconds from the start of the replay.
    from: f64,
    to: f64,
}

/// The order and timing of a slide's ink.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    turns: Vec<Turn>,
    duration: f64,
}

impl Plan {
    /// Seconds the replay takes.
    pub fn duration(&self) -> f64 {
        self.duration
    }
    /// How many strokes it draws.
    pub fn strokes(&self) -> usize {
        self.turns.len()
    }
    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }
}

/// Length of a stroke along its samples; points that are not numbers count for nothing.
fn length(stroke: &InkStroke) -> f64 {
    let mut total = 0.0;
    for pair in stroke.points.windows(2) {
        if let [a, b] = pair {
            let d = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            if d.is_finite() {
                total += d;
            }
        }
    }
    total
}

/// The replay of the ink in `shapes` (a slide's objects, groups included).
pub fn plan(shapes: &[Shape]) -> Plan {
    let mut turns: Vec<Turn> = Vec::new();
    deckcraft_model::walk(shapes, &mut |sh, _| {
        if sh.hidden {
            return;
        }
        // Stroke coordinates are the shape's own; a stretched ink shape draws at the same pace.
        if let ShapeKind::Ink { strokes } = &sh.kind {
            for (k, stroke) in strokes.iter().enumerate() {
                if turns.len() >= MAX_STROKES {
                    return;
                }
                if !stroke.points.is_empty() {
                    turns.push(Turn { shape: sh.id, stroke: k, length: length(stroke), from: 0.0, to: 0.0 });
                }
            }
        }
    });
    let wanted: f64 = turns.iter().map(|t| (t.length / SPEED).max(MIN_STROKE_SECS)).sum();
    let faster = if wanted > MAX_SECS { MAX_SECS / wanted } else { 1.0 };
    let mut at = 0.0;
    for t in &mut turns {
        t.from = at;
        at += (t.length / SPEED).max(MIN_STROKE_SECS) * faster;
        t.to = at;
    }
    Plan { turns, duration: at }
}

/// `stroke` as far as the pen has got: its first `fraction` (0..=1) by length.
fn partial(stroke: &InkStroke, total: f64, fraction: f64) -> InkStroke {
    let mut out = InkStroke { points: Vec::new(), ..stroke.clone() };
    let target = total * fraction.clamp(0.0, 1.0);
    let mut run = 0.0;
    let mut prev: Option<(f64, f64, f32)> = None;
    for p in &stroke.points {
        if let Some(q) = prev {
            let d = ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt();
            let d = if d.is_finite() { d } else { 0.0 };
            if run + d > target {
                // The pen is part-way along this segment.
                let f = if d > 0.0 { ((target - run) / d).clamp(0.0, 1.0) } else { 0.0 };
                if f > 0.0 {
                    out.points.push((q.0 + (p.0 - q.0) * f, q.1 + (p.1 - q.1) * f, q.2 + (p.2 - q.2) * f as f32));
                }
                return out;
            }
            run += d;
        }
        out.points.push(*p);
        prev = Some(*p);
    }
    out
}

/// `slide` as it looks `secs` into the replay `plan` made for it: strokes whose turn is over are
/// whole, the one being drawn reaches as far as the pen has got, and later ones are not there
/// yet. At or after [`Plan::duration`] it is the slide itself.
pub fn frame(slide: &Slide, plan: &Plan, secs: f64) -> Slide {
    let secs = if secs.is_finite() { secs.max(0.0) } else { 0.0 };
    let mut out = slide.clone();
    if secs >= plan.duration {
        return out;
    }
    // The turns are in the order the shapes are walked here, so one cursor finds each stroke's
    // turn without searching (a slide can hold thousands of strokes).
    fn rec(shapes: &mut [Shape], plan: &Plan, next: &mut usize, secs: f64, depth: usize) {
        if depth > 64 {
            return;
        }
        for sh in shapes {
            let id = sh.id;
            match &mut sh.kind {
                ShapeKind::Ink { strokes } => {
                    let mut kept = Vec::with_capacity(strokes.len());
                    for (k, stroke) in strokes.iter().enumerate() {
                        let turn = plan.turns.get(*next).filter(|t| t.shape == id && t.stroke == k);
                        if turn.is_some() {
                            *next += 1;
                        }
                        match turn {
                            // Not part of the replay (hidden, empty, or past the cap): as it is.
                            None => kept.push(stroke.clone()),
                            Some(t) if secs >= t.to => kept.push(stroke.clone()),
                            Some(t) if secs > t.from => {
                                let span = (t.to - t.from).max(1e-9);
                                let part = partial(stroke, t.length, (secs - t.from) / span);
                                if !part.points.is_empty() {
                                    kept.push(part);
                                }
                            }
                            Some(_) => {}
                        }
                    }
                    *strokes = kept;
                }
                ShapeKind::Group { children, .. } => rec(children, plan, next, secs, depth + 1),
                _ => {}
            }
        }
    }
    rec(&mut out.shapes, plan, &mut 0, secs, 0);
    out
}

#[cfg(test)]
mod patch_tests {
    use deckcraft_model::{InkStroke, Shape, ShapeId, ShapeKind, Slide};

    use super::*;

    fn stroke(points: &[(f64, f64)]) -> InkStroke {
        InkStroke { points: points.iter().map(|p| (p.0, p.1, 1.0)).collect(), width: 2.0, ..Default::default() }
    }

    fn ink(id: u32, strokes: Vec<InkStroke>) -> Shape {
        Shape { id: ShapeId(id), kind: ShapeKind::Ink { strokes }, ..Default::default() }
    }

    fn strokes_of(slide: &Slide, id: u32) -> Vec<InkStroke> {
        match &deckcraft_model::find_shape(&slide.shapes, ShapeId(id)).unwrap().kind {
            ShapeKind::Ink { strokes } => strokes.clone(),
            _ => panic!("ink"),
        }
    }

    /// Two ink objects: a 700 pt line then a 350 pt line, and one more 700 pt line in a group.
    fn slide() -> Slide {
        let first = ink(1, vec![stroke(&[(0.0, 0.0), (700.0, 0.0)]), stroke(&[(0.0, 10.0), (350.0, 10.0)])]);
        let grouped = ink(3, vec![stroke(&[(0.0, 20.0), (0.0, 720.0)])]);
        let group = Shape { id: ShapeId(2), kind: ShapeKind::Group { children: vec![grouped], child: Default::default() }, ..Default::default() };
        let rect = Shape { id: ShapeId(4), ..Default::default() };
        Slide { shapes: vec![first, rect, group], ..Default::default() }
    }

    #[test]
    fn strokes_take_their_turn_in_drawing_order_at_pen_speed() {
        let s = slide();
        let p = plan(&s.shapes);
        assert_eq!(p.strokes(), 3);
        assert!((p.duration() - 2.5).abs() < 1e-9, "{}", p.duration());
        // Nothing yet at the start; everything at the end.
        let start = frame(&s, &p, 0.0);
        assert!(strokes_of(&start, 1).is_empty() && strokes_of(&start, 3).is_empty());
        assert_eq!(frame(&s, &p, 2.5), s);
        assert_eq!(frame(&s, &p, 99.0), s);
        // Half a second in: the pen is half-way along the first line.
        let half = frame(&s, &p, 0.5);
        let drawn = strokes_of(&half, 1);
        assert_eq!(drawn.len(), 1);
        assert!((drawn[0].points.last().unwrap().0 - 350.0).abs() < 1e-6, "{:?}", drawn[0].points);
        // 1.25 s: the first is whole, the second half drawn, the grouped one not begun.
        let later = frame(&s, &p, 1.25);
        let drawn = strokes_of(&later, 1);
        assert_eq!(drawn[0], strokes_of(&s, 1)[0]);
        assert!((drawn[1].points.last().unwrap().0 - 175.0).abs() < 1e-6);
        assert!(strokes_of(&later, 3).is_empty());
        // 2 s: the grouped line is half drawn.
        let end = frame(&s, &p, 2.0);
        assert!((strokes_of(&end, 3)[0].points.last().unwrap().1 - 370.0).abs() < 1e-6);
        // The other objects are untouched throughout.
        assert_eq!(later.shapes[1], s.shapes[1]);
    }

    #[test]
    fn a_dot_still_takes_a_moment_and_pressure_is_carried_along() {
        let mut dot = stroke(&[(5.0, 5.0)]);
        dot.points[0].2 = 0.3;
        let mut line = stroke(&[(0.0, 0.0), (700.0, 0.0)]);
        line.points[0].2 = 0.2;
        line.points[1].2 = 1.0;
        let s = Slide { shapes: vec![ink(1, vec![dot.clone(), line])], ..Default::default() };
        let p = plan(&s.shapes);
        assert!((p.duration() - (MIN_STROKE_SECS + 1.0)).abs() < 1e-9);
        // The dot appears once its turn starts.
        assert!(strokes_of(&frame(&s, &p, 0.0), 1).is_empty());
        assert_eq!(strokes_of(&frame(&s, &p, 0.05), 1), vec![dot]);
        let mid = strokes_of(&frame(&s, &p, MIN_STROKE_SECS + 0.5), 1);
        let tip = mid[1].points.last().unwrap();
        assert!((tip.0 - 350.0).abs() < 1e-6 && (tip.2 - 0.6).abs() < 1e-4, "{tip:?}");
    }

    #[test]
    fn a_slide_full_of_ink_is_drawn_faster_not_longer() {
        let long: Vec<InkStroke> = (0..200).map(|k| stroke(&[(0.0, f64::from(k)), (2000.0, f64::from(k))])).collect();
        let s = Slide { shapes: vec![ink(1, long)], ..Default::default() };
        let p = plan(&s.shapes);
        assert!((p.duration() - MAX_SECS).abs() < 1e-6, "{}", p.duration());
        assert_eq!(strokes_of(&frame(&s, &p, MAX_SECS / 2.0), 1).len(), 100);
    }

    #[test]
    fn hostile_ink_is_bounded_and_never_panics() {
        let mut odd = stroke(&[(0.0, 0.0), (f64::NAN, 1.0), (f64::INFINITY, 2.0), (10.0, 0.0)]);
        odd.points.push((1e308, -1e308, f32::NAN));
        let hidden = Shape { hidden: true, ..ink(9, vec![stroke(&[(0.0, 0.0), (10.0, 0.0)])]) };
        let many: Vec<InkStroke> = (0..MAX_STROKES + 50).map(|_| stroke(&[(0.0, 0.0)])).collect();
        let s = Slide { shapes: vec![ink(1, vec![odd, InkStroke::default()]), hidden, ink(2, many)], ..Default::default() };
        let p = plan(&s.shapes);
        assert!(p.duration().is_finite() && p.duration() <= MAX_SECS + 1e-6);
        assert_eq!(p.strokes(), MAX_STROKES);
        for secs in [f64::NAN, -1.0, 0.0, 0.01, p.duration() / 2.0, f64::INFINITY] {
            let f = frame(&s, &p, secs);
            // Hidden ink is not replayed: it stays as it is.
            assert_eq!(strokes_of(&f, 9).len(), 1);
            assert!(strokes_of(&f, 2).len() <= MAX_STROKES + 50);
        }
        assert!(plan(&[]).is_empty());
        assert_eq!(frame(&Slide::default(), &plan(&[]), 0.0), Slide::default());
    }
}
