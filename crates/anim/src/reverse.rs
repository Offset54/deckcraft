//! Reverse Path Direction: the same motion path travelled the other way.
//!
//! The reversed path keeps its place on the slide. A path is written relative to where its shape
//! sits, so the reversed one starts at the old end point and finishes where the old one began:
//! played, the shape starts at the far end and comes back. Curves stay curves (a cubic's control
//! points swap, a quadratic keeps its one), so reversing twice gives the same route again.

use std::fmt::Write as _;

/// Longest path text taken, and the most segments read from it.
const MAX_TEXT: usize = 64 * 1024;
const MAX_SEGMENTS: usize = 4_000;

type Pt = (f64, f64);

/// One drawn piece of a path, in absolute coordinates, ending at its last point.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Seg {
    /// A jump without drawing (`M` after the first).
    Move(Pt),
    Line(Pt),
    Cubic(Pt, Pt, Pt),
    Quad(Pt, Pt),
    /// `Z`: a line back to the start of the subpath, which is the point held.
    Close(Pt),
}

impl Seg {
    fn end(&self) -> Pt {
        match *self {
            Seg::Move(p) | Seg::Line(p) | Seg::Cubic(_, _, p) | Seg::Quad(_, p) | Seg::Close(p) => p,
        }
    }
}

/// The path as its first point and the segments after it; `None` when it is not a motion path
/// (unknown letters, a number missing or not finite, nothing drawn, or too long).
fn read(text: &str) -> Option<(Pt, Vec<Seg>)> {
    if text.len() > MAX_TEXT {
        return None;
    }
    // Commands and numbers. A number may run straight into the next ("1-2", "0.5.5").
    let mut toks: Vec<Result<char, f64>> = Vec::new();
    let cs: Vec<char> = text.chars().collect();
    let mut i = 0;
    while let Some(&c) = cs.get(i) {
        if c.is_whitespace() || c == ',' {
            i += 1;
        } else if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' {
            let start = i;
            i += 1;
            let mut dot = c == '.';
            while let Some(&d) = cs.get(i) {
                let exp = (d == 'e' || d == 'E') && {
                    let n1 = cs.get(i + 1).copied().unwrap_or(' ');
                    let n2 = cs.get(i + 2).copied().unwrap_or(' ');
                    n1.is_ascii_digit() || ((n1 == '-' || n1 == '+') && n2.is_ascii_digit())
                };
                if d.is_ascii_digit() {
                    i += 1;
                } else if d == '.' && !dot {
                    dot = true;
                    i += 1;
                } else if exp {
                    i += 2;
                    while cs.get(i).is_some_and(char::is_ascii_digit) {
                        i += 1;
                    }
                    break;
                } else {
                    break;
                }
            }
            let num: String = cs.get(start..i)?.iter().collect();
            toks.push(Err(num.parse::<f64>().ok().filter(|v| v.is_finite())?));
        } else {
            toks.push(Ok(c));
            i += 1;
        }
        if toks.len() > MAX_SEGMENTS * 7 {
            return None;
        }
    }
    let mut first: Option<Pt> = None;
    let mut segs: Vec<Seg> = Vec::new();
    let (mut cur, mut sub) = ((0.0, 0.0), (0.0, 0.0));
    let mut cmd = 'M';
    let mut k = 0;
    while let Some(tok) = toks.get(k) {
        if let Ok(c) = tok {
            k += 1;
            match c {
                'E' | 'e' => break,
                'Z' | 'z' => {
                    first.get_or_insert(cur);
                    segs.push(Seg::Close(sub));
                    cur = sub;
                }
                'M' | 'm' | 'L' | 'l' | 'C' | 'c' | 'Q' | 'q' | 'H' | 'h' | 'V' | 'v' => cmd = *c,
                _ => return None,
            }
            continue;
        }
        let need = match cmd.to_ascii_uppercase() {
            'C' => 6,
            'Q' => 4,
            'H' | 'V' => 1,
            _ => 2,
        };
        let mut v = [0.0f64; 6];
        for (n, slot) in v.iter_mut().enumerate().take(need) {
            *slot = toks.get(k + n).and_then(|t| t.err())?;
        }
        k += need;
        let base = if cmd.is_ascii_lowercase() { cur } else { (0.0, 0.0) };
        let at = |x: f64, y: f64| (base.0 + x, base.1 + y);
        let seg = match cmd.to_ascii_uppercase() {
            'M' => {
                let p = at(v[0], v[1]);
                sub = p;
                // Numbers after a move's first pair are lines.
                cmd = if cmd.is_ascii_lowercase() { 'l' } else { 'L' };
                if first.is_none() {
                    first = Some(p);
                    cur = p;
                    continue;
                }
                Seg::Move(p)
            }
            'L' => Seg::Line(at(v[0], v[1])),
            'H' => Seg::Line((base.0 + v[0], cur.1)),
            'V' => Seg::Line((cur.0, base.1 + v[0])),
            'C' => Seg::Cubic(at(v[0], v[1]), at(v[2], v[3]), at(v[4], v[5])),
            _ => Seg::Quad(at(v[0], v[1]), at(v[2], v[3])),
        };
        first.get_or_insert(cur);
        cur = seg.end();
        if !(cur.0.is_finite() && cur.1.is_finite()) || segs.len() >= MAX_SEGMENTS {
            return None;
        }
        segs.push(seg);
    }
    let first = first?;
    if segs.iter().all(|s| matches!(s, Seg::Move(_))) {
        return None;
    }
    Some((first, segs))
}

