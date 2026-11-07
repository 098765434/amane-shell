//! Passwords: Bitwarden through rbw, the shell's password fill, and the apps it remembers.
//!
//! The vault's commands run in the shell (`passwords …` over its IPC), so an unlock is always
//! the shell's own rbw command and its dialog may ask for the master password. This page only
//! reads what the shell says, never a secret. The fill settings are
//! `~/.config/amane-shell/passwords.json`, readable only by you.

use amane_kit::service::ServiceExt as _;
use amane_kit::shell_ipc::{Passwords as Ask, ShellCommand};
use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use amane::{Center, Column, Row, Service, TextInput, Widget};
use amane_kit::app::{Dialog, field_box, text_field};
use amane_kit::buttons::{IconButton, PushButton, Style};
use amane_kit::dropdown::Dropdown;
use amane_kit::form::FormRow;
use amane_kit::icons::ICON_CLOSE;
use amane_kit::layout::boxed;
use amane_kit::settings::{change_passwords, passwords};

use crate::{group, groups, problem_row, switch_row};

const SERVER: &str = "settings.passwords.server";
const EMAIL: &str = "settings.passwords.email";

const FIELD_WIDTH: f32 = 260.0;

/// The auto-lock choices, in minutes.
const LOCK_MINUTES: [u64; 5] = [5, 10, 15, 30, 60];

/// What the shell says of the vault, refreshed every two seconds while the app runs.
#[derive(Default, PartialEq)]
pub struct VaultStatus {
    lines: HashMap<String, String>,
    reached: bool,
}

impl Service for VaultStatus {
    fn new() -> Self {
        Self::default()
    }

    fn listen() {
        loop {
            let answer = shell(Ask::Status);

            let next = match answer {
                Some(text) => VaultStatus {
                    lines: text
                        .lines()
                        .filter_map(|line| line.split_once('='))
                        .map(|(key, value)| (key.to_string(), value.to_string()))
                        .collect(),
                    reached: true,
                },
                None => VaultStatus::default(),
            };

            VaultStatus::replace(next);

            thread::sleep(Duration::from_secs(2));
        }
    }
}

impl VaultStatus {
    fn get(&self, key: &str) -> &str {
        self.lines.get(key).map_or("", String::as_str)
    }
}

// one call to the shell's passwords IPC
fn shell(ask: Ask) -> Option<String> {
    match ShellCommand::Passwords(ask).send() {
        Ok(answer) => Some(answer),
        Err(error) => {
            log::warn!("passwords: {error}");
            None
        }
    }
}

// a command, off the UI thread, then a fresh look
fn run(ask: Ask) {
    thread::spawn(move || {
        shell(ask);
    });
}

pub fn page() -> Column {
    let status = VaultStatus::read();

    let mut pages = vec![
        vault(&status),
        account(&status),
        security(&status),
        fill(&status),
    ];
    pages.extend(apps());

    groups(pages)
}

fn vault(status: &VaultStatus) -> amane_kit::form::Group {
    if !status.reached {
        return group().header("Bitwarden").row(
            FormRow::new("settings.passwords.no-shell", "The shell isn't answering")
                .subtitle("Passwords work through amane-shell; start it, then come back here."),
        );
    }

    let state = status.get("status");
    let busy = status.get("busy");

    let (title, subtitle) = match state {
        "Unlocked" => (
            String::from("Unlocked"),
            format!("{} logins{}", status.get("entries"), synced(status)),
        ),
        "Locked" => (
            String::from("Locked"),
            String::from("Unlocking asks for your master password in the shell's own dialog"),
        ),
        "LoggedOut" => (
            String::from("Not signed in"),
            String::from("Set the server and email below, then sign in"),
        ),
        _ => (
            String::from("Not set up"),
            String::from("Give your server and email below to begin"),
        ),
    };

    let subtitle = if busy.is_empty() {
        subtitle
    } else {
        format!("{busy}…")
    };

    let mut buttons: Vec<Box<dyn Widget>> = Vec::new();
    let mut button = |id: &str, label: &str, ask: Ask| {
        buttons.push(boxed(
            PushButton::new(format!("settings.passwords.{id}"), label)
                .disabled(!busy.is_empty())
                .on_press(move || run(ask.clone()))
                .build(),
        ));
    };

    match state {
        "Unlocked" => {
            button("sync", "Sync Now", Ask::Sync);
            button("lock", "Lock", Ask::Lock);
        }
        "Locked" => {
            button("sync", "Sync", Ask::Sync);
            button("unlock", "Unlock…", Ask::Unlock);
        }
        "LoggedOut" => button("sign-in", "Sign In…", Ask::SignIn),
        _ => {}
    }

    let mut shown = group().header("Bitwarden").row(
        FormRow::new("settings.passwords.state", &title)
            .subtitle(subtitle)
            .trailing(Row::new(buttons).gap(6.0).align(Center)),
    );

    let error = status.get("error");
    if !error.is_empty() {
        let mut row = problem_row(
            "settings.passwords.error",
            "The last command failed",
            error,
            "",
        );

        if error.contains("registering") {
            row = row.trailing(
                PushButton::new("settings.passwords.register", "Register…")
                    .on_press(|| run(Ask::Register))
                    .build(),
            );
        }

        shown = shown.row(row);
    }

    shown.footer(
        "rbw keeps the vault, encrypted, in ~/.cache/rbw, and holds it unlocked in its agent \
         until it locks. While unlocked, programs you run can ask the agent for passwords, as \
         with ssh-agent: keep the auto-lock short.",
    )
}

