//! Tailscale — a tile to connect (its disc logs in when logged out) and
//! its page: what's wrong if anything, this device, the exit node, the
//! three settings `tailscale set` changes, and the tailnet's devices.
//! Hidden without the `tailscale` binary; read-only, and saying how to fix
//! it, without the operator grant. See [`crate::services::network::tailscale`].

use amane::{Column, Parent, ScrollArea, Service, Size, Widget};

use super::super::{Page, open};
use crate::components::buttons::Switch;
use crate::components::icons::{Glyph, ICON_CHEVRON_RIGHT, ICON_COMPUTER, os_glyph};
use crate::components::layout::{boxed, caption, icon};
use crate::components::list::{self, List, ListItem};
use crate::components::material::{self, Level};
use crate::components::panel::close;
use crate::surfaces::quick_settings::detail::DetailPage;
use crate::surfaces::quick_settings::tile::QuickToggle;
use crate::surfaces::status_bar::Indicator;
use crate::services::run;
use crate::services::network::tailscale::{Peer, State, Tailscale};
use crate::tokens::sys;

const ADMIN_CONSOLE: &str = "https://login.tailscale.com/admin";

/// What the disc or the page's switch asks for: log in, or up and down.
/// Read-only, it asks for nothing (the page says why).
fn switch(on: bool) {
    let tailscale = Tailscale::read();
    let (state, read_only) = (tailscale.state(), tailscale.read_only());
    drop(tailscale);
    if read_only {
        return;
    }
    match (state, on) {
        (State::NeedsLogin, true) => Tailscale::login(),
        (_, true) => Tailscale::up(),
        (_, false) => Tailscale::down(),
    }
}

pub(crate) fn toggle() -> Option<QuickToggle> {
    let tailscale = Tailscale::read();
    if !tailscale.present() {
        return None;
    }
    let active = tailscale.active();
    let subtitle = tailscale.describe();
    drop(tailscale);

    Some(
        QuickToggle::new("amane.qs.tailscale", Glyph::Tailscale, "Tailscale")
            .subtitle(subtitle)
            .checked(active)
            .on_toggle(switch)
            .on_open(|| open(Page::Tailscale)),
    )
}

pub(crate) fn page() -> DetailPage {
    let tailscale = Tailscale::read();
    let active = tailscale.active();
    let subtitle = tailscale.describe();
    drop(tailscale);

    DetailPage::new("amane.qs.tailscale.page", "Tailscale")
        .subtitle(subtitle)
        .switch(active, switch)
        .status_bar(Indicator::Tailscale)
        .body(|height| {
            vec![boxed(
                ScrollArea::new(
                    "amane.qs.tailscale.body",
                    Column::new(sections())
                        .gap(sys::space::gap::xl())
                        .width(Parent),
                )
                .height(Size::Fixed(height)),
            )]
        })
        .footer("Admin Console…", || {
            close();
            run("xdg-open", vec![ADMIN_CONSOLE.to_string()]);
        })
}

fn sections() -> Vec<Box<dyn Widget>> {
    let tailscale = Tailscale::read();
    let mut sections = notices(&tailscale);

    if tailscale.state() == State::Running {
        sections.push(this_device(&tailscale));
        sections.push(exit_nodes(&tailscale));
    }
    if tailscale.state() != State::DaemonDown {
        sections.push(options(&tailscale));
    }
    if !tailscale.peers().is_empty() {
        sections.push(devices(tailscale.peers()));
    }
    sections
}

/// What's in the way: no daemon, no login, no grant, a write that failed,
/// and what tailscaled itself warns about.
fn notices(tailscale: &Tailscale) -> Vec<Box<dyn Widget>> {
    let mut notes: Vec<Box<dyn Widget>> = Vec::new();
    let critical = |text: String| boxed(caption(text).color(sys::color::status::critical::text()));

    if tailscale.state() == State::DaemonDown {
        notes.push(boxed(caption("The Tailscale daemon isn't running.")));
    }
    if tailscale.read_only() {
        let command = Tailscale::operator_command();
        let copy = command.clone();
        notes.push(boxed(caption(
            "Read-only: changing Tailscale needs the operator grant, once:",
        )));
        notes.push(boxed(
            List::new(vec![
                ListItem::new("amane.qs.tailscale.operator", command)
                    .trailing(caption("Copy"))
                    .on_press(move || Tailscale::copy(&copy)),
            ])
            .build(),
        ));
    } else if tailscale.state() == State::NeedsLogin {
        let label = if tailscale.logging_in() {
            "Contacting Tailscale…"
        } else {
            "Log In…"
        };
        notes.push(boxed(
            List::new(vec![
                ListItem::new("amane.qs.tailscale.login", label)
                    .trailing(icon(
                        ICON_CHEVRON_RIGHT,
                        sys::size::icon::m(),
                        material::label(Level::Secondary),
                    ))
                    .on_press(Tailscale::login),
            ])
            .build(),
        ));
    }
    if let Some(error) = tailscale.error() {
        notes.push(critical(error.to_string()));
    }
    // logged out says so already
    if tailscale.state() != State::NeedsLogin {
        notes.extend(tailscale.health().iter().cloned().map(critical));
    }
    notes
}

