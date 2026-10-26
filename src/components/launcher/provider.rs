//! Where a search's results come from. Each [`Provider`] adds what it
//! finds for the query, with how well it matched; [`search`] ranks them by
//! that and by their use (`rank`), puts an answer in the card over the
//! list, the fallbacks (the URL typed, a web search) last, and offers the
//! scopes of the best rows as chips. Actions' listing, unsearched, keeps
//! the order it's found in, a section at a time.

use std::collections::BTreeMap;

use amane::DesktopApp;
use amane_kit::settings::{QuickKey, launcher};

use crate::components::icons::{
    ICON_CALCULATE, ICON_CONTENT_COPY, ICON_FOLDER, ICON_OPEN_IN_NEW, ICON_SEARCH, ICON_SWAP,
};
use crate::services::clipboard_history::{self, Clip, Kind as ClipKind};

use super::actions::{self, Param, Spec, Value};
use super::answers;
use super::entries::{self, Action, Entry, Icon, Preview, Scope, app_key};
use super::mode::Mode;
use super::model;
use super::rank;
use super::usage::{self, Usage};

/// How many rows a search lists at most, and chips it offers.
const MAX_ROWS: usize = 40;
const MAX_SCOPES: usize = 4;
/// How many of the actions used most Actions suggests first.
const SUGGESTED: usize = 5;
/// What the actions used most are listed under.
const SUGGESTIONS: &str = "Suggestions";

/// What's being searched for.
pub(super) struct Query<'a> {
    pub(super) text: &'a str,
    /// The text trimmed and lowercased.
    pub(super) needle: String,
    pub(super) mode: Mode,
    pub(super) apps: &'a [DesktopApp],
}

impl<'a> Query<'a> {
    pub(super) fn new(text: &'a str, mode: Mode, apps: &'a [DesktopApp]) -> Self {
        Self {
            text: text.trim(),
            needle: text.trim().to_lowercase(),
            mode,
            apps,
        }
    }
}

/// Where a find goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Group {
    /// The card over the list: a sum's or a conversion's answer.
    Answer,
    /// The list, ranked.
    Ranked,
    /// The list, in the order found: Actions, unsearched.
    Listed,
    /// After everything else, in the order found.
    Fallback,
}

pub(super) struct Candidate {
    pub(super) entry: Entry,
    /// How well it matched, 0 … 1.
    pub(super) matched: f32,
    pub(super) group: Group,
}

pub(super) trait Provider {
    /// What the settings leave it out by, like "web".
    fn id(&self) -> &'static str;

    /// The modes it searches in.
    fn modes(&self) -> &'static [Mode];

    fn find(&self, query: &Query, out: &mut Vec<Candidate>);
}

/// What a search found.
#[derive(Default)]
pub(super) struct Found {
    pub(super) answer: Option<Entry>,
    pub(super) scopes: Vec<Scope>,
    pub(super) rows: Vec<Entry>,
}

/// Every provider, in the order their fallbacks list.
fn providers() -> [&'static dyn Provider; 6] {
    [&Answers, &Apps, &Actions, &Clipboard, &Url, &Web]
}

/// What `query` finds in its mode, narrowed to `scope` if there is one,
/// leaving out the providers `excluded` names.
pub(super) fn search(query: &Query, scope: Option<&Scope>, excluded: &[String]) -> Found {
    let mut found = Vec::new();
    match scope {
        Some(Scope::App(app)) => in_app(app, query, &mut found),
        Some(Scope::Action { key, .. }) => {
            if let Some(spec) = actions::find(key, query.apps) {
                asking(&spec, query, &mut found);
            }
        }
        None => {
            for provider in providers() {
                if provider.modes().contains(&query.mode)
                    && !excluded.iter().any(|id| id == provider.id())
                {
                    provider.find(query, &mut found);
                }
            }
        }
    }
    usage::read(|usage| arrange(found, query.text, usage, usage::now()))
}

/// Ranks `found` for `query` by match and use.
fn arrange(found: Vec<Candidate>, query: &str, usage: &Usage, now: u64) -> Found {
    let mut answer = None;
    let mut ranked = Vec::new();
    let mut listed = Vec::new();
    let mut fallback = Vec::new();
    for candidate in found {
        match candidate.group {
            Group::Answer if answer.is_none() => answer = Some(candidate.entry),
            Group::Answer => {}
            Group::Ranked => {
                let score = rank::rank(
                    candidate.matched,
                    usage.frecency(&candidate.entry.key, now),
                    usage.learned(query, &candidate.entry.key),
                );
                ranked.push((score, candidate.entry));
            }
            Group::Listed => listed.push(candidate.entry),
            Group::Fallback => fallback.push(candidate.entry),
        }
    }
    // the best first, and the same score in the order found
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut rows: Vec<Entry> = ranked.into_iter().map(|(_, entry)| entry).collect();
    rows.truncate(MAX_ROWS);

    let mut scopes: Vec<Scope> = Vec::new();
    for scope in rows.iter().filter_map(|entry| entry.scope.clone()) {
        if scopes.len() < MAX_SCOPES && !scopes.contains(&scope) {
            scopes.push(scope);
        }
    }
    rows.extend(listed);
    rows.extend(fallback);
    Found {
        answer,
        scopes,
        rows,
    }
}

