//! Vector networks (SlideCraft, Phase 3): commands to make, read and edit them, and to turn a
//! path drawn with the pen (Curve, Freeform) or pencil (Scribble) tools, or any shape, into one.
//! The model and its geometry are in `deckcraft_model::vector`.
//!
//! Points are given and returned in slide points; the network is stored in its shape's own box.

use deckcraft_geom::{PathEl, Point, Rect, Xfrm};
use deckcraft_model::vector::{MAX_ITEMS, Region, Segment, VectorNetwork};
use deckcraft_model::{Shape, ShapeId, ShapeKind};
use serde_json::{Value, json};

use super::{CommandSpec, bad, cmd, has_selection, has_slide, id_param, ids_param, xfrm_of};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "vector.create",
            "Vector Network",
            ["Insert", "Illustrations", "Shapes"],
            None,
            "{vertices: [[x,y]] pt, segments: [[a,b] | {a, b, ta?: [dx,dy], tb?: [dx,dy]}] (ta/tb: curve handles relative to the points), regions?: [{loops: [[segment index]], evenOdd?: bool}], name?} → {id}: points joined by straight or curved segments in any arrangement (a point can join more than two), with filled regions",
            has_slide,
            create
        ),
        cmd!(
            "vector.set",
            "Edit Vector Network",
            [],
            None,
            "{id?, vertices?: [[x,y]] pt, segments?, regions?} → {id}: replace parts of a vector network (the shape's box follows its points)",
            has_selection,
            set
        ),
        cmd!(
            "vector.moveVertex",
            "Move Vector Point",
            [],
            None,
            "{id?, index, x, y: pt} → {id}: move one point; every segment it joins follows",
            has_selection,
            move_vertex
        ),
        cmd!(
            "vector.fromPath",
            "Convert to Vector Network",
            ["Shape Format", "Insert Shapes", "Edit Shape"],
            None,
            "{ids?} → {ids}: turn shapes (a pen or pencil path, any preset) into vector networks of their outlines; points that meet are joined",
            has_selection,
            from_path
        ),
        cmd!(query "vector.get", "Vector Network", [], None, "{id?} → {vertices, segments, regions} in slide points", has_selection, get),
    ]
}

fn pair(v: &Value) -> Option<[f64; 2]> {
    let a = v.as_array()?;
    let x = a.first()?.as_f64().filter(|v| v.is_finite())?;
    let y = a.get(1)?.as_f64().filter(|v| v.is_finite())?;
    (a.len() == 2).then_some([x, y])
}

fn index(v: Option<&Value>) -> Option<u32> {
    v?.as_u64().and_then(|n| u32::try_from(n).ok())
}

fn list<'a>(p: &'a Value, key: &str, cmd: &str) -> Result<Option<&'a Vec<Value>>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(a)) if a.len() <= MAX_ITEMS => Ok(Some(a)),
        Some(_) => Err(bad(cmd, format!("`{key}` must be a list of at most {MAX_ITEMS} items"))),
    }
}

fn parse_vertices(p: &Value, cmd: &str) -> Result<Option<Vec<[f64; 2]>>> {
    let Some(a) = list(p, "vertices", cmd)? else { return Ok(None) };
    a.iter()
        .enumerate()
        .map(|(i, v)| pair(v).ok_or_else(|| bad(cmd, format!("vertex {i} must be [x, y] in points"))))
        .collect::<Result<Vec<_>>>()
        .map(Some)
}

fn parse_segments(p: &Value, cmd: &str) -> Result<Option<Vec<Segment>>> {
    let Some(a) = list(p, "segments", cmd)? else { return Ok(None) };
    let mut out = vec![];
    for (i, v) in a.iter().enumerate() {
        let s = match v {
            Value::Array(ab) if ab.len() == 2 => index(ab.first()).zip(index(ab.get(1))).map(|(a, b)| Segment { a, b, ..Default::default() }),
            Value::Object(_) => {
                let handle = |k: &str| match v.get(k) {
                    None | Some(Value::Null) => Some([0.0, 0.0]),
                    Some(h) => pair(h),
                };
                match (index(v.get("a")), index(v.get("b")), handle("ta"), handle("tb")) {
                    (Some(a), Some(b), Some(ta), Some(tb)) => Some(Segment { a, b, ta, tb }),
                    _ => None,
                }
            }
            _ => None,
        };
        out.push(s.ok_or_else(|| bad(cmd, format!("segment {i} must be [a, b] or {{a, b, ta?, tb?}}")))?);
    }
    Ok(Some(out))
}

