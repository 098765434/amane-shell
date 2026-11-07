//! What the activities are: other programs' over the bus, Now Playing, the timer, and what's
//! sharing the screen.

use super::*;

/// Every activity the bar may show now, most relevant first.
pub(super) fn gather() -> Vec<Shown> {
    let bar = status_bar();
    let mut shown: Vec<Shown> = [now_playing(), timer(), sharing()]
        .into_iter()
        .flatten()
        .chain(registry::list().into_iter().map(from_bus))
        .filter(|activity| bar.allows_activity(&activity.content.app))
        .collect();
    drop(bar);
    // stable, so ties keep the order they started in
    shown.sort_by(|a, b| b.content.relevance.total_cmp(&a.content.relevance));
    shown
}

pub(super) fn from_bus(live: Live) -> Shown {
    let now = Instant::now();
    let handle = live.handle;
    Shown {
        key: format!("bus.{handle}"),
        dim: live.leaving.is_some() || live.stale(now),
        content: live.content,
        act: Rc::new(move |button| registry::act(handle, button)),
        middle: None,
    }
}

/// What plays, or played in the last few minutes; with `media_always`,
/// "Not Playing" when nothing does.
pub(super) fn now_playing() -> Option<Shown> {
    let media = Media::read();
    let playing = media.playing();
    let title = media.title().to_string();
    let artist = media.artist().to_string();
    let player = media
        .active()
        .map(|player| player.identity().to_string())
        .unwrap_or_default();
    let art = crate::services::media_art::path(media.art_url());
    drop(media);

    let now = Instant::now();
    if playing {
        LAST_PLAYED.set(Some(now));
    }
    let lately = LAST_PLAYED
        .get()
        .is_some_and(|at| now.duration_since(at) < PAUSED_FOR);
    let active = !title.is_empty() && (playing || lately);
    if !active && !status_bar().media_always {
        return None;
    }
    if active && !playing {
        // gone once the pause has lasted long enough
        let left = LAST_PLAYED.get().map_or(PAUSED_FOR, |at| {
            PAUSED_FOR.saturating_sub(now.duration_since(at))
        });
        // once per wait, not once per frame drawn while paused
        tick_in(left);
    }

    let mut content = ActivityContent::new(
        NOW_PLAYING,
        art.map_or_else(
            || ItemIcon::Glyph(String::from("music")),
            |path| ItemIcon::File(PathBuf::from(path)),
        ),
        if active { &title } else { "Not Playing" },
    );
    content.subtitle = if active { artist } else { player.clone() };
    content.trailing = Trailing::Text(if active {
        title
    } else {
        String::from("Not Playing")
    });
    content.relevance = 0.3;
    if active {
        let play = if playing { "pause" } else { "play" };
        content.buttons = vec![
            glyph_button("previous", "skip-previous"),
            glyph_button("play-pause", play),
            glyph_button("next", "skip-next"),
        ];
    }
    let more = if player.is_empty() {
        String::from("Quick Settings")
    } else {
        format!("{player} in Quick Settings")
    };
    content.buttons.push(ActivityButton::new("more", &more));

    Some(Shown {
        key: String::from("now-playing"),
        content,
        dim: !playing,
        act: Rc::new(|button| match button {
            "previous" => Media::previous(),
            "play-pause" => Media::play_pause(),
            "next" => Media::next(),
            _ => {
                close();
                quick_settings::open(Page::Grid);
            }
        }),
        middle: Some(Rc::new(Media::play_pause)),
    })
}

/// The launcher's timer while it runs, counting down to its end.
pub(super) fn timer() -> Option<Shown> {
    let (ends, label) = timer::ends()?;
    let ends_ms = ends
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as i64);

    let mut content = ActivityContent::new(TIMER, ItemIcon::Glyph(String::from("timer")), &label);
    content.subtitle = String::from("Timer");
    content.trailing = Trailing::Countdown(ends_ms);
    content.tint = Some(Tint::Orange);
    content.relevance = 0.7;
    content.buttons = vec![
        ActivityButton {
            destructive: true,
            ..ActivityButton::new("cancel", "Cancel")
        },
        ActivityButton::new("open", "Open in the Launcher"),
    ];

    Some(Shown {
        key: String::from("timer"),
        content,
        dim: false,
        act: Rc::new(|button| {
            close();
            match button {
                "cancel" => timer::cancel(),
                _ => {
                    launcher::open(launcher::Mode::Actions);
                }
            }
        }),
        middle: None,
    })
}

/// While niri casts the screen or a window: what's shared, each with Stop
/// Sharing.
pub(super) fn sharing() -> Option<Shown> {
    let privacy = Privacy::read();
    if !privacy.screen() {
        return None;
    }
    let casts = privacy.casts().to_vec();
    drop(privacy);

    let windows = window_titles();
    let what = |target: &CastTarget| match target {
        CastTarget::Output(name) => format!("Screen {name}"),
        CastTarget::Window(id) => windows
            .iter()
            .find(|(window, _)| window == id)
            .map_or_else(|| String::from("A window"), |(_, title)| title.clone()),
        CastTarget::Nothing => String::from("Nothing yet"),
    };

    let mut content = ActivityContent::new(
        SHARING,
        ItemIcon::Glyph(String::from("screen-share")),
        "Sharing",
    );
    content.subtitle = casts
        .iter()
        .map(|cast| what(&cast.target))
        .collect::<Vec<_>>()
        .join(", ");
    content.trailing = Trailing::Text(String::from("Sharing"));
    content.tint = Some(Tint::Purple);
    content.relevance = 0.9;
    content.buttons = casts
        .iter()
        .take(3)
        .map(|cast| ActivityButton {
            destructive: true,
            ..ActivityButton::new(
                &format!("stop:{}", cast.session),
                if casts.len() == 1 {
                    "Stop Sharing".to_string()
                } else {
                    format!("Stop {}", what(&cast.target))
                }
                .as_str(),
            )
        })
        .collect();

    Some(Shown {
        key: String::from("sharing"),
        content,
        dim: false,
        act: Rc::new(|button| {
            if let Some(session) = button.strip_prefix("stop:").and_then(|s| s.parse().ok()) {
                stop_cast(session);
                close();
            }
        }),
        middle: None,
    })
}

/// Every window by id with its title, from niri.
pub(super) fn window_titles() -> Vec<(u64, String)> {
    amane::Windows::read()
        .list()
        .iter()
        .map(|window| (window.id(), window.title().unwrap_or_default().to_string()))
        .collect()
}
