//! The shell's IPC as a type: every command `amane-shell` answers on its
//! socket ([`ShellCommand`]), written as the same lines (a verb, then one
//! argument a line) that scripts and keybinds send with `nc`. The shell reads
//! them into this type, and other programs send this type, so neither builds
//! a verb by hand. See `docs/integration/ipc.md`.
//!
//! Names the shell knows (pages, controls, indicators, widget kinds) stay
//! text here: the shell checks them and answers with what it knows.

use std::io;

/// The program name the shell's socket is known by.
pub const SHELL: &str = "amane";

/// One thing to ask the shell.
#[derive(Clone, Debug, PartialEq)]
pub enum ShellCommand {
    /// `arm [name]`: the next right-click opens the menu `name` (`file`).
    Arm(String),
    /// `open [name] [x] [y]`: opens the menu `name` at a screen point.
    OpenMenu {
        /// The menu's name.
        name: String,
        /// Where, on the screen.
        x: i32,
        /// Where, on the screen.
        y: i32,
    },
    /// `close`: closes the open menu.
    CloseMenu,
    /// `notifications …`.
    Notifications(Notifications),
    /// `quick-settings …`.
    QuickSettings(QuickSettings),
    /// `launcher …`.
    Launcher(Launcher),
    /// `assistant …`.
    Assistant(Assistant),
    /// `osd …`: the media and brightness keys.
    Osd(Osd),
    /// `widgets …`.
    Widgets(Widgets),
    /// `status-bar …`.
    StatusBar(StatusBar),
    /// `night-light …`.
    NightLight(NightLight),
    /// `passwords …`.
    Passwords(Passwords),
    /// `alert-test [stacked]`: a sample alert, in debug builds.
    AlertTest {
        /// Two alerts, one over the other.
        stacked: bool,
    },
}

/// `notifications`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notifications {
    /// `toggle` (or nothing): the center.
    Toggle,
    /// `clear`: every notification.
    Clear,
    /// `dnd [toggle]`, `dnd on`, `dnd off`.
    DoNotDisturb(Switch),
}

/// Turning something on, off, or the other way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Switch {
    /// Turns it on.
    On,
    /// Turns it off.
    Off,
    /// Turns it the other way.
    Toggle,
}

/// `quick-settings`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuickSettings {
    /// `toggle` (or nothing).
    Toggle,
    /// `open`: on its grid.
    Open,
    /// `open <page>`: the page on its own.
    OpenPage(String),
    /// `toggle <page>`.
    TogglePage(String),
    /// Closes it.
    Close,
    /// `edit`: edit mode.
    Edit,
    /// `customize`: the same, as Shell Settings asks.
    Customize,
    /// `controls`: each control this machine has, and whether it shows.
    Controls,
    /// `show <control>`.
    Show(String),
    /// `hide <control>`.
    Hide(String),
    /// `reset-layout`.
    ResetLayout,
}

/// `launcher [toggle | open | close] [mode]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launcher {
    /// What it does.
    pub action: LauncherAction,
    /// `apps`, `files`, `actions`, `clipboard` or `passwords`.
    pub mode: Option<String>,
}

/// What `launcher` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LauncherAction {
    /// Opens it, or closes it if it shows.
    Toggle,
    /// Opens it.
    Open,
    /// Closes it.
    Close,
}

/// `assistant`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Assistant {
    /// `listen start | stop | toggle | cancel`.
    Listen(Listen),
    /// `status`.
    Status,
}

/// Listening for the assistant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Listen {
    /// Starts listening.
    Start,
    /// Stops, and asks what was heard.
    Stop,
    /// Starts or stops.
    Toggle,
    /// Stops, and forgets what was heard.
    Cancel,
}

/// `osd`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Osd {
    /// `volume up|down [fine]`.
    Volume {
        /// Louder.
        up: bool,
        /// A small step.
        fine: bool,
    },
    /// `volume mute`.
    Mute,
    /// `mic mute`.
    MicMute,
    /// `brightness up|down [fine]`.
    Brightness {
        /// Brighter.
        up: bool,
        /// A small step.
        fine: bool,
    },
    /// `brightness dim <percent>`.
    Dim(u8),
    /// `brightness restore`.
    Restore,
    /// `kbd-light up | down | cycle`.
    KeyboardLight(Light),
    /// `airplane toggle`.
    Airplane,
    /// `profile cycle`.
    Profile,
    /// `show <what>`: shows a level without changing it.
    Show(String),
}

