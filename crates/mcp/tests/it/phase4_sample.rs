//! The Phase 4 sample deck, `samples/phase-4.deckcraft`, built through MCP tool calls and nothing
//! else, as an agent would: every step is a `tools/call` request to the server.
//!
//! It exercises the design system: the theme kit (Light and Dark), the layout library, a brand
//! collection of variables bound to shapes with a slide in another mode, paint, effect and grid
//! styles, a component with properties, variants, overrides and a swap, and Tidy up.
//!
//! The file in `samples/` is what this script writes. `DECKCRAFT_BLESS_GOLDEN=1` rewrites it;
//! only do that when the script changed on purpose, and bless its golden pictures too
//! (`crates/engine/tests/it/golden.rs`).

use std::path::PathBuf;

use deckcraft_mcp::{Headless, Server};
use deckcraft_model::{ColorBase, Fill, Presentation, Shape, ShapeKind};
use serde_json::{Value, json};

struct Mcp {
    server: Server,
    requests: u64,
}

impl Mcp {
    fn new() -> Self {
        Mcp { server: Server::new(Box::new(Headless::new())), requests: 0 }
    }

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

    fn shape(&mut self, preset: &str, x: u32, y: u32, w: u32, h: u32, fill: &str) -> u64 {
        let id = Self::id(&self.tool("add_shape", json!({"preset": preset, "x": x, "y": y, "w": w, "h": h, "fill": fill})));
        self.cmd("shape.line", json!({"ids": [id], "none": true}));
        id
    }

    fn text(&mut self, x: u32, y: u32, w: u32, text: &str, size: u32) -> u64 {
        Self::id(&self.tool("add_text_box", json!({"x": x, "y": y, "w": w, "text": text, "size": size})))
    }

    /// The id of the shape named `name` on the current slide (any depth).
    fn named(&mut self, name: &str) -> u64 {
        let s = self.tool("inspect_slide", json!({}));
        fn find(v: &Value, name: &str) -> Option<u64> {
            for sh in v.as_array()? {
                if sh["name"] == name {
                    return sh["id"].as_u64();
                }
                if let Some(id) = find(&sh["children"], name) {
                    return Some(id);
                }
            }
            None
        }
        find(&s["shapes"], name).unwrap_or_else(|| panic!("no shape named {name}"))
    }
}

