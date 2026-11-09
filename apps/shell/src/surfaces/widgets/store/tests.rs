//! The store's tests: the file, refitting and committing.
use super::*;

const SCREEN: Canvas = Canvas { cols: 20, rows: 11 };

fn store() -> Store {
    let mut store = Store::empty();
    store.seeded = true;
    store
}

#[test]
fn the_file_round_trips() {
    let mut store = store();
    let layout = Layout::new(SCREEN);
    let cpu = store
        .add("eDP-1", &layout, WidgetKind::Cpu, WidgetSize::Medium, None)
        .unwrap();
    let shown = store.layout_for("eDP-1", SCREEN);
    let disk = store
        .add("eDP-1", &shown, WidgetKind::Disk, WidgetSize::Small, None)
        .unwrap();
    store.set_settings(
        disk.id,
        Settings::default().with("mount", Some(String::from("/home"))),
    );
    let text = serde_json::to_string(&store.to_kept()).unwrap();
    let parsed = Store::from_kept(serde_json::from_str(&text).unwrap());
    assert_eq!(parsed, store);
    let shown = parsed.clone().layout_for("eDP-1", SCREEN);
    assert_eq!(
        shown.rect_of(cpu),
        Some(Rect::new(Place::new(0, 0), Span::new(4, 2)))
    );
    assert_eq!(shown.rect_of(disk).map(|r| r.at), Some(Place::new(4, 0)));
    assert_eq!(parsed.settings(disk.id).get("mount"), Some("/home"));
    assert!(Kept::from_old(r#"{"version":0,"outputs":{}}"#).is_none());
    assert!(Kept::from_old("not json").is_none());
}

#[test]
fn a_file_is_repaired() {
    let text = r#"{"version":1,"next_id":2,"outputs":{"DP-1":[
        {"id":5,"kind":"cpu","cols":2,"rows":2,"col":0,"row":0},
        {"id":6,"kind":"memory","cols":2,"rows":2,"col":1,"row":1},
        {"id":7,"kind":"teleport","cols":2,"rows":2,"col":8,"row":0},
        {"id":5,"kind":"disk","cols":2,"rows":2,"col":8,"row":0},
        {"id":8,"kind":"disk","cols":8,"rows":4,"col":10,"row":0}
    ]}}"#;
    let mut store = Store::from_kept(Kept::from_old(text).expect("an old file"));
    let layout = store.layout_for("DP-1", SCREEN);
    assert_eq!(layout.entries.len(), 3);
    // the overlap moved, a size Disk lacks snapped to one it has
    let memory = WidgetKey {
        id: 6,
        kind: WidgetKind::Memory,
    };
    assert_eq!(layout.rect_of(memory).map(|r| r.at), Some(Place::new(2, 0)));
    let disk = WidgetKey {
        id: 8,
        kind: WidgetKind::Disk,
    };
    assert_eq!(layout.span_of(disk), Some(WidgetSize::Medium.span()));
    // ids never repeat
    assert_eq!(store.next_id, 9);
}

#[test]
fn a_smaller_screen_moves_widgets_without_losing_their_place() {
    let mut store = store();
    let big = Layout::new(SCREEN);
    let far = store
        .add(
            "eDP-1",
            &big,
            WidgetKind::Cpu,
            WidgetSize::Small,
            Some(Place::new(18, 9)),
        )
        .unwrap();
    // shown on a small screen it moves to where there's room
    let small = Canvas { cols: 4, rows: 2 };
    let shown = store.layout_for("eDP-1", small);
    assert_eq!(shown.rect_of(far).map(|r| r.at), Some(Place::new(0, 0)));
    // a widget added there is kept, and the far one keeps its cells
    let near = store
        .add("eDP-1", &shown, WidgetKind::Disk, WidgetSize::Small, None)
        .unwrap();
    assert_eq!(
        store.layout_for("eDP-1", small).rect_of(near).map(|r| r.at),
        Some(Place::new(2, 0))
    );
    let back = store.layout_for("eDP-1", SCREEN);
    assert_eq!(back.rect_of(far).map(|r| r.at), Some(Place::new(18, 9)));
    assert_eq!(back.rect_of(near).map(|r| r.at), Some(Place::new(2, 0)));
    // with no room at all a widget hides there, and still comes back
    let tiny = store.layout_for("eDP-1", Canvas { cols: 2, rows: 2 });
    assert_eq!(tiny.placed().count(), 1);
    store.commit("eDP-1", &tiny);
    assert_eq!(store.layout_for("eDP-1", SCREEN), back);
    // a widget moved by hand on the small screen is kept where it went
    let moved = store
        .layout_for("eDP-1", small)
        .moved(near, Place::new(0, 0))
        .unwrap();
    store.commit("eDP-1", &moved);
    let back = store.layout_for("eDP-1", SCREEN);
    assert_eq!(back.rect_of(near).map(|r| r.at), Some(Place::new(0, 0)));
}