/// The keyboard light.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Light {
    /// Brighter.
    Up,
    /// Dimmer.
    Down,
    /// The next level round.
    Cycle,
}

/// `widgets`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Widgets {
    /// Every widget, a line each.
    List,
    /// `add <kind> [size] [output]`.
    Add {
        /// The kind, as `cpu`.
        kind: String,
        /// Its size, as `medium`; the kind's own when none.
        size: Option<String>,
        /// The monitor, by name; the focused one when none.
        output: Option<String>,
    },
    /// `remove <id>`.
    Remove(u32),
    /// `reset-layout [output]`.
    ResetLayout(Option<String>),
    /// `edit [output]`.
    Edit(Option<String>),
    /// `done`: leaves edit mode.
    Done,
}

/// `status-bar`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusBar {
    /// `show | hide | toggle <indicator>`.
    Indicator {
        /// The indicator, as `battery`.
        indicator: String,
        /// Shown, hidden, or the other way.
        shown: Switch,
    },
    /// `move <item> <index>`.
    Move {
        /// The item, as `clock`.
        item: String,
        /// Its place, from 0.
        index: usize,
    },
    /// `reset-order`.
    ResetOrder,
    /// `tray [list] | activate <id> | menu <id>`.
    Tray(Listed),
    /// `apps list | activate <id>`.
    Apps(Listed),
    /// `activities list | end <key> | press <key> <button>`.
    Activities(Activities),
}

/// Listing the items of one kind, or acting on one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Listed {
    /// Every one, a line each.
    List,
    /// Activates the one with this id, as a click would.
    Activate(String),
    /// Opens its menu.
    Menu(String),
}

/// `status-bar activities`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Activities {
    /// Each going on, a line each.
    List,
    /// Ends the one with this key.
    End(String),
    /// Presses one of its buttons, as a click would.
    Press {
        /// The activity, as `bus.4`.
        key: String,
        /// The button's id.
        button: String,
    },
}

/// `night-light`.
#[derive(Clone, Debug, PartialEq)]
pub enum NightLight {
    /// `state` (or nothing): `on 4000 manual`.
    State,
    /// Turns it on.
    On,
    /// Turns it off.
    Off,
    /// Turns it the other way.
    Toggle,
    /// `temperature <kelvin>`.
    Temperature(u32),
    /// `schedule manual | sun <lat> <lon> | hours <HH:MM> <HH:MM>`.
    Schedule(Vec<String>),
}

/// `passwords`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Passwords {
    /// Offers the focused field's logins.
    Suggest,
    /// Whether the vault is set up, locked and synced.
    Status,
    /// Unlocks the vault.
    Unlock,
    /// Signs in to the server.
    SignIn,
    /// Registers this device with the server.
    Register,
    /// Syncs with the server.
    Sync,
    /// Locks the vault.
    Lock,
    /// `configure <server> <email> [forget]`.
    Configure {
        /// The server, `https://`.
        server: String,
        /// The account's email.
        email: String,
        /// Forgets what was kept for the old account.
        forget: bool,
    },
    /// `lock-timeout <seconds>`.
    LockTimeout(u64),
}

/// The verbs the shell answers, as their names.
pub const VERBS: [&str; 13] = [
    "arm",
    "open",
    "close",
    "notifications",
    "quick-settings",
    "launcher",
    "assistant",
    "osd",
    "widgets",
    "status-bar",
    "night-light",
    "passwords",
    "alert-test",
];

