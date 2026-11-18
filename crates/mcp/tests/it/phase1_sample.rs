//! The Phase 1 sample deck, `samples/phase-1.deckcraft`, built through MCP tool calls and nothing
//! else: every step below is a `tools/call` request to the server, the way an agent sends it.
//!
//! The deck is the proof that the command surface is whole for what exists so far: slides, text,
//! shapes, connectors, a group, pictures, a table, a chart, an animation and a transition, and the
//! commands this fork's patches add (crop to an aspect ratio, a reversed motion path, slide
//! numbers from a chosen start, kerning from a size, recent colours).
//!
//! The file in `samples/` is what this script writes. `DECKCRAFT_BLESS_GOLDEN=1` rewrites it;
//! only do that when the script changed on purpose, and bless its golden pictures too
//! (`crates/engine/tests/it/golden.rs`).

use std::path::PathBuf;

use deckcraft_mcp::{Headless, Server};
use deckcraft_model::anim::AnimClass;
use deckcraft_model::{Presentation, ShapeKind};
use serde_json::{Value, json};

/// A 96 × 64 PNG drawn for this test (a diagonal gradient with a lighter band), as base64. It is
/// spelled out so the deck's bytes don't change with the PNG encoder.
const PICTURE: &str = concat!(
    "iVBORw0KGgoAAAANSUhEUgAAAGAAAABACAIAAABqVuVZAAAERklEQVR42u3Y9VIbBhgA8O9JJje5yU1u201ucpMbXWWVVdZ2lbWlFEpxd0sIwUJIIAGCBOLu",
    "7h4IEiAQXEpZZd1eYW/wvcC+V/j9+YNXEtuvxrdei22+Htt4I5p9M7r+VmTt7UjmnfDqu6GV90LL7wfTHwSXPgwsfhRY+Ng//4kv9alv7jPv7Ofe5Bee5Jee",
    "xFfu+Neu2Deu6LfOyHfO8PeO0A+O4I/2wE82/882X47Ve8LqOWlxn7K4TpudZ0yOsyb7OaPtvNF2wWC9aLBc0psv60xXdMarWsM1rf66RndDo72p1txSqW+r",
    "VHeUyrtKxT2FPFchy5NLH8gk+bKZAulMoXT6kURcJJkqmZksnZ4omx4vF4sqxWNVU6PVUyM1k8K6CUH9xHDD+FDjOL9ZxGsRDbaOcdvHuB2jA4wRDnOknyXs",
    "6xL2sgU93YLu3mF231BX/xCLw+/k8pmDPAaP18EfbB/mtgHpIDoCbiuQDqIjHGgB0kF0RgeagXQQnTFOM5AOoiPiNAHpIDrj/Y1AOojOZF8DkA6iM9VXD6SD",
    "6Ih764B0EJ3p3logHURH0lMDpIPoSHuqgXQQHVl3FZAOoiNnVwLpIDpKdgWQDqKj6ioH0kF01F1lQDqIjpZVCqSD6OhYJUA6iI6+sxhIB9ExMIuAdBAdE/MR",
    "kA6iY2YUQu7hy/sHf+cdvMjff16w/+zh3tPC3b+Kdo+Ld56U7ByVbT8u3z6s2Dqo3Nyv3tyr2dit3dity+40ZLcb17ea1jab1zZaM9m2zHr76hpjNcNcWe1c",
    "XmEtL7PT6e70Us/SYu/SQv/iPGchNbAwx52f483P8lPJoVRCMBcXzsZGZqOjyYgoGR5PhCYSwcl4QBzzT8d8M1GvJOqRRdzyiEsRdqrCTnXIoQnatUGbPmA1",
    "BCxGv9nkN1l8RqvXYPPq7R6d06N1uTVut9rjUvmcSr9TEXDIQw5Z2C6L2KVRmyRunUlYp5MW8axlKmWenDdPLJjGF42itHFs2TC6YhjJ6IVresG6bjirHdrU",
    "8rc0vG0ND0gH0dlRDwLpIDp7ai6QDqKzrxoA0kF0DpQcIB1E51DZD6SD6Bwp+oB0EJ0nil4gHUTnWN4DpIPoPJV3A+kgOs9kbCAdROe5tAtIB9F5IWUB6SA6",
    "LyWdQDqIzj8SJpAOovPvDAP+V/tlYRRaOx7aOwoc7fnOtgeutjxP631va66v5V6g5W6w+U6o6c9w0+1o461Y4814w41Ewx9AOojObP11IB1EZ67uGpAOopOq",
    "uwqkg+jM114F0kF0Fmt/B9JBdJZqrgDpIDrpmstAOojOSvUlIB1EZ7XqIpAOopOp+g1IB9FZq7wApIPoZCvPA+kgOhsV54B0EJ3NirNAOojOVvmvQDqIzk7Z",
    "GSAdRGe37DSQDqKzV3oKSAfR2S89CaSD6ByW/AKkg+g8LjkBpIPoHBXnAOkgOsfFOf8BJgX+2hkbX/kAAAAASUVORK5CYII=",
);

