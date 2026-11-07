//! The launcher's input: typing, the pointer and the wheel, voice, switching modes and scopes,
//! and running what's picked.

use super::*;

/// Opens the launcher on a fresh search and starts listening, Whistle told
/// the names of what's here.
pub(super) fn listen() {
    switch_mode(Mode::Home);
    if !is_open(Panel::Launcher) {
        panel::toggle(Panel::Launcher);
    }
    let keywords = assistant::snapshot(Apps::read().list()).keywords();
    assistant_service::listen_start(keywords);
}

/// What was heard, once listening stopped: typed into the field, so
/// the launcher searches it as if typed. What a rule reads that doesn't end
/// the session runs at once, as its key would; anything less certain
/// waits on the card for Return.
pub(super) fn heard() {
    let Some(heard) = assistant_service::take_heard() else {
        return;
    };
    let text = heard
        .trim()
        .trim_end_matches(['.', '!', '?'])
        .trim()
        .to_string();
    if !is_open(Panel::Launcher) || text.is_empty() {
        return;
    }
    type_into(|field| {
        *field = Field::new();
        field.insert(&text);
        true
    });
    let apps = Apps::read();
    let reading = assistant_service::read(&text, || assistant::snapshot(apps.list()));
    let sure = reading.filter(|reading| {
        reading.source == amane_assistant::Source::Rules
            && reading.command.risk() < amane_assistant::Risk::Destructive
    });
    let entry = sure.and_then(|reading| assistant::entry(&reading, apps.list()));
    drop(apps);
    if let Some(entry) = entry {
        entries::run(&entry, &text);
    }
    Launcher::changed();
}

/// Switches to `mode`, from a fresh search; the one showing again goes
/// back to the pill.
pub(super) fn switch_mode(mode: Mode) {
    let to = with(|state| {
        let to = if state.mode == mode { Mode::Home } else { mode };
        state.mode = to;
        state.field = Field::new();
        state.scope = None;
        state.selected = 0;
        state.history_at = None;
        state.hover_mode = None;
        state.back_hover = false;
        state.focus = if to == Mode::Apps {
            Zone::Body
        } else {
            Zone::Search
        };
        to
    });
    if to == Mode::Apps {
        PAGER.with_borrow_mut(|pager| pager.jump(0, &PAGING));
    }
    SCROLL.with_borrow_mut(|scroll| scroll.jump(0.0));
}

/// Narrows the search to `scope`, from an empty query.
pub(super) fn enter_scope(scope: Scope) {
    with(|state| {
        state.scope = Some(scope);
        state.field = Field::new();
        state.selected = 0;
        state.history_at = None;
        state.focus = Zone::Search;
    });
    SCROLL.with_borrow_mut(|scroll| scroll.jump(0.0));
    Launcher::changed();
}

/// Steps back one level: the query, then the scope, then the mode; with
/// none of them, closes the launcher.
pub(super) fn back() {
    let (typed, scoped, mode) = with(|state| {
        (
            !state.field.text().is_empty(),
            state.scope.is_some(),
            state.mode,
        )
    });
    if typed {
        type_into(|field| {
            *field = Field::new();
            true
        });
    } else if scoped {
        with(|state| {
            state.scope = None;
            state.selected = 0;
        });
    } else if mode != Mode::Home {
        switch_mode(Mode::Home);
    } else {
        close();
    }
}

/// Starts typing quick keys for the action `action`, named `title`, from
/// the ones it has, `keys`.
pub(super) fn start_capture(action: &str, title: &str, keys: &str) {
    with(|state| {
        state.capture = Some(Capture {
            action: action.to_string(),
            title: title.to_string(),
            keys: keys.to_string(),
            error: None,
        });
    });
    Launcher::changed();
}

/// A key while quick keys are being typed: letters and digits add to
/// them, Backspace takes one back, Return saves them (none clears them)
/// and Escape gives up.
pub(super) fn capture_key(key: Key) {
    let Some(mut capture) = with(|state| state.capture.clone()) else {
        return;
    };
    match key {
        Key::Character(c) if c.is_ascii_alphanumeric() => {
            if capture.keys.chars().count() < model::MAX_QUICK_KEYS {
                capture.keys.push(c.to_ascii_lowercase());
            }
            capture.error = None;
        }
        Key::Backspace => {
            capture.keys.pop();
            capture.error = None;
        }
        Key::Escape => {
            with(|state| state.capture = None);
            Launcher::changed();
            return;
        }
        Key::Enter => {
            let taken = launcher()
                .quick_keys
                .get(&capture.keys)
                .filter(|key| key.action != capture.action)
                .map(|key| key.title.clone());
            if let Some(owner) = taken {
                capture.error = Some(format!("Used by {owner}"));
            } else if capture.keys.is_empty() || model::valid_quick_keys(&capture.keys) {
                let Capture {
                    action,
                    title,
                    keys,
                    ..
                } = capture;
                change_launcher(|settings| {
                    settings.quick_keys.retain(|_, key| key.action != action);
                    if !keys.is_empty() {
                        settings.quick_keys.insert(keys, QuickKey { action, title });
                    }
                });
                with(|state| state.capture = None);
                Launcher::changed();
                return;
            }
        }
        _ => return,
    }
    with(|state| state.capture = Some(capture));
    Launcher::changed();
}

/// The body moved: the scrollbar shows for a while.
pub(super) fn moved() {
    SCROLLBAR.with_borrow_mut(ScrollbarFade::moved);
}