impl ShellCommand {
    /// Reads `verb` and its arguments; what it can't read is answered with
    /// the verb's usage.
    pub fn parse(verb: &str, args: &[String]) -> Result<Self, String> {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let usage = |text: &str| Err(format!("usage: {verb} {text}"));
        let owned = |s: &&str| s.to_string();
        Ok(match verb {
            "arm" => Self::Arm(args.first().map_or("file", |n| n).to_string()),
            "open" => Self::OpenMenu {
                name: args.first().map_or("file", |n| n).to_string(),
                x: args.get(1).and_then(|s| s.parse().ok()).unwrap_or(240),
                y: args.get(2).and_then(|s| s.parse().ok()).unwrap_or(180),
            },
            "close" => Self::CloseMenu,
            "notifications" => Self::Notifications(match args.as_slice() {
                [] | ["toggle"] => Notifications::Toggle,
                ["clear"] => Notifications::Clear,
                ["dnd"] | ["dnd", "toggle"] => Notifications::DoNotDisturb(Switch::Toggle),
                ["dnd", "on"] => Notifications::DoNotDisturb(Switch::On),
                ["dnd", "off"] => Notifications::DoNotDisturb(Switch::Off),
                _ => return usage("[toggle | clear | dnd [on|off|toggle]]"),
            }),
            "quick-settings" => Self::QuickSettings(match args.as_slice() {
                [] | ["toggle"] => QuickSettings::Toggle,
                ["open"] => QuickSettings::Open,
                ["open", page] => QuickSettings::OpenPage(owned(page)),
                ["toggle", page] => QuickSettings::TogglePage(owned(page)),
                ["close"] => QuickSettings::Close,
                ["edit"] => QuickSettings::Edit,
                ["customize"] => QuickSettings::Customize,
                ["controls"] => QuickSettings::Controls,
                ["show", control] => QuickSettings::Show(owned(control)),
                ["hide", control] => QuickSettings::Hide(owned(control)),
                ["reset-layout"] => QuickSettings::ResetLayout,
                _ => {
                    return usage(
                        "[toggle [<page>] | open [<page>] | close | edit | customize | controls | (show | hide) <control> | reset-layout]",
                    );
                }
            }),
            "launcher" => {
                const USAGE: &str =
                    "[toggle | open | close] [apps | files | actions | clipboard | passwords]";
                let (action, mode) = match args.as_slice() {
                    [] => ("toggle", None),
                    [action] => (*action, None),
                    [action, mode] => (*action, Some(mode.to_string())),
                    _ => return usage(USAGE),
                };
                let action = match action {
                    "toggle" => LauncherAction::Toggle,
                    "open" => LauncherAction::Open,
                    "close" if mode.is_none() => LauncherAction::Close,
                    _ => return usage(USAGE),
                };
                Self::Launcher(Launcher { action, mode })
            }
            "assistant" => Self::Assistant(match args.as_slice() {
                ["listen", "start"] => Assistant::Listen(Listen::Start),
                ["listen", "stop"] => Assistant::Listen(Listen::Stop),
                ["listen", "toggle"] => Assistant::Listen(Listen::Toggle),
                ["listen", "cancel"] => Assistant::Listen(Listen::Cancel),
                ["status"] => Assistant::Status,
                _ => return usage("listen start|stop|toggle|cancel | status"),
            }),
            "osd" => {
                let fine = args.last() == Some(&"fine");
                Self::Osd(match args.as_slice() {
                    ["volume", "up", ..] => Osd::Volume { up: true, fine },
                    ["volume", "down", ..] => Osd::Volume { up: false, fine },
                    ["volume", "mute", ..] => Osd::Mute,
                    ["mic", "mute"] => Osd::MicMute,
                    ["brightness", "up", ..] => Osd::Brightness { up: true, fine },
                    ["brightness", "down", ..] => Osd::Brightness { up: false, fine },
                    ["brightness", "dim", percent] => match percent.parse() {
                        Ok(percent) => Osd::Dim(percent),
                        Err(_) => return usage(OSD_USAGE),
                    },
                    ["brightness", "restore"] => Osd::Restore,
                    ["kbd-light", "up"] => Osd::KeyboardLight(Light::Up),
                    ["kbd-light", "down"] => Osd::KeyboardLight(Light::Down),
                    ["kbd-light", "cycle"] => Osd::KeyboardLight(Light::Cycle),
                    ["airplane", "toggle"] => Osd::Airplane,
                    ["profile", "cycle"] => Osd::Profile,
                    ["show", what] => Osd::Show(owned(what)),
                    _ => return usage(OSD_USAGE),
                })
            }
            "widgets" => {
                const USAGE: &str = "list | add <kind> [size] [output] | remove <id> | reset-layout [output] | edit [output] | done";
                let at = |i: usize| args.get(i).map(owned);
                Self::Widgets(match args.first().copied() {
                    Some("list") => Widgets::List,
                    Some("add") => match at(1) {
                        Some(kind) => Widgets::Add {
                            kind,
                            size: at(2),
                            output: at(3),
                        },
                        None => return usage(USAGE),
                    },
                    Some("remove") => match args.get(1).and_then(|id| id.parse().ok()) {
                        Some(id) => Widgets::Remove(id),
                        None => return Err(String::from("error: widgets remove <id>")),
                    },
                    Some("reset-layout") => Widgets::ResetLayout(at(1)),
                    Some("edit") => Widgets::Edit(at(1)),
                    Some("done") => Widgets::Done,
                    _ => return usage(USAGE),
                })
            }
            "status-bar" => Self::StatusBar(match args.as_slice() {
                ["move", item, index] => match index.parse() {
                    Ok(index) => StatusBar::Move {
                        item: owned(item),
                        index,
                    },
                    Err(_) => return Err(format!("not an index: {index}")),
                },
                ["reset-order"] => StatusBar::ResetOrder,
                ["tray"] | ["tray", "list"] => StatusBar::Tray(Listed::List),
                ["tray", "activate", id] => StatusBar::Tray(Listed::Activate(owned(id))),
                ["tray", "menu", id] => StatusBar::Tray(Listed::Menu(owned(id))),
                ["tray", ..] => return usage("tray (list | activate <id> | menu <id>)"),
                ["apps", "list"] => StatusBar::Apps(Listed::List),
                ["apps", "activate", id] => StatusBar::Apps(Listed::Activate(owned(id))),
                ["apps", ..] => return usage("apps (list | activate <id>)"),
                ["activities", "list"] => StatusBar::Activities(Activities::List),
                ["activities", "end", key] => StatusBar::Activities(Activities::End(owned(key))),
                ["activities", "press", key, button] => StatusBar::Activities(Activities::Press {
                    key: owned(key),
                    button: owned(button),
                }),
                ["activities", ..] => {
                    return usage(
                        "activities (list | end bus.<handle> | press bus.<handle> <button>)",
                    );
                }
                [action @ ("show" | "hide" | "toggle"), indicator] => StatusBar::Indicator {
                    indicator: owned(indicator),
                    shown: match *action {
                        "show" => Switch::On,
                        "hide" => Switch::Off,
                        _ => Switch::Toggle,
                    },
                },
                _ => {
                    return usage(
                        "(show | hide | toggle) <indicator> | move <item> <index> | reset-order | tray … | apps … | activities …",
                    );
                }
            }),
            "night-light" => Self::NightLight(match args.as_slice() {
                [] | ["state"] => NightLight::State,
                ["on"] => NightLight::On,
                ["off"] => NightLight::Off,
                ["toggle"] => NightLight::Toggle,
                ["temperature", kelvin] => match kelvin.parse() {
                    Ok(kelvin) => NightLight::Temperature(kelvin),
                    Err(_) => return Err(format!("not a temperature: {kelvin}")),
                },
                ["schedule", rest @ ..] => NightLight::Schedule(rest.iter().map(owned).collect()),
                _ => return usage("state | on | off | toggle | temperature <K> | schedule …"),
            }),
            "passwords" => Self::Passwords(match args.as_slice() {
                ["suggest"] => Passwords::Suggest,
                ["status"] => Passwords::Status,
                ["unlock"] => Passwords::Unlock,
                ["sign-in"] => Passwords::SignIn,
                ["register"] => Passwords::Register,
                ["sync"] => Passwords::Sync,
                ["lock"] => Passwords::Lock,
                ["configure", server, email, rest @ ..] => Passwords::Configure {
                    server: owned(server),
                    email: owned(email),
                    forget: rest.first() == Some(&"forget"),
                },
                ["lock-timeout", seconds] => match seconds.parse() {
                    Ok(seconds) => Passwords::LockTimeout(seconds),
                    Err(_) => return Err(String::from("error: seconds")),
                },
                _ => {
                    return usage(
                        "[suggest | status | unlock | sign-in | register | sync | lock | configure <server> <email> [forget] | lock-timeout <seconds>]",
                    );
                }
            }),
            "alert-test" => Self::AlertTest {
                stacked: args.first() == Some(&"stacked"),
            },
            _ => return Err(format!("no handler named {verb}")),
        })
    }