fn parse_regions(p: &Value, cmd: &str) -> Result<Option<Vec<Region>>> {
    let Some(a) = list(p, "regions", cmd)? else { return Ok(None) };
    let mut out = vec![];
    for (i, v) in a.iter().enumerate() {
        let loops = v.get("loops").and_then(Value::as_array).ok_or_else(|| bad(cmd, format!("region {i} needs `loops`")))?;
        let mut ls = vec![];
        for l in loops {
            let l = l.as_array().ok_or_else(|| bad(cmd, format!("region {i}: a loop is a list of segment indices")))?;
            ls.push(
                l.iter()
                    .map(|x| index(Some(x)).ok_or_else(|| bad(cmd, format!("region {i}: {x} is not a segment index"))))
                    .collect::<Result<Vec<u32>>>()?,
            );
        }
        out.push(Region { loops: ls, even_odd: v.get("evenOdd").and_then(Value::as_bool).unwrap_or(false) });
    }
    Ok(Some(out))
}

/// The box a network's points and handles need (at least 1 pt each way).
fn bounds(vertices: &[[f64; 2]], segments: &[Segment]) -> Option<Rect> {
    let mut b: Option<Rect> = None;
    let mut add = |x: f64, y: f64| {
        let r = Rect::new(x, y, x, y);
        b = Some(b.map_or(r, |q| q.union(r)));
    };
    for v in vertices {
        add(v[0], v[1]);
    }
    for s in segments {
        for (vi, t) in [(s.a, s.ta), (s.b, s.tb)] {
            if let Some(v) = vertices.get(vi as usize) {
                add(v[0] + t[0], v[1] + t[1]);
            }
        }
    }
    let b = b?;
    Some(Rect::new(b.x0, b.y0, b.x0 + b.width().max(1.0), b.y0 + b.height().max(1.0)))
}

/// A network from slide-point parts: its box and the network relative to it.
fn place(vertices: Vec<[f64; 2]>, segments: Vec<Segment>, regions: Vec<Region>, cmd: &str) -> Result<(Xfrm, VectorNetwork)> {
    let b = bounds(&vertices, &segments).ok_or_else(|| bad(cmd, "a vector network needs at least one point"))?;
    let net =
        VectorNetwork { w: b.width(), h: b.height(), vertices: vertices.iter().map(|v| [v[0] - b.x0, v[1] - b.y0]).collect(), segments, regions };
    net.check().map_err(|e| bad(cmd, e))?;
    Ok((Xfrm::new(b.x0, b.y0, b.width(), b.height()), net))
}

/// A stored network in slide points (its shape's box `x`; unrotated).
fn to_slide(net: &VectorNetwork, x: &Xfrm) -> Vec<[f64; 2]> {
    let (sx, sy) = (if net.w > 0.0 { x.w / net.w } else { 1.0 }, if net.h > 0.0 { x.h / net.h } else { 1.0 });
    net.vertices.iter().map(|v| [x.x + v[0] * sx, x.y + v[1] * sy]).collect()
}

fn scale_handles(segments: &[Segment], sx: f64, sy: f64) -> Vec<Segment> {
    segments.iter().map(|s| Segment { ta: [s.ta[0] * sx, s.ta[1] * sy], tb: [s.tb[0] * sx, s.tb[1] * sy], ..*s }).collect()
}

fn create(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "vector.create";
    let vertices = parse_vertices(p, CMD)?.ok_or_else(|| bad(CMD, "give `vertices`"))?;
    let segments = parse_segments(p, CMD)?.unwrap_or_default();
    let regions = parse_regions(p, CMD)?.unwrap_or_default();
    let (x, net) = place(vertices, segments, regions, CMD)?;
    let name = super::str_param(p, "name").map(|n| n.chars().take(256).collect::<String>());
    // The default shape style (theme fill and outline), as a drawn freeform gets: a nested
    // command, so the whole thing is one undo step.
    let r = s.execute("shape.insert", &json!({"preset": "rect", "rect": [x.x, x.y, x.w, x.h]}))?;
    let id = r.get("id").and_then(Value::as_u64).and_then(|v| u32::try_from(v).ok()).map(ShapeId).ok_or_else(|| bad(CMD, "insert failed"))?;
    s.edit(|doc, sel| {
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad(CMD, "no slide"))?;
        let sh = deckcraft_model::find_shape_mut(list, id).ok_or_else(|| bad(CMD, "insert failed"))?;
        sh.name = name.unwrap_or_else(|| format!("Vector {}", id.0));
        sh.xfrm = Some(x);
        sh.geom = net.to_geom();
        sh.vector = Some(net);
        sel.shapes = vec![id];
        Ok(json!({"id": id}))
    })
}

