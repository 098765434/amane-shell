//! The launcher's actions, Spotlight's Actions: things it does without
//! opening an app. Quick Settings' controls (turning Night Light on, taking
//! a screenshot), the session (locking, restarting), the power mode, keeping
//! the screen awake, a timer, and every app's desktop actions ("New Private
//! Window"). Some ask for something first (how long, which mode), typed in
//! the field once the search is narrowed to them, or after their name
//! ("timer 5m").
//!
//! Each is found by a key like `action:qs:night-light`, which its use and
//! its quick keys are kept under.

use std::rc::Rc;
use std::time::Duration;

use amane::{DesktopApp, PowerProfile, Service};

use crate::components::icons::{
    Glyph, ICON_KEEP_AWAKE, ICON_LOCK, ICON_LOG_OUT, ICON_PACKAGE, ICON_POWER, ICON_RESTART,
    ICON_SLEEP, ICON_TIMER, ICON_TUNE, ICON_UPDATE, ICON_VPN,
};
use crate::components::quick_settings::{self, ControlId};
use crate::services::keep_awake::KeepAwake;
use crate::services::session::Session;
use crate::services::timer;

use super::entries::Icon;
use super::model::{self, SessionAction};

/// What an action asks for before it runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Param {
    /// How long, typed: "5m", "1h30m", "1:30".
    Duration,
    /// One of the action's choices.
    Choice,
}

/// What was given for an action's [`Param`].
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Value {
    None,
    Duration(Duration),
    Choice(String),
}

pub(super) struct Spec {
    pub(super) key: String,
    pub(super) title: String,
    /// Where it's listed: "Control Center", "System", an app's name.
    pub(super) section: String,
    pub(super) icon: Icon,
    /// Other words it's found by, lowercased; the first few also start an
    /// argument typed after them ("timer 5m").
    pub(super) keywords: Vec<String>,
    pub(super) param: Option<Param>,
    /// What the field says while it waits for the param.
    pub(super) placeholder: &'static str,
    /// For a choice: what each option is called, and what it gives.
    pub(super) choices: Vec<(String, String)>,
    pub(super) run: Rc<dyn Fn(Value)>,
}

impl Spec {
    fn new(key: impl Into<String>, title: impl Into<String>, section: &str, icon: Icon) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            section: section.to_string(),
            icon,
            keywords: Vec::new(),
            param: None,
            placeholder: "",
            choices: Vec::new(),
            run: Rc::new(|_| {}),
        }
    }

    fn keywords(mut self, keywords: &[&str]) -> Self {
        self.keywords = keywords.iter().map(|word| word.to_string()).collect();
        self
    }

    fn asks(mut self, param: Param, placeholder: &'static str) -> Self {
        self.param = Some(param);
        self.placeholder = placeholder;
        self
    }

    fn run(mut self, run: impl Fn(Value) + 'static) -> Self {
        self.run = Rc::new(run);
        self
    }

    /// `value`, as what this asks for, from `text` typed.
    pub(super) fn read(&self, text: &str) -> Option<Value> {
        match self.param? {
            Param::Duration => model::parse_duration(text).map(Value::Duration),
            Param::Choice => {
                let text = text.trim().to_lowercase();
                self.choices
                    .iter()
                    .find(|(name, _)| name.to_lowercase() == text)
                    .map(|(_, value)| Value::Choice(value.clone()))
            }
        }
    }

    /// The title for running it with `value`: "Start Timer · 5 min".
    pub(super) fn titled(&self, value: &Value) -> String {
        match value {
            Value::None => self.title.clone(),
            Value::Duration(length) => {
                format!("{} · {}", self.title, model::format_duration(*length))
            }
            Value::Choice(choice) => {
                let name = self
                    .choices
                    .iter()
                    .find(|(_, value)| value == choice)
                    .map_or(choice.as_str(), |(name, _)| name.as_str());
                format!("{} · {name}", self.title)
            }
        }
    }
}

const CONTROL_CENTER: &str = "Control Center";
const SYSTEM: &str = "System";
const CLOCK: &str = "Clock";