    /// The verb and its arguments, a line each, as the shell reads them.
    pub fn to_lines(&self) -> (&'static str, Vec<String>) {
        let words = |words: &[&str]| words.iter().map(|w| w.to_string()).collect::<Vec<_>>();
        let switch = |on: Switch| match on {
            Switch::On => "on",
            Switch::Off => "off",
            Switch::Toggle => "toggle",
        };
        match self {
            Self::Arm(name) => ("arm", vec![name.clone()]),
            Self::OpenMenu { name, x, y } => {
                ("open", vec![name.clone(), x.to_string(), y.to_string()])
            }
            Self::CloseMenu => ("close", Vec::new()),
            Self::Notifications(n) => (
                "notifications",
                match n {
                    Notifications::Toggle => words(&["toggle"]),
                    Notifications::Clear => words(&["clear"]),
                    Notifications::DoNotDisturb(on) => words(&["dnd", switch(*on)]),
                },
            ),
            Self::QuickSettings(q) => (
                "quick-settings",
                match q {
                    QuickSettings::Toggle => words(&["toggle"]),
                    QuickSettings::Open => words(&["open"]),
                    QuickSettings::OpenPage(page) => words(&["open", page]),
                    QuickSettings::TogglePage(page) => words(&["toggle", page]),
                    QuickSettings::Close => words(&["close"]),
                    QuickSettings::Edit => words(&["edit"]),
                    QuickSettings::Customize => words(&["customize"]),
                    QuickSettings::Controls => words(&["controls"]),
                    QuickSettings::Show(control) => words(&["show", control]),
                    QuickSettings::Hide(control) => words(&["hide", control]),
                    QuickSettings::ResetLayout => words(&["reset-layout"]),
                },
            ),
            Self::Launcher(Launcher { action, mode }) => {
                let mut lines = words(&[match action {
                    LauncherAction::Toggle => "toggle",
                    LauncherAction::Open => "open",
                    LauncherAction::Close => "close",
                }]);
                lines.extend(mode.clone());
                ("launcher", lines)
            }
            Self::Assistant(a) => (
                "assistant",
                match a {
                    Assistant::Listen(listen) => words(&[
                        "listen",
                        match listen {
                            Listen::Start => "start",
                            Listen::Stop => "stop",
                            Listen::Toggle => "toggle",
                            Listen::Cancel => "cancel",
                        },
                    ]),
                    Assistant::Status => words(&["status"]),
                },
            ),
            Self::Osd(o) => {
                let fine = |mut lines: Vec<String>, fine: bool| {
                    if fine {
                        lines.push(String::from("fine"));
                    }
                    lines
                };
                let way = |up: bool| if up { "up" } else { "down" };
                (
                    "osd",
                    match o {
                        Osd::Volume { up, fine: f } => fine(words(&["volume", way(*up)]), *f),
                        Osd::Mute => words(&["volume", "mute"]),
                        Osd::MicMute => words(&["mic", "mute"]),
                        Osd::Brightness { up, fine: f } => {
                            fine(words(&["brightness", way(*up)]), *f)
                        }
                        Osd::Dim(percent) => words(&["brightness", "dim", &percent.to_string()]),
                        Osd::Restore => words(&["brightness", "restore"]),
                        Osd::KeyboardLight(light) => words(&[
                            "kbd-light",
                            match light {
                                Light::Up => "up",
                                Light::Down => "down",
                                Light::Cycle => "cycle",
                            },
                        ]),
                        Osd::Airplane => words(&["airplane", "toggle"]),
                        Osd::Profile => words(&["profile", "cycle"]),
                        Osd::Show(what) => words(&["show", what]),
                    },
                )
            }
            Self::Widgets(w) => {
                let with = |verb: &str, rest: &[&Option<String>]| {
                    let mut lines = words(&[verb]);
                    lines.extend(rest.iter().filter_map(|s| (*s).clone()));
                    lines
                };
                (
                    "widgets",
                    match w {
                        Widgets::List => words(&["list"]),
                        Widgets::Add { kind, size, output } => {
                            let mut lines = words(&["add", kind]);
                            lines.extend(size.clone());
                            lines.extend(output.clone());
                            lines
                        }
                        Widgets::Remove(id) => words(&["remove", &id.to_string()]),
                        Widgets::ResetLayout(output) => with("reset-layout", &[output]),
                        Widgets::Edit(output) => with("edit", &[output]),
                        Widgets::Done => words(&["done"]),
                    },
                )
            }
            Self::StatusBar(s) => {
                let listed = |kind: &str, listed: &Listed| match listed {
                    Listed::List => words(&[kind, "list"]),
                    Listed::Activate(id) => words(&[kind, "activate", id]),
                    Listed::Menu(id) => words(&[kind, "menu", id]),
                };
                (
                    "status-bar",
                    match s {
                        StatusBar::Indicator { indicator, shown } => words(&[
                            match shown {
                                Switch::On => "show",
                                Switch::Off => "hide",
                                Switch::Toggle => "toggle",
                            },
                            indicator,
                        ]),
                        StatusBar::Move { item, index } => {
                            words(&["move", item, &index.to_string()])
                        }
                        StatusBar::ResetOrder => words(&["reset-order"]),
                        StatusBar::Tray(l) => listed("tray", l),
                        StatusBar::Apps(l) => listed("apps", l),
                        StatusBar::Activities(a) => match a {
                            Activities::List => words(&["activities", "list"]),
                            Activities::End(key) => words(&["activities", "end", key]),
                            Activities::Press { key, button } => {
                                words(&["activities", "press", key, button])
                            }
                        },
                    },
                )
            }
            Self::NightLight(n) => (
                "night-light",
                match n {
                    NightLight::State => words(&["state"]),
                    NightLight::On => words(&["on"]),
                    NightLight::Off => words(&["off"]),
                    NightLight::Toggle => words(&["toggle"]),
                    NightLight::Temperature(kelvin) => words(&["temperature", &kelvin.to_string()]),
                    NightLight::Schedule(rest) => {
                        let mut lines = words(&["schedule"]);
                        lines.extend(rest.iter().cloned());
                        lines
                    }
                },
            ),
            Self::Passwords(p) => (
                "passwords",
                match p {
                    Passwords::Suggest => words(&["suggest"]),
                    Passwords::Status => words(&["status"]),
                    Passwords::Unlock => words(&["unlock"]),
                    Passwords::SignIn => words(&["sign-in"]),
                    Passwords::Register => words(&["register"]),
                    Passwords::Sync => words(&["sync"]),
                    Passwords::Lock => words(&["lock"]),
                    Passwords::Configure {
                        server,
                        email,
                        forget,
                    } => {
                        let mut lines = words(&["configure", server, email]);
                        if *forget {
                            lines.push(String::from("forget"));
                        }
                        lines
                    }
                    Passwords::LockTimeout(seconds) => {
                        words(&["lock-timeout", &seconds.to_string()])
                    }
                },
            ),
            Self::AlertTest { stacked } => (
                "alert-test",
                if *stacked {
                    words(&["stacked"])
                } else {
                    Vec::new()
                },
            ),
        }
    }