/// A scroll over the body: a wheel scrolls it, a sideways tilt turns the
/// category, and fingers do either, whichever way they set out.
pub(super) fn on_body_scroll(scroll: Scroll) {
    let (paging, pages) = with(|state| (state.shown.0 == Kind::Apps, state.pages));
    match scroll.source {
        ScrollSource::Wheel => {
            SCROLL.with_borrow_mut(|body| body.scroll(&scroll, Axis::Vertical, WHEEL_STEP));
            moved();
        }
        ScrollSource::Tilt if paging => {
            PAGER.with_borrow_mut(|pager| pager.step(scroll.x, pages, &PAGING));
        }
        ScrollSource::Tilt => {}
        ScrollSource::Finger | ScrollSource::Continuous => {
            let (x, y) = scroll.pixels();
            let now = Instant::now();
            let picked = with(|state| {
                let picked = state.gesture.feed(x, y, paging && pages > 1, now);
                if scroll.stop {
                    state.gesture.release();
                }
                picked
            });
            match picked {
                Some((Axis::Vertical, along)) => {
                    SCROLL.with_borrow_mut(|body| body.follow(along, scroll.stop, now));
                    moved();
                }
                Some((Axis::Horizontal, along)) => PAGER.with_borrow_mut(|pager| {
                    pager.follow(along, scroll.stop, GRID_W, pages, &PAGING);
                }),
                // fingers lifting before the gesture picked a way still end
                // whatever they'd started
                None if scroll.stop => {
                    SCROLL.with_borrow_mut(|body| body.follow(0.0, true, now));
                    PAGER.with_borrow_mut(|pager| pager.follow(0.0, true, GRID_W, pages, &PAGING));
                }
                None => {}
            }
        }
    }
    Launcher::changed();
}

/// The pill bar follows a sideways swipe, or a wheel turned over it.
pub(super) fn on_pill_scroll(scroll: Scroll) {
    PILL_SCROLL.with_borrow_mut(|pills| match scroll.source {
        ScrollSource::Finger | ScrollSource::Continuous => {
            pills.scroll(&scroll, Axis::Horizontal, WHEEL_STEP)
        }
        ScrollSource::Wheel | ScrollSource::Tilt => {
            let lines = if scroll.x.abs() > f32::EPSILON {
                scroll.x
            } else {
                scroll.y
            };
            let to = pills.target() + lines * WHEEL_STEP;
            pills.go(to);
        }
    });
    Launcher::changed();
}

/// Highlights item `index` of `layout`, scrolling it into view in a body
/// `height` tall.
pub(super) fn select(index: usize, layout: &Layout, height: f32) {
    with(|state| state.selected = index);
    let (top, span) = layout.span(index);
    SCROLL.with_borrow_mut(|scroll| {
        let at = scroll.target();
        if top < at {
            scroll.go(top);
        } else if top + span > at + height {
            scroll.go(top + span - height);
        }
    });
}

/// Hovering an app or entry highlights it, and brings the focus to them.
pub(super) fn hover(index: usize) -> impl Fn(bool) + 'static {
    move |inside| {
        if inside
            && with(|state| {
                state.focus = Zone::Body;
                std::mem::replace(&mut state.selected, index)
            }) != index
        {
            Launcher::changed();
        }
    }
}

/// Turns to category page `page`: sliding there from the next one, or
/// fading in from further away rather than flashing every one between.
pub(super) fn turn_to(page: usize) {
    let pages = with(|state| state.pages);
    PAGER.with_borrow_mut(|pager| {
        if pager.page().abs_diff(page) > 1 {
            pager.jump(page, &PAGING);
            SWITCH.with_borrow_mut(|switch| *switch = motion::fade(0.0, motion::FADE));
        } else {
            pager.go(page, pages, &PAGING);
        }
    });
    Launcher::changed();
}

/// Edits the search text; a change starts the list over at the top. The
/// keys typed go to the search field.
pub(super) fn type_into(edit: impl FnOnce(&mut Field) -> bool) {
    let changed = with(|state| {
        let before = state.field.text().to_string();
        let changed = edit(&mut state.field) || state.field.text() != before;
        state.focus = Zone::Search;
        if changed {
            state.selected = 0;
            state.history_at = None;
        }
        changed
    });
    if changed {
        SCROLL.with_borrow_mut(|scroll| scroll.go(0.0));
    }
}

/// Up and Down through the searches run before, while the field is empty
/// or holds one of them: `older` goes back one, else forward to the empty
/// field. Whether there was one to go to.
pub(super) fn walk_history(older: bool) -> bool {
    let history: Vec<String> = usage::read(|usage| usage.history().iter().cloned().collect());
    with(|state| {
        let Some(to) = model::history_step(state.history_at, older, history.len()) else {
            return false;
        };
        state.history_at = to;
        let mut field = Field::new();
        if let Some(at) = to {
            field.insert(&history[at]);
        }
        state.field = field;
        state.selected = 0;
        state.focus = Zone::Search;
        true
    })
}

/// What Enter runs: the highlighted app or entry, else the first.
pub(super) fn run_selected() {
    let apps = Apps::read();
    let view = View::new(apps.list());
    let query = view.field.text().to_string();
    let entry = if view.grid() {
        view.shown
            .get(view.selected)
            .or(view.shown.first())
            .map(|app| Entry::app(app))
    } else {
        view.entry(view.selected).or(view.entry(0)).cloned()
    };
    drop(view);
    drop(apps);
    if let Some(entry) = entry {
        entries::run(&entry, &query);
    }
}

/// A click on `app`: a launch, or its menu at the pointer.
pub(super) fn on_app_click(app: DesktopApp) -> impl Fn(Button) + 'static {
    move |button| match button {
        Button::Left => entries::run(&Entry::app(&app), ""),
        Button::Right => app_menu::open(&app),
        #[allow(unreachable_patterns)]
        _ => {}
    }
}