/// Every action this machine has, in the order Actions lists them: Control
/// Center, the system, the clock, then each app's.
pub(super) fn registry(apps: &[DesktopApp]) -> Vec<Spec> {
    let mut specs = Vec::new();
    controls(&mut specs);
    power_mode(&mut specs);
    keep_awake(&mut specs);
    session(&mut specs);
    nixos(&mut specs);
    clock(&mut specs);
    for app in apps {
        for (index, action) in app.actions().iter().enumerate() {
            let launched = app.clone();
            specs.push(
                Spec::new(
                    format!("action:app:{}#{index}", app.id()),
                    action.name(),
                    app.name(),
                    Icon::App(Box::new(app.clone())),
                )
                .run(move |_| {
                    if let Some(action) = launched.actions().get(index) {
                        action.launch();
                    }
                }),
            );
        }
    }
    specs
}

/// The action kept under `key`.
pub(super) fn find(key: &str, apps: &[DesktopApp]) -> Option<Spec> {
    registry(apps).into_iter().find(|spec| spec.key == key)
}

/// Quick Settings' toggles and buttons, each as it is now: "Turn Night
/// Light On" while it's off. Lock and Power Off are the system's.
fn controls(specs: &mut Vec<Spec>) {
    for id in ControlId::ALL {
        if matches!(id, ControlId::Lock | ControlId::PowerOff)
            || !quick_settings::control_available(id)
        {
            continue;
        }
        let Some(toggle) = quick_settings::quick_toggle(id).filter(|toggle| toggle.acts()) else {
            continue;
        };
        let title = if toggle.switches() {
            let to = if toggle.is_checked() { "Off" } else { "On" };
            format!("Turn {} {to}", toggle.title())
        } else {
            toggle.title().to_string()
        };
        let glyph = match toggle.glyph() {
            Glyph::Font(glyph) => glyph,
            Glyph::Tailscale => ICON_VPN,
        };
        let name = toggle.title().to_lowercase();
        let toggle = Rc::new(toggle);
        specs.push(
            Spec::new(
                format!("action:qs:{}", id.name()),
                title,
                CONTROL_CENTER,
                Icon::Glyph(glyph),
            )
            .keywords(&[&name])
            .run(move |_| toggle.fire()),
        );
    }
}

/// The power modes, when the daemon offers more than one.
fn power_mode(specs: &mut Vec<Spec>) {
    let profile = PowerProfile::read();
    if !profile.present() || profile.available().len() < 2 {
        return;
    }
    let choices = profile
        .available()
        .iter()
        .map(|name| (profile_name(name), name.clone()))
        .collect();
    drop(profile);
    let mut spec = Spec::new(
        "action:power-mode",
        "Set Power Mode",
        CONTROL_CENTER,
        Icon::Glyph(ICON_TUNE),
    )
    .keywords(&["power mode", "performance", "power saver", "balanced"])
    .asks(Param::Choice, "Which power mode?")
    .run(|value| {
        if let Value::Choice(profile) = value {
            PowerProfile::set(&profile);
        }
    });
    spec.choices = choices;
    specs.push(spec);
}

