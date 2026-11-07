//! Where a search's results come from. Each [`Provider`] adds what it
//! finds for the query, with how well it matched; [`search`] ranks them by
//! that and by their use (`rank`), puts an answer in the card over the
//! list, the fallbacks (the URL typed, a web search) last, and offers the
//! scopes of the best rows as chips. Actions' listing, unsearched, keeps
//! the order it's found in, a section at a time.

mod clipboard;
mod commands;
mod files;
mod passwords;

use commands::*;
pub(super) use files::shows_as_image;
use files::*;

use std::collections::BTreeMap;
use std::path::PathBuf;

use amane::DesktopApp;
use amane_kit::settings::{QuickKey, launcher};

use crate::services::assistant as assistant_service;
use crate::services::launcher::clipboard_history::{self, Clip, Kind as ClipKind};
use crate::services::launcher::file_search::{self, FileKind, Hit, Request};
use amane_kit::icons::{
    ICON_CALCULATE, ICON_CONTENT_COPY, ICON_DESCRIPTION, ICON_FOLDER, ICON_LOCK, ICON_OPEN_IN_NEW,
    ICON_SEARCH, ICON_SWAP,
};

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

/// Every provider, in the order their fallbacks list. The assistant comes
/// after what it might stand aside for.
fn providers() -> [&'static dyn Provider; 10] {
    [
        &Passwords,
        &Answers,
        &Apps,
        &super::phone_apps::PhoneApps,
        &Actions,
        &Files,
        &Clipboard,
        &Assistant,
        &Url,
        &Web,
    ]
}

/// What `query` finds in its mode, narrowed to `scope` if there is one,
/// leaving out the providers `excluded` names.
pub(super) fn search(query: &Query, scope: Option<&Scope>, excluded: &[String]) -> Found {
    let mut found = Vec::new();
    match scope {
        Some(Scope::App(app)) => in_app(app, query, &mut found),
        Some(Scope::Folder(folder)) => files(query, None, Some(folder.clone()), &mut found),
        Some(Scope::Kind(kind)) => files(query, Some(*kind), None, &mut found),
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

/// A command read from the query ("turn the volume down", "open firefox"),
/// as the card over the list: by the assistant's rules at once, or by
/// Needle 3 a moment later. A sum or a conversion keeps the card, and what
/// Needle reads stands aside for a row that already matches outright.
struct Assistant;

impl Provider for Assistant {
    fn id(&self) -> &'static str {
        "assistant"
    }

    fn modes(&self) -> &'static [Mode] {
        HOME
    }

    fn find(&self, query: &Query, out: &mut Vec<Candidate>) {
        if query.needle.is_empty() || out.iter().any(|found| found.group == Group::Answer) {
            return;
        }
        let Some(reading) =
            assistant_service::read(query.text, || super::assistant::snapshot(query.apps))
        else {
            return;
        };
        let outright = out
            .iter()
            .any(|found| found.group == Group::Ranked && found.matched >= 1.0);
        if reading.source == amane_assistant::Source::Model && outright {
            return;
        }
        if let Some(entry) = super::assistant::entry(&reading, query.apps) {
            out.push(Candidate {
                entry,
                matched: reading.confidence,
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

/// How an action found by its quick keys, or with an argument typed, and
/// one found by name compare with the rest.
const QUICK_KEYS: f32 = 3.0;
const INLINE: f32 = 1.5;
const ACTION_WEIGHT: f32 = 0.95;

/// Files, by name and (with LocalSearch's index) what's in them: in Files
/// all it finds, newest first, or the recent ones before anything's typed,
/// "/" first picking a kind; from the pill a few, under the apps.
struct Files;

/// How many letters from the pill before files are looked for, how many
/// it lists, and how they compare with apps and actions.
const FILES_FROM_PILL_CHARS: usize = 3;
const FILES_FROM_PILL: usize = 5;
const FILE_WEIGHT: f32 = 0.6;
/// What the files changed lately are listed under.
const RECENTS: &str = "Recents";

/// Bitwarden's logins, in the Passwords mode, or after `pw ` in the pill: Enter fills the
/// password into the window behind the launcher. The best match also offers its username,
/// its code, and a copy. Locked, the one row unlocks.
struct Passwords;

/// What, typed first in the pill, searches the passwords.
const PASSWORDS_PREFIX: &str = "pw ";

/// The clipboard's history, newest first, narrowed to the copies with what's
/// typed in them.
struct Clipboard;

/// How much of a copy its row's title shows.
const CLIP_TITLE: usize = 90;

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
