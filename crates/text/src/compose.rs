//! The paragraph composer: chooses a paragraph's line breaks all at once (Knuth–Plass total fit,
//! through `craft_text::linebreak`) instead of filling one line at a time, with optional
//! hyphenation (US English, `craft_text::hyphen`) and optical margin alignment.
//!
//! It works on the same shaped cells as the single-line breaker ([`super::breaking::break_lines`])
//! and returns the same [`RawLine`]s, so placement, justification, bidi and drawing are shared.
//! A paragraph the composer does not handle (tabs, no wrapping, vertical text runs through the
//! caller's checks) falls back to the single-line breaker: [`compose_lines`] returns `None`.
//!
//! Item model, per cell (one cell per character):
//! - a space is glue: elastic when the paragraph is justified, rigid otherwise (ragged lines are
//!   rated against a rag stretch instead, `Params::rag`);
//! - any other cell is a box of its advance;
//! - a break opportunity that is not a space (after a hyphen, between ideographs) is a penalty;
//!   after an explicit hyphen it is flagged, like a hyphenated break;
//! - a soft hyphen (U+00AD) and an automatic hyphenation point are flagged penalties that add a
//!   hyphen's width;
//! - a line break (U+000B, `\n`) is a forced break after fill glue;
//! - inside a word wider than the line, emergency penalties at cluster boundaries.

use craft_text::hyphen::{self, Limits};
use craft_text::linebreak::{self, Item, Kind, Params, Rag};
use deckcraft_model::text::Align;

use crate::breaking::{BreakParams, RawLine, break_opportunities, measure_str};
use crate::style::ParaShaped;

/// Cost of a hyphenated break: slides prefer a looser line to a hyphen.
const HYPHEN_COST: f64 = 275.0;
/// Cost of a break after an explicit hyphen (`well-|known`).
const EXPLICIT_HYPHEN_COST: f64 = 50.0;
/// At most this many consecutive lines end with a hyphen.
const HYPHEN_LIMIT: usize = 2;
/// Justified word spaces: up to a third wider, a fifth narrower.
const WORD_STRETCH: f64 = 1.0 / 3.0;
const WORD_SHRINK: f64 = 0.2;
/// Knuth–Plass passes, the last accepting overfull lines.
const TOLERANCES: [f64; 2] = [2.5, 12.0];
/// Paragraphs longer than this keep the single-line breaker (bounded work per paragraph).
const MAX_CELLS: usize = 20_000;

/// Composer settings of one paragraph, resolved from its properties.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ComposeOpts {
    pub(crate) justify: bool,
    pub(crate) hyphenate: bool,
    pub(crate) optical: bool,
}

impl ComposeOpts {
    pub(crate) fn of(p: &ParaShaped) -> ComposeOpts {
        let props = &p.props;
        // Kashida, distributed and Thai justification keep their own spreading; the composer
        // rates them like justified text.
        let justify = matches!(props.align, Some(Align::Justify | Align::JustLow | Align::Distributed | Align::ThaiDistributed));
        // Hyphenation and protrusion are set for left-to-right text only.
        ComposeOpts { justify, hyphenate: props.hyphenate.unwrap_or(false) && !p.rtl, optical: props.optical_margin.unwrap_or(false) && !p.rtl }
    }
}

/// Optical margin protrusion of `c`, as fractions of its advance: (start of line, end of line).
/// Copied from DesignCraft (`crates/compose/src/breaker.rs`, `hang`, at d3190ef; Copyright (c) 2026
/// ArtCraft Team and the DesignCraft contributors, MIT OR Apache-2.0; see NOTICE).
pub(crate) fn hang(c: char) -> (f64, f64) {
    match c {
        '.' | ',' => (0.0, 1.0),
        '-' | '\u{2010}' | '\u{2011}' | '\u{AD}' => (0.75, 0.75),
        '\'' | '"' | '‘' | '’' | '“' | '”' | '‚' | '„' | '‹' | '›' | '«' | '»' => (1.0, 1.0),
        '\u{2013}' | '\u{2014}' => (0.5, 0.5),
        ':' | ';' => (0.0, 0.5),
        '!' | '?' => (0.0, 0.2),
        '*' => (0.3, 0.3),
        '(' | '[' | '{' => (0.1, 0.0),
        ')' | ']' | '}' => (0.0, 0.1),
        'A' | 'V' | 'W' | 'Y' | 'v' | 'w' | 'y' | 'T' => (0.05, 0.05),
        'O' | 'C' | 'G' | 'Q' | 'o' | 'c' => (0.03, 0.03),
        _ => (0.0, 0.0),
    }
}

fn is_space(c: char) -> bool {
    c == ' '
}

fn is_forced(c: char) -> bool {
    c == '\u{b}' || c == '\n'
}

