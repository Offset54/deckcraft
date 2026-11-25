#![allow(clippy::unwrap_used)]

use super::*;
use crate::{TextBody, Xfrm};

struct Ids(u32);
impl IdSource for Ids {
    fn next_id(&mut self) -> Option<u32> {
        self.0 += 1;
        Some(self.0)
    }
}

fn layer(id: u32, name: &str, text: &str) -> Shape {
    Shape {
        id: ShapeId(id),
        name: name.into(),
        xfrm: Some(Xfrm::new(10.0, 10.0, 80.0, 20.0)),
        text: Some(TextBody::from_text(text)),
        ..Default::default()
    }
}

fn card(id: u32, set: &str, size: &str) -> Component {
    let root = Shape {
        id: ShapeId(id),
        name: "Card".into(),
        xfrm: Some(Xfrm::new(0.0, 0.0, 100.0, 60.0)),
        kind: ShapeKind::Group {
            children: vec![layer(id + 1, "Value", "42"), layer(id + 2, "Label", "users")],
            child: Xfrm::new(0.0, 0.0, 100.0, 60.0),
        },
        frame: Some(crate::canvas::Frame::default()),
        ..Default::default()
    };
    Component {
        id,
        name: format!("Card {size}"),
        set: set.into(),
        variant: vec![("Size".into(), size.into())],
        props: vec![PropDef { name: "Value".into(), default: PropValue::Text("42".into()), targets: vec![id + 1] }],
        root,
        description: String::new(),
    }
}

fn frame() -> Shape {
    Shape { id: ShapeId(500), name: "My card".into(), xfrm: Some(Xfrm::new(200.0, 100.0, 100.0, 60.0)), ..Default::default() }
}

fn texts(s: &Shape) -> Vec<String> {
    s.children().iter().map(|c| c.text.as_ref().map(TextBody::text).unwrap_or_default()).collect()
}

#[test]
fn an_instance_is_the_content_with_props_and_overrides_in_its_own_ids() {
    let c = card(100, "Stat", "Small");
    let lib = vec![c.clone()];
    let mut ids = Ids(1000);
    let inst = Instance { component: 100, props: vec![("value".into(), PropValue::Text("7".into()))], ..Default::default() };
    let built = materialize(&lib, &c, &frame(), &inst, &mut ids).unwrap();
    assert_eq!(built.id, ShapeId(500));
    assert_eq!(built.xfrm, frame().xfrm);
    assert_eq!(texts(&built), vec!["7", "users"]);
    let child_ids: Vec<u32> = built.children().iter().map(|c| c.id.0).collect();
    assert_eq!(child_ids, vec![1001, 1002], "fresh ids");
    let inst2 = built.instance.as_deref().cloned().unwrap();
    assert_eq!(inst2.map, vec![(101, 1001), (102, 1002)]);
    // Built again: the same ids, no new ones.
    let again = materialize(&lib, &c, &built, &inst2, &mut ids).unwrap();
    assert_eq!(again.children().iter().map(|c| c.id.0).collect::<Vec<_>>(), child_ids);
    assert_eq!(ids.0, 1002);
}

#[test]
fn edits_become_overrides_and_survive_a_component_change() {
    let mut c = card(100, "", "Small");
    let mut lib = vec![c.clone()];
    let mut ids = Ids(1000);
    let built = materialize(&lib, &c, &frame(), &Instance { component: 100, ..Default::default() }, &mut ids).unwrap();
    let mut edited = built.clone();
    if let ShapeKind::Group { children, .. } = &mut edited.kind {
        children[1].text = Some(TextBody::from_text("customers"));
        children[0].hidden = true;
    }
    let mut inst = edited.instance.as_deref().cloned().unwrap();
    assert!(capture_overrides(&lib, &c, &edited, &mut inst));
    assert_eq!(inst.overrides.len(), 2);
    // The component's label changes; the instance keeps its override, its value follows.
    if let ShapeKind::Group { children, .. } = &mut c.root.kind {
        children[1].text = Some(TextBody::from_text("people"));
        children[0].fill = Some(Fill::None);
    }
    lib = vec![c.clone()];
    let again = materialize(&lib, &c, &edited, &inst, &mut ids).unwrap();
    assert_eq!(texts(&again), vec!["42", "customers"]);
    assert!(again.children()[0].hidden);
    assert_eq!(again.children()[0].fill, Some(Fill::None));
    assert!(!capture_overrides(&lib, &c, &again, &mut inst), "nothing new");
}