/// An app's own rows, narrowed to it: opening it, then its actions.
fn in_app(app: &DesktopApp, query: &Query, out: &mut Vec<Candidate>) {
    let open = Entry::new(
        app_key(app),
        format!("Open {}", app.name()),
        Icon::App(Box::new(app.clone())),
        Action::Launch(app.clone()),
    )
    .subtitle("Application");
    if let Some(matched) = rank::fuzzy(&query.needle, app.name()) {
        out.push(Candidate {
            entry: open,
            matched: matched + 1.0,
            group: Group::Ranked,
        });
    }
    for (index, action) in app.actions().iter().enumerate() {
        if let Some(matched) = rank::fuzzy(&query.needle, action.name()) {
            out.push(Candidate {
                entry: Entry::new(
                    format!("action:app:{}#{index}", app.id()),
                    action.name(),
                    Icon::App(Box::new(app.clone())),
                    Action::AppShortcut(app.clone(), index),
                )
                .subtitle(app.name()),
                matched,
                group: Group::Ranked,
            });
        }
    }
}

const HOME: &[Mode] = &[Mode::Home];

/// A sum's answer, or a conversion's.
struct Answers;

impl Provider for Answers {
    fn id(&self) -> &'static str {
        "answers"
    }

    fn modes(&self) -> &'static [Mode] {
        HOME
    }

    fn find(&self, query: &Query, out: &mut Vec<Candidate>) {
        if let Some(answer) = model::evaluate(query.text) {
            let value = model::format_number(answer);
            out.push(Candidate {
                entry: Entry::new(
                    "answer",
                    value.clone(),
                    Icon::Glyph(ICON_CALCULATE),
                    Action::Copy(value),
                )
                .subtitle(query.text),
                matched: 1.0,
                group: Group::Answer,
            });
        }
        if let Some(converted) = answers::convert(query.text) {
            out.push(Candidate {
                entry: Entry::new(
                    "answer",
                    converted.to,
                    Icon::Glyph(ICON_SWAP),
                    Action::Copy(converted.value),
                )
                .subtitle(converted.from),
                matched: 1.0,
                group: Group::Answer,
            });
        }
    }
}

/// The installed apps, by their names and keywords.
struct Apps;

impl Provider for Apps {
    fn id(&self) -> &'static str {
        "apps"
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Home, Mode::Apps]
    }

    fn find(&self, query: &Query, out: &mut Vec<Candidate>) {
        if query.needle.is_empty() {
            return;
        }
        for app in query.apps {
            let by_name = rank::fuzzy(&query.needle, app.name());
            // a keyword's start counts, a little under the name's
            let by_keyword = app
                .keywords()
                .iter()
                .any(|keyword| keyword.to_lowercase().starts_with(&query.needle))
                .then_some(0.6);
            if let Some(matched) = by_name.into_iter().chain(by_keyword).reduce(f32::max) {
                out.push(Candidate {
                    entry: Entry::app(app),
                    matched,
                    group: Group::Ranked,
                });
            }
        }
    }
}

/// The launcher's actions ([`actions`]): by their quick keys first, then by
/// name, with an argument typed after it ("timer 5m"). Unsearched in
/// Actions, all of them a section at a time, the ones used most first.
struct Actions;

impl Provider for Actions {
    fn id(&self) -> &'static str {
        "actions"
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Home, Mode::Actions]
    }

    fn find(&self, query: &Query, out: &mut Vec<Candidate>) {
        let quick = launcher().quick_keys.clone();
        let specs = actions::registry(query.apps);
        if query.needle.is_empty() {
            if query.mode == Mode::Actions {
                listing(&specs, &quick, out);
            }
            return;
        }
        let pinned = quick.get(&query.needle).map(|key| key.action.as_str());
        for spec in &specs {
            if pinned == Some(spec.key.as_str()) {
                out.push(Candidate {
                    entry: action_entry(spec, None, &quick),
                    matched: QUICK_KEYS,
                    group: Group::Ranked,
                });
                continue;
            }
            if let Some(value) = actions::inline(spec, &query.needle) {
                out.push(Candidate {
                    entry: action_entry(spec, Some(value), &quick),
                    matched: INLINE,
                    group: Group::Ranked,
                });
                continue;
            }
            let by_title = rank::fuzzy(&query.needle, &spec.title);
            // a keyword's start counts, a little under the name's
            let by_keyword = spec
                .keywords
                .iter()
                .any(|keyword| keyword.starts_with(&query.needle))
                .then_some(0.7);
            if let Some(matched) = by_title.into_iter().chain(by_keyword).reduce(f32::max) {
                out.push(Candidate {
                    entry: action_entry(spec, None, &quick),
                    // an app named the same comes first
                    matched: matched * ACTION_WEIGHT,
                    group: Group::Ranked,
                });
            }
        }
    }
}

