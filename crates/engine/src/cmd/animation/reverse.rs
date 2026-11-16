//! Animations › Effect Options › Reverse Path Direction: `animation.reversePath`.

use std::sync::Arc;

use deckcraft_model::anim::AnimClass;
use serde_json::{Value, json};

use crate::cmd::{bad, opt_usize_param, targets};
use crate::{Result, Session};

const CMD: &str = "animation.reversePath";

/// Turn round the motion paths of animation `index`, or of the selected shapes. The path stays
/// where it is on the slide, so the shape now starts at the far end and travels back. A preset
/// path that was never edited is reversed from the route it plays.
pub(super) fn reverse_path(s: &mut Session, p: &Value) -> Result<Value> {
    let index = opt_usize_param(p, "index", CMD)?;
    let ids = if index.is_none() { targets(s, p)? } else { vec![] };
    s.edit(|doc, sel| {
        let sl = Arc::make_mut(doc.slides.get_mut(sel.slide).ok_or_else(|| bad(CMD, "no slide"))?);
        // Every new path is worked out before any is stored, so a refusal changes nothing.
        let mut turned: Vec<(usize, String)> = Vec::new();
        for (k, a) in sl.animations.iter().enumerate() {
            let hit = index == Some(k) || (index.is_none() && ids.contains(&a.shape));
            if !hit || a.class != AnimClass::Path {
                continue;
            }
            let now = match a.path.as_deref() {
                Some(own) if !own.trim().is_empty() => own.to_string(),
                _ => deckcraft_anim::default_path(&a.effect, &a.option),
            };
            let rev = deckcraft_anim::reverse_path(&now).ok_or_else(|| bad(CMD, format!("animation {k} has no motion path that can be reversed")))?;
            turned.push((k, rev));
        }
        if turned.is_empty() {
            return Err(bad(CMD, "no motion path to reverse (select an object with a motion path or give `index`)"));
        }
        for (k, rev) in &turned {
            if let Some(a) = sl.animations.get_mut(*k) {
                a.path = Some(rev.clone());
            }
        }
        Ok(json!({"changed": turned.len(), "path": turned.first().map(|t| t.1.clone())}))
    })
}

#[cfg(test)]
mod patch_tests {
    use deckcraft_anim::MotionPath;
    use serde_json::json;

    use crate::Session;

    /// A slide with a rectangle on a rightward line path, and a second rectangle that fades in.
    fn session() -> (Session, u64, u64) {
        let mut s = Session::with_new();
        s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
        let a = s.execute("shape.insert", &json!({"preset": "rect", "rect": [100.0, 100.0, 80.0, 60.0]})).unwrap()["id"].as_u64().unwrap();
        let b = s.execute("shape.insert", &json!({"preset": "rect", "rect": [300.0, 100.0, 80.0, 60.0]})).unwrap()["id"].as_u64().unwrap();
        s.execute("animation.add", &json!({"effect": "lines", "class": "path", "path": [[0.0, 0.0], [240.0, 0.0], [240.0, 108.0]], "ids": [a]}))
            .unwrap();
        s.execute("animation.add", &json!({"effect": "fade", "ids": [b]})).unwrap();
        (s, a, b)
    }

    fn path(s: &Session, k: usize) -> Option<String> {
        s.doc().unwrap().current_slide().unwrap().animations.get(k).and_then(|a| a.path.clone())
    }

    fn near(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4
    }

    #[test]
    fn the_path_runs_from_its_end_back_to_the_shape() {
        let (mut s, a, _) = session();
        let before = MotionPath::parse(&path(&s, 0).unwrap());
        let r = s.execute("animation.reversePath", &json!({"ids": [a]})).unwrap();
        assert_eq!(r["changed"], 1);
        let after = MotionPath::parse(&path(&s, 0).unwrap());
        // 240 x 108 pt on a 960 x 540 slide: it now starts at the old end and ends at the shape.
        assert!(near(after.at(0.0), (0.25, 0.2)) && near(after.at(1.0), (0.0, 0.0)), "{:?}", after.points);
        for k in 0..=10 {
            let p = f64::from(k) / 10.0;
            assert!(near(before.at(p), after.at(1.0 - p)));
        }
        // One undo step; and reversing twice is the first route again.
        s.execute("edit.undo", &json!({})).unwrap();
        assert!(near(MotionPath::parse(&path(&s, 0).unwrap()).at(1.0), (0.25, 0.2)));
        s.execute("animation.reversePath", &json!({"index": 0})).unwrap();
        s.execute("animation.reversePath", &json!({"index": 0})).unwrap();
        assert!(near(MotionPath::parse(&path(&s, 0).unwrap()).at(1.0), (0.25, 0.2)));
    }

    #[test]
    fn a_preset_path_is_reversed_from_the_route_it_plays() {
        let (mut s, a, _) = session();
        s.execute("animation.set", &json!({"effect": "arcs", "class": "path", "ids": [a]})).unwrap();
        std::sync::Arc::make_mut(&mut std::sync::Arc::make_mut(&mut s.active_mut().unwrap().doc).slides[1]).animations[0].path = None;
        let played = MotionPath::parse(&deckcraft_anim::default_path("arcs", &s.doc().unwrap().current_slide().unwrap().animations[0].option));
        s.execute("animation.reversePath", &json!({"ids": [a]})).unwrap();
        let after = MotionPath::parse(&path(&s, 0).unwrap());
        assert!(near(after.at(0.0), played.at(1.0)) && near(after.at(1.0), played.at(0.0)));
    }

    #[test]
    fn only_motion_paths_are_reversed() {
        let (mut s, a, b) = session();
        // The fade is not a path: alone it is refused, beside a path it is left as it was.
        assert!(s.execute("animation.reversePath", &json!({"ids": [b]})).is_err());
        assert!(s.execute("animation.reversePath", &json!({"index": 1})).is_err());
        assert!(s.execute("animation.reversePath", &json!({"index": 99})).is_err());
        assert!(s.execute("animation.reversePath", &json!({"index": -1})).is_err());
        let r = s.execute("animation.reversePath", &json!({"ids": [a, b]})).unwrap();
        assert_eq!(r["changed"], 1);
        assert_eq!(path(&s, 1), None);
    }

    #[test]
    fn a_reversed_path_survives_a_pptx_round_trip() {
        let (mut s, a, _) = session();
        s.execute("animation.reversePath", &json!({"ids": [a]})).unwrap();
        let want = MotionPath::parse(&path(&s, 0).unwrap());
        let back = deckcraft_pptx::import(&deckcraft_pptx::export(&s.doc().unwrap().doc).unwrap()).unwrap();
        let anim = back.slides[1].animations.iter().find(|x| x.path.is_some()).expect("the motion path");
        let got = MotionPath::parse(anim.path.as_deref().unwrap());
        for k in 0..=10 {
            let p = f64::from(k) / 10.0;
            assert!(near(want.at(p), got.at(p)), "{:?} vs {:?}", want.points, got.points);
        }
    }
}
