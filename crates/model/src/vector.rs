//! Vector networks (SlideCraft, Phase 3, after FigCraft and Figma): points joined by straight or
//! curved segments in any arrangement, not only chains. Any point can join any number of
//! segments, and closed loops of segments can be filled as regions.
//!
//! A network is stored on a shape (`Shape::vector`) in its own `w` × `h` space, like a custom
//! path, so it scales with the shape. The shape's geometry is derived from it
//! ([`VectorNetwork::to_geom`]): one filled path per region and stroked paths for the segments.
//! That is what the renderer draws and what `.pptx` gets (`custGeom` paths can be fill-only or
//! stroke-only), so PowerPoint shows the same picture.

use serde::{Deserialize, Serialize};

use crate::{CustomPath, Geom};
use deckcraft_geom::preset::FillMode;

/// Most points, segments or regions in one network.
pub const MAX_ITEMS: usize = 10_000;

/// A segment between points `a` and `b`. `ta` and `tb` are the curve handles, relative to `a`
/// and `b`; both zero is a straight line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Segment {
    pub a: u32,
    pub b: u32,
    #[serde(skip_serializing_if = "is_zero")]
    pub ta: [f64; 2],
    #[serde(skip_serializing_if = "is_zero")]
    pub tb: [f64; 2],
}

fn is_zero(v: &[f64; 2]) -> bool {
    v[0] == 0.0 && v[1] == 0.0
}

/// A filled area: one or more closed loops of segments (by index). With `even_odd`, a loop
/// inside another cuts a hole whichever way it runs; otherwise the winding decides.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Region {
    pub loops: Vec<Vec<u32>>,
    pub even_odd: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct VectorNetwork {
    /// The space the points are in (scaled to the shape's box).
    pub w: f64,
    pub h: f64,
    pub vertices: Vec<[f64; 2]>,
    pub segments: Vec<Segment>,
    pub regions: Vec<Region>,
}

fn num(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() { "0".into() } else { s.to_string() }
}

impl VectorNetwork {
    /// Why the network can't be used, if it can't: indices out of range, too many items, or
    /// non-finite numbers.
    pub fn check(&self) -> Result<(), String> {
        if self.vertices.len() > MAX_ITEMS || self.segments.len() > MAX_ITEMS || self.regions.len() > MAX_ITEMS {
            return Err(format!("a vector network holds at most {MAX_ITEMS} points, segments and regions"));
        }
        if !(self.w.is_finite() && self.h.is_finite() && self.w >= 0.0 && self.h >= 0.0) {
            return Err("the network's size is not finite".into());
        }
        if self.vertices.iter().flatten().any(|v| !v.is_finite()) {
            return Err("a point is not finite".into());
        }
        let n = self.vertices.len();
        for (i, s) in self.segments.iter().enumerate() {
            if s.a as usize >= n || s.b as usize >= n {
                return Err(format!("segment {i} joins a point that doesn't exist"));
            }
            if s.ta.iter().chain(s.tb.iter()).any(|v| !v.is_finite()) {
                return Err(format!("segment {i} has a handle that is not finite"));
            }
        }
        let m = self.segments.len();
        for (i, r) in self.regions.iter().enumerate() {
            if r.loops.iter().flatten().any(|s| *s as usize >= m) {
                return Err(format!("region {i} names a segment that doesn't exist"));
            }
            if r.loops.iter().map(Vec::len).sum::<usize>() > MAX_ITEMS {
                return Err(format!("region {i} is too long"));
            }
        }
        Ok(())
    }

    fn pt(&self, i: u32) -> [f64; 2] {
        self.vertices.get(i as usize).copied().unwrap_or([0.0, 0.0])
    }

