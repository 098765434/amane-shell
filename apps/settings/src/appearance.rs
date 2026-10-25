//! How the shell looks: light or dark, the accent, how dense and how big
//! the text is, how see-through the glass is, and whether motion is
//! reduced. Each control changes `appearance.json`, and the shell
//! redraws in the new look while this window shows it too.

use amane::{Column, Widget};
use amane_kit::buttons::Radio;
use amane_kit::dropdown::Dropdown;
use amane_kit::layout::boxed;
use amane_kit::settings::{appearance, change_appearance};
use amane_kit::theme::{Density, Glass, Mode};
use amane_kit::tokens::sys;

use crate::setting;

/// The accents to pick from, after macOS's, and none for the system's.
const ACCENTS: [(&str, Option<&str>); 9] = [
    ("System", None),
    ("Blue", Some("#0a84ff")),
    ("Purple", Some("#bf5af2")),
    ("Pink", Some("#ff375f")),
    ("Red", Some("#ff453a")),
    ("Orange", Some("#ff9f0a")),
    ("Yellow", Some("#ffd60a")),
    ("Green", Some("#30d158")),
    ("Graphite", Some("#8e8e93")),
];

/// The text sizes to pick from.
const TEXT_SIZES: [(&str, f32); 5] = [
    ("Smaller", 0.9),
    ("Default", 1.0),
    ("Larger", 1.1),
    ("Large", 1.25),
    ("Largest", 1.5),
];

pub fn page() -> Column {
    let (mode, accent, density, text_scale, glass, reduce_motion) = {
        let now = appearance();

        (
            now.mode,
            now.accent.clone(),
            now.density,
            now.text_scale,
            now.glass,
            now.reduce_motion,
        )
    };

    let modes = [
        ("Follow the system", None),
        ("Light", Some(Mode::Light)),
        ("Dark", Some(Mode::Dark)),
    ];

    let mode_radios: Vec<Box<dyn Widget>> = modes
        .into_iter()
        .enumerate()
        .map(|(index, (label, choice))| {
            boxed(
                Radio::new(format!("settings.mode.{index}"), mode == choice)
                    .label(label)
                    .on_select(move || change_appearance(|now| now.mode = choice))
                    .build(),
            )
        })
        .collect();

    let densities = [
        ("Comfortable", Density::Comfortable),
        ("Compact", Density::Compact),
    ];

    let density_radios: Vec<Box<dyn Widget>> = densities
        .into_iter()
        .enumerate()
        .map(|(index, (label, choice))| {
            boxed(
                Radio::new(format!("settings.density.{index}"), density == choice)
                    .label(label)
                    .on_select(move || change_appearance(|now| now.density = choice))
                    .build(),
            )
        })
        .collect();

    let glasses = [
        ("Clear", Glass::Clear),
        ("Tinted", Glass::Tinted),
        ("Solid", Glass::Solid),
    ];

    let glass_radios: Vec<Box<dyn Widget>> = glasses
        .into_iter()
        .enumerate()
        .map(|(index, (label, choice))| {
            boxed(
                Radio::new(format!("settings.glass.{index}"), glass == choice)
                    .label(label)
                    .on_select(move || change_appearance(|now| now.glass = choice))
                    .build(),
            )
        })
        .collect();

    let motions = [
        ("Follow the system", None),
        ("Reduce motion", Some(true)),
        ("Full motion", Some(false)),
    ];

    let motion_radios: Vec<Box<dyn Widget>> = motions
        .into_iter()
        .enumerate()
        .map(|(index, (label, choice))| {
            boxed(
                Radio::new(format!("settings.motion.{index}"), reduce_motion == choice)
                    .label(label)
                    .on_select(move || change_appearance(|now| now.reduce_motion = choice))
                    .build(),
            )
        })
        .collect();

    let chosen_accent = ACCENTS
        .iter()
        .position(|(_, hex)| *hex == accent.as_deref())
        .unwrap_or(0);

    let accents = Dropdown::new(
        "settings.accent",
        ACCENTS
            .iter()
            .map(|(name, _)| String::from(*name))
            .collect(),
        chosen_accent,
    )
    .width(180.0)
    .on_select(|index| {
        let hex = ACCENTS[index].1.map(String::from);

        change_appearance(|now| now.accent = hex);
    });

    // the nearest size to what's set, so a hand-edited file still shows one
    let chosen_size = TEXT_SIZES
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            (a.1 - text_scale)
                .abs()
                .total_cmp(&(b.1 - text_scale).abs())
        })
        .map_or(1, |(index, _)| index);

    let sizes = Dropdown::new(
        "settings.text-size",
        TEXT_SIZES
            .iter()
            .map(|(name, _)| String::from(*name))
            .collect(),
        chosen_size,
    )
    .width(180.0)
    .on_select(|index| change_appearance(|now| now.text_scale = TEXT_SIZES[index].1));

    Column::new(vec![
        boxed(setting("Appearance", Column::new(mode_radios))),
        boxed(setting("Accent colour", accents.build())),
        boxed(setting("Density", Column::new(density_radios))),
        boxed(setting("Text size", sizes.build())),
        boxed(setting("Glass", Column::new(glass_radios))),
        boxed(setting("Motion", Column::new(motion_radios))),
    ])
    .gap(sys::space::gap::l())
}