    /// Sends this to the running shell and gives its answer.
    pub fn send(&self) -> io::Result<String> {
        let (verb, args) = self.to_lines();
        amane::ipc_call(SHELL, verb, &args)
    }

    /// [`send`](Self::send), off the UI thread; a failure is logged.
    pub fn tell(self) {
        std::thread::spawn(move || {
            if let Err(error) = self.send() {
                let (verb, _) = self.to_lines();
                log::warn!("couldn't tell the shell {verb}: {error}");
            }
        });
    }
}

const OSD_USAGE: &str = "volume up|down|mute [fine] | mic mute | brightness up|down [fine] | brightness dim <percent> | brightness restore | kbd-light up|down|cycle | airplane toggle | profile cycle | show <what>";

#[cfg(test)]
mod tests {
    use super::*;

    /// Lines as scripts and keybinds send them today, each read and written
    /// back the same.
    const LINES: &[&[&str]] = &[
        &["arm", "file"],
        &["open", "file", "240", "180"],
        &["close"],
        &["notifications", "toggle"],
        &["notifications", "clear"],
        &["notifications", "dnd", "toggle"],
        &["notifications", "dnd", "on"],
        &["quick-settings", "toggle"],
        &["quick-settings", "open"],
        &["quick-settings", "open", "wifi"],
        &["quick-settings", "toggle", "bluetooth"],
        &["quick-settings", "controls"],
        &["quick-settings", "show", "vpn"],
        &["quick-settings", "hide", "lock"],
        &["quick-settings", "reset-layout"],
        &["quick-settings", "customize"],
        &["launcher", "toggle"],
        &["launcher", "open", "vault"],
        &["launcher", "toggle", "clipboard"],
        &["launcher", "close"],
        &["assistant", "listen", "toggle"],
        &["assistant", "status"],
        &["osd", "volume", "up"],
        &["osd", "volume", "down", "fine"],
        &["osd", "volume", "mute"],
        &["osd", "mic", "mute"],
        &["osd", "brightness", "up", "fine"],
        &["osd", "brightness", "dim", "30"],
        &["osd", "brightness", "restore"],
        &["osd", "kbd-light", "cycle"],
        &["osd", "airplane", "toggle"],
        &["osd", "profile", "cycle"],
        &["osd", "show", "volume"],
        &["widgets", "list"],
        &["widgets", "add", "cpu", "medium", "eDP-1"],
        &["widgets", "add", "clock"],
        &["widgets", "remove", "7"],
        &["widgets", "reset-layout"],
        &["widgets", "edit", "eDP-1"],
        &["widgets", "done"],
        &["status-bar", "hide", "battery"],
        &["status-bar", "toggle", "volume"],
        &["status-bar", "move", "clock", "3"],
        &["status-bar", "reset-order"],
        &["status-bar", "tray", "list"],
        &["status-bar", "tray", "menu", "nm-applet"],
        &["status-bar", "apps", "activate", "org.amane.drop"],
        &["status-bar", "activities", "end", "bus.4"],
        &["status-bar", "activities", "press", "bus.4", "stop"],
        &["night-light", "state"],
        &["night-light", "on"],
        &["night-light", "temperature", "3200"],
        &["night-light", "schedule", "hours", "21:00", "07:00"],
        &["night-light", "schedule", "manual"],
        &["passwords", "suggest"],
        &["passwords", "sign-in"],
        &[
            "passwords",
            "configure",
            "https://vault.example",
            "a@b.c",
            "forget",
        ],
        &["passwords", "lock-timeout", "900"],
        &["alert-test", "stacked"],
    ];

