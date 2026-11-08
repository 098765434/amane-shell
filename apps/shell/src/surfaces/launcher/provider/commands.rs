//! Actions: the shell's own commands, and what the assistant was asked.

use super::*;

/// Every action a section at a time, the ones used most first.
pub(super) fn listing(
    specs: &[Spec],
    quick: &BTreeMap<String, QuickKey>,
    out: &mut Vec<Candidate>,
) {
    let used = usage::read(|usage| usage.top("action:", SUGGESTED, emaki_kit::time::unix_s()));
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
pub(super) fn action_entry(
    spec: &Spec,
    value: Option<Value>,
    quick: &BTreeMap<String, QuickKey>,
) -> Entry {
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
pub(super) fn runs(spec: &Spec, title: String, quick: &BTreeMap<String, QuickKey>) -> Entry {
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
pub(super) fn asking(spec: &Spec, query: &Query, out: &mut Vec<Candidate>) {
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