/// This device and its address, which a click copies.
fn this_device(tailscale: &Tailscale) -> Box<dyn Widget> {
    let ip = tailscale.ip().unwrap_or_default().to_string();
    let mut item = ListItem::new("amane.qs.tailscale.self", tailscale.hostname())
        .leading(ICON_COMPUTER)
        .color(material::label(Level::Primary));
    if !ip.is_empty() {
        item = item
            .trailing(list::trailing_text(
                ip.clone(),
                material::label(Level::Secondary),
            ))
            .on_press(move || Tailscale::copy(&ip));
    }
    boxed(List::new(vec![item]).header("This Device").build())
}

/// None, or a peer that offers itself; an offline one can't be picked.
fn exit_nodes(tailscale: &Tailscale) -> Box<dyn Widget> {
    let offered: Vec<&Peer> = tailscale.exit_nodes().collect();
    if offered.is_empty() {
        return boxed(
            List::new(Vec::new())
                .header("Exit Node")
                .empty("No exit nodes available")
                .build(),
        );
    }

    let none = tailscale.exit_node().is_none();
    let mut items = vec![picked(
        ListItem::new("amane.qs.tailscale.exit.none", "None").on_press(move || {
            if !none {
                Tailscale::set_exit_node(None);
            }
        }),
        none,
    )];
    for peer in offered {
        let item = ListItem::new(format!("amane.qs.tailscale.exit.{}", peer.name), &peer.name)
            .leading(os_glyph(&peer.os));
        let item = match (&peer.ip, peer.online) {
            (_, false) => item
                .color(material::label(Level::Secondary))
                .trailing(caption("Offline")),
            (Some(ip), true) => {
                let (ip, active) = (ip.clone(), peer.exit_active);
                item.on_press(move || {
                    if !active {
                        Tailscale::set_exit_node(Some(&ip));
                    }
                })
            }
            (None, true) => item,
        };
        items.push(picked(item, peer.exit_active));
    }
    boxed(List::new(items).header("Exit Node").build())
}

/// `item`, checked when it's the one in use.
fn picked(item: ListItem, selected: bool) -> ListItem {
    let item = item.selected(selected);
    if selected {
        item.trailing(list::check())
    } else {
        item
    }
}

/// A switch row: its id, its label, whether it's on, and what flips it.
type Setting = (&'static str, &'static str, bool, fn(bool));

/// The three settings `tailscale set` flips.
fn options(tailscale: &Tailscale) -> Box<dyn Widget> {
    let rows: [Setting; 3] = [
        (
            "accept-routes",
            "Accept Routes",
            tailscale.accept_routes(),
            Tailscale::set_accept_routes,
        ),
        (
            "accept-dns",
            "Accept DNS",
            tailscale.accept_dns(),
            Tailscale::set_accept_dns,
        ),
        (
            "shields-up",
            "Block Incoming",
            tailscale.shields_up(),
            Tailscale::set_shields_up,
        ),
    ];
    let items = rows
        .into_iter()
        .map(|(id, label, on, set)| {
            ListItem::new(format!("amane.qs.tailscale.{id}"), label)
                .trailing(
                    Switch::new(format!("amane.qs.tailscale.{id}.switch"), on)
                        .on_toggle(set)
                        .build(),
                )
                .on_press(move || set(!on))
        })
        .collect();
    boxed(List::new(items).header("Options").build())
}

/// The tailnet's other devices, online first; a click copies the address.
fn devices(peers: &[Peer]) -> Box<dyn Widget> {
    let items = peers
        .iter()
        .map(|peer| {
            let mut item = ListItem::new(
                format!("amane.qs.tailscale.device.{}", peer.dns),
                &peer.name,
            )
            .leading(os_glyph(&peer.os))
            .color(if peer.online {
                material::label(Level::Primary)
            } else {
                material::label(Level::Secondary)
            });
            if let Some(ip) = peer.ip.clone() {
                item = item
                    .trailing(list::trailing_text(
                        ip.clone(),
                        material::label(Level::Secondary),
                    ))
                    .on_press(move || Tailscale::copy(&ip));
            }
            item
        })
        .collect();
    boxed(List::new(items).header("Devices").build())
}