/// What a power profile is called: "power-saver" is Power Saver.
fn profile_name(profile: &str) -> String {
    profile
        .split('-')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + chars.as_str()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Holding the screen on for a while.
fn keep_awake(specs: &mut Vec<Spec>) {
    if !KeepAwake::read().present() {
        return;
    }
    specs.push(
        Spec::new(
            "action:keep-awake-for",
            "Keep Awake For",
            CONTROL_CENTER,
            Icon::Glyph(ICON_KEEP_AWAKE),
        )
        .keywords(&["awake", "caffeine", "inhibit"])
        .asks(Param::Duration, "For how long? 30m, 2h")
        .run(|value| {
            if let Value::Duration(length) = value {
                KeepAwake::set_limit(Some(length));
            }
        }),
    );
}

/// Locking, logging out, sleeping, restarting and powering off, each when
/// logind allows it.
fn session(specs: &mut Vec<Spec>) {
    let session = Session::read();
    for power in SessionAction::ALL {
        let allowed = match power {
            SessionAction::Suspend => session.can_suspend(),
            SessionAction::Restart => session.can_reboot(),
            SessionAction::PowerOff => session.can_power_off(),
            SessionAction::Lock | SessionAction::LogOut => true,
        };
        if !allowed {
            continue;
        }
        let glyph = match power {
            SessionAction::Lock => ICON_LOCK,
            SessionAction::LogOut => ICON_LOG_OUT,
            SessionAction::Suspend => ICON_SLEEP,
            SessionAction::Restart => ICON_RESTART,
            SessionAction::PowerOff => ICON_POWER,
        };
        specs.push(
            Spec::new(
                format!("action:session:{}", power.name()),
                power.label(),
                SYSTEM,
                Icon::Glyph(glyph),
            )
            .keywords(power.aliases())
            .run(move |_| match power {
                SessionAction::Lock => Session::lock(),
                SessionAction::LogOut => Session::log_out(),
                SessionAction::Suspend => Session::suspend(),
                SessionAction::Restart => Session::reboot(),
                SessionAction::PowerOff => Session::power_off(),
            }),
        );
    }
}

/// The NixOS configuration, through Shell Settings' System pages: checking
/// for updates, applying what's changed, installing software. Only with
/// Shell Settings installed.
fn nixos(specs: &mut Vec<Spec>) {
    if !crate::services::installed("amane-settings") {
        return;
    }
    let pages: [(&str, &str, &str, &[&str], char); 3] = [
        (
            "check-updates",
            "Check for Updates",
            "update",
            &["update", "upgrade", "nixos", "software update"],
            ICON_UPDATE,
        ),
        (
            "apply",
            "Apply System Changes",
            "about",
            &["apply", "rebuild", "nixos", "switch"],
            ICON_TUNE,
        ),
        (
            "install",
            "Install Software",
            "apps",
            &["install", "app", "package", "flatpak"],
            ICON_PACKAGE,
        ),
    ];
    for (key, title, page, keywords, glyph) in pages {
        specs.push(
            Spec::new(
                format!("action:system:{key}"),
                title,
                SYSTEM,
                Icon::Glyph(glyph),
            )
            .keywords(keywords)
            .run(move |_| amane::spawn_activated(&format!("amane-settings --page {page}"))),
        );
    }
}

/// Starting a timer, and stopping the one running.
fn clock(specs: &mut Vec<Spec>) {
    specs.push(
        Spec::new(
            "action:timer",
            "Start Timer",
            CLOCK,
            Icon::Glyph(ICON_TIMER),
        )
        .keywords(&["timer", "countdown"])
        .asks(Param::Duration, "For how long? 5m, 1h30m, 1:30")
        .run(|value| {
            if let Value::Duration(length) = value {
                timer::start(length, format!("{} timer", model::format_duration(length)));
            }
        }),
    );
    if let Some((left, _)) = timer::left() {
        specs.push(
            Spec::new(
                "action:timer-cancel",
                format!("Cancel Timer ({} left)", timer::format(left)),
                CLOCK,
                Icon::Glyph(ICON_TIMER),
            )
            .keywords(&["timer", "stop timer"])
            .run(|_| timer::cancel()),
        );
    }
}

/// `needle` as the start of `spec` and an argument after it, like "timer
/// 5m": the argument, when it reads as what the action asks for.
pub(super) fn inline(spec: &Spec, needle: &str) -> Option<Value> {
    spec.param?;
    let title = spec.title.to_lowercase();
    std::iter::once(title.as_str())
        .chain(spec.keywords.iter().map(String::as_str))
        .find_map(|start| {
            let rest = needle.strip_prefix(start)?.strip_prefix(' ')?;
            spec.read(rest)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timer() -> Spec {
        Spec::new(
            "action:timer",
            "Start Timer",
            CLOCK,
            Icon::Glyph(ICON_TIMER),
        )
        .keywords(&["timer"])
        .asks(Param::Duration, "")
    }

    #[test]
    fn an_argument_can_follow_the_name() {
        let spec = timer();
        assert_eq!(
            inline(&spec, "timer 5m"),
            Some(Value::Duration(Duration::from_secs(300)))
        );
        assert_eq!(
            inline(&spec, "start timer 1:30"),
            Some(Value::Duration(Duration::from_secs(90)))
        );
        assert_eq!(inline(&spec, "timer soon"), None);
        assert_eq!(inline(&spec, "timer"), None);
        assert_eq!(inline(&spec, "timers 5m"), None);
    }

    #[test]
    fn a_choice_is_read_by_its_name() {
        let mut spec = timer().asks(Param::Choice, "");
        spec.choices = vec![(String::from("Power Saver"), String::from("power-saver"))];
        assert_eq!(
            spec.read("power saver"),
            Some(Value::Choice(String::from("power-saver")))
        );
        assert_eq!(spec.read("turbo"), None);
        assert_eq!(
            spec.titled(&Value::Choice(String::from("power-saver"))),
            "Start Timer · Power Saver"
        );
    }

    #[test]
    fn power_profiles_are_named_as_people_say_them() {
        assert_eq!(profile_name("power-saver"), "Power Saver");
        assert_eq!(profile_name("balanced"), "Balanced");
    }
}
