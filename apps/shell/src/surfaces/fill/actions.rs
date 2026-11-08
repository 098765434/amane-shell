//! What the fill popup does: filling the field, searching the vault, Emaki Settings.

use super::*;

pub(super) fn search() {
    crate::surfaces::launcher::open(crate::surfaces::launcher::Mode::Vault);
}

pub(super) fn open_settings() {
    crate::services::run(
        "emaki-settings",
        vec![String::from("--page"), String::from("passwords")],
    );
}

pub(super) fn fill(
    entry: &rbw::Entry,
    what: What,
    generation: u64,
    remember_for: Option<Verified>,
) {
    if with(|state| std::mem::replace(&mut state.filling, true)) {
        return;
    }

    let remember = with(|state| state.remember);
    let entry = entry.clone();

    std::thread::spawn(move || {
        // locked since the last look: say so, rather than let rbw ask in a dialog not the shell's
        if !rbw::unlocked() {
            Vault::look_now();
            return done(
                generation,
                Err(String::from("The vault locked; unlock it first")),
            );
        }

        let secret = match what {
            What::Password => rbw::password(&entry.id),
            What::Code => rbw::totp(&entry.id),
            What::Username => entry
                .user
                .as_deref()
                .and_then(|user| Secret::from_output(user.as_bytes()))
                .ok_or(rbw::Error::NotFound),
        };

        let secret = match secret {
            Ok(secret) if !secret.is_empty() => secret,
            Ok(_) => {
                return done(
                    generation,
                    Err(String::from("That entry has nothing to fill")),
                );
            }
            Err(error) => return done(generation, Err(error.describe().to_string())),
        };

        FocusedField::fill(secret.into_text(), generation, move |result| match result {
            Ok(()) => {
                if remember && let Some(app) = remember_for {
                    settings::change_passwords(|passwords: &mut Passwords| {
                        passwords.remember(AppLink {
                            desktop_id: app.desktop_id.clone(),
                            package: app.package.clone(),
                            entry_id: entry.id.clone(),
                            entry_name: entry.name.clone(),
                        });
                    });
                }

                done(generation, Ok(()));
            }
            Err(error) => done(generation, Err(describe(error).to_string())),
        });
    });
}

pub(super) fn done(generation: u64, result: Result<(), String>) {
    with(|state| {
        state.filling = false;
        state.confirming = None;

        match result {
            // filled: the popup goes away for this field
            Ok(()) => {
                state.note = None;
                state.dismissed = Some(generation);
            }
            Err(note) => state.note = Some((generation, note)),
        }
    });

    FillChanged::changed();
}

pub(super) fn describe(error: FillError) -> &'static str {
    match error {
        FillError::Changed => "The field changed, so nothing was filled",
        FillError::NoTextInput => "This field takes no text from the shell",
        FillError::Locked => "The screen is locked",
        FillError::Expired => "Too long since the field was focused; click again",
        FillError::Used => "Already filled; click again to fill once more",
        FillError::TooSoon => "Too quick; click again",
        FillError::InvalidText => "That entry can't be typed into a field",
        FillError::Unsupported => "This compositor can't fill fields safely",
    }
}