/// A coordinate as path text: six decimals, no trailing zeros.
fn num(v: f64) -> String {
    let t = format!("{v:.6}");
    let t = t.trim_end_matches('0').trim_end_matches('.');
    if t == "-0" || t.is_empty() { "0".into() } else { t.into() }
}

/// `path` travelled from its end to its start, in the same motion path syntax (absolute
/// commands, slide fractions). `None` when `path` is not a motion path that draws anything.
pub fn reverse_path(path: &str) -> Option<String> {
    let (first, segs) = read(path)?;
    // The point each segment starts from.
    let mut starts = Vec::with_capacity(segs.len());
    let mut cur = first;
    for s in &segs {
        starts.push(cur);
        cur = s.end();
    }
    // One subpath that ends by closing can close again, when the piece that now comes last (the
    // old first one) is a straight line back to where the reversed path starts.
    let closes =
        matches!(segs.last(), Some(Seg::Close(_))) && !segs.iter().any(|s| matches!(s, Seg::Move(_))) && matches!(segs.first(), Some(Seg::Line(_)));
    let mut out = format!("M {} {}", num(cur.0), num(cur.1));
    for (n, (seg, from)) in segs.iter().zip(&starts).enumerate().rev() {
        let (x, y) = (num(from.0), num(from.1));
        let _ = match *seg {
            _ if n == 0 && closes => write!(out, " Z"),
            Seg::Move(_) => write!(out, " M {x} {y}"),
            Seg::Line(_) | Seg::Close(_) => write!(out, " L {x} {y}"),
            Seg::Cubic(c1, c2, _) => write!(out, " C {} {} {} {} {x} {y}", num(c2.0), num(c2.1), num(c1.0), num(c1.1)),
            Seg::Quad(c, _) => write!(out, " Q {} {} {x} {y}", num(c.0), num(c.1)),
        };
    }
    out.push_str(" E");
    Some(out)
}