/// The script. Coordinates are points on a 960 × 540 slide.
fn build() -> Mcp {
    let mut m = Mcp::new();
    m.tool("new_presentation", json!({}));
    // The theme as variables, Light and Dark.
    m.cmd("design.kit", json!({"theme": "Harbor"}));
    let first = m.tool("inspect_slide", json!({}));
    let (title, subtitle) = (Mcp::id(&first["shapes"][0]), Mcp::id(&first["shapes"][1]));
    m.tool("set_text", json!({"id": title, "text": "The design system"}));
    m.tool("set_text", json!({"id": subtitle, "text": "Phase 4 sample deck, built through MCP only"}));

    // 2–4. Library layouts: stats (Stat components), three features, pricing (a variant set).
    for layout in ["stats", "threeUp", "pricing"] {
        m.cmd("slide.fromLibrary", json!({"layout": layout}));
    }

    // 5. A brand collection bound to shapes, paint and effect styles, a grid style; this slide is
    // shown in the Theme kit's Dark mode while the deck stays Light.
    m.tool("add_slide", json!({"layout": "titleOnly", "title": "Variables and styles"}));
    m.cmd("variable.createCollection", json!({"name": "Brand", "modes": ["Calm", "Bold"]}));
    let brand = Mcp::id(&m.cmd(
        "variable.create",
        json!({"collection": "Brand", "name": "brand/primary", "type": "color", "values": {"Calm": "#1B7FA6", "Bold": "#E4572E"}}),
    ));
    m.cmd("variable.create", json!({"collection": "Brand", "name": "brand/radius", "type": "number", "values": {"Calm": 12, "Bold": 36}}));
    let a = m.shape("rect", 60, 150, 260, 160, "#CCCCCC");
    m.cmd("variable.bind", json!({"ids": [a], "field": "fill", "variable": brand}));
    let b = m.shape("rect", 350, 150, 260, 160, "#CCCCCC");
    let paint = Mcp::id(&m.cmd("designStyle.create", json!({"type": "paint", "name": "Brand fill", "color": {"variable": brand}})));
    m.cmd("designStyle.apply", json!({"ids": [b], "style": paint}));
    m.cmd("format.blur", json!({"ids": [b], "layer": 0}));
    let c = m.shape("ellipse", 660, 150, 160, 160, "#17A398");
    let f = Mcp::id(&m.cmd("frame.create", json!({"ids": [c], "padding": 20, "fill": "#F2F2F2"})));
    m.cmd("variable.bind", json!({"ids": [f], "field": "cornerRadius", "variable": "brand/radius"}));
    let grid = Mcp::id(&m.cmd("designStyle.create", json!({"type": "grid", "name": "12 columns", "columns": 12, "gutter": 16, "margin": 48})));
    let here = m.cmd("variable.list", json!({}));
    let _ = here;
    let s5 = m.tool("inspect_slide", json!({}))["index"].as_u64().unwrap_or(4);
    m.cmd("designStyle.apply", json!({"style": grid, "slides": [s5]}));
    m.cmd("variable.mode", json!({"collection": "Brand", "mode": "Bold", "slides": [s5]}));
    m.cmd("variable.mode", json!({"collection": "Theme", "mode": "Dark", "slides": [s5]}));

    // 6. A component with a text property, two instances, an override, and a swap.
    m.tool("add_slide", json!({"layout": "titleOnly", "title": "Components"}));
    let card = m.shape("roundRect", 60, 150, 240, 90, "#1B7FA6");
    m.tool("set_text", json!({"id": card, "text": "Ship it"}));
    m.cmd("shape.rename", json!({"id": card, "name": "Label"}));
    let comp = m.cmd("component.create", json!({"ids": [card], "name": "Badge", "set": "Badge", "variant": {"Tone": "Primary"}}));
    let comp_id = Mcp::id(&comp);
    m.cmd("component.addProperty", json!({"component": comp_id, "name": "Label", "type": "text", "layers": ["Label"]}));
    let alt = m.shape("roundRect", 60, 330, 240, 90, "#B4441B");
    m.tool("set_text", json!({"id": alt, "text": "Ship it"}));
    m.cmd("shape.rename", json!({"id": alt, "name": "Label"}));
    let alt_comp = Mcp::id(&m.cmd("component.create", json!({"ids": [alt], "name": "Badge warm", "set": "Badge", "variant": {"Tone": "Warm"}})));
    m.cmd("component.addProperty", json!({"component": alt_comp, "name": "Label", "type": "text", "layers": ["Label"]}));
    let i1 = Mcp::id(&m.cmd("component.insert", json!({"component": comp_id, "at": [400, 150], "props": {"Label": "Review"}})));
    let i2 = Mcp::id(&m.cmd("component.insert", json!({"component": comp_id, "at": [680, 150], "props": {"Label": "Launch"}})));
    m.cmd("instance.override", json!({"ids": [i2], "layer": "Label", "fill": "#2EA36F"}));
    m.cmd("instance.set", json!({"ids": [i1], "variant": {"Tone": "Warm"}}));
    // Editing the main updates both instances (the text keeps its property value).
    let main_label = m.named("Label");
    m.cmd("text.set", json!({"id": main_label, "text": "Ship it!"}));

    // 7. Tidy up a messy selection.
    m.tool("add_slide", json!({"layout": "titleOnly", "title": "Tidy up"}));
    let colors = ["#1B7FA6", "#36B3A8", "#F2C14E", "#F78154", "#5D576B", "#8AA29E"];
    let spots = [(430, 140), (90, 160), (260, 150), (100, 330), (450, 320), (270, 345)];
    let mut ids = vec![];
    for (i, (x, y)) in spots.iter().enumerate() {
        ids.push(m.shape("roundRect", *x, *y, 140, 120, colors[i]));
    }
    m.cmd("arrange.tidy", json!({"ids": ids, "gap": 24}));
    m.text(640, 200, 260, "Six shapes dropped anywhere, lined up in reading order with even 24 pt gaps.", 18);

    // 8. A library quote, then switch the whole deck to the kit's Dark mode and back.
    m.cmd("slide.fromLibrary", json!({"layout": "quote"}));
    m.cmd("variable.mode", json!({"collection": "Theme", "mode": "Dark"}));
    m.cmd("variable.mode", json!({"collection": "Theme", "mode": "Light"}));
    m
}