#[test]
fn removing_a_widget_forgets_its_settings() {
    let mut store = store();
    let shown = Layout::new(SCREEN);
    let key = store
        .add("eDP-1", &shown, WidgetKind::Disk, WidgetSize::Small, None)
        .unwrap();
    assert!(store.settings.contains_key(&key.id));
    let mut shown = store.layout_for("eDP-1", SCREEN);
    shown.remove(key);
    store.commit("eDP-1", &shown);
    assert!(!store.settings.contains_key(&key.id));
    assert!(store.all().next().is_none());
}

#[test]
fn a_file_that_cant_be_read_is_kept_aside() {
    let newer = r#"{"version":9,"data":{"next_id":2,"outputs":{}}}"#;
    crate::services::state::tests::in_folder("widgets-newer", &[("widgets.json", newer)], |root| {
        let store = Store::load();
        assert!(!store.seeded, "the default shows in its place");
        assert_eq!(
            std::fs::read_to_string(root.join("widgets.json.bad")).unwrap(),
            newer
        );
    });
    // a file that reads is never copied
    let fine = r#"{"version":1,"data":{"next_id":1,"outputs":{}}}"#;
    crate::services::state::tests::in_folder("widgets-fine", &[("widgets.json", fine)], |root| {
        assert!(Store::load().seeded);
        assert!(!root.join("widgets.json.bad").exists());
    });
}

#[test]
fn one_bad_widget_is_dropped_not_the_file() {
    let text = r#"{"next_id":3,"outputs":{"eDP-1":[
        {"id":1,"kind":"cpu","cols":"two","rows":2,"col":0,"row":0},
        {"id":2,"kind":"memory","cols":2,"rows":2,"col":2,"row":0}
    ]}}"#;
    let store = Store::from_kept(serde_json::from_str(text).unwrap());
    let ids: Vec<u32> = store.all().map(|(_, e)| e.id.id).collect();
    assert_eq!(ids, [2]);
}

#[test]
fn a_widget_kept_past_the_last_cell_comes_back_on_the_canvas() {
    let text = r#"{"next_id":2,"outputs":{"eDP-1":[
        {"id":1,"kind":"system","cols":8,"rows":4,"col":250,"row":254}
    ]}}"#;
    let mut store = Store::from_kept(serde_json::from_str(text).unwrap());
    let shown = store.layout_for("eDP-1", SCREEN);
    let rect = shown.rect_of(WidgetKey {
        id: 1,
        kind: WidgetKind::System,
    });
    assert_eq!(rect.map(|r| r.at), Some(Place::new(0, 0)));
}

#[test]
fn a_widget_moved_onto_one_kept_off_screen_moves_that_one() {
    let mut store = store();
    let big = Layout::new(SCREEN);
    // far right on the big screen, so the small one shows it elsewhere
    let far = store
        .add(
            "eDP-1",
            &big,
            WidgetKind::Cpu,
            WidgetSize::Small,
            Some(Place::new(18, 0)),
        )
        .unwrap();
    let small = Canvas { cols: 8, rows: 4 };
    let shown = store.layout_for("eDP-1", small);
    let near = store
        .add("eDP-1", &shown, WidgetKind::Disk, WidgetSize::Small, None)
        .unwrap();
    // on a screen as wide as its old cells, the new one goes over them
    let wide = Canvas { cols: 20, rows: 4 };
    let moved = store
        .layout_for("eDP-1", wide)
        .placed_at(near, WidgetSize::Small.span(), Place::new(18, 0))
        .unwrap();
    store.commit("eDP-1", &moved);
    let back = store.layout_for("eDP-1", SCREEN);
    let (far_at, near_at) = (back.rect_of(far).unwrap(), back.rect_of(near).unwrap());
    assert_eq!(near_at.at, Place::new(18, 0));
    assert_ne!(far_at.at, near_at.at, "the two never share cells");
}

#[test]
fn a_live_old_file_migrates_and_round_trips() {
    let old = include_str!("../../../../tests/fixtures/state/widgets");
    crate::services::state::tests::in_folder("widgets", &[(OLD_FILE, old)], |_| {
        let store = Store::from_kept(state::load::<Kept>().expect("the old file"));
        assert_eq!(store.next_id, 8);
        assert_eq!(store.all().count(), 7);
        assert!(
            store
                .settings
                .values()
                .any(|s| s.get("interface").is_some())
        );
        let text = serde_json::to_string(&store.to_kept()).unwrap();
        assert_eq!(
            Store::from_kept(serde_json::from_str(&text).unwrap()),
            store
        );
    });
}

#[test]
fn settings_keep_only_what_the_kind_has() {
    let text = r#"{"next_id":3,"outputs":{"eDP-1":[
        {"id":1,"kind":"clock","cols":4,"rows":2,"col":0,"row":0,
         "settings":{"seconds":"on","hours":"13","mount":"/home","line":"bold"}},
        {"id":2,"kind":"disk","cols":2,"rows":2,"col":4,"row":0,"settings":{"mount":"/home"}}
    ]}}"#;
    let store = Store::from_kept(serde_json::from_str(text).unwrap());
    let clock: Vec<(&str, &str)> = store.settings[&1].iter().collect();
    assert_eq!(clock, [("line", "bold"), ("seconds", "on")]);
    assert_eq!(store.settings(2).get("mount"), Some("/home"));
}