#[cfg(test)]
mod patch_tests {
    use super::reverse_path;
    use crate::MotionPath;

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4
    }

    /// The reversed path is at every moment where the original was at the mirrored moment.
    fn mirrors(path: &str) {
        let rev = reverse_path(path).unwrap_or_else(|| panic!("{path} reverses"));
        let (a, b) = (MotionPath::parse(path), MotionPath::parse(&rev));
        assert!((a.length() - b.length()).abs() < 1e-4, "{path} → {rev}: {} vs {}", a.length(), b.length());
        for k in 0..=20 {
            let p = f64::from(k) / 20.0;
            assert!(close(a.at(p), b.at(1.0 - p)), "{path} → {rev} at {p}: {:?} vs {:?}", a.at(p), b.at(1.0 - p));
        }
    }

    #[test]
    fn a_line_comes_back_from_its_end() {
        assert_eq!(reverse_path("M 0 0 L 0.25 0 E").as_deref(), Some("M 0.25 0 L 0 0 E"));
        mirrors("M 0 0 L 0.25 0 L 0.25 0.1 E");
    }

    #[test]
    fn curves_stay_curves_and_mirror() {
        assert_eq!(reverse_path("M 0 0 C 0.1 0 0.2 0.1 0.3 0.3 E").as_deref(), Some("M 0.3 0.3 C 0.2 0.1 0.1 0 0 0 E"));
        mirrors("M 0 0 C 0.1 -0.1 0.2 0.1 0.1 0.1 C 0 0.1 0.1 -0.1 0.25 0 E");
        mirrors("M 0 0 Q 0.2 -0.3 0.4 0 L 0.5 0.2 E");
    }

    #[test]
    fn relative_and_one_axis_commands_are_read() {
        mirrors("m 0 0 l 0.1 0.1 h 0.2 v -0.3 c 0.1 0 0.1 0.1 0 0.1 q -0.1 0 -0.1 -0.1 e");
        mirrors("M0,0L.1.2-.3-.4E");
    }

    #[test]
    fn a_closed_path_stays_closed() {
        let rev = reverse_path("M 0 0 L 0.2 0 L 0.2 0.2 Z E").unwrap();
        assert!(rev.ends_with("Z E"), "{rev}");
        mirrors("M 0 0 L 0.2 0 L 0.2 0.2 Z E");
        // A closing curve can't be a Z, and still returns to the start.
        mirrors("M 0 0 C 0.1 0 0.1 0.15 0 0.15 C -0.1 0.15 -0.1 0 0 0 Z E");
    }

    #[test]
    fn reversing_twice_is_the_same_route() {
        for path in ["M 0 0 L 0.25 0 E", "M 0 0 C 0.1 0 0.2 0.1 0.3 0.3 L 0.1 0.1 E", "M 0 0 L 0.2 0 L 0.2 0.2 Z E"] {
            let twice = reverse_path(&reverse_path(path).unwrap()).unwrap();
            let (a, b) = (MotionPath::parse(path), MotionPath::parse(&twice));
            for k in 0..=10 {
                let p = f64::from(k) / 10.0;
                assert!(close(a.at(p), b.at(p)), "{path} → {twice}");
            }
        }
    }

    #[test]
    fn every_preset_path_reverses() {
        for a in deckcraft_model::anim::ANIMATIONS.iter().filter(|a| a.2 == deckcraft_model::anim::AnimClass::Path) {
            let options: Vec<&str> = if a.5.is_empty() { vec![""] } else { a.5.to_vec() };
            for o in options {
                // A custom path has no preset route: nothing to reverse until it is drawn.
                let path = crate::default_path(a.0, o);
                if path.is_empty() {
                    assert_eq!(a.0, "customPath");
                    continue;
                }
                mirrors(&path);
            }
        }
    }

    #[test]
    fn hostile_text_is_refused_not_guessed() {
        for bad in ["", "E", "M 0 0 E", "M 0 0 L 1 E", "M 0 0 L 1e999 0 E", "M 0 0 X 1 1 E", "M 0 0 L nan 1 E", "L", "M 0 0 M 1 1 E"] {
            assert_eq!(reverse_path(bad), None, "{bad:?}");
        }
        let long = format!("M 0 0 {}E", "L 0.1 0.1 ".repeat(10_000));
        assert_eq!(reverse_path(&long), None);
    }
}
