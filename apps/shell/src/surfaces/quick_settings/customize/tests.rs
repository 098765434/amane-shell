use super::*;
use crate::theme::{Density, Mode};
use crate::tokens::{Key as TokenKey, with_key};

fn placed(screen: (f32, f32), card_x: f32) -> Area {
    with_key(TokenKey::new(Mode::Dark, Density::Comfortable), || {
        place(
            screen,
            Area {
                x: card_x,
                y: 0.0,
                width: 392.0,
                height: 800.0,
            },
        )
    })
}

#[test]
fn the_window_is_centred_and_clears_the_card() {
    // a wide screen: in the middle, the card far to the right
    let on_wide = placed((2560.0, 1440.0), 2150.0);
    assert_eq!((on_wide.width, on_wide.height), (WIDTH, HEIGHT));
    assert_eq!(on_wide.x, ((2560.0 - WIDTH) / 2.0_f32).round());
    assert_eq!(on_wide.y, ((1440.0 - HEIGHT) / 2.0_f32).round());
    // a narrower one: moved left so a gap stays between it and the card
    let on_narrow = placed((1920.0, 1200.0), 1510.0);
    assert!(on_narrow.x + on_narrow.width < 1510.0);
    assert!(on_narrow.x > 0.0);
}

#[test]
fn a_short_screen_shortens_the_window() {
    let short = placed((1920.0, 900.0), 1510.0);
    assert!(short.height < HEIGHT);
    assert!(short.height < 900.0);
    assert!(short.y >= 0.0);
}

#[test]
fn an_area_clips_to_another() {
    let pane = Area {
        x: 100.0,
        y: 100.0,
        width: 200.0,
        height: 200.0,
    };
    let inside = Area {
        x: 150.0,
        y: 150.0,
        width: 20.0,
        height: 20.0,
    };
    assert_eq!(inside.clipped(&pane), Some(inside));
    let across = Area {
        x: 250.0,
        y: 280.0,
        width: 100.0,
        height: 100.0,
    };
    assert_eq!(
        across.clipped(&pane),
        Some(Area {
            x: 250.0,
            y: 280.0,
            width: 50.0,
            height: 20.0
        })
    );
    let out = Area {
        x: 0.0,
        y: 0.0,
        width: 50.0,
        height: 50.0,
    };
    assert_eq!(out.clipped(&pane), None);
    assert!(pane.contains(Point { x: 100.0, y: 100.0 }));
    assert!(!pane.contains(Point { x: 300.0, y: 100.0 }));
}
