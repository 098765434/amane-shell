//! How the shell looks: light or dark, the accent, how dense and how big
//! the text is, how see-through the glass is, whether small glass adapts
//! to its backdrop, and whether motion is reduced. Each control changes `appearance.json`, and the shell
//! redraws in the new look while this window shows it too.

use amane::{Column, Widget};
use amane_kit::buttons::{Radio, Switch};
use amane_kit::dropdown::Dropdown;
use amane_kit::form::FormRow;
use amane_kit::layout::boxed;
use amane_kit::settings::{ACCENTS, appearance, change_appearance};
use amane_kit::theme::{Density, Glass, Mode};

use crate::{group, groups, radios_height};

/// Accents saved before macOS 27's colours, and the entry that shows for
/// each; picking it again saves the new colour.
const OLD_ACCENTS: [(&str, &str); 6] = [
    ("#0a84ff", "#0091ff"),
    ("#bf5af2", "#db34f2"),
    ("#ff453a", "#ff4245"),
    ("#ff9f0a", "#ff9230"),
    ("#ffd60a", "#ffd600"),
    ("#8e8e93", "#98989f"),
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
    let (mode, accent, density, text_scale, glass, reduce_motion, adaptive_glass, vibrancy) = {
        let now = appearance();

        (
            now.mode,
            now.accent.clone(),
            now.density,
            now.text_scale,
            now.glass,
            now.reduce_motion,
            now.adaptive_glass,
            now.vibrancy,
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

    // small glass turning light or dark with what's behind it; solid glass
    // has no backdrop to follow
    let adaptive = Switch::new("settings.adaptive-glass", adaptive_glass)
        .disabled(glass == Glass::Solid)
        .on_toggle(|on| change_appearance(|now| now.adaptive_glass = on))
        .build();

    // labels blending with the frosted backdrop; solid glass has none
    let vibrant = Switch::new("settings.vibrancy", vibrancy)
        .disabled(glass == Glass::Solid)
        .on_toggle(|on| change_appearance(|now| now.vibrancy = on))
        .build();

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

    let saved = accent.as_deref().map(|hex| {
        OLD_ACCENTS
            .iter()
            .find(|(old, _)| old.eq_ignore_ascii_case(hex))
            .map_or(hex, |(_, new)| *new)
    });
    let chosen_accent = ACCENTS
        .iter()
        .position(|(_, hex)| *hex == saved)
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

    let solid = glass == Glass::Solid;
    let glass_note = if solid {
        "Solid glass has no backdrop to follow or blend with."
    } else {
        ""
    };

    groups(vec![
        group()
            .row(
                FormRow::new("settings.appearance.mode", "Appearance")
                    .detail(Column::new(mode_radios), radios_height(3)),
            )
            .row(
                FormRow::new("settings.appearance.accent", "Accent colour")
                    .trailing(accents.build()),
            )
            .row(FormRow::new("settings.appearance.text", "Text size").trailing(sizes.build())),
        group().row(
            FormRow::new("settings.appearance.density", "Density")
                .detail(Column::new(density_radios), radios_height(2)),
        ),
        group()
            .row(
                FormRow::new("settings.appearance.glass", "Glass")
                    .detail(Column::new(glass_radios), radios_height(3)),
            )
            .row(
                FormRow::new("settings.appearance.adaptive", "Adaptive glass")
                    .subtitle("Small glass turns light or dark with what's behind it")
                    .trailing(adaptive)
                    .disabled(solid),
            )
            .row(
                FormRow::new("settings.appearance.vibrant", "Vibrant labels")
                    .subtitle("Labels blend with the frosted backdrop")
                    .trailing(vibrant)
                    .disabled(solid),
            )
            .footer(glass_note),
        group().row(
            FormRow::new("settings.appearance.motion", "Motion")
                .detail(Column::new(motion_radios), radios_height(3)),
        ),
    ])
}