/// The vector shape a command acts on: `id`, else the one selected shape.
fn target(s: &Session, p: &Value, cmd: &str) -> Result<(ShapeId, Xfrm, VectorNetwork)> {
    let st = s.doc()?;
    let id = match id_param(p, "id", cmd)? {
        Some(id) => id,
        None => match st.selection.shapes.as_slice() {
            [one] => *one,
            _ => return Err(bad(cmd, "select one vector network or give `id`")),
        },
    };
    let sh = st.shape(id).ok_or_else(|| bad(cmd, format!("no object {id}")))?;
    let net = sh.vector.clone().ok_or_else(|| bad(cmd, format!("object {id} is not a vector network (vector.fromPath makes one)")))?;
    let x = xfrm_of(&st.doc, &st.selection, sh);
    Ok((id, x, net))
}

fn store(s: &mut Session, id: ShapeId, x: Xfrm, net: VectorNetwork, cmd: &'static str) -> Result<Value> {
    s.edit(|doc, sel| {
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad(cmd, "no slide"))?;
        let sh = deckcraft_model::find_shape_mut(list, id).ok_or_else(|| bad(cmd, format!("no object {id}")))?;
        sh.geom = net.to_geom();
        sh.vector = Some(net);
        sh.xfrm = Some(Xfrm { rot: sh.xfrm.map(|o| o.rot).unwrap_or(0.0), flip_h: false, flip_v: false, ..x });
        Ok(json!({"id": id}))
    })
}

fn set(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "vector.set";
    let (id, x, net) = target(s, p, CMD)?;
    let (sx, sy) = (if net.w > 0.0 { x.w / net.w } else { 1.0 }, if net.h > 0.0 { x.h / net.h } else { 1.0 });
    let vertices = parse_vertices(p, CMD)?.unwrap_or_else(|| to_slide(&net, &x));
    let segments = parse_segments(p, CMD)?.unwrap_or_else(|| scale_handles(&net.segments, sx, sy));
    let regions = parse_regions(p, CMD)?.unwrap_or_else(|| net.regions.clone());
    let (x2, net2) = place(vertices, segments, regions, CMD)?;
    store(s, id, x2, net2, CMD)
}

fn move_vertex(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "vector.moveVertex";
    let (id, x, net) = target(s, p, CMD)?;
    let i = index(p.get("index")).map(|i| i as usize).filter(|i| *i < net.vertices.len()).ok_or_else(|| bad(CMD, "`index` must name a point"))?;
    let px = super::f64_param(p, "x").ok_or_else(|| bad(CMD, "give `x` in points"))?;
    let py = super::f64_param(p, "y").ok_or_else(|| bad(CMD, "give `y` in points"))?;
    let (sx, sy) = (if net.w > 0.0 { x.w / net.w } else { 1.0 }, if net.h > 0.0 { x.h / net.h } else { 1.0 });
    let mut vertices = to_slide(&net, &x);
    if let Some(v) = vertices.get_mut(i) {
        *v = [px, py];
    }
    let (x2, net2) = place(vertices, scale_handles(&net.segments, sx, sy), net.regions.clone(), CMD)?;
    store(s, id, x2, net2, CMD)
}

fn get(s: &mut Session, p: &Value) -> Result<Value> {
    let (_, x, net) = target(s, p, "vector.get")?;
    let (sx, sy) = (if net.w > 0.0 { x.w / net.w } else { 1.0 }, if net.h > 0.0 { x.h / net.h } else { 1.0 });
    let segs: Vec<Value> = scale_handles(&net.segments, sx, sy).iter().map(|s| json!({"a": s.a, "b": s.b, "ta": s.ta, "tb": s.tb})).collect();
    let regions: Vec<Value> = net.regions.iter().map(|r| json!({"loops": r.loops, "evenOdd": r.even_odd})).collect();
    Ok(json!({"vertices": to_slide(&net, &x), "segments": segs, "regions": regions}))
}

