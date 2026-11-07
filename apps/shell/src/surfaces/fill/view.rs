//! How the fill popup draws: what it shows for the field, its header and its rows.

use super::*;

pub(super) fn plan(field: &FocusedField) -> Option<Shown> {
    let settings = settings::passwords();

    if !settings.fill_in_apps || !field.can_fill() || !field.active() || field.own() {
        return None;
    }

    let generation = field.generation();

    let (dismissed, forced) = with(|state| (state.dismissed, state.forced));
    if dismissed == Some(generation) {
        return None;
    }
    let forced = forced == Some(generation);

    let owner = field.owner()?.clone();
    if !owner.takes_text || settings.excludes(&owner.app_id) {
        return None;
    }

    if settings.hide_while_sharing && Privacy::read().screen() {
        return None;
    }

    let what = match field.purpose() {
        _ if field.is_password() => What::Password,
        FieldPurpose::Terminal => {
            if !forced {
                return None;
            }
            What::Password
        }
        FieldPurpose::Digits | FieldPurpose::Pin => What::Code,
        _ => What::Username,
    };

    let vault = Vault::read();
    let status = vault.status();

    // nothing set up: only when asked, so a fresh install isn't nagged
    if status == Status::NotConfigured && !forced {
        return None;
    }

    let identity = Identities::of(&owner);

    // a browser is many sites in one app: its Bitwarden extension knows the site, the shell can't
    if matches!(&identity, Some(Identity::Verified(app)) if app.browser) {
        return None;
    }

    let matches = match (&identity, status) {
        (Some(Identity::Verified(app)), Status::Unlocked) => {
            matching::matches(app, &owner.app_id, &vault.entries(), &settings)
        }
        _ => Vec::new(),
    };
    drop(vault);

    // a username or code field only with an entry known for this app, or when asked
    let strong = matches
        .iter()
        .any(|found| matches!(found.reason, Reason::Remembered | Reason::Named));

    match what {
        What::Password => {}
        What::Username if forced || (settings.suggest_on_usernames && strong) => {}
        What::Code if forced || (field.empty() == Some(true) && strong) => {}
        _ => return None,
    }

    Some(Shown {
        what,
        identity,
        status,
        matches,
        empty: field.empty(),
    })
}

pub(super) fn content(shown: &Shown, generation: u64) -> (Vec<Box<dyn Widget>>, f32) {
    let row_height = theme::menu_item_height();
    let mut rows: Vec<Box<dyn Widget>> = Vec::new();

    rows.push(boxed(header(shown, generation)));

    let verified = match &shown.identity {
        Some(Identity::Verified(app)) => Some(app.clone()),
        _ => None,
    };

    match shown.status {
        Status::Unlocked => {
            for found in shown.matches.iter().take(ROWS) {
                rows.push(boxed(entry_row(shown, found, verified.clone(), generation)));
            }
        }
        Status::Locked => rows.push(boxed(action_row(
            "fill.unlock",
            ICON_LOCK,
            "Unlock Bitwarden…",
            Vault::unlock,
        ))),
        Status::LoggedOut => rows.push(boxed(action_row(
            "fill.sign-in",
            ICON_LOCK,
            "Sign in to Bitwarden…",
            Vault::sign_in,
        ))),
        Status::NotConfigured => rows.push(boxed(action_row(
            "fill.set-up",
            ICON_LOCK,
            "Set up Passwords…",
            open_settings,
        ))),
    }

    rows.push(boxed(action_row(
        "fill.search",
        ICON_SEARCH,
        "Search vault…",
        search,
    )));

    let note = with(|state| state.note.clone()).filter(|(at, _)| *at == generation);
    if let Some((_, note)) = note {
        rows.push(boxed(note_row(&note)));
    }

    if shown.status == Status::Unlocked && verified.is_some() && !shown.matches.is_empty() {
        rows.push(boxed(remember_row(verified.as_ref())));
    }

    let height = rows.len() as f32 * row_height + px(8.0);

    (rows, height)
}

// who the field belongs to, as checked, and a close button
pub(super) fn header(shown: &Shown, generation: u64) -> Rectangle {
    let (text, warn) = match &shown.identity {
        Some(Identity::Verified(app)) => (format!("Passwords for {}", app.name), false),
        Some(Identity::Unverified(why)) => (format!("Unverified app · {}", why.describe()), true),
        None => (String::from("Checking the app…"), false),
    };

    let color = if warn {
        theme::error()
    } else {
        theme::label_secondary()
    };

    let mut parts: Vec<Box<dyn Widget>> = Vec::new();
    if warn {
        parts.push(boxed(icon(ICON_WARNING, px(13.0), color)));
    }
    parts.push(boxed(
        Text::new(text)
            .style(theme::subtitle_font())
            .color(color)
            .elide(),
    ));

    let close = interactive(
        Rectangle::new()
            .width(Size::Fixed(px(20.0)))
            .height(Size::Fixed(px(20.0)))
            .radius(px(10.0))
            .fill(hover("fill.close"))
            .align_child(Center, Center)
            .child(icon(ICON_CLOSE, px(13.0), theme::label_secondary())),
        String::from("fill.close"),
        false,
        press(move || {
            with(|state| state.dismissed = Some(generation));
            FillChanged::changed();
        }),
    );
    parts.push(boxed(close));

    Rectangle::new()
        .width(Size::Parent)
        .height(Size::Fixed(theme::menu_item_height()))
        .padding(pad(px(8.0), 0.0))
        .align_child(amane::Start, Center)
        .child(
            Row::new(parts)
                .gap(px(6.0))
                .align(Center)
                .width(Size::Parent),
        )
}