    /// Path data for segment `s` drawn from `from` to its other end (`C` for curves, `L` lines).
    fn seg_to(&self, s: &Segment, from_a: bool) -> String {
        let (p0, p1, t0, t1) = if from_a { (self.pt(s.a), self.pt(s.b), s.ta, s.tb) } else { (self.pt(s.b), self.pt(s.a), s.tb, s.ta) };
        if is_zero(&t0) && is_zero(&t1) {
            format!("L {} {}", num(p1[0]), num(p1[1]))
        } else {
            format!("C {} {} {} {} {} {}", num(p0[0] + t0[0]), num(p0[1] + t0[1]), num(p1[0] + t1[0]), num(p1[1] + t1[1]), num(p1[0]), num(p1[1]))
        }
    }

    /// The closed path of one loop: its segments in order, each turned to start where the last
    /// ended. A segment that doesn't connect starts a new sub-path.
    fn loop_path(&self, segs: &[u32]) -> String {
        let mut d = String::new();
        let mut at: Option<u32> = None;
        for &si in segs {
            let Some(s) = self.segments.get(si as usize) else { continue };
            let from_a = match at {
                Some(v) if v == s.a => true,
                Some(v) if v == s.b => false,
                _ => {
                    if !d.is_empty() {
                        d += " Z ";
                    }
                    let p = self.pt(s.a);
                    d += &format!("M {} {} ", num(p[0]), num(p[1]));
                    true
                }
            };
            d += &self.seg_to(s, from_a);
            d.push(' ');
            at = Some(if from_a { s.b } else { s.a });
        }
        if !d.is_empty() {
            d += "Z";
        }
        d.trim_end().to_string()
    }

    /// The segments as stroked sub-paths: runs through points that join exactly two segments
    /// are drawn as one path, so their corners join instead of overlapping caps.
    fn stroke_path(&self) -> String {
        let n = self.vertices.len();
        let mut degree = vec![0u32; n];
        for s in &self.segments {
            for v in [s.a, s.b] {
                if let Some(d) = degree.get_mut(v as usize) {
                    *d = d.saturating_add(1);
                }
            }
        }
        let mut used = vec![false; self.segments.len()];
        let mut by_vertex: Vec<Vec<usize>> = vec![vec![]; n];
        for (i, s) in self.segments.iter().enumerate() {
            for v in [s.a, s.b] {
                if let Some(l) = by_vertex.get_mut(v as usize) {
                    l.push(i);
                }
            }
        }
        let mut d = String::new();
        // Start runs at points that don't continue a run (ends and branches), then close loops.
        let starts: Vec<usize> = (0..self.segments.len())
            .filter(|i| self.segments.get(*i).is_some_and(|s| degree.get(s.a as usize).copied() != Some(2)))
            .chain(0..self.segments.len())
            .collect();
        for first in starts {
            if used.get(first).copied().unwrap_or(true) {
                continue;
            }
            let Some(s) = self.segments.get(first) else { continue };
            let p = self.pt(s.a);
            d += &format!("M {} {} ", num(p[0]), num(p[1]));
            let mut cur = first;
            let mut from_a = true;
            let start_vertex = s.a;
            loop {
                if let Some(u) = used.get_mut(cur) {
                    *u = true;
                }
                let Some(s) = self.segments.get(cur) else { break };
                d += &self.seg_to(s, from_a);
                d.push(' ');
                let end = if from_a { s.b } else { s.a };
                if end == start_vertex {
                    d += "Z ";
                    break;
                }
                if degree.get(end as usize).copied() != Some(2) {
                    break;
                }
                let next = by_vertex.get(end as usize).and_then(|l| l.iter().copied().find(|i| !used.get(*i).copied().unwrap_or(true)));
                let Some(next) = next else { break };
                cur = next;
                from_a = self.segments.get(next).is_some_and(|s| s.a == end);
            }
        }
        d.trim_end().to_string()
    }

