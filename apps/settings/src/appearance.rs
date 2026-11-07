//! How the shell looks: light or dark, the accent, how dense and how big
//! the text is, how see-through the glass is, whether small glass adapts
//! to its backdrop, and whether motion is reduced. Each control changes `appearance.json`, and the shell
//! redraws in the new look while this window shows it too.

use amane::{Color, Column};
use amane_kit::buttons::Switch;
use amane_kit::color_picker::ColorWells;
use amane_kit::dropdown::Dropdown;
use amane_kit::form::FormRow;
use amane_kit::settings::{ACCENTS, appearance, change_appearance};
use amane_kit::style;
use amane_kit::theme::{Density, Glass, Mode};
use amane_kit::thumbnail_picker::ThumbnailPicker;

use crate::{group, groups, previews};

/// Accents saved before the current colours, and the entry that shows for
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

    let width = crate::page_width() - 2.0 * style::row_pad_h();

    let modes = [
        ("Light", Some(Mode::Light)),
        ("Dark", Some(Mode::Dark)),
        ("Auto", None),
    ];
    let mut mode_picker = ThumbnailPicker::new("settings.mode", previews::WIDTH, previews::HEIGHT)
        .width(width)
        .chosen(modes.iter().position(|(_, choice)| *choice == mode))
        .on_select(move |index| {
            let choice = modes[index].1;
            change_appearance(|now| now.mode = choice);
        });
    for (label, choice) in modes {
        mode_picker = mode_picker.option(previews::appearance(choice), label);
    }
    let mode_height = mode_picker.height();

    let density_control = Dropdown::new(
        "settings.density",
        vec![String::from("Comfortable"), String::from("Compact")],
        usize::from(density == Density::Compact),
    )
    .width(180.0)
    .on_select(|index| {
        change_appearance(|now| {
            now.density = if index == 0 {
                Density::Comfortable
            } else {
                Density::Compact
            }
        })
    })
    .build();

    let glasses = [
        ("Clear", Glass::Clear),
        ("Tinted", Glass::Tinted),
        ("Solid", Glass::Solid),
    ];
    let mut glass_picker =
        ThumbnailPicker::new("settings.glass", previews::WIDTH, previews::HEIGHT)
            .width(width)
            .chosen(glasses.iter().position(|(_, choice)| *choice == glass))
            .on_select(move |index| {
                let choice = glasses[index].1;
                change_appearance(|now| now.glass = choice);
            });
    for (label, choice) in glasses {
        glass_picker = glass_picker.option(previews::glass(choice), label);
    }
    let glass_height = glass_picker.height();

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

    let motion_control = Dropdown::choice(
        "settings.motion",
        [
            ("Automatic", None),
            ("Reduced", Some(true)),
            ("Full", Some(false)),
        ],
        reduce_motion,
        |choice| change_appearance(|now| now.reduce_motion = choice),
    )
    .width(180.0)
    .build();

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

    let wells = ColorWells::new(
        "settings.accent",
        ACCENTS
            .iter()
            .map(|(name, hex)| (String::from(*name), hex.map(Color::from)))
            .collect(),
        chosen_accent,
    )
    .on_select(|index| {
        let hex = ACCENTS[index].1.map(String::from);
        change_appearance(|now| now.accent = hex);
    });
    let accent_name = wells.shown_name();

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
                    .detail(mode_picker.build(), mode_height),
            )
            .row(
                FormRow::new("settings.appearance.accent", "Accent color")
                    .subtitle(accent_name)
                    .trailing(wells.build()),
            )
            .row(FormRow::new("settings.appearance.text", "Text size").trailing(sizes.build()))
            .row(
                FormRow::new("settings.appearance.density", "Density")
                    .subtitle("Compact fits more in the bar, panels and lists")
                    .trailing(density_control),
            ),
        group()
            .header("Glass")
            .row(
                FormRow::new("settings.appearance.glass", "Material")
                    .detail(glass_picker.build(), glass_height),
            )
            .row(
                FormRow::new("settings.appearance.adaptive", "Adaptive glass")
                    .subtitle("Small glass turns light or dark with what's behind it")
                    .trailing(adaptive)
                    .child()
                    .disabled(solid),
            )
            .row(
                FormRow::new("settings.appearance.vibrant", "Vibrant labels")
                    .subtitle("Labels blend with the frosted backdrop")
                    .trailing(vibrant)
                    .child()
                    .disabled(solid),
            )
            .footer(glass_note),
        group().row(
            FormRow::new("settings.appearance.motion", "Motion")
                .subtitle("Automatic follows the system's setting")
                .trailing(motion_control),
        ),
    ])
}
