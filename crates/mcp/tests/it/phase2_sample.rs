//! The Phase 2 sample deck, `samples/phase-2.deckcraft`, built through MCP tool calls and nothing
//! else, as an agent would: every step is a `tools/call` request to the server.
//!
//! One slide per typography feature of Phase 2: the paragraph composer with hyphenation and
//! optical margins beside the single-line composer, paragraph and character styles (built-in and
//! a deck style), a baseline grid, threaded text, text wrap, and a table with a decimal column
//! and a header row style.
//!
//! The file in `samples/` is what this script writes. `DECKCRAFT_BLESS_GOLDEN=1` rewrites it;
//! only do that when the script changed on purpose, and bless its golden pictures too
//! (`crates/engine/tests/it/golden.rs`).

use std::path::PathBuf;

use deckcraft_mcp::{Headless, Server};
use deckcraft_model::text::Composer;
use deckcraft_model::{Presentation, ShapeKind};
use serde_json::{Value, json};

struct Mcp {
    server: Server,
    requests: u64,
}

impl Mcp {
    fn new() -> Self {
        Mcp { server: Server::new(Box::new(Headless::new())), requests: 0 }
    }

    /// One `tools/call`; the tool's answer, parsed when it is JSON. A tool error fails the test.
    fn tool(&mut self, name: &str, arguments: Value) -> Value {
        self.requests += 1;
        let line = json!({"jsonrpc": "2.0", "id": self.requests, "method": "tools/call", "params": {"name": name, "arguments": arguments}});
        let reply: Value = serde_json::from_str(&self.server.handle_line(&line.to_string()).unwrap()).unwrap();
        let text = reply["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string();
        assert!(reply["error"].is_null() && reply["result"]["isError"] != true, "{name}: {text} {}", reply["error"]);
        serde_json::from_str(&text).unwrap_or(Value::String(text))
    }

    fn cmd(&mut self, command: &str, params: Value) -> Value {
        self.tool("run_command", json!({"command": command, "params": params}))
    }

    fn id(v: &Value) -> u64 {
        v["id"].as_u64().unwrap_or_else(|| panic!("no id in {v}"))
    }

    /// A text box with `text` at 18 pt, selected. No colour is given: text takes the theme's, so
    /// the deck reads in dark themes too.
    fn text_box(&mut self, x: u32, y: u32, w: u32, text: &str) -> u64 {
        Self::id(&self.tool("add_text_box", json!({"x": x, "y": y, "w": w, "text": text, "size": 18})))
    }
}

const PASSAGE: &str = "Good typography is mostly invisible. When the spacing between words stays even, when lines end without awkward gaps and the edge of the text looks straight, a reader takes in the slide without noticing why it feels calm.";

/// The script. Coordinates are points on a 960 × 540 slide.
fn build() -> Mcp {
    let mut m = Mcp::new();
    m.tool("new_presentation", json!({}));
    let first = m.tool("inspect_slide", json!({}));
    let (title, subtitle) = (Mcp::id(&first["shapes"][0]), Mcp::id(&first["shapes"][1]));
    m.tool("set_text", json!({"id": title, "text": "Typography"}));
    m.tool("set_text", json!({"id": subtitle, "text": "Phase 2 sample deck, built through MCP only"}));

    // 2. The two composers, justified, side by side.
    m.tool("add_slide", json!({"layout": "titleOnly", "title": "One line at a time, or the whole paragraph"}));
    let left = m.text_box(70, 170, 380, PASSAGE);
    m.cmd("format.justify", json!({}));
    let right = m.text_box(510, 170, 380, PASSAGE);
    m.cmd("format.justify", json!({}));
    m.cmd("format.typography", json!({"composer": "paragraph", "hyphenate": true, "opticalMargin": true}));
    for (x, label) in [(70, "Single-line composer"), (510, "Paragraph composer, hyphenation, optical margins")] {
        m.tool("add_text_box", json!({"x": x, "y": 440, "w": 380, "text": label, "size": 14}));
    }
    let _ = (left, right);

    // 3. Styles: built-in paragraph styles, a deck style based on one, and character styles.
    m.tool("add_slide", json!({"layout": "blank"}));
    m.text_box(70, 50, 820, "Styles follow the theme");
    m.cmd("style.apply", json!({"id": "heading"}));
    m.cmd("style.define", json!({"id": "lead", "name": "Lead", "basedOn": "body", "run": {"italic": true, "size": 22}}));
    m.text_box(70, 120, 520, "A deck style, Lead, starts from Body and makes it larger and italic.");
    m.cmd("style.apply", json!({"id": "lead"}));
    m.text_box(70, 220, 520, "Body text sets with the paragraph composer and optical margins by default, so it reads evenly.");
    m.cmd("style.apply", json!({"id": "body"}));
    let accent = m.text_box(70, 330, 520, "Strong and Accent are character styles.");
    m.cmd("text.edit", json!({"id": accent, "at": [0, 0]}));
    m.cmd("text.select", json!({"anchor": [0, 0], "caret": [0, 6]}));
    m.cmd("style.apply", json!({"id": "strong"}));
    m.cmd("text.select", json!({"anchor": [0, 11], "caret": [0, 17]}));
    m.cmd("style.apply", json!({"id": "accent"}));
    m.cmd("text.exit", json!({}));
    m.text_box(640, 120, 260, "42%");
    m.cmd("style.apply", json!({"id": "bigNumber"}));
    m.text_box(640, 260, 260, "of decks use no styles at all");
    m.cmd("style.apply", json!({"id": "caption"}));
    m.text_box(70, 420, 820, "\u{201C}Style is a simple way of saying complicated things.\u{201D}");
    m.cmd("style.apply", json!({"id": "quote"}));

    // 4. A baseline grid: text in different sizes lines up across the slide.
    m.tool("add_slide", json!({"layout": "titleOnly", "title": "Lines that share a grid"}));
    m.cmd("design.baselineGrid", json!({"increment": 24, "offset": 6}));
    let big = m.tool(
        "add_text_box",
        json!({"x": 70, "y": 165, "w": 380, "text": "Larger text on the left, set at twenty points with a little more air.", "size": 20}),
    );
    m.cmd("format.typography", json!({"snapToGrid": true}));
    let small = m.tool(
        "add_text_box",
        json!({"x": 510, "y": 170, "w": 380, "text": "Smaller text on the right still sits on the same baselines, line for line, because both boxes snap to the slide's grid.", "size": 15}),
    );
    m.cmd("format.typography", json!({"snapToGrid": true}));
    let _ = (big, small);

    // 5. Threaded text: one passage across three columns.
    m.tool("add_slide", json!({"layout": "titleOnly", "title": "One text, three columns"}));
    let story = format!("{PASSAGE} {PASSAGE} {PASSAGE}");
    let cols: Vec<u64> =
        [70, 365, 660].iter().map(|&x| Mcp::id(&m.tool("add_text_box", json!({"x": x, "y": 165, "w": 250, "text": "", "size": 16})))).collect();
    m.tool("set_text", json!({"id": cols[0], "text": story}));
    // Fixed-height columns: a text box grows to fit its text unless autofit is off.
    m.tool("select", json!({"ids": cols}));
    m.cmd("format.autofit", json!({"mode": "none"}));
    m.cmd("shape.resize", json!({"h": 300, "ids": cols}));
    m.cmd("text.threadTo", json!({"from": cols[0], "to": cols[1]}));
    m.cmd("text.threadTo", json!({"from": cols[1], "to": cols[2]}));

    // 6. Text wrap around a shape.
    m.tool("add_slide", json!({"layout": "titleOnly", "title": "Text that makes room"}));
    let body = m.tool("add_text_box", json!({"x": 70, "y": 160, "w": 820, "text": story, "size": 17}));
    let disc = Mcp::id(&m.tool("add_shape", json!({"preset": "ellipse", "x": 640, "y": 175, "w": 220, "h": 150, "fill": "#2E6FD8"})));
    m.tool("select", json!({"ids": [disc]}));
    m.cmd("arrange.textWrap", json!({"mode": "around", "margin": 10}));
    let _ = body;

    // 7. A table with a decimal column and styled header and first column.
    m.tool("add_slide", json!({"layout": "titleOnly", "title": "Numbers that line up"}));
    let table = Mcp::id(&m.tool(
        "insert_table",
        json!({"rows": 5, "cols": 3, "x": 160, "y": 170, "w": 640, "h": 250, "data": [
            ["Region", "Share", "Change"],
            ["North", "12.5", "+0.75"],
            ["South", "1,234.25", "-12.125"],
            ["East", "7", "+3.5"],
            ["West", "0.125", "0"]]}),
    ));
    m.cmd("table.columnFormat", json!({"id": table, "col": 1, "decimal": true}));
    m.cmd("table.columnFormat", json!({"id": table, "col": 2, "decimal": true}));
    m.cmd("table.columnFormat", json!({"id": table, "col": 0, "style": "caption"}));
    m.cmd("table.headerStyle", json!({"id": table, "style": "heading"}));
    m
}

fn saved(m: &mut Mcp, tag: &str) -> Vec<u8> {
    let path = std::env::temp_dir().join(format!("deckcraft-phase2-{}-{tag}.deckcraft", std::process::id()));
    m.tool("save_presentation", json!({"path": path.to_string_lossy()}));
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).ok();
    bytes
}

