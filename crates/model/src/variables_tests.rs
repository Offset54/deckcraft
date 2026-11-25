use super::*;
use crate::{Presentation, Slide, resolve::Ctx};

fn ds() -> DesignSystem {
    DesignSystem {
        collections: vec![Collection {
            id: 1,
            name: "Theme".into(),
            modes: vec![Mode { id: 2, name: "Light".into() }, Mode { id: 3, name: "Dark".into() }],
        }],
        variables: vec![
            Variable {
                id: 10,
                name: "color/background".into(),
                collection: 1,
                kind: VarKind::Color,
                values: vec![(2, VarValue::Color(Rgba::WHITE)), (3, VarValue::Color(Rgba::BLACK))],
                theme: Some(ThemeSlot::Color(SchemeSlot::Lt1)),
                ..Default::default()
            },
            Variable {
                id: 11,
                name: "color/surface".into(),
                collection: 1,
                kind: VarKind::Color,
                values: vec![(2, VarValue::Alias(10)), (3, VarValue::Color(Rgba::rgb(30, 30, 30)))],
                ..Default::default()
            },
            Variable {
                id: 12,
                name: "radius".into(),
                collection: 1,
                kind: VarKind::Number,
                values: vec![(2, VarValue::Number(8.0))],
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

#[test]
fn values_follow_the_mode_and_aliases() {
    let mut d = ds();
    assert_eq!(d.color(10, &[]), Some(Rgba::WHITE));
    assert_eq!(d.color(11, &[]), Some(Rgba::WHITE), "light surface aliases background");
    assert_eq!(d.color(11, &[(1, 3)]), Some(Rgba::rgb(30, 30, 30)));
    // A mode without its own value takes the first mode's.
    assert_eq!(d.resolve(12, &[(1, 3)]).and_then(VarValue::as_number), Some(8.0));
    d.modes = vec![(1, 3)];
    assert_eq!(d.color(10, &[]), Some(Rgba::BLACK));
    // An override naming a mode the collection doesn't have is ignored.
    assert_eq!(d.color(10, &[(1, 99)]), Some(Rgba::BLACK));
}

#[test]
fn alias_cycles_resolve_to_nothing_and_are_detected() {
    let mut d = ds();
    assert!(d.alias_cycles(10, 11), "10 → 11 → 10");
    assert!(!d.alias_cycles(11, 10));
    if let Some(v) = d.variable_mut(10) {
        v.values = vec![(2, VarValue::Alias(11))];
    }
    assert_eq!(d.resolve(10, &[]), None);
    assert_eq!(d.resolve(11, &[]), None);
}

#[test]
fn theme_follows_the_variables_both_ways() {
    let mut d = ds();
    let mut t = crate::Theme::default();
    assert!(d.apply_to_theme(&mut t) || t.colors.get(SchemeSlot::Lt1) == Rgba::WHITE);
    d.modes = vec![(1, 3)];
    assert!(d.apply_to_theme(&mut t));
    assert_eq!(t.colors.get(SchemeSlot::Lt1), Rgba::BLACK);
    assert!(!d.apply_to_theme(&mut t), "nothing left to change");
    t.colors.set(SchemeSlot::Lt1, Rgba::rgb(1, 2, 3));
    assert!(d.take_from_theme(&t));
    assert_eq!(d.color(10, &[]), Some(Rgba::rgb(1, 2, 3)), "the active (dark) mode takes the theme's colour");
    assert_eq!(d.color(10, &[(1, 2)]), Some(Rgba::WHITE), "light mode untouched");
}

#[test]
fn var_colours_resolve_when_drawn_and_slides_can_override_the_mode() {
    let mut p = Presentation { design: Some(std::sync::Arc::new(ds())), ..Default::default() };
    let slide = Slide { layout: p.masters[0].layouts[0].id, ..Default::default() };
    let c = var_color(11, Rgba::rgb(9, 9, 9));
    let ctx = Ctx::for_slide(&p, &slide).unwrap();
    assert_eq!(ctx.color(&c, None), Rgba::WHITE);
    let dark = Slide { modes: vec![(1, 3)], ..slide.clone() };
    let ctx = Ctx::for_slide(&p, &dark).unwrap();
    assert_eq!(ctx.color(&c, None), Rgba::rgb(30, 30, 30));
    // The theme slot the variable maps to follows the slide's mode too.
    assert_eq!(ctx.color(&ColorRef::scheme(SchemeSlot::Lt1), None), Rgba::BLACK);
    assert_eq!(ctx.color(&ColorRef::scheme(SchemeSlot::Bg1), None), Rgba::BLACK, "through the colour map");
    // Without the deck's variables the last resolved value is used.
    p.design = None;
    let ctx = Ctx::for_slide(&p, &slide).unwrap();
    assert_eq!(ctx.color(&c, None), Rgba::rgb(9, 9, 9));
    assert_eq!(c.resolve(&p.masters[0].theme.colors, None), Rgba::rgb(9, 9, 9));
}

#[test]
fn bindings_write_resolved_values() {
    let mut d = ds();
    d.variables.push(Variable {
        id: 13,
        name: "title".into(),
        collection: 1,
        kind: VarKind::String,
        values: vec![(2, VarValue::String("Hi\nthere".into()))],
        ..Default::default()
    });
    d.variables.push(Variable {
        id: 14,
        name: "show".into(),
        collection: 1,
        kind: VarKind::Boolean,
        values: vec![(2, VarValue::Boolean(false))],
        ..Default::default()
    });
    let mut sh = Shape {
        frame: Some(crate::canvas::Frame::default()),
        text: Some(crate::TextBody::from_text("old")),
        bindings: vec![
            Binding { field: BindField::CornerRadius, variable: 12 },
            Binding { field: BindField::Text, variable: 13 },
            Binding { field: BindField::Visible, variable: 14 },
            Binding { field: BindField::FontSize, variable: 12 },
            Binding { field: BindField::Gap, variable: 999 },
        ],
        ..Default::default()
    };
    let mut list = vec![sh.clone()];
    assert!(apply_bindings(&d, &mut list, &[]));
    sh = list.remove(0);
    assert_eq!(sh.frame.map(|f| f.radius), Some(8.0));
    assert_eq!(sh.text.as_ref().map(crate::TextBody::text).as_deref(), Some("Hi\nthere"));
    assert!(sh.hidden);
    assert!(sh.text.as_ref().unwrap().paragraphs.iter().flat_map(|p| &p.runs).all(|r| r.props.size == Some(8.0)));
    let mut list = vec![sh];
    assert!(!apply_bindings(&d, &mut list, &[]), "settled");
}

#[test]
fn hostile_numbers_and_kinds_are_cleaned() {
    let mut d = ds();
    d.variables.push(Variable {
        id: 20,
        name: "x".into(),
        collection: 1,
        kind: VarKind::Number,
        values: vec![(2, VarValue::Number(f64::NAN)), (3, VarValue::Color(Rgba::WHITE))],
        ..Default::default()
    });
    d.variables.push(Variable { id: 21, name: "orphan".into(), collection: 77, ..Default::default() });
    d.variables.push(Variable {
        id: 22,
        name: "bad alias".into(),
        collection: 1,
        kind: VarKind::Number,
        values: vec![(2, VarValue::Alias(10))],
        ..Default::default()
    });
    d.variables.push(Variable {
        id: 23,
        name: "font".into(),
        collection: 1,
        kind: VarKind::Number,
        theme: Some(ThemeSlot::MajorFont),
        ..Default::default()
    });
    d.collections.push(Collection { id: 5, name: "empty".into(), modes: vec![] });
    d.sanitize();
    assert_eq!(d.variable(20).map(|v| v.values.clone()), Some(vec![(2, VarValue::Number(0.0))]));
    assert!(d.variable(21).is_none());
    assert_eq!(d.variable(22).map(|v| v.values.len()), Some(0));
    assert_eq!(d.variable(23).and_then(|v| v.theme), None);
    assert!(d.collection(5).is_none());
    let mut sh = Shape { xfrm: Some(crate::Xfrm::new(0.0, 0.0, 1.0, 1.0)), ..Default::default() };
    assert!(apply_one(&mut sh, BindField::Width, &VarValue::Number(1e300)));
    assert_eq!(sh.xfrm.map(|x| x.w), Some(1.0e5));
}

#[test]
fn colour_paths_cover_fill_line_and_runs() {
    let mut sh = Shape {
        fill: Some(Fill::solid(var_color(10, Rgba::WHITE))),
        line: Some(crate::Line::solid(var_color(11, Rgba::WHITE), 1.0)),
        text: Some(crate::TextBody::from_text("a")),
        ..Default::default()
    };
    if let Some(r) = sh.text.as_mut().and_then(|t| t.paragraphs.get_mut(0)).and_then(|p| p.runs.get_mut(0)) {
        r.props.fill = Some(Fill::solid(var_color(12, Rgba::BLACK)));
    }
    assert_eq!(color_bindings(&sh), vec![("fill".into(), 10), ("line".into(), 11), ("text/0/0".into(), 12)]);
    let mut d = ds();
    d.modes = vec![(1, 3)];
    let mut list = vec![sh];
    assert!(refresh_last(&d, &mut list, &[]));
    assert!(matches!(list[0].fill, Some(Fill::Solid { color: ColorRef { base: ColorBase::Var { id: 10, last }, .. } }) if last == Rgba::BLACK));
}
