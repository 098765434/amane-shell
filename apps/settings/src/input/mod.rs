//! Input: how the keyboard repeats, how the touchpad and mouse move and
//! scroll, and a list of niri's keyboard shortcuts.
//!
//! These are niri's own `input` settings. They're written to `input.kdl`
//! ([`kdl`]), which `config.kdl` includes, and niri reads the file again by
//! itself, so a change takes at once. The page follows niri's config
//! loads, so it reads the file again after a change from anywhere. When a
//! load just after one of its own changes fails, it puts the file back as
//! it was and says so.
//!
//! The shortcuts are read from niri's config ([`binds`]) and only shown;
//! they're changed in `config.kdl`.

mod binds;
mod kdl;

use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use amane::{Column, Service, TextInput};
use emaki_kit::app::{Dialog, field_box, text_field};
use emaki_kit::buttons::PushButton;
use emaki_kit::color::Accent;
use emaki_kit::dropdown::Dropdown;
use emaki_kit::form::{FormRow, Group, SliderRow};
use emaki_kit::icons::{ICON_INFO, ICON_KEYBOARD};
use emaki_kit::layout::boxed;
use emaki_kit::slider::Slider;
use emaki_kit::stepper::Stepper;

use crate::{group, groups, switch_row};
use binds::{Bind, Category};
use kdl::{InputConfig, Pointer, Setup};

/// How long a slider or stepper rests before its change is written, so a
/// drag is one write and one commit.
const SETTLE: Duration = Duration::from_millis(400);

/// How soon after a write of ours a failed load counts as ours.
const OURS_FOR: Duration = Duration::from_secs(5);

/// niri's defaults, shown for a setting the file leaves unset.
const REPEAT_DELAY: u16 = 600;
const REPEAT_RATE: u8 = 25;

const TRY: &str = "settings.input.try";
const FILTER: &str = "settings.input.shortcuts.filter";

struct Input {
    setup: Setup,
    /// A touchpad is connected.
    touchpad: bool,
    binds: Vec<Bind>,
    /// What went wrong last.
    error: Option<String>,
    /// The file before our last write, and when: put back if niri refuses
    /// what was written.
    before: Option<(Instant, Option<String>)>,
    looked: bool,
}

/// Each change's number, so only a slider's last one is written.
static CHANGES: AtomicU64 = AtomicU64::new(0);

impl Service for Input {
    // read from listen(), so opening the page never waits on the disk
    fn new() -> Self {
        Self {
            setup: Setup::Missing,
            touchpad: false,
            binds: Vec::new(),
            error: None,
            before: None,
            looked: false,
        }
    }

    fn listen() {
        refresh();
        amane::niri::follow_config_loads(|failed| {
            let ours = Input::write().before.take();
            let ours = ours.filter(|(at, _)| at.elapsed() < OURS_FOR);
            if failed {
                if let Some((_, before)) = ours {
                    let undone = match before {
                        Some(text) => kdl::restore(&text).err(),
                        None => None,
                    };
                    Input::write().error = Some(match undone {
                        None => String::from(
                            "niri didn't take that change, so it was put back as it was.",
                        ),
                        Some(why) => format!(
                            "niri didn't take that change, and it couldn't be put back: {why}"
                        ),
                    });
                }
                return;
            }
            refresh();
        });
    }
}

/// Reads the file, the shortcuts and the devices again (off the UI thread).
fn refresh() {
    let setup = kdl::setup();
    let binds = binds::read();
    let touchpad = has_touchpad();
    let mut now = Input::write();
    let changed =
        !now.looked || now.setup != setup || now.binds != binds || now.touchpad != touchpad;
    now.setup = setup;
    now.binds = binds;
    now.touchpad = touchpad;
    now.looked = true;
    if !changed {
        now.quiet();
    }
}