/// How an action found by its quick keys, or with an argument typed, and
/// one found by name compare with the rest.
const QUICK_KEYS: f32 = 3.0;
const INLINE: f32 = 1.5;
const ACTION_WEIGHT: f32 = 0.95;

/// Every action a section at a time, the ones used most first.
fn listing(specs: &[Spec], quick: &BTreeMap<String, QuickKey>, out: &mut Vec<Candidate>) {
    let used = usage::read(|usage| usage.top("action:", SUGGESTED, usage::now()));
    let mut listed = |spec: &Spec, section: &str| {
        let mut entry = action_entry(spec, None, quick);
        entry.section = Some(section.to_string());
        out.push(Candidate {
            entry,
            matched: 0.0,
            group: Group::Listed,
        });
    };
    for key in &used {
        if let Some(spec) = specs.iter().find(|spec| &spec.key == key) {
            listed(spec, SUGGESTIONS);
        }
    }
    for spec in specs {
        listed(spec, &spec.section);
    }
}

/// `spec`'s row: run with `value`, or, if it asks for something and
/// nothing was given, narrowing the search to ask for it.
fn action_entry(spec: &Spec, value: Option<Value>, quick: &BTreeMap<String, QuickKey>) -> Entry {
    let action = match (spec.param, value) {
        (Some(_), None) => Action::Ask(Scope::Action {
            key: spec.key.clone(),
            title: spec.title.clone(),
            placeholder: spec.placeholder,
        }),
        (_, value) => {
            let run = spec.run.clone();
            let value = value.unwrap_or(Value::None);
            let title = spec.titled(&value);
            let mut entry = runs(spec, title, quick);
            entry.action = Action::Run(std::rc::Rc::new(move || run(value.clone())));
            return entry;
        }
    };
    let mut entry = runs(spec, spec.title.clone(), quick);
    entry.action = action;
    entry
}

/// `spec`'s row titled `title`, with its quick keys, doing nothing yet.
fn runs(spec: &Spec, title: String, quick: &BTreeMap<String, QuickKey>) -> Entry {
    let mut entry = Entry::new(spec.key.clone(), title, spec.icon.clone(), Action::Hint)
        .subtitle(spec.section.clone());
    entry.action_title = Some(spec.title.clone());
    entry.quick_keys = quick
        .iter()
        .find(|(_, key)| key.action == spec.key)
        .map(|(keys, _)| keys.clone());
    entry
}

/// The rows of an action waiting for what it asks: each choice, or what
/// was typed read as it.
fn asking(spec: &Spec, query: &Query, out: &mut Vec<Candidate>) {
    let quick = launcher().quick_keys.clone();
    match spec.param {
        Some(Param::Choice) => {
            for (name, value) in &spec.choices {
                if let Some(matched) = rank::fuzzy(&query.needle, name) {
                    out.push(Candidate {
                        entry: action_entry(spec, Some(Value::Choice(value.clone())), &quick),
                        matched,
                        group: Group::Listed,
                    });
                }
            }
        }
        Some(Param::Duration) => {
            let entry = match spec.read(query.text) {
                Some(value) => action_entry(spec, Some(value), &quick),
                None => Entry::new(
                    spec.key.clone(),
                    spec.placeholder,
                    spec.icon.clone(),
                    Action::Hint,
                )
                .subtitle("Type how long, then Return"),
            };
            out.push(Candidate {
                entry,
                matched: 1.0,
                group: Group::Listed,
            });
        }
        None => {}
    }
}

/// The clipboard's history, newest first, narrowed to the copies with what's
/// typed in them.
struct Clipboard;

impl Provider for Clipboard {
    fn id(&self) -> &'static str {
        "clipboard"
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Clipboard]
    }

    fn find(&self, query: &Query, out: &mut Vec<Candidate>) {
        let now = usage::now();
        for clip in clipboard_history::clips() {
            if !query.needle.is_empty() && !clip.preview.to_lowercase().contains(&query.needle) {
                continue;
            }
            out.push(Candidate {
                entry: clip_entry(clip, now),
                matched: 1.0,
                group: Group::Listed,
            });
        }
    }
}

