//! The Phase 3 sample deck, `samples/phase-3.deckcraft`, built through MCP tool calls and nothing
//! else, as an agent would: every step is a `tools/call` request to the server.
//!
//! One slide per canvas feature of Phase 3: auto layout cards, an auto layout grid with `fr`
//! tracks and a spanning tile, frames (clip, corner radius, constraints) and a group edited in
//! place, a vector network and the four editable booleans, and the new effects with a Display P3
//! colour.
//!
//! The file in `samples/` is what this script writes. `DECKCRAFT_BLESS_GOLDEN=1` rewrites it;
//! only do that when the script changed on purpose, and bless its golden pictures too
//! (`crates/engine/tests/it/golden.rs`).

use std::path::PathBuf;

use deckcraft_mcp::{Headless, Server};
use deckcraft_model::canvas::{BoolOp, Flow, Pin};
use deckcraft_model::{Presentation, Shape, ShapeKind};
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
        v["id"].as_u64().or_else(|| v["ids"][0].as_u64()).unwrap_or_else(|| panic!("no id in {v}"))
    }

    fn text(&mut self, x: u32, y: u32, w: u32, text: &str, size: u32) -> u64 {
        Self::id(&self.tool("add_text_box", json!({"x": x, "y": y, "w": w, "text": text, "size": size})))
    }

    fn shape(&mut self, preset: &str, x: u32, y: u32, w: u32, h: u32, fill: &str) -> u64 {
        let id = Self::id(&self.tool("add_shape", json!({"preset": preset, "x": x, "y": y, "w": w, "h": h, "fill": fill})));
        self.cmd("shape.line", json!({"ids": [id], "none": true}));
        id
    }
}

const CARDS: [(&str, &str, &str); 3] = [
    ("Plan", "Outline the story first: one idea per slide, in the order the audience needs it.", "#2E6FD8"),
    ("Design", "Auto layout keeps cards aligned and evenly spaced as their text changes.", "#17A398"),
    ("Present", "Frames, grids and styles follow the theme, so the deck stays consistent.", "#E4572E"),
];