/// Whether udev knows a touchpad, or failing that, an input device is
/// named like one.
fn has_touchpad() -> bool {
    let udev = fs::read_dir("/run/udev/data")
        .into_iter()
        .flatten()
        .flatten();
    let tagged = udev
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("c13:"))
        .any(|entry| {
            fs::read_to_string(entry.path())
                .is_ok_and(|text| text.lines().any(|l| l == "E:ID_INPUT_TOUCHPAD=1"))
        });
    tagged
        || fs::read_dir("/sys/class/input")
            .into_iter()
            .flatten()
            .flatten()
            .any(|entry| {
                fs::read_to_string(entry.path().join("device/name"))
                    .is_ok_and(|name| name.to_lowercase().contains("touchpad"))
            })
}

/// Makes `edit` at once on the page, and writes it after `wait`, unless
/// another change comes first.
fn change(what: &'static str, wait: Duration, edit: impl FnOnce(&mut InputConfig)) {
    {
        let mut now = Input::write();
        let Setup::Ready(config) = &mut now.setup else {
            return;
        };
        edit(config);
        now.error = None;
    }
    let number = CHANGES.fetch_add(1, Ordering::SeqCst) + 1;
    thread::spawn(move || {
        if !wait.is_zero() {
            thread::sleep(wait);
        }
        if CHANGES.load(Ordering::SeqCst) != number {
            return;
        }
        let Setup::Ready(config) = Input::read().setup.clone() else {
            return;
        };
        let before = kdl::current_text();
        Input::write().before = Some((Instant::now(), before));
        if let Err(error) = kdl::save(&config, what) {
            let mut now = Input::write();
            now.error = Some(error);
            now.before = None;
        }
    });
}

fn at_once(what: &'static str, edit: impl FnOnce(&mut InputConfig)) {
    change(what, Duration::ZERO, edit);
}

fn settled(what: &'static str, edit: impl FnOnce(&mut InputConfig)) {
    change(what, SETTLE, edit);
}

// ───── the page ─────────────────────────────────────────────────────────────

pub fn page() -> Column {
    let (setup, touchpad, error, looked) = {
        let now = Input::read();
        (
            now.setup.clone(),
            now.touchpad,
            now.error.clone(),
            now.looked,
        )
    };
    if !looked {
        return groups(vec![group().row(FormRow::new(
            "settings.input.reading",
            "Reading the input settings…",
        ))]);
    }
    let (config, editable) = match &setup {
        Setup::Ready(config) => (config.clone(), true),
        Setup::Unmoved(config) => (config.clone(), false),
        Setup::Missing => (InputConfig::default(), false),
    };

    let mut list: Vec<Group> = Vec::new();
    if let Some(banner) = setup_banner(&setup) {
        list.push(banner);
    }
    if let Some(error) = error {
        list.push(
            group().row(
                FormRow::new("settings.input.error", "Not changed")
                    .icon(emaki_kit::icons::ICON_WARNING, Accent::Orange)
                    .subtitle(error)
                    .trailing(
                        PushButton::new("settings.input.error.dismiss", "OK")
                            .on_press(|| Input::write().error = None)
                            .build(),
                    ),
            ),
        );
    }
    list.push(keyboard(&config, editable));
    if touchpad {
        list.push(touchpad_group(&config.touchpad, editable));
    }
    list.push(mouse_group(&config.mouse, editable));
    list.push(pointer_group(&config, editable));
    if touchpad {
        list.extend(gestures_group());
    }
    list.push(
        group().row(
            FormRow::new("settings.input.shortcuts-row", "Keyboard Shortcuts")
                .icon(ICON_KEYBOARD, Accent::Gray)
                .subtitle("Every key niri's config binds, and what it does")
                .on_press(|| crate::open(crate::View::Sub("shortcuts"))),
        ),
    );
    groups(list)
}

