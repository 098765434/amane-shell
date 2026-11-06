//! The assistant in Spotlight: a command read from the query
//! ([`crate::services::assistant`]) as the card over the results, "Turn On
//! Night Light", run with Return like any answer. Running it is the shell's
//! ordinary action: the Control Center switch, the volume key (with its
//! OSD), the app's launch, the session's actions. What ends the session is
//! asked about first, every time.

use std::rc::Rc;
use std::time::Duration;

use amane::{Audio, Brightness, DesktopApp, Media as Player, PowerProfile, Service};
use amane_assistant::snapshot::{self, App, Snapshot, Switch};
use amane_assistant::{Command, Level, Media, Power, Reading, Risk, Source, State, Target};

use crate::components::dialog::Dialog;
use crate::components::icons::{
    Glyph, ICON_BRIGHTNESS_HIGH, ICON_BRIGHTNESS_LOW, ICON_LOCK, ICON_LOG_OUT, ICON_POWER,
    ICON_RESTART, ICON_SKIP_NEXT, ICON_SKIP_PREVIOUS, ICON_SLEEP, ICON_TIMER, ICON_TUNE,
    ICON_VOLUME_DOWN, ICON_VOLUME_OFF, ICON_VOLUME_UP, ICON_VPN,
};
use crate::services::launcher::timer;
use crate::services::power::session::Session;
use crate::surfaces::alert;
use crate::surfaces::osd;
use crate::surfaces::quick_settings::{self, ControlId};

use super::actions::profile_name;
use super::entries::{Action, Entry, Icon, app_key};
use super::model::{self, SessionAction};
use super::usage;

/// The key of a card whose command is asked about before it runs.
pub(super) const CONFIRMS: &str = "assistant:confirm";

/// At this or above, what Needle reads is offered plainly; below it, as a
/// suggestion.
const SURE: f32 = 0.75;

/// What the machine has now, for a command to name: Control Center's
/// switches, the apps (the ones used most first), the power modes, what
/// the session allows.
pub(super) fn snapshot(apps: &[DesktopApp]) -> Snapshot {
    let mut switches = Vec::new();
    for id in ControlId::ALL {
        // locking and powering off are the session's
        if matches!(id, ControlId::Lock | ControlId::PowerOff)
            || !quick_settings::control_available(id)
        {
            continue;
        }
        let Some(toggle) = quick_settings::quick_toggle(id).filter(|toggle| toggle.acts()) else {
            continue;
        };
        let on = toggle.switches().then(|| toggle.is_checked());
        switches.push(Switch::new(id.name(), id.title(), on));
    }

    let now = usage::now();
    let mut ranked: Vec<(f64, &DesktopApp)> = usage::read(|usage| {
        apps.iter()
            .map(|app| (usage.frecency(&app_key(app), now), app))
            .collect()
    });
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
    let apps = ranked
        .into_iter()
        .map(|(_, app)| App {
            id: app.id().to_string(),
            name: app.name().to_string(),
            kinds: snapshot::kinds(app.categories(), app.keywords()),
        })
        .collect();

    let profiles = PowerProfile::read();
    let power_modes = if profiles.present() && profiles.available().len() > 1 {
        profiles
            .available()
            .iter()
            .map(|name| (profile_name(name), name.clone()))
            .collect()
    } else {
        Vec::new()
    };
    drop(profiles);

    let session = Session::read();
    let powers = Power::ALL
        .into_iter()
        .filter(|power| match power {
            Power::Sleep => session.can_suspend(),
            Power::Restart => session.can_reboot(),
            Power::PowerOff => session.can_power_off(),
            Power::Lock | Power::LogOut => true,
        })
        .collect();
    drop(session);

    Snapshot {
        switches,
        apps,
        power_modes,
        powers,
        brightness: Brightness::read().present(),
        timer_running: timer::left().is_some(),
    }
}