#[test]
fn variants_are_found_and_overrides_carry_by_name() {
    let small = card(100, "Stat", "Small");
    let large = card(200, "Stat", "Large");
    let lib = vec![small.clone(), large.clone()];
    let v = find_variant(&lib, "Stat", &small.variant, &[("size".into(), "large".into())]).unwrap();
    assert_eq!(v.id, 200);
    assert!(find_variant(&lib, "Stat", &small.variant, &[("Size".into(), "Huge".into())]).is_none());
    let mut inst = Instance {
        component: 100,
        props: vec![("Value".into(), PropValue::Text("9".into())), ("Gone".into(), PropValue::Boolean(true))],
        overrides: vec![Override { layer: 102, text: Some(TextBody::from_text("x")), ..Default::default() }],
        map: vec![(101, 1001), (102, 1002)],
    };
    carry_over(&small, &large, &mut inst);
    assert_eq!(inst.component, 200);
    assert_eq!(inst.overrides[0].layer, 202, "the Label layer of the large variant");
    assert_eq!(inst.map, vec![(201, 1001), (202, 1002)]);
    assert_eq!(inst.props.len(), 1, "a property the variant doesn't have goes");
}

#[test]
fn nested_instances_expand_and_swap() {
    let badge = Component {
        id: 300,
        name: "Badge".into(),
        root: Shape {
            id: ShapeId(300),
            kind: ShapeKind::Group { children: vec![layer(301, "Text", "NEW")], child: Xfrm::new(0.0, 0.0, 40.0, 20.0) },
            xfrm: Some(Xfrm::new(0.0, 0.0, 40.0, 20.0)),
            ..Default::default()
        },
        ..Default::default()
    };
    let hot = Component {
        id: 400,
        name: "Hot".into(),
        root: Shape {
            id: ShapeId(400),
            kind: ShapeKind::Group { children: vec![layer(401, "Text", "HOT")], child: Xfrm::default() },
            ..Default::default()
        },
        ..Default::default()
    };
    let mut c = card(100, "", "Small");
    if let ShapeKind::Group { children, .. } = &mut c.root.kind {
        children.push(Shape {
            id: ShapeId(103),
            name: "Badge".into(),
            instance: Some(Box::new(Instance { component: 300, ..Default::default() })),
            ..Default::default()
        });
    }
    c.props.push(PropDef { name: "Badge".into(), default: PropValue::Swap(300), targets: vec![103] });
    let lib = vec![c.clone(), badge, hot];
    let mut ids = Ids(1000);
    let built = materialize(&lib, &c, &frame(), &Instance { component: 100, ..Default::default() }, &mut ids).unwrap();
    let nested = &built.children()[2];
    assert_eq!(texts(nested), vec!["NEW"]);
    let swapped = materialize(
        &lib,
        &c,
        &frame(),
        &Instance { component: 100, props: vec![("Badge".into(), PropValue::Swap(400))], ..Default::default() },
        &mut ids,
    )
    .unwrap();
    assert_eq!(texts(&swapped.children()[2]), vec!["HOT"]);
    // Ids are unique across the nested instance.
    let mut all = vec![];
    crate::walk(swapped.children(), &mut |s, _| all.push(s.id));
    let n = all.len();
    all.sort();
    all.dedup();
    assert_eq!(all.len(), n);
}

#[test]
fn a_component_that_contains_itself_stops_expanding() {
    let mut c = card(100, "", "Small");
    if let ShapeKind::Group { children, .. } = &mut c.root.kind {
        children.push(Shape { id: ShapeId(103), instance: Some(Box::new(Instance { component: 100, ..Default::default() })), ..Default::default() });
    }
    let lib = vec![c.clone()];
    let mut ids = Ids(1000);
    let built = materialize(&lib, &c, &frame(), &Instance { component: 100, ..Default::default() }, &mut ids);
    assert!(built.is_some(), "bounded by MAX_NESTING, no stack overflow");
}

#[test]
fn hostile_libraries_are_cleaned() {
    let mut v = vec![card(1, "", "a"), card(1, "", "b"), Component { id: 2, root: Shape::default(), ..Default::default() }];
    v[0].root.main_of = Some(1);
    v[0].props = (0..500).map(|i| PropDef { name: format!("p{i}"), default: PropValue::Boolean(true), targets: vec![] }).collect();
    sanitize(&mut v);
    assert_eq!(v.len(), 1, "duplicate id and a root that isn't a frame go");
    assert_eq!(v[0].props.len(), MAX_PROPS);
    assert_eq!(v[0].root.main_of, None);
}