fn setup_banner(setup: &Setup) -> Option<Group> {
    match setup {
        Setup::Ready(_) => None,
        Setup::Missing => Some(
            group().row(
                FormRow::new("settings.input.missing", "Changes here can't be kept")
                    .icon(ICON_INFO, Accent::Orange)
                    .subtitle("niri's config.kdl wasn't found"),
            ),
        ),
        Setup::Unmoved(_) => Some(
            group()
                .row(
                    FormRow::new("settings.input.unmoved", "Shown, not changed here")
                        .icon(ICON_INFO, Accent::Orange)
                        .subtitle("niri's config.kdl sets the input itself.")
                        .trailing(
                            PushButton::new("settings.input.move", "Move to Settings…")
                                .on_press(confirm_move)
                                .build(),
                        ),
                )
                .footer(
                    "Move the input settings to Emaki Settings to change them here; \
                     until then this page only shows them.",
                ),
        ),
    }
}

fn confirm_move() {
    Dialog::new(
        "settings.input.move-dialog",
        "Move input settings to Settings?",
    )
    .message(format!(
        "The input block in {} becomes comments, and an include of {} takes its \
             place with the same settings, as a commit of its own.",
        crate::niri_files::niri_config().display(),
        kdl::input_file().display()
    ))
    .button("Cancel", || {})
    .main_button("Move", || {
        thread::spawn(|| {
            if let Err(error) = kdl::migrate() {
                Input::write().error = Some(error);
            }
            refresh();
        });
    })
    .open("main");
}

fn keyboard(config: &InputConfig, editable: bool) -> Group {
    let keyboard = &config.keyboard;
    let delay = keyboard.repeat_delay.unwrap_or(REPEAT_DELAY);
    let rate = keyboard.repeat_rate.unwrap_or(REPEAT_RATE);
    let try_it = field_box(TRY, text_field(TRY, "Hold a key here to try it"), 260.0);
    group()
        .header("Keyboard")
        .row(
            FormRow::new("settings.input.repeat-delay", "Delay before repeat")
                .value(format!("{delay} ms"))
                .trailing(
                    Stepper::new("settings.input.repeat-delay.stepper", f64::from(delay))
                        .range(150.0, 1000.0)
                        .step(50.0)
                        .width(80.0)
                        .disabled(!editable)
                        .on_change(|ms| {
                            settled("repeat delay", |c| {
                                c.keyboard.repeat_delay = Some(ms.round() as u16)
                            })
                        })
                        .build(),
                ),
        )
        .row(
            FormRow::new("settings.input.repeat-rate", "Repeat rate")
                .value(format!("{rate} a second"))
                .trailing(
                    Stepper::new("settings.input.repeat-rate.stepper", f64::from(rate))
                        .range(10.0, 60.0)
                        .step(1.0)
                        .width(80.0)
                        .disabled(!editable)
                        .on_change(|per| {
                            settled("repeat rate", |c| {
                                c.keyboard.repeat_rate = Some(per.round() as u8)
                            })
                        })
                        .build(),
                ),
        )
        .row(FormRow::new("settings.input.try-row", "Try it").trailing(try_it))
        .row(
            switch_row(
                "settings.input.numlock",
                "Num Lock on at login",
                keyboard.numlock,
                |on| at_once("num lock", move |c| c.keyboard.numlock = on),
            )
            .disabled(!editable),
        )
        .row(
            FormRow::new("settings.input.layout", "Layout")
                .subtitle("Set under General, with the system's language")
                .on_press(|| crate::go("general")),
        )
}

/// A speed from -1 to 1 on a slider's 0 to 100.
fn speed_at(speed: Option<f64>) -> u8 {
    ((speed.unwrap_or(0.0).clamp(-1.0, 1.0) + 1.0) * 50.0).round() as u8
}

fn speed_of(value: u8) -> f64 {
    f64::from(value) / 50.0 - 1.0
}

fn speed_words(speed: Option<f64>) -> String {
    match speed.unwrap_or(0.0) {
        s if s.abs() < 0.01 => String::from("Normal"),
        s if s < 0.0 => format!("{:.0}% slower", -s * 100.0),
        s => format!("{:.0}% faster", s * 100.0),
    }
}

/// Which pointer a setting belongs to.
#[derive(Clone, Copy)]
enum Which {
    Touchpad,
    Mouse,
}

