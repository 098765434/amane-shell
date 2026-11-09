use super::*;
use motion::as_shipped;

fn monitor() -> Monitor {
    Monitor {
        name: String::from("eDP-1"),
        width: 1600,
        height: 1000,
    }
}

#[test]
fn hidden_the_card_sits_wholly_under_the_bottom_edge() {
    let at = Measures::new(&monitor(), &as_shipped(), 3, 1);
    assert_eq!(at.sunk(1.0), 0.0);
    assert!(at.top + at.sunk(0.0) >= at.height);
}

#[test]
fn the_card_is_centred_and_grows_by_a_pitch_an_icon() {
    let at = Measures::new(&monitor(), &as_shipped(), 3, 1);
    let one = at.card_width(1.0);
    let two = at.card_width(2.0);
    assert!((two - one - at.pitch).abs() < 0.001);
    assert!((at.card_left(one) * 2.0 + one - 1600.0).abs() < 0.001);
}

#[test]
fn the_pointer_over_an_icon_is_in_its_slot() {
    let at = Measures::new(&monitor(), &as_shipped(), 3, 0);
    let width = at.card_width(3.0);
    let first = at.card_left(width) + at.pad;
    // an icon's centre is half way along its slot, its gap counted in
    assert!((along(at, width, first + at.side / 2.0) - 0.5).abs() < 0.01);
    let third = first + at.pitch * 2.0 + 1.0;
    assert_eq!(along(at, width, third).floor(), 2.0);
}

#[test]
fn too_many_icons_shrink_to_fit_the_monitor_and_the_surface_keeps_its_size() {
    let few = Measures::new(&monitor(), &as_shipped(), 5, 1);
    let many = Measures::new(&monitor(), &as_shipped(), 60, 4);
    assert_eq!(few.side, dock_settings().icon());
    assert!(many.side < dock_settings().icon());
    let slots = 60.0 + 4.0 * many.rule;
    assert!(many.card_width(slots) <= 1600.0);
    assert_eq!(few.height, many.height);
    assert_eq!(
        few.top + few.card,
        many.top + many.card,
        "the card's foot stays put"
    );
}

#[test]
fn a_rule_sits_between_sections_and_the_pointer_is_never_over_it() {
    let section = |workspace: i64, icons: Vec<Icon>| Section {
        workspace: Some(workspace),
        index: workspace as u32,
        name: None,
        active: false,
        icons,
    };
    let pin = |app: &str| Icon::Pin(pins::Pin::new(app, "eDP-1", None, 1));
    let sections = [section(1, vec![pin("a")]), section(2, vec![])];
    let entries = row(&sections, 0.5);
    let kinds: Vec<&str> = entries
        .iter()
        .map(|entry| match entry.kind {
            Kind::Icon(_) => "icon",
            Kind::Rule => "rule",
            Kind::Empty => "empty",
        })
        .collect();
    assert_eq!(kinds, ["icon", "rule", "empty"]);

    let laid = Laid {
        rests: [0.0, 1.0, 1.5]
            .iter()
            .map(|&slot| Rest {
                slot,
                presence: 1.0,
            })
            .collect(),
        shown: vec![1.0; 3],
        width: 0.0,
        entries,
    };
    assert_eq!(laid.under(0.5), Some(0));
    assert_eq!(laid.under(1.2), None);
    assert_eq!(laid.under(1.6), Some(2));
}

#[test]
fn without_magnification_or_a_name_there_is_no_room_above_for_them() {
    let plain = DockMotion {
        magnify: None,
        tooltip: None,
        ..as_shipped()
    };
    let at = Measures::new(&monitor(), &plain, 3, 0);
    assert!((at.headroom - sys::space::gap::s()).abs() < 0.001);
}