    #[test]
    fn todays_lines_read_and_write_back_the_same() {
        for line in LINES {
            let (verb, args) = (
                line[0],
                line[1..].iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            );
            let command =
                ShellCommand::parse(verb, &args).unwrap_or_else(|e| panic!("{line:?}: {e}"));
            let (back, written) = command.to_lines();
            assert_eq!(
                (back, written.as_slice()),
                (verb, args.as_slice()),
                "{line:?}"
            );
            assert!(VERBS.contains(&verb));
        }
    }

    #[test]
    fn the_short_forms_mean_what_they_did() {
        let parse = |verb: &str, args: &[&str]| {
            ShellCommand::parse(
                verb,
                &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            )
        };
        assert_eq!(
            parse("notifications", &[]),
            Ok(ShellCommand::Notifications(Notifications::Toggle))
        );
        assert_eq!(
            parse("notifications", &["dnd"]),
            Ok(ShellCommand::Notifications(Notifications::DoNotDisturb(
                Switch::Toggle
            )))
        );
        assert_eq!(
            parse("night-light", &[]),
            Ok(ShellCommand::NightLight(NightLight::State))
        );
        assert_eq!(
            parse("arm", &[]),
            Ok(ShellCommand::Arm(String::from("file")))
        );
        assert_eq!(
            parse("status-bar", &["tray"]),
            Ok(ShellCommand::StatusBar(StatusBar::Tray(Listed::List)))
        );
        assert!(parse("launcher", &["close", "apps"]).is_err());
        assert!(parse("osd", &["volume"]).is_err());
        assert!(parse("teleport", &[]).is_err());
    }
}