fn sample_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples/phase-2.deckcraft")
}

fn load(bytes: &[u8]) -> Presentation {
    deckcraft_format::load(bytes).unwrap()
}

#[test]
fn the_sample_deck_is_what_the_mcp_script_builds() {
    let built = saved(&mut build(), "sample");
    if std::env::var_os("DECKCRAFT_BLESS_GOLDEN").is_some() {
        std::fs::write(sample_path(), &built).unwrap();
    }
    let stored =
        std::fs::read(sample_path()).unwrap_or_else(|e| panic!("{}: {e}; run with DECKCRAFT_BLESS_GOLDEN=1 to write it", sample_path().display()));
    assert!(load(&built) == load(&stored), "samples/phase-2.deckcraft is not what the script builds; bless it if the script changed on purpose");
}

#[test]
fn the_script_builds_the_same_deck_every_time() {
    let (a, b) = (saved(&mut build(), "a"), saved(&mut build(), "b"));
    assert!(load(&a) == load(&b), "two runs of the script gave different decks");
}

/// Every Phase 2 feature the script used is in the saved deck, not only in the session.
#[test]
fn each_phase_2_feature_leaves_its_mark_in_the_deck() {
    let doc = load(&saved(&mut build(), "features"));
    assert_eq!(doc.slides.len(), 7);
    let paras = |si: usize| -> Vec<deckcraft_model::text::Paragraph> {
        doc.slides[si].shapes.iter().filter_map(|s| s.text.as_ref()).flat_map(|t| t.paragraphs.clone()).collect()
    };
    let composed = paras(1).into_iter().filter(|p| p.props.composer == Some(Composer::Paragraph)).count();
    assert_eq!(composed, 1, "format.typography composer on one of the two passages");
    assert!(paras(1).iter().any(|p| p.props.hyphenate == Some(true) && p.props.optical_margin == Some(true)));

    let styles: Vec<String> = paras(2).iter().filter_map(|p| p.props.style.clone()).collect();
    for want in ["heading", "lead", "body", "bigNumber", "caption", "quote"] {
        assert!(styles.iter().any(|s| s == want), "style.apply {want}: {styles:?}");
    }
    assert!(doc.text_styles.iter().any(|s| s.id == "lead" && s.based_on.as_deref() == Some("body")), "style.define lead");
    for want in ["strong", "accent"] {
        assert!(paras(2).iter().flat_map(|p| p.runs.clone()).any(|r| r.props.style.as_deref() == Some(want)), "character style {want}");
    }

    assert_eq!(doc.slides[3].baseline_grid.and_then(|g| g.active()), Some((24.0, 6.0)));
    assert_eq!(paras(3).iter().filter(|p| p.props.snap_to_grid == Some(true)).count(), 2);

    let threaded = doc.slides[4].shapes.iter().filter(|s| s.thread_next.is_some()).count();
    assert_eq!(threaded, 2, "text.threadTo twice");

    assert!(doc.slides[5].shapes.iter().any(|s| s.text_wrap.is_some_and(|w| w.margin == 10.0)), "arrange.textWrap");

    let table = doc.slides[6].shapes.iter().find_map(|s| match &s.kind {
        ShapeKind::Table(t) => Some(t.clone()),
        _ => None,
    });
    let table = table.expect("a table");
    assert!(table.col_formats.get(1).is_some_and(|f| f.decimal) && table.col_formats.get(2).is_some_and(|f| f.decimal));
    assert_eq!(table.col_formats.first().and_then(|f| f.style.as_deref()), Some("caption"));
    assert_eq!(table.header_style.as_deref(), Some("heading"));
}
