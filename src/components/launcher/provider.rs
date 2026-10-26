//! Where a search's results come from. Each [`Provider`] adds what it
//! finds for the query, with how well it matched; [`search`] ranks them by
//! that and by their use (`rank`), puts an answer in the card over the
//! list, the fallbacks (the URL typed, a web search) last, and offers the
//! scopes of the best rows as chips.

use amane::{DesktopApp, Service};

use crate::components::icons::{
    ICON_CALCULATE, ICON_LOCK, ICON_LOG_OUT, ICON_OPEN_IN_NEW, ICON_POWER, ICON_RESTART,
    ICON_SEARCH, ICON_SLEEP, ICON_SWAP,
};
use crate::services::session::Session;

use super::answers;
use super::entries::{Action, Entry, Icon, Scope, app_key};
use super::mode::Mode;
use super::model::{self, SessionAction};
use super::rank;
use super::usage::{self, Usage};

/// How many rows a search lists at most, and chips it offers.
const MAX_ROWS: usize = 40;
const MAX_SCOPES: usize = 4;

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
fn providers() -> [&'static dyn Provider; 5] {
    [&Answers, &Apps, &Sessions, &Url, &Web]
}

/// What `query` finds in its mode, narrowed to `scope` if there is one,
/// leaving out the providers `excluded` names.
pub(super) fn search(query: &Query, scope: Option<&Scope>, excluded: &[String]) -> Found {
    let mut found = Vec::new();
    match scope {
        Some(Scope::App(app)) => in_app(app, query, &mut found),
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
                    format!("{}#{index}", app_key(app)),
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

/// Locking, logging out, sleeping, restarting and powering off, each when
/// logind allows it.
struct Sessions;

impl Provider for Sessions {
    fn id(&self) -> &'static str {
        "session"
    }

    fn modes(&self) -> &'static [Mode] {
        HOME
    }

    fn find(&self, query: &Query, out: &mut Vec<Candidate>) {
        let session = Session::read();
        for power in SessionAction::ALL {
            let allowed = match power {
                SessionAction::Suspend => session.can_suspend(),
                SessionAction::Restart => session.can_reboot(),
                SessionAction::PowerOff => session.can_power_off(),
                SessionAction::Lock | SessionAction::LogOut => true,
            };
            if !allowed || query.needle.is_empty() {
                continue;
            }
            let by_label = rank::fuzzy(&query.needle, power.label());
            let by_alias = power.matches(&query.needle).then_some(0.7);
            if let Some(matched) = by_label.into_iter().chain(by_alias).reduce(f32::max) {
                out.push(Candidate {
                    entry: Entry::new(
                        format!("session:{}", power.label()),
                        power.label(),
                        Icon::Glyph(glyph(power)),
                        Action::SessionAction(power),
                    )
                    .subtitle("System"),
                    matched,
                    group: Group::Ranked,
                });
            }
        }
    }
}

fn glyph(power: SessionAction) -> char {
    match power {
        SessionAction::Lock => ICON_LOCK,
        SessionAction::LogOut => ICON_LOG_OUT,
        SessionAction::Suspend => ICON_SLEEP,
        SessionAction::Restart => ICON_RESTART,
        SessionAction::PowerOff => ICON_POWER,
    }
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