/// The script. Coordinates are points on a 960 × 540 slide.
fn build() -> Mcp {
    let mut m = Mcp::new();
    m.tool("new_presentation", json!({}));
    let first = m.tool("inspect_slide", json!({}));
    let (title, subtitle) = (Mcp::id(&first["shapes"][0]), Mcp::id(&first["shapes"][1]));
    m.tool("set_text", json!({"id": title, "text": "Canvas and layout"}));
    m.tool("set_text", json!({"id": subtitle, "text": "Phase 3 sample deck, built through MCP only"}));

    // 2. Auto layout cards: each card hugs its text, the row spaces the cards evenly.
    m.tool("add_slide", json!({"layout": "titleOnly", "title": "Cards that line themselves up"}));
    let mut cards = vec![];
    for (i, (head, body, color)) in CARDS.iter().enumerate() {
        let x = 60 + i as u32 * 300;
        let bar = m.shape("rect", x, 170, 240, 6, color);
        let h = m.text(x, 190, 240, head, 24);
        let b = m.text(x, 240, 240, body, 15);
        let card = Mcp::id(&m.cmd("frame.create", json!({"ids": [bar, h, b], "fill": "#F2F4F8", "radius": 14})));
        m.cmd("layout.set", json!({"ids": [card], "flow": "vertical", "gap": 10, "padding": 20, "width": "hug", "height": "hug"}));
        cards.push(card);
    }
    let row = Mcp::id(&m.cmd("layout.set", json!({"ids": cards, "flow": "horizontal", "gap": 24, "padding": 0, "width": "hug", "height": "hug"})));
    m.cmd("shape.move", json!({"ids": [row], "x": 36, "y": 165}));

    // 3. An auto layout grid: fr tracks, a tile spanning two columns, tiles filling their cells.
    m.tool("add_slide", json!({"layout": "titleOnly", "title": "A grid with fr tracks"}));
    let colors = ["#2E6FD8", "#17A398", "#FFC914", "#E4572E", "#76B041"];
    let tiles: Vec<u64> = colors.iter().enumerate().map(|(i, c)| m.shape("roundRect", 80 + i as u32 * 30, 160, 100, 80, c)).collect();
    let grid = Mcp::id(&m.cmd(
        "layout.set",
        json!({"ids": tiles, "flow": "grid", "columns": ["2fr", "1fr", "1fr"], "rows": ["1fr", "1fr"], "gap": 16, "crossGap": 16, "padding": 16}),
    ));
    m.cmd("shape.setBounds", json!({"ids": [grid], "x": 80, "y": 150, "w": 800, "h": 340}));
    m.cmd("shape.fill", json!({"ids": [grid], "color": "#EEF1F6"}));
    m.cmd("frame.set", json!({"ids": [grid], "radius": 20}));
    m.cmd("layout.child", json!({"ids": tiles, "width": "fill", "height": "fill"}));
    m.cmd("layout.child", json!({"ids": [tiles[0]], "columnSpan": 2}));

    // 4. Frames: one clips a large circle with rounded corners; one is resized and its members
    //    follow their constraints; a group's member is moved without ungrouping.
    m.tool("add_slide", json!({"layout": "titleOnly", "title": "Frames clip, constrain and keep their size"}));
    let sun = m.shape("ellipse", 40, 230, 300, 300, "#FFC914");
    let clipped = Mcp::id(&m.cmd("frame.create", json!({"ids": [sun], "fill": "#2E6FD8"})));
    m.cmd("shape.setBounds", json!({"ids": [clipped], "x": 60, "y": 160, "w": 240, "h": 300}));
    m.cmd("frame.set", json!({"ids": [clipped], "clip": true, "radius": 24}));
    // A panel whose header stretches and whose badge stays at the right when it widens.
    let header = m.shape("rect", 360, 160, 240, 40, "#17A398");
    let badge = m.shape("ellipse", 560, 220, 28, 28, "#E4572E");
    let note = m.text(372, 260, 200, "Header: stretch. Badge: right.", 14);
    let panel = Mcp::id(&m.cmd("frame.create", json!({"ids": [header, badge, note], "fill": "#F2F4F8", "radius": 10})));
    m.cmd("shape.setBounds", json!({"ids": [panel], "x": 360, "y": 160, "w": 240, "h": 300}));
    m.cmd("layout.constraints", json!({"ids": [header], "horizontal": "both"}));
    m.cmd("layout.constraints", json!({"ids": [badge], "horizontal": "end"}));
    m.cmd("shape.setBounds", json!({"ids": [panel], "x": 360, "y": 160, "w": 300, "h": 300}));
    // A group: Enter selects its members, one moves, the group stays.
    let a = m.shape("rect", 700, 180, 80, 80, "#76B041");
    let b = m.shape("rect", 800, 180, 80, 80, "#76B041");
    let group = Mcp::id(&m.cmd("arrange.group", json!({"ids": [a, b]})));
    m.tool("select", json!({"ids": [group]}));
    m.cmd("edit.selectChildren", json!({}));
    m.cmd("shape.move", json!({"ids": [b], "dy": 120}));
    m.text(700, 400, 200, "Edited inside the group", 14);

    // 5. A vector network (a point joins three segments) and the four editable booleans.
    m.tool("add_slide", json!({"layout": "titleOnly", "title": "Vector networks and editable booleans"}));
    m.cmd(
        "vector.create",
        json!({
            "vertices": [[80, 330], [200, 180], [320, 330], [200, 280], [200, 450]],
            "segments": [[0, 1], [1, 2], [2, 0], [1, 3], {"a": 3, "b": 4, "ta": [60, 40], "tb": [-60, -40]}],
            "regions": [{"loops": [[0, 1, 2]]}],
            "name": "Network"
        }),
    );
    m.cmd("shape.fill", json!({"color": "#BFD4F6"}));
    m.cmd("shape.line", json!({"color": "#2E6FD8", "width": 3}));
    let ops = [("union", 380), ("subtract", 520), ("intersect", 660), ("exclude", 800)];
    for (op, x) in ops {
        let (p, q) = if op == "subtract" {
            (m.shape("ellipse", x, 220, 110, 110, "#E4572E"), m.shape("ellipse", x + 30, 250, 50, 50, "#E4572E"))
        } else {
            (m.shape("ellipse", x, 220, 80, 80, "#E4572E"), m.shape("ellipse", x + 35, 250, 80, 80, "#E4572E"))
        };
        m.cmd("boolean.create", json!({"op": op, "ids": [p, q]}));
        m.text(x, 360, 120, op, 14);
    }

    // 6. Effects: frosted glass over colour, a blurred disc, grain, and a Display P3 swatch.
    m.tool("add_slide", json!({"layout": "blank"}));
    m.shape("ellipse", 60, 60, 300, 300, "#2E6FD8");
    m.shape("ellipse", 260, 160, 300, 300, "#E4572E");
    m.shape("ellipse", 120, 260, 260, 260, "#FFC914");
    let glass = m.shape("roundRect", 140, 140, 320, 220, "#FFFFFF");
    m.cmd("shape.fill", json!({"ids": [glass], "transparency": 0.55}));
    m.cmd("format.blur", json!({"ids": [glass], "background": 24}));
    m.text(160, 230, 280, "Frosted glass", 28);
    let soft = m.shape("ellipse", 620, 70, 120, 120, "#17A398");
    m.cmd("format.blur", json!({"ids": [soft], "layer": 12}));
    m.text(600, 200, 200, "Layer blur", 14);
    let grain = m.shape("rect", 780, 70, 120, 120, "#76B041");
    m.cmd("format.noise", json!({"ids": [grain], "amount": 0.6}));
    m.text(780, 200, 140, "Noise", 14);
    let p3 = m.shape("rect", 620, 300, 280, 120, "#000000");
    m.cmd("format.p3", json!({"ids": [p3], "fill": "color(display-p3 0.18 0.72 0.45)"}));
    m.text(620, 430, 300, "Display P3 colour, drawn as its nearest sRGB", 14);
    m
}