/// A hyphenatable letter of a word.
fn is_word_letter(c: char) -> bool {
    c.is_alphabetic() || matches!(c, '\'' | '’')
}

/// Width of a hyphen in the style of cell `i`.
fn hyphen_width(p: &ParaShaped, i: usize) -> f64 {
    p.cells.get(i).and_then(|c| p.faces.get(c.face).zip(p.styles.get(c.style))).map(|(f, st)| measure_str(f, st.size, "-").0).unwrap_or(0.0)
}

/// The language of cell `i` is one the bundled hyphenator knows (US English, or unset).
fn english(p: &ParaShaped, i: usize) -> bool {
    p.cells
        .get(i)
        .and_then(|c| p.styles.get(c.style))
        .and_then(|st| st.lang.as_deref())
        .is_none_or(|l| l.is_empty() || l.to_ascii_lowercase().starts_with("en"))
}

/// Cells before which an automatic hyphen may go.
fn hyphenation_points(p: &ParaShaped) -> Vec<bool> {
    let n = p.cells.len();
    let mut out = vec![false; n];
    let mut i = 0;
    while i < n {
        if !p.cells.get(i).is_some_and(|c| is_word_letter(c.ch)) {
            i += 1;
            continue;
        }
        let start = i;
        while i < n && p.cells.get(i).is_some_and(|c| is_word_letter(c.ch)) {
            i += 1;
        }
        if !english(p, start) {
            continue;
        }
        let word: String = p.cells.get(start..i).unwrap_or(&[]).iter().map(|c| c.ch).collect();
        let starts: Vec<usize> = word.char_indices().map(|(b, _)| b).collect();
        for b in hyphen::hyphen_points(&word, &Limits::default()) {
            if let Some(k) = starts.iter().position(|&s| s == b)
                && let Some(slot) = out.get_mut(start + k)
            {
                *slot = true;
            }
        }
    }
    out
}

/// Items for the paragraph plus, for each item, the cell it stands before (`n` past the end).
fn items_of(p: &ParaShaped, o: ComposeOpts, allowed: &[bool], hyph: &[bool], emergency: &[bool]) -> (Vec<Item>, Vec<usize>) {
    let cells = &p.cells;
    let n = cells.len();
    let mut items = Vec::with_capacity(n + n / 4 + 2);
    let mut at = Vec::with_capacity(items.capacity());
    let breakable = |j: usize| cells.get(j).is_none_or(|c| c.breakable(j));
    // Right-edge protrusion of the cell before `i`, for a line that ends there.
    let hang_before = |i: usize| -> f64 {
        if !o.optical || i == 0 {
            return 0.0;
        }
        cells.get(i - 1).map(|c| hang(c.ch).1 * c.adv).unwrap_or(0.0)
    };
    for (i, c) in cells.iter().enumerate() {
        if is_forced(c.ch) {
            let h = hang_before(i);
            items.push(Item::fill());
            at.push(i);
            items.push(Item { hang: h, ..Item::forced() });
            at.push(i);
            continue;
        }
        if is_space(c.ch) {
            let prev_space = i > 0 && cells.get(i - 1).is_some_and(|c| is_space(c.ch));
            // A space that UAX #14 forbids breaking after (or a run of them) is held to the box
            // before it with an infinite penalty; glue after a penalty is not a break.
            if !prev_space && !allowed.get(i + 1).copied().unwrap_or(true) {
                items.push(Item::no_break());
                at.push(i);
            }
            let w = c.adv;
            let h = hang_before(i);
            let glue = if o.justify { Item::glue(w, w * WORD_STRETCH, w * WORD_SHRINK) } else { Item::glue(w, 0.0, 0.0) };
            items.push(Item { hang: h, ..glue });
            at.push(i);
            continue;
        }
        let prev = i.checked_sub(1).and_then(|j| cells.get(j));
        let after_space = prev.is_some_and(|c| is_space(c.ch) || is_forced(c.ch));
        if i > 0 && !after_space && breakable(i) {
            let prev_ch = prev.map(|c| c.ch).unwrap_or(' ');
            if prev_ch == '\u{AD}' {
                // The soft hyphen's cell stays on the line; the line draws it as a hyphen.
                let w = hyphen_width(p, i - 1);
                let h = if o.optical { w * hang('-').1 } else { 0.0 };
                items.push(Item { hyphenation: true, hang: h, ..Item::penalty(w, HYPHEN_COST, true) });
                at.push(i);
            } else if allowed.get(i).copied().unwrap_or(false) {
                let explicit = matches!(prev_ch, '-' | '\u{2010}');
                let cost = if explicit { EXPLICIT_HYPHEN_COST } else { 0.0 };
                items.push(Item { hang: hang_before(i), ..Item::penalty(0.0, cost, explicit) });
                at.push(i);
            } else if hyph.get(i).copied().unwrap_or(false) {
                let w = hyphen_width(p, i);
                let h = if o.optical { w * hang('-').1 } else { 0.0 };
                items.push(Item { hyphenation: true, hang: h, ..Item::penalty(w, HYPHEN_COST, true) });
                at.push(i);
            } else if emergency.get(i).copied().unwrap_or(false) {
                items.push(Item::emergency());
                at.push(i);
            }
        }
        let start_hang = if o.optical { hang(c.ch).0 * c.adv } else { 0.0 };
        items.push(Item { hang: start_hang, ..Item::boxed(c.adv) });
        at.push(i);
    }
    // The paragraph's end: the last line is never stretched.
    let h = hang_before(n);
    items.push(Item::fill());
    at.push(n);
    items.push(Item { hang: h, ..Item::forced() });
    at.push(n);
    (items, at)
}