struct Mcp {
    server: Server,
    requests: u64,
}

impl Mcp {
    fn new() -> Self {
        Mcp { server: Server::new(Box::new(Headless::new())), requests: 0 }
    }

    /// One ; the tool's answer, parsed when it is JSON. A tool error fails the test.
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

    /// The id a tool or command answered with.
    fn id(v: &Value) -> u64 {
        v["id"].as_u64().unwrap_or_else(|| panic!("no id in {v}"))
    }
}

/// The script. Coordinates are points on a 960 × 540 slide.
fn build() -> Mcp {
    let mut m = Mcp::new();
    m.tool("new_presentation", json!({}));
    let first = m.tool("inspect_slide", json!({}));
    let (title, subtitle) = (Mcp::id(&first["shapes"][0]), Mcp::id(&first["shapes"][1]));
    m.tool("set_text", json!({"id": title, "text": "Foundations"}));
    m.tool("set_text", json!({"id": subtitle, "text": "Phase 1 sample deck, built through MCP only"}));

    m.tool(
        "add_slide",
        json!({"layout": "titleAndContent", "title": "What this phase checks",
            "body": "Every action is a command with a stable id\nUndo keeps one shared snapshot per step\nSlides render the same on every run\nPowerPoint files survive open, save and reopen"}),
    );

    m.tool("add_slide", json!({"layout": "titleOnly", "title": "Shapes, lines and a group"}));
    let plan =
        Mcp::id(&m.tool("add_shape", json!({"preset": "roundRect", "x": 80, "y": 180, "w": 200, "h": 110, "text": "Plan", "fill": "#2E6FD8"})));
    let build =
        Mcp::id(&m.tool("add_shape", json!({"preset": "ellipse", "x": 380, "y": 180, "w": 200, "h": 110, "text": "Build", "fill": "#F26B1D"})));
    let check =
        Mcp::id(&m.tool("add_shape", json!({"preset": "hexagon", "x": 680, "y": 180, "w": 200, "h": 110, "text": "Check", "fill": "#2EA36F"})));
    for (from, to) in [(plan, build), (build, check)] {
        m.cmd("shape.connect", json!({"from": from, "to": to, "preset": "straightConnector1"}));
        m.cmd("shape.line", json!({"color": "#1E2B3C", "width": 2.5, "tail": "triangle"}));
    }
    let grouped: Vec<u64> = [("star5", 330, "#E3B21C"), ("triangle", 440, "#8A5CD8"), ("diamond", 550, "#D83F6B")]
        .into_iter()
        .map(|(preset, x, fill)| Mcp::id(&m.tool("add_shape", json!({"preset": preset, "x": x, "y": 340, "w": 90, "h": 90, "fill": fill}))))
        .collect();
    m.tool("select", json!({"ids": grouped}));
    m.cmd("arrange.group", json!({}));
    m.tool(
        "add_text_box",
        json!({"x": 80, "y": 448, "w": 800, "text": "Three shapes joined by connectors, and three more in a group", "size": 16, "align": "center", "color": "#1E2B3C"}),
    );

    m.tool("add_slide", json!({"layout": "titleOnly", "title": "A picture, cropped to a square"}));
    m.tool("insert_picture", json!({"data": PICTURE, "name": "gradient.png", "x": 90, "y": 170, "w": 420, "h": 280}));
    let square = Mcp::id(&m.tool("insert_picture", json!({"data": PICTURE, "name": "gradient.png", "x": 570, "y": 170, "w": 300, "h": 280})));
    m.cmd("picture.cropAspect", json!({"ratio": 1, "ids": [square]}));

    m.tool("add_slide", json!({"layout": "titleOnly", "title": "A table"}));
    m.tool(
        "insert_table",
        json!({"rows": 4, "cols": 3, "x": 120, "y": 170, "w": 720, "h": 240, "data": [
            ["Check", "Tool", "Result"],
            ["Layering", "cargo xtask layers", "enforced"],
            ["Undo", "deckcraft-cli bench", "measured"],
            ["Rendering", "golden images", "compared"]]}),
    );

    m.tool("add_slide", json!({"layout": "titleOnly", "title": "A chart"}));
    m.tool(
        "insert_chart",
        json!({"type": "bar", "title": "Sample data", "categories": ["North", "South", "East", "West"],
            "series": [{"name": "This year", "values": [42, 31, 55, 27]}, {"name": "Last year", "values": [35, 36, 40, 30]}],
            "x": 140, "y": 160, "w": 680, "h": 320}),
    );

    m.tool("add_slide", json!({"layout": "titleOnly", "title": "A motion path, reversed"}));
    let ball = Mcp::id(&m.tool("add_shape", json!({"preset": "ellipse", "x": 120, "y": 260, "w": 80, "h": 80, "fill": "#F26B1D"})));
    m.cmd("animation.set", json!({"effect": "lines", "class": "path", "path": [[0, 0], [300, -80], [600, 0]], "ids": [ball]}));
    m.cmd("animation.reversePath", json!({"ids": [ball]}));
    m.cmd("transition.set", json!({"kind": "morph"}));
    m.tool(
        "add_text_box",
        json!({"x": 80, "y": 400, "w": 800, "text": "In the show, the ball starts at the far end of its path and travels back", "size": 16, "align": "center", "color": "#1E2B3C"}),
    );

    m.tool(
        "add_slide",
        json!({"layout": "titleAndContent", "title": "Numbered from five",
            "body": "The deck numbers its slides from 5\nThis text kerns letter pairs from 12 pt\nTwo recent colours are kept with the file"}),
    );
    m.cmd("design.slideSize", json!({"firstNumber": 5}));
    m.cmd("format.recentColor", json!({"color": "#0B7285"}));
    m.cmd("format.recentColor", json!({"color": "#C2255C"}));
    let last = m.tool("inspect_slide", json!({}));
    let body = last["shapes"].as_array().unwrap().iter().find(|s| s["placeholder"] == "obj").map(Mcp::id).unwrap();
    m.tool("select", json!({"ids": [body]}));
    m.cmd("format.fontDialog", json!({"kern": 12}));
    m.cmd("design.headerFooter", json!({"slideNumber": true, "footer": true, "footerText": "Phase 1 sample"}));
    m.tool("go_to_slide", json!({"index": 0}));
    m
}