fn saved(m: &mut Mcp, tag: &str) -> Vec<u8> {
    let path = std::env::temp_dir().join(format!("deckcraft-phase3-{}-{tag}.deckcraft", std::process::id()));
    m.tool("save_presentation", json!({"path": path.to_string_lossy()}));
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).ok();
    bytes
}

fn sample_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples/phase-3.deckcraft")
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
    assert!(load(&built) == load(&stored), "samples/phase-3.deckcraft is not what the script builds; bless it if the script changed on purpose");
}

#[test]
fn the_script_builds_the_same_deck_every_time() {
    let (a, b) = (saved(&mut build(), "a"), saved(&mut build(), "b"));
    assert!(load(&a) == load(&b), "two runs of the script gave different decks");
}

fn all(shapes: &[Shape]) -> Vec<&Shape> {
    let mut v = vec![];
    deckcraft_model::walk(shapes, &mut |s, _| v.push(s));
    v
}

/// Every Phase 3 feature the script used is in the saved deck, not only in the session.
#[test]
fn each_phase_3_feature_leaves_its_mark_in_the_deck() {
    let doc = load(&saved(&mut build(), "features"));
    assert_eq!(doc.slides.len(), 6);
    let s = |i: usize| all(&doc.slides[i].shapes);

    // Cards: three vertical hugging frames in a horizontal hugging row, evenly spaced.
    let row = s(1).into_iter().find(|x| x.auto_layout.as_ref().is_some_and(|a| a.flow == Flow::Horizontal)).expect("the row");
    let ShapeKind::Group { children, .. } = &row.kind else { panic!() };
    assert_eq!(children.len(), 3);
    assert!(children.iter().all(|c| c.frame.is_some() && c.auto_layout.as_ref().is_some_and(|a| a.flow == Flow::Vertical)));
    let xs: Vec<(f64, f64)> = children.iter().map(|c| c.xfrm.map(|x| (x.x, x.w)).unwrap()).collect();
    assert!((xs[1].0 - (xs[0].0 + xs[0].1) - 24.0).abs() < 1e-6, "gap 24: {xs:?}");

    // Grid: a spanning tile twice as wide as a one-column tile plus the gap would allow.
    let grid = s(2).into_iter().find(|x| x.auto_layout.as_ref().is_some_and(|a| a.flow == Flow::Grid)).expect("the grid");
    let ShapeKind::Group { children, .. } = &grid.kind else { panic!() };
    assert_eq!(children.first().and_then(|c| c.layout_child).map(|c| c.column_span), Some(2));
    assert!(grid.frame.is_some_and(|f| f.radius == 20.0));

    // Frames: a clipping frame, constraints, and a group edited in place.
    assert!(s(3).iter().any(|x| x.frame.is_some_and(|f| f.clip && f.radius == 24.0)));
    assert!(s(3).iter().any(|x| x.constraints.is_some_and(|k| k.horizontal == Pin::Both)));
    assert!(s(3).iter().any(|x| x.constraints.is_some_and(|k| k.horizontal == Pin::End)));
    let group = s(3).into_iter().find(|x| x.frame.is_none() && matches!(x.kind, ShapeKind::Group { .. }) && x.boolean.is_none()).expect("the group");
    let ShapeKind::Group { children, .. } = &group.kind else { panic!() };
    assert!((children[1].xfrm.unwrap().y - children[0].xfrm.unwrap().y - 120.0).abs() < 1e-6, "member moved in place");

    // A vector network with a three-way point, and the four booleans.
    let net = s(4).into_iter().find_map(|x| x.vector.clone()).expect("a vector network");
    assert!(net.segments.iter().filter(|g| g.a == 1 || g.b == 1).count() >= 3);
    let ops: Vec<BoolOp> = s(4).iter().filter_map(|x| x.boolean).collect();
    assert_eq!(ops, vec![BoolOp::Union, BoolOp::Subtract, BoolOp::Intersect, BoolOp::Exclude]);

    // Effects and P3.
    let fx: Vec<_> = s(5).iter().filter_map(|x| x.effects.clone()).collect();
    assert!(fx.iter().any(|e| e.background_blur == Some(24.0)));
    assert!(fx.iter().any(|e| e.layer_blur == Some(12.0)));
    assert!(fx.iter().any(|e| e.noise == Some(0.6)));
    assert!(s(5).iter().any(|x| x.p3.and_then(|k| k.fill) == Some([0.18, 0.72, 0.45])));
}