fn saved(m: &mut Mcp, tag: &str) -> Vec<u8> {
    let path = std::env::temp_dir().join(format!("deckcraft-phase4-{}-{tag}.deckcraft", std::process::id()));
    m.tool("save_presentation", json!({"path": path.to_string_lossy()}));
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).ok();
    bytes
}

fn sample_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples/phase-4.deckcraft")
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
    assert!(load(&built) == load(&stored), "samples/phase-4.deckcraft is not what the script builds; bless it if the script changed on purpose");
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

/// Every Phase 4 feature the script used is in the saved deck, not only in the session.
#[test]
fn each_phase_4_feature_leaves_its_mark_in_the_deck() {
    let doc = load(&saved(&mut build(), "features"));
    assert_eq!(doc.slides.len(), 8);
    let ds = doc.design.as_deref().unwrap();
    assert_eq!(ds.collections.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["Theme", "Brand"]);
    assert!(ds.variables.iter().filter(|v| v.theme.is_some()).count() >= 14, "the kit drives the theme");
    assert!(ds.components.iter().any(|c| c.set == "Stat") && ds.components.iter().any(|c| c.set == "Price tier"));
    let s = |i: usize| all(&doc.slides[i].shapes);
    assert!(s(1).iter().filter(|x| x.instance.is_some()).count() >= 3, "stat instances");
    assert!(s(1).iter().any(|x| !x.bindings.is_empty()), "number bindings");
    // Slide 5: variables, styles, modes.
    assert_eq!(doc.slides[4].modes.len(), 2);
    assert!(doc.slides[4].grid_style.is_some());
    assert!(s(4).iter().any(|x| matches!(&x.fill, Some(Fill::Solid { color }) if matches!(color.base, ColorBase::Var { .. }))));
    assert!(s(4).iter().any(|x| x.styles.and_then(|r| r.fill).is_some()));
    assert!(s(4).iter().any(|x| x.frame.is_some_and(|f| f.radius == 36.0)), "Bold mode radius on this slide");
    // Slide 6: a main, instances, an override and a variant switch.
    let six = s(5);
    assert_eq!(six.iter().filter(|x| x.main_of.is_some()).count(), 2);
    let inst: Vec<_> = six.iter().filter(|x| x.instance.is_some()).collect();
    assert_eq!(inst.len(), 2);
    assert!(inst.iter().any(|x| x.instance.as_ref().unwrap().overrides.iter().any(|o| o.fill.is_some())));
    let texts: Vec<String> = inst.iter().flat_map(|x| x.children().iter().filter_map(|c| c.text.as_ref().map(|t| t.text()))).collect();
    assert!(texts.contains(&"Review".to_string()) && texts.contains(&"Launch".to_string()), "{texts:?}");
    // Slide 7: one auto layout grid frame of six members.
    let tidy = s(6).into_iter().find(|x| x.name == "Tidy").unwrap();
    assert!(tidy.auto_layout.is_some());
    let ShapeKind::Group { children, .. } = &tidy.kind else { panic!() };
    assert_eq!(children.len(), 6);
}
