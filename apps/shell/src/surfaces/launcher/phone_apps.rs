//! The phone's apps in the launcher, found by name; each opens in a window
//! of its own. The list is `services::phone::apps`'.

use std::rc::Rc;

use amane::Service;
use amane_phone::KdeConnect;

use crate::services::phone::apps;

use amane_kit::icons::ICON_PHONE;

use super::entries::{Action, Entry, Icon};
use super::mode::Mode;
use super::provider::{Candidate, Group, Provider, Query};
use super::rank;

/// A phone's app counts a little under one of this computer's.
const WEIGHT: f32 = 0.9;

pub(super) struct PhoneApps;

impl Provider for PhoneApps {
    fn id(&self) -> &'static str {
        "phone-apps"
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Home, Mode::Apps]
    }

    fn find(&self, query: &Query, out: &mut Vec<Candidate>) {
        if query.needle.is_empty() {
            return;
        }

        // the phone KDE Connect has now
        let Some(phone) = KdeConnect::read().primary().cloned() else {
            return;
        };

        for app in apps::of(&phone) {
            let Some(matched) = rank::fuzzy(&query.needle, &app.name) else {
                continue;
            };
            let entry = Entry::new(
                format!("phone-app:{}", app.package),
                app.name.clone(),
                Icon::Glyph(ICON_PHONE),
                Action::Run(Rc::new({
                    let (phone, app) = (phone.id().to_string(), app.clone());
                    move || apps::open(&phone, &app)
                })),
            )
            .subtitle(format!("On {}", phone.name()))
            .detail(app.package.clone());
            out.push(Candidate {
                entry,
                matched: matched * WEIGHT,
                group: Group::Ranked,
            });
        }
    }
}