/// The network of a shape's outline, in its own box (`w` × `h`): each sub-path becomes a chain of
/// segments, closed sub-paths of a filled path become a region, and points that meet are merged.
pub(crate) fn network_of(sh: &Shape, w: f64, h: f64) -> VectorNetwork {
    let geo = deckcraft_render::shape_geometry(sh, w, h);
    let mut net = VectorNetwork { w, h, ..Default::default() };
    let vertex = |net: &mut VectorNetwork, p: Point| -> u32 {
        if let Some(i) = net.vertices.iter().position(|v| (v[0] - p.x).abs() < 1e-6 && (v[1] - p.y).abs() < 1e-6) {
            return i as u32;
        }
        net.vertices.push([p.x, p.y]);
        (net.vertices.len() - 1) as u32
    };
    for sp in &geo.paths {
        let mut loops: Vec<Vec<u32>> = vec![];
        let mut cur: Vec<u32> = vec![];
        let (mut start, mut at): (Option<(u32, Point)>, Option<(u32, Point)>) = (None, None);
        let push = |net: &mut VectorNetwork, a: (u32, Point), to: Point, c1: Point, c2: Point, cur: &mut Vec<u32>| -> (u32, Point) {
            let b = vertex(net, to);
            if net.segments.len() < MAX_ITEMS {
                net.segments.push(Segment { a: a.0, b, ta: [c1.x - a.1.x, c1.y - a.1.y], tb: [c2.x - to.x, c2.y - to.y] });
                cur.push((net.segments.len() - 1) as u32);
            }
            (b, to)
        };
        for el in sp.path.elements() {
            match *el {
                PathEl::MoveTo(p) => {
                    if !cur.is_empty() {
                        loops.push(std::mem::take(&mut cur));
                    }
                    let v = (vertex(&mut net, p), p);
                    start = Some(v);
                    at = Some(v);
                }
                PathEl::LineTo(p) => {
                    if let Some(a) = at {
                        at = Some(push(&mut net, a, p, a.1, p, &mut cur));
                    }
                }
                PathEl::QuadTo(c, p) => {
                    if let Some(a) = at {
                        let c1 = a.1 + (c - a.1) * (2.0 / 3.0);
                        let c2 = p + (c - p) * (2.0 / 3.0);
                        at = Some(push(&mut net, a, p, c1, c2, &mut cur));
                    }
                }
                PathEl::CurveTo(c1, c2, p) => {
                    if let Some(a) = at {
                        at = Some(push(&mut net, a, p, c1, c2, &mut cur));
                    }
                }
                PathEl::ClosePath => {
                    if let (Some(a), Some(s0)) = (at, start)
                        && a.0 != s0.0
                    {
                        push(&mut net, a, s0.1, a.1, s0.1, &mut cur);
                    }
                    loops.push(std::mem::take(&mut cur));
                    at = start;
                }
            }
        }
        if !cur.is_empty() {
            loops.push(cur);
        }
        if sp.fill != deckcraft_geom::preset::FillMode::None {
            let closed: Vec<Vec<u32>> = loops.into_iter().filter(|l| l.len() > 1).collect();
            if !closed.is_empty() && net.regions.len() < MAX_ITEMS {
                net.regions.push(Region { loops: closed, even_odd: sp.even_odd });
            }
        }
    }
    net
}

fn from_path(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "vector.fromPath";
    let st = s.doc()?;
    let ids = ids_param(p, "ids").unwrap_or_else(|| st.selection.shapes.clone());
    let mut made = vec![];
    for id in &ids {
        let sh = st.shape(*id).ok_or_else(|| bad(CMD, format!("no object {id}")))?;
        if !matches!(sh.kind, ShapeKind::Shape) {
            return Err(bad(CMD, format!("object {id} is not a shape with an outline")));
        }
        let x = xfrm_of(&st.doc, &st.selection, sh);
        let net = network_of(sh, x.w, x.h);
        net.check().map_err(|e| bad(CMD, e))?;
        made.push((*id, net));
    }
    if made.is_empty() {
        return Err(bad(CMD, "select a shape"));
    }
    s.edit(|doc, sel| {
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad(CMD, "no slide"))?;
        for (id, net) in made {
            if let Some(sh) = deckcraft_model::find_shape_mut(list, id) {
                sh.geom = net.to_geom();
                sh.vector = Some(net);
            }
        }
        Ok(json!({"ids": ids}))
    })
}

/// A network whose geometry was edited some other way (Edit Points, Change Shape) is no longer
/// what it draws: the shape becomes a plain path (the canvas pass).
pub(crate) fn settle(list: &mut [Shape], depth: usize) {
    if depth > deckcraft_model::canvas::MAX_DEPTH {
        return;
    }
    for sh in list.iter_mut() {
        if let ShapeKind::Group { children, .. } = &mut sh.kind {
            settle(children, depth + 1);
        }
        if let Some(net) = &sh.vector
            && net.to_geom() != sh.geom
        {
            sh.vector = None;
        }
    }
}

#[cfg(test)]
#[path = "vector_tests.rs"]
mod tests;