/// `reading`'s card: what it does, and doing it.
pub(super) fn entry(reading: &Reading, apps: &[DesktopApp]) -> Option<Entry> {
    let (title, icon, action) = proposal(&reading.command, apps)?;
    let subtitle = if reading.source == Source::Rules || reading.confidence >= SURE {
        "Assistant"
    } else {
        "Assistant · Suggested"
    };
    let destructive = reading.command.risk() == Risk::Destructive;
    let action = match action {
        Some(run) if destructive => {
            let asked = title.clone();
            Action::Run(Rc::new(move || confirm(&asked, run.clone())))
        }
        Some(run) => Action::Run(run),
        None => Action::Hint,
    };
    let key = if destructive { CONFIRMS } else { "assistant" };
    Some(Entry::new(key, title, icon, action).subtitle(subtitle))
}

/// What `command` is called, its icon, and running it; `None` to run when
/// there's nothing to do ("Night Light Is On").
#[allow(clippy::type_complexity)]
fn proposal(
    command: &Command,
    apps: &[DesktopApp],
) -> Option<(String, Icon, Option<Rc<dyn Fn()>>)> {
    let run = |f: Rc<dyn Fn()>| Some(f);
    Some(match command {
        Command::Switch { control, state } => {
            let id = ControlId::named(control)?;
            let toggle = quick_settings::quick_toggle(id)?;
            let glyph = match toggle.glyph() {
                Glyph::Font(glyph) => glyph,
                Glyph::Tailscale => ICON_VPN,
            };
            let name = id.title();
            let on = toggle.is_checked();
            if !toggle.switches() {
                let toggle = Rc::new(toggle);
                return Some((
                    name.to_string(),
                    Icon::Glyph(glyph),
                    run(Rc::new(move || toggle.fire())),
                ));
            }
            let to = match state {
                State::On => true,
                State::Off => false,
                State::Toggle => !on,
            };
            if to == on {
                let now = if on { "On" } else { "Off" };
                return Some((format!("{name} Is Already {now}"), Icon::Glyph(glyph), None));
            }
            let title = format!("Turn {name} {}", if to { "On" } else { "Off" });
            let toggle = Rc::new(toggle);
            (
                title,
                Icon::Glyph(glyph),
                run(Rc::new(move || toggle.fire())),
            )
        }
        Command::Level { level, target } => {
            let (name, up, down) = match level {
                Level::Volume => ("Volume", ICON_VOLUME_UP, ICON_VOLUME_DOWN),
                Level::Brightness => ("Brightness", ICON_BRIGHTNESS_HIGH, ICON_BRIGHTNESS_LOW),
            };
            let key = match level {
                Level::Volume => "volume",
                Level::Brightness => "brightness",
            };
            match *target {
                Target::Percent(percent) => {
                    let level = *level;
                    (
                        format!("Set {name} to {percent}%"),
                        Icon::Glyph(if percent >= 50 { up } else { down }),
                        run(Rc::new(move || match level {
                            Level::Volume => osd::set_volume(percent),
                            Level::Brightness => osd::set_brightness(percent),
                        })),
                    )
                }
                Target::Up => (
                    format!("Turn {name} Up"),
                    Icon::Glyph(up),
                    run(Rc::new(move || {
                        osd::ipc(&[key.to_string(), String::from("up")]);
                    })),
                ),
                Target::Down => (
                    format!("Turn {name} Down"),
                    Icon::Glyph(down),
                    run(Rc::new(move || {
                        osd::ipc(&[key.to_string(), String::from("down")]);
                    })),
                ),
            }
        }
        Command::Mute { state } => {
            let muted = Audio::read().muted();
            let to = match state {
                State::On => true,
                State::Off => false,
                State::Toggle => !muted,
            };
            if to == muted {
                let now = if muted { "Muted" } else { "On" };
                return Some((
                    format!("Sound Is Already {now}"),
                    Icon::Glyph(ICON_VOLUME_OFF),
                    None,
                ));
            }
            (
                String::from(if to { "Mute Sound" } else { "Unmute Sound" }),
                Icon::Glyph(if to { ICON_VOLUME_OFF } else { ICON_VOLUME_UP }),
                run(Rc::new(|| {
                    osd::ipc(&[String::from("volume"), String::from("mute")]);
                })),
            )
        }
        Command::Open { app } => {
            let app = apps.iter().find(|candidate| candidate.id() == app)?.clone();
            let title = format!("Open {}", app.name());
            let icon = Icon::App(Box::new(app.clone()));
            (title, icon, run(Rc::new(move || app.launch())))
        }
        Command::Media { media } => {
            let playing = Player::read().playing();
            let media = *media;
            let (title, glyph) = match media {
                Media::Play => ("Play", crate::components::icons::ICON_PLAY),
                Media::Pause => ("Pause", crate::components::icons::ICON_MUSIC),
                Media::Next => ("Next Track", ICON_SKIP_NEXT),
                Media::Previous => ("Previous Track", ICON_SKIP_PREVIOUS),
            };
            if (media == Media::Play && playing) || (media == Media::Pause && !playing) {
                let now = if playing { "Playing" } else { "Paused" };
                return Some((format!("Already {now}"), Icon::Glyph(glyph), None));
            }
            (
                title.to_string(),
                Icon::Glyph(glyph),
                run(Rc::new(move || match media {
                    Media::Play | Media::Pause => Player::play_pause(),
                    Media::Next => Player::next(),
                    Media::Previous => Player::previous(),
                })),
            )
        }
        Command::Power { power } => {
            let action = match power {
                Power::Lock => SessionAction::Lock,
                Power::LogOut => SessionAction::LogOut,
                Power::Sleep => SessionAction::Suspend,
                Power::Restart => SessionAction::Restart,
                Power::PowerOff => SessionAction::PowerOff,
            };
            let glyph = match power {
                Power::Lock => ICON_LOCK,
                Power::LogOut => ICON_LOG_OUT,
                Power::Sleep => ICON_SLEEP,
                Power::Restart => ICON_RESTART,
                Power::PowerOff => ICON_POWER,
            };
            (
                action.label().to_string(),
                Icon::Glyph(glyph),
                run(Rc::new(move || match action {
                    SessionAction::Lock => Session::lock(),
                    SessionAction::LogOut => Session::log_out(),
                    SessionAction::Suspend => Session::suspend(),
                    SessionAction::Restart => Session::reboot(),
                    SessionAction::PowerOff => Session::power_off(),
                })),
            )
        }
        Command::Timer { seconds } => {
            let length = Duration::from_secs(*seconds);
            let named = model::format_duration(length);
            (
                format!("Start Timer · {named}"),
                Icon::Glyph(ICON_TIMER),
                run(Rc::new(move || {
                    timer::start(length, format!("{named} timer"))
                })),
            )
        }
        Command::CancelTimer => (
            String::from("Cancel Timer"),
            Icon::Glyph(ICON_TIMER),
            run(Rc::new(timer::cancel)),
        ),
        Command::PowerMode { profile } => {
            let name = profile_name(profile);
            let profile = profile.clone();
            (
                format!("Use {name} Mode"),
                Icon::Glyph(ICON_TUNE),
                run(Rc::new(move || PowerProfile::set(&profile))),
            )
        }
    })
}

/// Asks before ending the session: "Restart?", with Cancel.
fn confirm(title: &str, run: Rc<dyn Fn()>) {
    let question = format!("{}?", title.trim_end_matches('…'));
    alert::show(
        Dialog::new("amane.assistant.confirm", &question)
            .message("The assistant was asked to. Apps that are open may lose what isn't saved.")
            .button("Cancel", || {})
            .destructive(title.trim_end_matches('…'), move || run()),
    );
}