pub(super) fn entry_row(
    shown: &Shown,
    found: &Match,
    app: Option<Verified>,
    generation: u64,
) -> Rectangle {
    let entry = found.entry.clone();
    let what = shown.what;
    let key = format!("fill.entry.{}", entry.id);

    let confirming = with(|state| state.confirming.clone()) == Some((generation, entry.id.clone()));
    let has_text = shown.empty == Some(false);

    let label = if confirming {
        String::from("The field has text: fill anyway?")
    } else {
        match (&entry.user, what) {
            (Some(user), What::Password | What::Code) => format!("{} · {user}", entry.name),
            _ => entry.name.clone(),
        }
    };

    let detail = match what {
        What::Password => "password",
        What::Username => "username",
        What::Code => "code",
    };

    let parts: Vec<Box<dyn Widget>> = vec![
        boxed(
            Text::new(label)
                .style(theme::control_font())
                .color(theme::label_primary())
                .elide(),
        ),
        boxed(
            Text::new(detail)
                .style(theme::subtitle_font())
                .color(theme::label_tertiary()),
        ),
    ];

    let id = entry.id.clone();
    let remembered = found.reason == Reason::Remembered;

    interactive(
        Rectangle::new()
            .width(Size::Parent)
            .height(Size::Fixed(theme::menu_item_height()))
            .radius(px(6.0))
            .fill(hover(&key))
            .padding(pad(px(10.0), 0.0))
            .align_child(amane::Start, Center)
            .child(
                Row::new(parts)
                    .gap(px(8.0))
                    .align(Center)
                    .width(Size::Parent),
            ),
        key,
        false,
        press(move || {
            if has_text && !confirming {
                with(|state| state.confirming = Some((generation, id.clone())));
                FillChanged::changed();
                return;
            }

            fill(
                &entry,
                what,
                generation,
                app.clone().filter(|_| !remembered),
            );
        }),
    )
}

pub(super) fn action_row(key: &'static str, glyph: char, text: &str, run: fn()) -> Rectangle {
    interactive(
        Rectangle::new()
            .width(Size::Parent)
            .height(Size::Fixed(theme::menu_item_height()))
            .radius(px(6.0))
            .fill(hover(key))
            .padding(pad(px(10.0), 0.0))
            .align_child(amane::Start, Center)
            .child(
                Row::new(vec![
                    boxed(icon(glyph, px(15.0), theme::label_secondary())),
                    boxed(
                        Text::new(text.to_string())
                            .style(theme::control_font())
                            .color(theme::label_primary()),
                    ),
                ])
                .gap(px(8.0))
                .align(Center),
            ),
        String::from(key),
        false,
        press(run),
    )
}

pub(super) fn note_row(note: &str) -> Rectangle {
    Rectangle::new()
        .width(Size::Parent)
        .height(Size::Fixed(theme::menu_item_height()))
        .padding(pad(px(10.0), 0.0))
        .align_child(amane::Start, Center)
        .child(
            Text::new(note.to_string())
                .style(theme::subtitle_font())
                .color(theme::error())
                .elide(),
        )
}

// whether a fill is remembered for this app, a checkbox in words
pub(super) fn remember_row(app: Option<&Verified>) -> Rectangle {
    let on = with(|state| state.remember);
    let name = app.map_or("this app", |app| app.name.as_str());

    let text = format!(
        "{} Remember the choice for {name}",
        if on { "☑" } else { "☐" }
    );

    interactive(
        Rectangle::new()
            .width(Size::Parent)
            .height(Size::Fixed(theme::menu_item_height()))
            .radius(px(6.0))
            .fill(hover("fill.remember"))
            .padding(pad(px(10.0), 0.0))
            .align_child(amane::Start, Center)
            .child(
                Text::new(text)
                    .style(theme::subtitle_font())
                    .color(theme::label_secondary())
                    .elide(),
            ),
        String::from("fill.remember"),
        false,
        press(|| {
            with(|state| state.remember = !state.remember);
            FillChanged::changed();
        }),
    )
}

pub(super) fn hover(key: &str) -> Color {
    if hovered(key) {
        theme::hover_of(theme::label_primary())
    } else {
        Color::TRANSPARENT
    }
}

pub(super) fn press(run: impl Fn() + 'static) -> Actions {
    Actions {
        press: Some(std::rc::Rc::new(run)),
        ..Default::default()
    }
}