fn synced(status: &VaultStatus) -> String {
    let Ok(at) = status.get("synced").parse::<u64>() else {
        return String::new();
    };

    let elapsed = amane_kit::time::since_unix_s(at);
    if elapsed < Duration::from_secs(2 * 60 * 60) {
        format!(" · read {}", amane_kit::format::ago(elapsed))
    } else {
        String::new()
    }
}

fn account(status: &VaultStatus) -> amane_kit::form::Group {
    if status.reached {
        amane_kit::app::fill_once(SERVER, || status.get("server").to_string());
        amane_kit::app::fill_once(EMAIL, || status.get("email").to_string());
    }

    let had_account = !status.get("email").is_empty();

    let save = PushButton::new("settings.passwords.save", "Save")
        .style(Style::Filled)
        .on_press(move || save(had_account))
        .build();

    let mut shown = group()
        .header("Account")
        .row(
            FormRow::new("settings.passwords.server.row", "Server")
                .subtitle("Your Vaultwarden or Bitwarden address")
                .trailing(field_box(
                    SERVER,
                    text_field(SERVER, "https://vault.example.org"),
                    FIELD_WIDTH,
                )),
        )
        .row(
            FormRow::new("settings.passwords.email.row", "Email").trailing(field_box(
                EMAIL,
                text_field(EMAIL, "you@example.org"),
                FIELD_WIDTH,
            )),
        )
        .row(FormRow::new("settings.passwords.save.row", "").trailing(save));

    if status.reached && status.get("pinentry_ok") != "true" && had_account {
        shown = shown.row(problem_row(
            "settings.passwords.pinentry",
            "rbw asks for the password elsewhere",
            "Save once to have rbw ask in the shell's dialog (amane-pinentry).",
            "",
        ));
    }

    shown.footer("Only https servers, or http on this computer. The master password is never kept.")
}

fn save(had_account: bool) {
    let server = TextInput::text(SERVER)
        .trim()
        .trim_end_matches('/')
        .to_string();
    let email = TextInput::text(EMAIL).trim().to_string();

    let send = move |forget: bool| {
        let (server, email) = (server.clone(), email.clone());

        thread::spawn(move || {
            let answer = shell(Ask::Configure {
                server,
                email,
                forget,
            })
            .unwrap_or_default();

            if answer.starts_with("error") {
                log::warn!("passwords: {answer}");
            }
        });
    };

    let status = VaultStatus::read();
    let changed = status.get("server") != TextInput::text(SERVER).trim().trim_end_matches('/')
        || status.get("email") != TextInput::text(EMAIL).trim();
    drop(status);

    if had_account && changed {
        let keep = send.clone();
        Dialog::new("settings.passwords.change", "Change the account?")
            .message(
                "The vault copy for the old account is removed from this computer. You'll sign \
                 in to the new one.",
            )
            .button("Cancel", || {})
            .destructive("Change", move || keep(true))
            .open("main");
    } else {
        send(false);
    }
}

fn security(status: &VaultStatus) -> amane_kit::form::Group {
    let minutes = status.get("lock_timeout").parse::<u64>().unwrap_or(900) / 60;

    let at = LOCK_MINUTES
        .iter()
        .position(|choice| *choice >= minutes)
        .unwrap_or(LOCK_MINUTES.len() - 1);

    let lock = Dropdown::new(
        "settings.passwords.lock-after",
        LOCK_MINUTES
            .iter()
            .map(|minutes| format!("{minutes} minutes"))
            .collect(),
        at,
    )
    .width(130.0)
    .on_select(|index| {
        let seconds = LOCK_MINUTES[index] * 60;

        thread::spawn(move || {
            shell(Ask::LockTimeout(seconds));
        });
    });

    let settings = passwords();

    group()
        .header("Security")
        .row(
            FormRow::new("settings.passwords.lock-after.row", "Lock after")
                .subtitle("Unused this long, the vault locks")
                .trailing(lock.build()),
        )
        .row(switch_row(
            "settings.passwords.lock-with-screen",
            "Lock with the screen",
            settings.lock_with_screen,
            |on| change_passwords(|passwords| passwords.lock_with_screen = on),
        ))
        .footer("It also locks before the computer sleeps, when this is on.")
}