    /// The shape geometry the network draws as: a filled path for each region (no outline), and
    /// one stroked path for the segments (no fill).
    pub fn to_geom(&self) -> Geom {
        let (w, h) = (self.w.max(1e-6), self.h.max(1e-6));
        let mut paths: Vec<CustomPath> = vec![];
        for r in &self.regions {
            let d = r.loops.iter().map(|l| self.loop_path(l)).filter(|d| !d.is_empty()).collect::<Vec<_>>().join(" ");
            if !d.is_empty() {
                paths.push(CustomPath { w, h, d, fill: FillMode::Norm, stroke: false });
            }
        }
        let strokes = self.stroke_path();
        if !strokes.is_empty() {
            paths.push(CustomPath { w, h, d: strokes, fill: FillMode::None, stroke: true });
        }
        if paths.is_empty() {
            // Nothing to draw: an empty path keeps the shape a custom one.
            paths.push(CustomPath { w, h, d: String::new(), fill: FillMode::None, stroke: false });
        }
        Geom::Custom { paths }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn square_with_a_tail() -> VectorNetwork {
        // A square 0..10 with a line from its corner (10,10) out to (20,20): the corner joins three
        // segments, which a path can't do.
        VectorNetwork {
            w: 20.0,
            h: 20.0,
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0], [20.0, 20.0]],
            segments: vec![
                Segment { a: 0, b: 1, ..Default::default() },
                Segment { a: 1, b: 2, ..Default::default() },
                Segment { a: 2, b: 3, ..Default::default() },
                Segment { a: 3, b: 0, ..Default::default() },
                Segment { a: 2, b: 4, ..Default::default() },
            ],
            regions: vec![Region { loops: vec![vec![0, 1, 2, 3]], even_odd: false }],
        }
    }

    #[test]
    fn a_branching_network_fills_its_region_and_strokes_every_segment() {
        let n = square_with_a_tail();
        n.check().unwrap();
        let Geom::Custom { paths } = n.to_geom() else { panic!() };
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].d, "M 0 0 L 10 0 L 10 10 L 0 10 L 0 0 Z");
        assert_eq!((paths[0].fill, paths[0].stroke), (FillMode::Norm, false));
        assert_eq!((paths[1].fill, paths[1].stroke), (FillMode::None, true));
        // Every segment is stroked exactly once.
        let strokes = paths[1].d.matches(['L', 'C']).count();
        assert_eq!(strokes, 5, "{}", paths[1].d);
    }

    #[test]
    fn loops_follow_segments_whichever_way_they_were_drawn() {
        let mut n = square_with_a_tail();
        // Reverse segment 2 (3 → 2) and give segment 0 a curve.
        n.segments[2] = Segment { a: 3, b: 2, ..Default::default() };
        n.segments[0].ta = [3.0, -4.0];
        n.segments[0].tb = [-3.0, -4.0];
        let Geom::Custom { paths } = n.to_geom() else { panic!() };
        assert_eq!(paths[0].d, "M 0 0 C 3 -4 7 -4 10 0 L 10 10 L 0 10 L 0 0 Z");
    }

    #[test]
    fn bad_networks_are_refused_and_never_panic() {
        let mut n = square_with_a_tail();
        n.segments.push(Segment { a: 0, b: 99, ..Default::default() });
        assert!(n.check().is_err());
        // Drawing one anyway (it is never stored) doesn't panic.
        let _ = n.to_geom();
        let mut n = square_with_a_tail();
        n.regions[0].loops[0].push(500);
        assert!(n.check().is_err());
        let mut n = square_with_a_tail();
        n.vertices[1][0] = f64::NAN;
        assert!(n.check().is_err());
        let n = VectorNetwork { w: 1.0, h: 1.0, vertices: vec![[0.0, 0.0]; MAX_ITEMS + 1], ..Default::default() };
        assert!(n.check().is_err());
        // An empty network draws nothing but stays a custom geometry.
        let Geom::Custom { paths } = VectorNetwork::default().to_geom() else { panic!() };
        assert_eq!(paths.len(), 1);
    }
}