/// Breaks one paragraph with the paragraph composer; `None` when it should keep the
/// single-line breaker.
pub(crate) fn compose_lines(p: &ParaShaped, params: &BreakParams, o: ComposeOpts) -> Option<Vec<RawLine>> {
    let n = p.cells.len();
    if !params.wrap || n == 0 || n > MAX_CELLS || p.cells.iter().any(|c| c.ch == '\t') {
        return None;
    }
    let text: String = p.cells.iter().map(|c| c.ch).collect();
    let (allowed, _) = break_opportunities(&text, p.props.latin_line_break.unwrap_or(false));
    let hyph = if o.hyphenate { hyphenation_points(p) } else { Vec::new() };
    let (mut items, mut at) = items_of(p, o, &allowed, &hyph, &[]);
    // Words no line can hold get emergency breaks at their cluster boundaries.
    let narrow = params.first_w.min(params.rest_w) - params.cuts.iter().map(|c| c.1).fold(0.0, f64::max);
    let long = linebreak::overlong(&items, narrow);
    if !long.is_empty() {
        let mut emergency = vec![false; n];
        for r in long {
            for k in r {
                if let Some(&ci) = at.get(k)
                    && let Some(e) = emergency.get_mut(ci)
                {
                    *e = true;
                }
            }
        }
        (items, at) = items_of(p, o, &allowed, &hyph, &emergency);
    }
    let cut = |k: usize| params.cuts.get(k).map_or(0.0, |c| c.1);
    let width = |k: usize| ((if k == 0 { params.first_w } else { params.rest_w }) - cut(k)).max(1.0);
    let kp = Params { hyphen_limit: HYPHEN_LIMIT, uniform_from: params.cuts.len().max(1), rag: (!o.justify).then(Rag::default), ..Params::default() };
    let breaks = linebreak::break_paragraph(&items, &width, &kp, &TOLERANCES);
    let cell_at = |k: usize| at.get(k).copied().unwrap_or(n);
    let mut lines = Vec::with_capacity(breaks.lines.len());
    for l in &breaks.lines {
        let start = cell_at(l.start);
        let end_item = items.get(l.end);
        let mut end = cell_at(l.end);
        let mut next = cell_at(l.next);
        let mut hyphen = false;
        match end_item.map(|it| it.kind) {
            // A forced break consumes its line-break character; the next line starts after it.
            Some(Kind::Penalty) if end_item.is_some_and(|it| it.cost <= -linebreak::INFINITE_PENALTY) => {
                next = if end < n { end + 1 } else { n };
            }
            Some(Kind::Penalty) => {
                let it = end_item.copied().unwrap_or(Item::forced());
                // An automatic hyphen is drawn after the line; a soft hyphen draws itself.
                hyphen = it.hyphenation && p.cells.get(end.wrapping_sub(1)).is_none_or(|c| c.ch != '\u{AD}');
            }
            Some(Kind::Glue) => {}
            // Greedy split a word with no break in it: start the next line at a cluster.
            Some(Kind::Box) | None => {
                end = (end..n).find(|&j| p.cells.get(j).is_none_or(|c| c.breakable(j))).unwrap_or(n);
                next = end;
            }
            Some(_) => {}
        }
        // Trailing spaces belong to the line but don't count.
        while end > start && p.cells.get(end - 1).is_some_and(|c| is_space(c.ch)) {
            end -= 1;
        }
        let next = next.max(end).min(n);
        let left = if o.optical {
            items.get(l.start..l.end).unwrap_or(&[]).iter().find(|it| it.kind == Kind::Box).map(|it| it.hang).unwrap_or(0.0)
        } else {
            0.0
        };
        let right = if o.optical { end_item.map(|it| it.hang).unwrap_or(0.0) } else { 0.0 };
        if lines.last().is_some_and(|prev: &RawLine| prev.start == start && start < n) {
            continue;
        }
        lines.push(RawLine::composed(start, end, next, hyphen, (left, right)));
    }
    if lines.is_empty() {
        return None;
    }
    Some(lines)
}