fn fill(status: &VaultStatus) -> amane_kit::form::Group {
    let settings = passwords().clone();

    let mut shown = group()
        .header("Fill")
        .row(switch_row(
            "settings.passwords.fill",
            "Fill passwords in apps",
            settings.fill_in_apps,
            |on| change_passwords(|passwords| passwords.fill_in_apps = on),
        ))
        .row(switch_row(
            "settings.passwords.usernames",
            "Suggest on username fields",
            settings.suggest_on_usernames,
            |on| change_passwords(|passwords| passwords.suggest_on_usernames = on),
        ))
        .row(switch_row(
            "settings.passwords.launcher",
            "Passwords in the Launcher",
            settings.show_in_launcher,
            |on| change_passwords(|passwords| passwords.show_in_launcher = on),
        ))
        .row(switch_row(
            "settings.passwords.sharing",
            "Hide while the screen is shared",
            settings.hide_while_sharing,
            |on| change_passwords(|passwords| passwords.hide_while_sharing = on),
        ));

    match status.get("input_method") {
        "unavailable" => {
            shown = shown.row(problem_row(
                "settings.passwords.ime",
                "Another input method has the keyboard",
                "fcitx5 or IBus is running, so the shell can't offer fill in apps. The \
                 launcher's passwords still work.",
                "",
            ));
        }
        "on" if status.get("secure_fill") != "true" => {
            shown = shown.row(problem_row(
                "settings.passwords.secure-fill",
                "niri can't fill fields safely",
                "This niri lacks niri-secure-fill-v1, so nothing is filled. Update niri.",
                "",
            ));
        }
        _ => {}
    }

    shown.footer(
        "A popup by an app's password field offers that app's entries; a click fills one in, \
         only if the same field still has the keyboard. Turning fill on or off takes effect \
         when the shell starts again. Browsers are left to the Bitwarden extension. Chromium \
         and Electron apps need --enable-wayland-ime --wayland-text-input-version=3. In \
         the launcher, type pw and a name.",
    )
}

fn apps() -> Vec<amane_kit::form::Group> {
    let settings = passwords().clone();

    let links: Vec<FormRow> = settings
        .app_links
        .iter()
        .enumerate()
        .map(|(index, link)| {
            let id = format!("settings.passwords.link.{index}");
            let (desktop, package, entry) = (
                link.desktop_id.clone(),
                link.package.clone(),
                link.entry_id.clone(),
            );

            FormRow::new(id.clone(), &link.entry_name)
                .subtitle(format!("{} · {}", link.desktop_id, link.package))
                .trailing(
                    IconButton::new(format!("{id}.forget"), ICON_CLOSE)
                        .style(Style::Destructive)
                        .on_press(move || {
                            change_passwords(|passwords| {
                                passwords.app_links.retain(|link| {
                                    !(link.desktop_id == desktop
                                        && link.package == package
                                        && link.entry_id == entry)
                                });
                            });
                        })
                        .build(),
                )
        })
        .collect();

    let excluded: Vec<FormRow> = settings
        .excluded_apps
        .iter()
        .map(|app| {
            let id = format!("settings.passwords.excluded.{app}");
            let gone = app.clone();

            FormRow::new(id.clone(), app).trailing(
                IconButton::new(format!("{id}.remove"), ICON_CLOSE)
                    .on_press(move || {
                        change_passwords(|passwords| {
                            passwords.excluded_apps.retain(|app| *app != gone)
                        });
                    })
                    .build(),
            )
        })
        .collect();

    let mut shown = group().header("Remembered for apps");

    if links.is_empty() {
        shown = shown.row(
            FormRow::new("settings.passwords.no-links", "None yet")
                .subtitle("An entry you fill into an app is remembered for it, if you let it"),
        );
    } else {
        shown = shown.rows(links);
    }

    let remembered = shown.footer(
        "Kept by the app's desktop entry and package, never by the name an app gives itself.",
    );

    let never = group()
        .header("Never offered in")
        .rows(excluded)
        .footer("Browsers by default: their Bitwarden extension checks the website itself.");

    vec![remembered, never]
}