/// The deck as  writes it.
fn saved(m: &mut Mcp, tag: &str) -> Vec<u8> {
    let path = std::env::temp_dir().join(format!("deckcraft-phase1-{}-{tag}.deckcraft", std::process::id()));
    m.tool("save_presentation", json!({"path": path.to_string_lossy()}));
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).ok();
    bytes
}

fn sample_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples/phase-1.deckcraft")
}

fn load(bytes: &[u8]) -> Presentation {
    deckcraft_format::load(bytes).unwrap()
}

#[test]
fn the_sample_deck_is_what_the_mcp_script_builds() {
    let built = saved(&mut build(), "sample");
    if std::env::var_os("DECKCRAFT_BLESS_GOLDEN").is_some() {
        std::fs::create_dir_all(sample_path().parent().unwrap()).unwrap();
        std::fs::write(sample_path(), &built).unwrap();
    }
    let stored =
        std::fs::read(sample_path()).unwrap_or_else(|e| panic!("{}: {e}; run with DECKCRAFT_BLESS_GOLDEN=1 to write it", sample_path().display()));
    assert!(load(&built) == load(&stored), "samples/phase-1.deckcraft is not what the script builds; bless it if the script changed on purpose");
}

#[test]
fn the_script_builds_the_same_deck_every_time() {
    let (a, b) = (saved(&mut build(), "a"), saved(&mut build(), "b"));
    assert!(load(&a) == load(&b), "two runs of the script gave different decks");
}

/// What the fork's patch commands did is in the saved deck, not only in the session.
#[test]
fn the_fork_patch_commands_leave_their_mark_in_the_deck() {
    let doc = load(&saved(&mut build(), "patches"));
    assert_eq!(doc.slides.len(), 8);
    assert_eq!(doc.first_slide_number, 5, "design.slideSize firstNumber");
    let recent: Vec<String> = doc.recent_colors.iter().map(|c| c.hex()).collect();
    assert_eq!(recent, ["C2255C", "0B7285"], "format.recentColor, newest first");
    let crops: Vec<[f64; 4]> = doc.slides[3]
        .shapes
        .iter()
        .filter_map(|s| match &s.kind {
            ShapeKind::Picture { fill } => Some(fill.crop),
            _ => None,
        })
        .collect();
    assert_eq!(crops.len(), 2);
    assert_eq!(crops[0], [0.0; 4], "the first picture is whole");
    assert!(crops[1][0] > 0.0 && crops[1][0] == crops[1][2] && crops[1][1] == 0.0, "picture.cropAspect cut both sides: {:?}", crops[1]);
    let motion = &doc.slides[6].animations;
    assert_eq!(motion.len(), 1);
    assert_eq!(motion[0].class, AnimClass::Path);
    assert!(
        motion[0].path.as_deref().is_some_and(|p| p.starts_with("M 0.625 0")),
        "animation.reversePath starts at the far end: {:?}",
        motion[0].path
    );
    assert_eq!(doc.slides[6].transition.as_ref().map(|t| t.kind.as_str()), Some("morph"));
    let kerned = doc.slides[7]
        .shapes
        .iter()
        .filter_map(|s| s.text.as_ref())
        .flat_map(|t| &t.paragraphs)
        .flat_map(|p| &p.runs)
        .filter(|r| r.props.kern == Some(12.0))
        .count();
    assert_eq!(kerned, 3, "format.fontDialog kern on the three body paragraphs");
    assert!(doc.header_footer.slide_number && doc.header_footer.footer);
}