/// How much of a copy its row's title shows.
const CLIP_TITLE: usize = 90;

/// A copy's row: its start or its image, what it is, and how long ago.
fn clip_entry(clip: Clip, now: u64) -> Entry {
    let (title, subtitle, icon, preview) = match clip.kind {
        ClipKind::Text => (
            entries::truncate(&clip.preview, CLIP_TITLE),
            format!("Text · {}", model::size(clip.bytes)),
            Icon::Glyph(ICON_CONTENT_COPY),
            Preview::Text(clip.preview.clone()),
        ),
        ClipKind::Image => (
            String::from("Image"),
            format!(
                "{} image · {}",
                if clip.mime == "image/png" {
                    "PNG"
                } else {
                    "JPEG"
                },
                model::size(clip.bytes)
            ),
            Icon::Image(clip.path()),
            Preview::Image(clip.path()),
        ),
        ClipKind::Files => (
            entries::truncate(&clip.preview, CLIP_TITLE),
            String::from("Files"),
            Icon::Glyph(ICON_FOLDER),
            Preview::Text(clip.preview.clone()),
        ),
    };
    let subtitle = match &clip.app {
        Some(app) => format!("{subtitle} · {app}"),
        None => subtitle,
    };
    let mut entry =
        Entry::new(format!("clip:{:016x}", clip.id), title, icon, Action::Hint).subtitle(subtitle);
    entry.trailing = Some(model::ago(now.saturating_sub(clip.at)));
    entry.preview = Some(preview);
    entry.action = Action::Paste(Box::new(clip));
    entry
}

/// The URL typed, opened in the browser.
struct Url;

impl Provider for Url {
    fn id(&self) -> &'static str {
        "url"
    }

    fn modes(&self) -> &'static [Mode] {
        HOME
    }

    fn find(&self, query: &Query, out: &mut Vec<Candidate>) {
        if model::looks_like_url(query.text) {
            let url = model::normalize_url(query.text);
            out.push(Candidate {
                entry: Entry::new(
                    "url",
                    format!("Open {}", query.text),
                    Icon::Glyph(ICON_OPEN_IN_NEW),
                    Action::Open(url.clone()),
                )
                .subtitle("Default browser")
                .detail(url),
                matched: 1.0,
                group: Group::Fallback,
            });
        }
    }
}

/// A web search, so the list is never empty.
struct Web;

impl Provider for Web {
    fn id(&self) -> &'static str {
        "web"
    }

    fn modes(&self) -> &'static [Mode] {
        HOME
    }

    fn find(&self, query: &Query, out: &mut Vec<Candidate>) {
        if query.text.is_empty() {
            return;
        }
        let url = format!("{}{}", model::SEARCH_URL, model::percent_encode(query.text));
        out.push(Candidate {
            entry: Entry::new(
                "web",
                format!("Search the web for \u{201c}{}\u{201d}", query.text),
                Icon::Glyph(ICON_SEARCH),
                Action::Open(url.clone()),
            )
            .subtitle("Google")
            .detail(url),
            matched: 0.0,
            group: Group::Fallback,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(key: &str, matched: f32, group: Group) -> Candidate {
        Candidate {
            entry: Entry::new(
                key,
                key,
                Icon::Glyph(ICON_SEARCH),
                Action::Copy(String::new()),
            ),
            matched,
            group,
        }
    }

    fn keys(rows: &[Entry]) -> Vec<&str> {
        rows.iter().map(|entry| entry.key.as_str()).collect()
    }

    #[test]
    fn the_best_match_leads_and_the_fallbacks_trail() {
        let arranged = arrange(
            vec![
                found("web", 0.0, Group::Fallback),
                found("weak", 0.4, Group::Ranked),
                found("answer", 1.0, Group::Answer),
                found("strong", 0.9, Group::Ranked),
            ],
            "q",
            &Usage::default(),
            0,
        );
        assert_eq!(keys(&arranged.rows), vec!["strong", "weak", "web"]);
        assert_eq!(
            arranged.answer.map(|entry| entry.key),
            Some(String::from("answer"))
        );
    }

    #[test]
    fn what_was_picked_before_rises() {
        let mut usage = Usage::default();
        for at in 0..5 {
            usage.record("weak", "q", at);
        }
        let arranged = arrange(
            vec![
                found("strong", 0.8, Group::Ranked),
                found("weak", 0.6, Group::Ranked),
            ],
            "q",
            &usage,
            10,
        );
        assert_eq!(keys(&arranged.rows), vec!["weak", "strong"]);
    }
}