impl Which {
    fn of(self, config: &mut InputConfig) -> &mut Pointer {
        match self {
            Which::Touchpad => &mut config.touchpad,
            Which::Mouse => &mut config.mouse,
        }
    }
}

fn speed_row(which: Which, pointer: &Pointer, editable: bool) -> SliderRow {
    let at = speed_at(pointer.accel_speed);
    SliderRow::new("Speed", move |width| {
        Slider::new(at, width)
            .wheel(false)
            .inert(!editable)
            .on_value_change(move |value| {
                settled("speed", move |c| {
                    let speed = (speed_of(value) * 100.0).round() / 100.0;
                    which.of(c).accel_speed = (speed != 0.0).then_some(speed);
                })
            })
            .build()
    })
    .value(speed_words(pointer.accel_speed))
}

/// What a commit says was changed: the row's id after the page's.
fn what(id: &'static str) -> &'static str {
    id.strip_prefix("settings.input.").unwrap_or(id)
}

/// A dropdown of niri's words for a setting, with "Default" for unset.
fn words_row(
    which: Which,
    id: &'static str,
    title: &str,
    choices: &[(&'static str, &'static str)],
    now: &Option<String>,
    editable: bool,
    set: fn(&mut Pointer, Option<String>),
) -> FormRow {
    let mut labels = vec![String::from("Default")];
    labels.extend(choices.iter().map(|(label, _)| label.to_string()));
    let values: Vec<Option<String>> = std::iter::once(None)
        .chain(choices.iter().map(|(_, word)| Some(word.to_string())))
        .collect();
    let at = values.iter().position(|v| v == now).unwrap_or(0);
    let name = what(id);
    FormRow::new(id, title).trailing(
        Dropdown::new(format!("{id}.choice"), labels, at)
            .width(200.0)
            .disabled(!editable)
            .on_select(move |index| {
                if index == at {
                    return;
                }
                let value = values.get(index).cloned().flatten();
                at_once(name, move |c| set(which.of(c), value));
            })
            .build(),
    )
}

fn pointer_switch(
    which: Which,
    id: &'static str,
    title: &str,
    on: bool,
    editable: bool,
    set: fn(&mut Pointer, bool),
) -> FormRow {
    let name = what(id);
    switch_row(id, title, on, move |on| {
        at_once(name, move |c| set(which.of(c), on))
    })
    .disabled(!editable)
}

const PROFILES: [(&str, &str); 2] = [("Adaptive", "adaptive"), ("Flat", "flat")];

fn touchpad_group(pointer: &Pointer, editable: bool) -> Group {
    let which = Which::Touchpad;
    group()
        .header("Touchpad")
        .row(pointer_switch(
            which,
            "settings.input.touchpad.tap",
            "Tap to click",
            pointer.tap,
            editable,
            |p, on| p.tap = on,
        ))
        .row(pointer_switch(
            which,
            "settings.input.touchpad.natural-scroll",
            "Natural scrolling",
            pointer.natural_scroll,
            editable,
            |p, on| p.natural_scroll = on,
        ))
        .row(pointer_switch(
            which,
            "settings.input.touchpad.dwt",
            "Off while typing",
            pointer.dwt,
            editable,
            |p, on| p.dwt = on,
        ))
        .slider(speed_row(which, pointer, editable))
        .row(words_row(
            which,
            "settings.input.touchpad.accel-profile",
            "Acceleration",
            &PROFILES,
            &pointer.accel_profile,
            editable,
            |p, v| p.accel_profile = v,
        ))
        .row(words_row(
            which,
            "settings.input.touchpad.click-method",
            "Right click",
            &[
                ("Bottom right corner", "button-areas"),
                ("Two fingers", "clickfinger"),
            ],
            &pointer.click_method,
            editable,
            |p, v| p.click_method = v,
        ))
        .row(words_row(
            which,
            "settings.input.touchpad.scroll-method",
            "Scroll with",
            &[("Two fingers", "two-finger"), ("The edge", "edge")],
            &pointer.scroll_method,
            editable,
            |p, v| p.scroll_method = v,
        ))
        .slider(scroll_row(which, pointer, editable))
        .row(pointer_switch(
            which,
            "settings.input.touchpad.off-with-mouse",
            "Off while a mouse is connected",
            pointer.disabled_on_external_mouse,
            editable,
            |p, on| p.disabled_on_external_mouse = on,
        ))
        .footer("Natural scrolling moves the content with your fingers, as on a phone.")
}

fn mouse_group(pointer: &Pointer, editable: bool) -> Group {
    let which = Which::Mouse;
    group()
        .header("Mouse")
        .row(pointer_switch(
            which,
            "settings.input.mouse.natural-scroll",
            "Natural scrolling",
            pointer.natural_scroll,
            editable,
            |p, on| p.natural_scroll = on,
        ))
        .slider(speed_row(which, pointer, editable))
        .row(words_row(
            which,
            "settings.input.mouse.accel-profile",
            "Acceleration",
            &PROFILES,
            &pointer.accel_profile,
            editable,
            |p, v| p.accel_profile = v,
        ))
        .slider(scroll_row(which, pointer, editable))
        .row(pointer_switch(
            which,
            "settings.input.mouse.middle-emulation",
            "Middle click by pressing both buttons",
            pointer.middle_emulation,
            editable,
            |p, on| p.middle_emulation = on,
        ))
        .footer("Flat moves the pointer as far as the mouse moves, however fast; for games.")
}

/// How far a scroll goes, on a slider's 0 to 100: a quarter to twice.
fn factor_at(factor: Option<f64>) -> u8 {
    (((factor.unwrap_or(1.0).clamp(0.25, 2.0) - 0.25) / 1.75) * 100.0).round() as u8
}

fn factor_of(value: u8) -> f64 {
    ((0.25 + f64::from(value) / 100.0 * 1.75) * 20.0).round() / 20.0
}

fn scroll_row(which: Which, pointer: &Pointer, editable: bool) -> SliderRow {
    let at = factor_at(pointer.scroll_factor);
    let factor = pointer.scroll_factor.unwrap_or(1.0);
    SliderRow::new("Scroll speed", move |width| {
        Slider::new(at, width)
            .wheel(false)
            .inert(!editable)
            .on_value_change(move |value| {
                settled("scroll speed", move |c| {
                    let factor = factor_of(value);
                    which.of(c).scroll_factor = ((factor - 1.0).abs() > 0.01).then_some(factor);
                })
            })
            .build()
    })
    .value(format!("{factor:.2}×"))
}

/// Where focus goes with the pointer, and which hand the buttons suit.
fn pointer_group(config: &InputConfig, editable: bool) -> Group {
    let left = config.mouse.left_handed || config.touchpad.left_handed;
    group()
        .header("Pointer")
        .row(
            switch_row(
                "settings.input.focus-follows-mouse",
                "Focus follows the pointer",
                config.focus_follows_mouse,
                |on| at_once("focus follows mouse", move |c| c.focus_follows_mouse = on),
            )
            .subtitle("The window under the pointer takes the keys")
            .disabled(!editable),
        )
        .row(
            switch_row(
                "settings.input.warp-to-focus",
                "Pointer jumps to the focused window",
                config.warp_mouse_to_focus,
                |on| at_once("warp to focus", move |c| c.warp_mouse_to_focus = on),
            )
            .subtitle("When a window is focused from the keyboard")
            .disabled(!editable),
        )
        .row(
            switch_row("settings.input.left-handed", "Left-handed", left, |on| {
                at_once("left-handed", move |c| {
                    c.mouse.left_handed = on;
                    c.touchpad.left_handed = on;
                })
            })
            .subtitle("Swaps the left and right buttons, on the mouse and the touchpad")
            .disabled(!editable),
        )
}

/// Touchpad gestures niri's `gestures` section sets, in `settings.kdl`.
fn gestures_group() -> Vec<Group> {
    let shown = crate::niri_settings::shown();
    let swipe = !shown
        .get(MOD_SWIPE)
        .is_some_and(|value| value.contains("off"));
    let mut list = crate::niri_settings::notices("settings.input.gestures");
    list.push(
        group().header("Gestures").row(
            switch_row(
                "settings.input.mod-swipe",
                "Super and swipe moves windows",
                swipe,
                |on| {
                    crate::niri_settings::set(
                        "mod swipe",
                        MOD_SWIPE,
                        if on { "{ }" } else { "{ off; }" },
                    )
                },
            )
            .subtitle("Holding Super, three fingers move the focused window and workspaces")
            .disabled(!shown.editable),
        ),
    );
    list
}

const MOD_SWIPE: crate::niri_settings::Key = ("gestures", "mod-swipe");
const SKIP_OVERLAY: crate::niri_settings::Key = ("hotkey-overlay", "skip-at-startup");

// ───── Keyboard Shortcuts ───────────────────────────────────────────────────

/// The page's sub-pages: Keyboard Shortcuts.
pub fn sub(name: &str) -> Option<(String, Box<dyn amane::Widget>)> {
    if name != "shortcuts" {
        return None;
    }
    Some((String::from("Keyboard Shortcuts"), boxed(shortcuts())))
}

fn shortcuts() -> Column {
    let all = Input::read().binds.clone();
    let query = TextInput::text(FILTER).trim().to_lowercase();
    let filter = group()
        .row(
            FormRow::new("settings.input.shortcuts.filter-row", "Find").trailing(field_box(
                FILTER,
                text_field(FILTER, "A key or what it does"),
                260.0,
            )),
        )
        .footer(format!(
            "Shortcuts are changed in {}; this list follows it.",
            crate::niri_files::niri_config().display()
        ));
    let shown = crate::niri_settings::shown();
    let mut list = crate::niri_settings::notices("settings.input.overlay");
    list.push(
        group().row(
            switch_row(
                "settings.input.shortcuts.at-login",
                "Show the shortcuts at login",
                !shown.on(SKIP_OVERLAY),
                |on| {
                    crate::niri_settings::set(
                        "shortcuts at login",
                        SKIP_OVERLAY,
                        if on { "false" } else { "true" },
                    )
                },
            )
            .subtitle("niri's overlay of its keys, which Super+Shift+/ also shows")
            .disabled(!shown.editable),
        ),
    );
    list.push(filter);
    let found: Vec<&Bind> = all
        .iter()
        .filter(|bind| query.is_empty() || bind.matches(&query))
        .collect();
    for category in Category::ALL {
        let rows: Vec<FormRow> = found
            .iter()
            .filter(|bind| bind.category == category)
            .enumerate()
            .map(|(index, bind)| {
                let mut row = FormRow::new(
                    format!("settings.input.shortcut.{:?}.{index}", category),
                    bind.label.clone(),
                )
                .value(bind.shown_keys());
                if let Some(command) = &bind.command
                    && !bind.label.starts_with("Run ")
                {
                    row = row.subtitle(command.chars().take(80).collect::<String>());
                }
                row
            })
            .collect();
        if !rows.is_empty() {
            list.push(group().header(category.title()).rows(rows));
        }
    }
    if found.is_empty() {
        list.push(group().row(FormRow::new(
            "settings.input.shortcuts.none",
            if all.is_empty() {
                "No shortcuts were found in niri's config"
            } else {
                "No shortcut matches"
            },
        )));
    }
    groups(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speeds_go_both_ways() {
        assert_eq!(speed_at(None), 50);
        assert_eq!(speed_at(Some(-1.0)), 0);
        assert_eq!(speed_at(Some(1.0)), 100);
        for value in [0u8, 17, 50, 83, 100] {
            assert_eq!(speed_at(Some(speed_of(value))), value);
        }
        assert_eq!(speed_words(Some(-0.3)), "30% slower");
        assert_eq!(speed_words(None), "Normal");
    }
}
