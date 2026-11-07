//! Passwords: the vault's logins, as results, and what picking one does.

use super::*;

impl Provider for Passwords {
    fn id(&self) -> &'static str {
        "passwords"
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Vault, Mode::Home]
    }

    fn find(&self, query: &Query, out: &mut Vec<Candidate>) {
        use crate::services::vault::deliver::{self, Give};
        use crate::services::vault::{Status, Vault};
        use amane::Service;

        let needle = match query.mode {
            Mode::Vault => query.needle.clone(),
            Mode::Home if amane_kit::settings::passwords().show_in_spotlight => {
                match query.needle.strip_prefix(PASSWORDS_PREFIX) {
                    Some(needle) => needle.trim().to_string(),
                    None => return,
                }
            }
            _ => return,
        };

        // nothing of the vault while the screen is shared, as with the popup
        if amane_kit::settings::passwords().hide_while_sharing
            && crate::services::desktop::privacy::Privacy::read().screen()
        {
            out.push(Candidate {
                entry: Entry::new(
                    format!("{}sharing", entries::VAULT_KEY),
                    "Passwords are hidden while the screen is shared",
                    Icon::Glyph(ICON_LOCK),
                    Action::Hint,
                ),
                matched: 1.0,
                group: Group::Listed,
            });
            return;
        }

        // rbw says nothing when the vault locks, so it's looked at as the passwords show
        Vault::look_soon();
        let vault = Vault::read();

        let mut push = |entry: Entry| {
            out.push(Candidate {
                entry,
                matched: 1.0,
                group: Group::Listed,
            });
        };

        let only = |key: &str, title: &str, run: fn()| {
            Entry::new(
                format!("{}{key}", entries::VAULT_KEY),
                title,
                Icon::Glyph(ICON_LOCK),
                Action::Run(std::rc::Rc::new(run)),
            )
        };

        match vault.status() {
            Status::Unlocked => {}
            Status::Locked => return push(only("unlock", "Unlock Bitwarden…", Vault::unlock)),
            Status::LoggedOut => {
                return push(only("sign-in", "Sign in to Bitwarden…", Vault::sign_in));
            }
            Status::NotConfigured => {
                return push(
                    only("set-up", "Set up Passwords in Shell Settings…", || {
                        crate::services::run(
                            "amane-settings",
                            vec![String::from("--page"), String::from("passwords")],
                        );
                    })
                    .subtitle("Bitwarden through rbw"),
                );
            }
        }

        let entries = vault.entries();
        drop(vault);

        let found: Vec<&crate::services::vault::Entry> = entries
            .iter()
            .filter(|entry| entry.is_login())
            .filter(|entry| {
                needle.is_empty()
                    || entry.name.to_lowercase().contains(&needle)
                    || entry
                        .user
                        .as_deref()
                        .is_some_and(|user| user.to_lowercase().contains(&needle))
                    || entry
                        .uris
                        .iter()
                        .any(|uri| uri.to_lowercase().contains(&needle))
            })
            .take(MAX_ROWS)
            .collect();

        for (at, entry) in found.iter().enumerate() {
            let shown = |what: &str| -> String {
                [entry.user.as_deref(), entry.folder.as_deref(), Some(what)]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · ")
            };

            let fills = (*entry).clone();
            push(
                Entry::new(
                    format!("{}{}", entries::VAULT_KEY, entry.id),
                    entry.name.clone(),
                    Icon::Glyph(ICON_LOCK),
                    Action::Run(std::rc::Rc::new(move || {
                        deliver::fill_behind(fills.clone(), Give::Password)
                    })),
                )
                .subtitle(shown("fills the password")),
            );

            // the best match also offers its other parts, in the Passwords mode
            if at == 0 && query.mode == Mode::Vault && !needle.is_empty() {
                let more: [(&str, Give, bool); 3] = [
                    ("Fill username", Give::Username, false),
                    ("Fill two-factor code", Give::Code, false),
                    ("Copy password", Give::Password, true),
                ];

                for (title, give, copies) in more {
                    if give == Give::Username && entry.user.is_none() {
                        continue;
                    }

                    let picked = (*entry).clone();
                    push(
                        Entry::new(
                            format!("{}{}.{title}", entries::VAULT_KEY, entry.id),
                            format!("{title} · {}", entry.name),
                            Icon::Glyph(if copies { ICON_CONTENT_COPY } else { ICON_LOCK }),
                            if copies {
                                Action::RunFirst(std::rc::Rc::new(move || {
                                    deliver::copy_entry(&picked, give)
                                }))
                            } else {
                                Action::Run(std::rc::Rc::new(move || {
                                    deliver::fill_behind(picked.clone(), give)
                                }))
                            },
                        )
                        .subtitle(if copies {
                            "Marked as a password, cleared after 30 seconds"
                        } else {
                            "Into the window behind"
                        }),
                    );
                }
            }
        }
    }
}
