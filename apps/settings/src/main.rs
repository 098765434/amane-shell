//! Shell Settings: changes amane-shell's settings file, which the running
//! shell follows at once, and, under System, the NixOS configuration the
//! machine is built from. One copy runs; launching it again brings it up.

mod about;
mod appearance;
mod desktop;
mod displays;
mod dock;
mod launcher;
mod previews;
mod quick_settings;
mod status_bar;
mod system;

use amane::{Center, Column, Key, Rectangle, Row, ScrollArea, Service, Size, Text, Widget, Window};
use amane_kit::app::{Dialog, Page, app, app_window, sidebar, standard_shortcut};
use amane_kit::buttons::Switch;
use amane_kit::color::Accent;
use amane_kit::form::{FormRow, Group};
use amane_kit::icons::{
    ICON_APPS, ICON_COMPUTER, ICON_DARK_MODE, ICON_DISPLAY, ICON_DNS, ICON_HISTORY, ICON_INFO,
    ICON_LAYERS, ICON_LOCK, ICON_MORE_HORIZ, ICON_PACKAGE, ICON_SEARCH, ICON_SETTINGS,
    ICON_STORAGE, ICON_TUNE, ICON_UPDATE,
};
use amane_kit::layout::boxed;
use amane_kit::macform::MacSegmentedNav;
use amane_kit::macform::theme as mac;
use amane_kit::material::{self, Level};
use amane_kit::tokens::{Styled, sys};

use system::apply_bar;
use system::job::BuildJob;

/// A view that takes a page's place, reached from it; back returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    /// What an Apply changes.
    Changes,
    /// The build's log.
    Log,
    /// A package's details, from the Apps page.
    Package,
    /// One of the page's own sub-pages, by name ("clock" on Status Bar).
    Sub(&'static str),
}

/// Where the window is: a page, and the views opened over it, the last
/// showing.
#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub page: usize,
    pub views: Vec<View>,
}

impl Place {
    fn page(page: usize) -> Self {
        Self {
            page,
            views: Vec::new(),
        }
    }

    pub fn view(&self) -> Option<View> {
        self.views.last().copied()
    }
}

/// How many places back remembers.
const HISTORY: usize = 50;

// where the window is and has been, kept while the app runs
pub struct Navigation {
    history: Vec<Place>,
    at: usize,
    /// The generation picked in History.
    pub selected_generation: Option<u32>,
}

impl Service for Navigation {
    fn new() -> Self {
        Self {
            history: vec![Place::page(1)],
            at: 0,
            selected_generation: None,
        }
    }

    // only clicks change it
    fn listen() {}
}

impl Navigation {
    pub fn here(&self) -> &Place {
        &self.history[self.at]
    }

    /// Goes to `place`, forgetting what was forward of here.
    fn go_to(&mut self, place: Place) {
        if *self.here() == place {
            return;
        }
        self.history.truncate(self.at + 1);
        self.history.push(place);
        if self.history.len() > HISTORY {
            self.history.remove(0);
        }
        self.at = self.history.len() - 1;
    }

    fn set_page(&mut self, page: usize) {
        self.go_to(Place::page(page));
    }

    fn open(&mut self, view: View) {
        let mut place = self.here().clone();
        if place.view() != Some(view) {
            place.views.push(view);
        }
        self.go_to(place);
    }

    pub fn can_back(&self) -> bool {
        self.at > 0
    }

    pub fn can_forward(&self) -> bool {
        self.at + 1 < self.history.len()
    }

    fn back(&mut self) {
        self.at = self.at.saturating_sub(1);
    }

    fn forward(&mut self) {
        if self.can_forward() {
            self.at += 1;
        }
    }
}

/// Opens `view` over the page showing.
pub fn open(view: View) {
    Navigation::write().open(view);
}

/// Back to where the window was before.
pub fn back() {
    let mut nav = Navigation::write();
    if nav.can_back() {
        nav.back();
    } else {
        nav.quiet();
    }
}

fn forward() {
    let mut nav = Navigation::write();
    if nav.can_forward() {
        nav.forward();
    } else {
        nav.quiet();
    }
}

// the sidebar's rows: two sections, each over its pages
const DISPLAYS: usize = 3;
const SYS_ABOUT: usize = 10;
const SYS_GENERAL: usize = 11;
const SYS_APPS: usize = 12;
const SYS_SERVICES: usize = 13;
const SYS_POWER: usize = 14;
const SYS_STORAGE: usize = 15;
const SYS_HISTORY: usize = 16;
const SYS_UPDATE: usize = 17;

/// The page `--page <name>` opens: the sidebar's names, lowercased, or a
/// System page's short name.
fn page_named(name: &str) -> Option<usize> {
    Some(match name.trim().to_lowercase().as_str() {
        "appearance" => 1,
        "desktop" => 2,
        "displays" | "display" | "monitors" => DISPLAYS,
        "status-bar" | "bar" => 4,
        "dock" => 5,
        "quick-settings" => 6,
        "spotlight" | "launcher" => 7,
        "about-shell" => 8,
        "about" | "system" | "computer" => SYS_ABOUT,
        "general" => SYS_GENERAL,
        "apps" | "software" => SYS_APPS,
        "services" => SYS_SERVICES,
        "power" | "lock" => SYS_POWER,
        "storage" => SYS_STORAGE,
        "history" => SYS_HISTORY,
        "update" | "updates" => SYS_UPDATE,
        _ => return None,
    })
}

/// The sub-pages a `--page` name can reach, as `page/sub`.
const SUBS: [&str; 5] = ["clock", "weather", "items", "files", "advanced"];

/// A place by name: a page's name, or `page/sub` for one of its sub-pages.
fn place_named(name: &str) -> Option<Place> {
    let (page, sub) = match name.split_once('/') {
        Some((page, sub)) => (page, Some(sub)),
        None => (name, None),
    };
    let mut place = Place::page(page_named(page)?);
    if let Some(sub) = sub {
        let sub = SUBS
            .iter()
            .copied()
            .find(|known| known.eq_ignore_ascii_case(sub.trim()))?;
        sub_page(place.page, sub)?;
        place.views.push(View::Sub(sub));
    }
    Some(place)
}

/// Shows the place `--page <name>` would, from a page's own button.
pub fn go(name: &str) {
    if let Some(place) = place_named(name) {
        Navigation::write().go_to(place);
    }
}

/// The place named after `--page` in `args`, if any.
fn page_arg(args: &[String]) -> Option<Place> {
    flag(args, "--page").and_then(|name| place_named(&name))
}

/// `--name value` or `--name=value`.
fn flag(args: &[String], name: &str) -> Option<String> {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if let Some(value) = arg
            .strip_prefix(name)
            .and_then(|rest| rest.strip_prefix('='))
        {
            return Some(value.to_string());
        }
        if arg == name {
            return args.next().cloned();
        }
    }
    None
}

/// `--package <attr>`: the Apps page, with that nixpkgs package's details.
fn package_arg(args: &[String]) {
    if let Some(attr) = flag(args, "--package") {
        Navigation::write().set_page(SYS_APPS);
        system::package::Details::open(&attr);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(place) = page_arg(&args) {
        *Navigation::write() = Navigation {
            history: vec![place],
            at: 0,
            selected_generation: None,
        };
    }
    package_arg(&args);
    app("amane-settings")
        .normal_window("main", main_window)
        // launched again (Spotlight, a notification) while running: show that page
        .on_activate(|args| {
            if let Some(place) = page_arg(args) {
                Navigation::write().go_to(place);
            }
            package_arg(args);
        })
        .run();
}

fn pages() -> [Page; 18] {
    [
        Page::section("Shell"),
        Page::new(ICON_DARK_MODE, "Appearance").accent(Accent::Blue),
        Page::new(ICON_APPS, "Desktop").accent(Accent::Cyan),
        Page::new(ICON_DISPLAY, "Displays").accent(Accent::Teal),
        Page::new(ICON_MORE_HORIZ, "Status Bar").accent(Accent::Gray),
        Page::new(ICON_LAYERS, "Dock").accent(Accent::Indigo),
        Page::new(ICON_TUNE, "Quick Settings").accent(Accent::Gray),
        Page::new(ICON_SEARCH, "Spotlight").accent(Accent::Purple),
        Page::new(ICON_COMPUTER, "About").accent(Accent::Gray),
        Page::section("System"),
        Page::new(ICON_INFO, "About This Computer").accent(Accent::Gray),
        Page::new(ICON_SETTINGS, "General").accent(Accent::Gray),
        Page::new(ICON_PACKAGE, "Apps").accent(Accent::Blue),
        Page::new(ICON_DNS, "Services").accent(Accent::Green),
        Page::new(ICON_LOCK, "Power & Lock").accent(Accent::Orange),
        Page::new(ICON_STORAGE, "Storage").accent(Accent::Gray),
        Page::new(ICON_HISTORY, "History").accent(Accent::Brown),
        Page::new(ICON_UPDATE, "Software Update").accent(Accent::Red),
    ]
}

/// Closing while a build runs would end it (nix stops a build whose client
/// goes away), so ask first.
fn may_close() -> bool {
    if !BuildJob::read().running() {
        return true;
    }
    Dialog::new("settings.close-dialog", "Stop the build?")
        .message("Closing Shell Settings stops the build. Nothing has been applied yet.")
        .button("Keep Building", || {})
        .destructive("Stop", || {
            BuildJob::cancel();
            amane::close_window("main");
        })
        .open("main");
    false
}

fn shortcut(key: Key) -> bool {
    let held = amane::modifiers();
    // Ctrl+W goes through the same question as the compositor's close
    if key == Key::Character('w') && held.ctrl && BuildJob::read().running() {
        return !may_close();
    }
    // back and forward, as a browser's
    if held.alt && !held.ctrl && key == Key::Left {
        back();
        return true;
    }
    if held.alt && !held.ctrl && key == Key::Right {
        forward();
        return true;
    }
    // Escape leaves a view for the page under it
    if key == Key::Escape && !held.ctrl && !held.alt {
        let in_view = Navigation::read().here().view().is_some();
        if in_view {
            back();
            return true;
        }
    }
    standard_shortcut(key)
}

/// A page's sub-page: its title and body, or None for a name it doesn't
/// have.
fn sub_page(page: usize, name: &str) -> Option<(String, Box<dyn Widget>)> {
    match page {
        4 => status_bar::sub(name),
        DISPLAYS => displays::sub(name),
        8 => about::sub(name),
        SYS_ABOUT => system::about::sub(name),
        _ => None,
    }
}

/// What shows in the page's place: its title and body, scrolled under a
/// name of its own.
fn content(page: usize, view: Option<View>) -> (String, Box<dyn Widget>) {
    let scrolled = |id: &'static str, body: Box<dyn Widget>| -> Box<dyn Widget> {
        boxed(
            ScrollArea::new(
                id,
                Column::new(vec![body]).width(Size::Parent).align(Center),
            )
            .width(Size::Parent)
            .height(Size::Parent),
        )
    };
    match view {
        Some(View::Changes) => (
            String::from("Changes"),
            scrolled("settings.scroll.changes", boxed(apply_bar::changes_view())),
        ),
        // the log scrolls itself
        Some(View::Log) => (String::from("Build Log"), boxed(apply_bar::log_view())),
        Some(View::Package) => (
            system::package::title(),
            scrolled("settings.scroll.package", boxed(system::package::view())),
        ),
        Some(View::Sub(name)) => match sub_page(page, name) {
            Some((title, body)) => (title, scrolled(sub_scroll(page, name), body)),
            None => content(page, None),
        },
        None => {
            let (title, scroll, body): (&str, &'static str, Box<dyn Widget>) = match page {
                1 => (
                    "Appearance",
                    "settings.scroll.appearance",
                    boxed(appearance::page()),
                ),
                2 => ("Desktop", "settings.scroll.desktop", boxed(desktop::page())),
                DISPLAYS => (
                    "Displays",
                    "settings.scroll.displays",
                    boxed(displays::page()),
                ),
                4 => (
                    "Status Bar",
                    "settings.scroll.status-bar",
                    boxed(status_bar::page()),
                ),
                5 => ("Dock", "settings.scroll.dock", boxed(dock::page())),
                6 => (
                    "Quick Settings",
                    "settings.scroll.quick-settings",
                    boxed(quick_settings::page()),
                ),
                7 => (
                    "Spotlight",
                    "settings.scroll.spotlight",
                    boxed(launcher::page()),
                ),
                SYS_ABOUT => (
                    "About This Computer",
                    "settings.scroll.system-about",
                    boxed(system::about::page()),
                ),
                SYS_GENERAL => (
                    "General",
                    "settings.scroll.system-general",
                    boxed(system::general::page()),
                ),
                SYS_APPS => (
                    "Apps",
                    "settings.scroll.system-apps",
                    boxed(system::apps::page()),
                ),
                SYS_SERVICES => (
                    "Services",
                    "settings.scroll.system-services",
                    boxed(system::services::page()),
                ),
                SYS_POWER => (
                    "Power & Lock",
                    "settings.scroll.system-power",
                    boxed(system::power::page()),
                ),
                SYS_HISTORY => (
                    "History",
                    "settings.scroll.system-history",
                    boxed(system::history::page()),
                ),
                SYS_STORAGE => (
                    "Storage",
                    "settings.scroll.system-storage",
                    boxed(system::storage::page()),
                ),
                SYS_UPDATE => (
                    "Software Update",
                    "settings.scroll.system-update",
                    boxed(system::update::page()),
                ),
                _ => ("About", "settings.scroll.about", boxed(about::page())),
            };
            (String::from(title), scrolled(scroll, body))
        }
    }
}

/// The scroll id a sub-page keeps its offset under: one per page and name,
/// made once.
fn sub_scroll(page: usize, name: &str) -> &'static str {
    use std::cell::RefCell;
    use std::collections::HashMap;
    thread_local! {
        static IDS: RefCell<HashMap<(usize, String), &'static str>> =
            RefCell::new(HashMap::new());
    }
    IDS.with_borrow_mut(|ids| {
        *ids.entry((page, name.to_string()))
            .or_insert_with(|| Box::leak(format!("settings.scroll.{page}.{name}").into_boxed_str()))
    })
}

fn main_window() -> Window {
    let (place, can_back, can_forward) = {
        let nav = Navigation::read();
        (nav.here().clone(), nav.can_back(), nav.can_forward())
    };
    let page = place.page;
    let view = place.view();

    let pages = pages();
    // its poll rests while another page shows
    displays::showing(page == DISPLAYS && view.is_none());

    let nav = sidebar("settings.nav", &pages, page, |page| {
        Navigation::write().set_page(page);
    });

    let (title, body) = content(page, view);

    let mut column: Vec<Box<dyn Widget>> = vec![
        boxed(header(
            &title,
            can_back,
            can_forward,
            toolbar_action(page, view),
        )),
        body,
    ];
    if let Some(bar) = apply_bar::apply_bar() {
        column.push(bar);
    }

    let content = Row::new(vec![
        boxed(
            Column::new(vec![boxed(nav)])
                .width(Size::Fixed(SIDEBAR))
                .height(Size::Parent),
        ),
        boxed(
            Column::new(column)
                .gap(sys::space::gap::m())
                .width(Size::Parent)
                .height(Size::Parent),
        ),
    ])
    .gap(sys::space::gap::xl())
    .width(Size::Parent)
    .height(Size::Parent);

    app_window(content)
        .title("Shell Settings")
        .size(760.0, 760.0)
        .min_size(
            SIDEBAR + MIN_PAGE + sys::space::gap::xl() + 2.0 * sys::space::inset::l(),
            600.0,
        )
        .on_shortcut(shortcut)
        .on_close(may_close)
}

/// A page's own button in the toolbar, such as Quick Settings' Customize.
fn toolbar_action(page: usize, view: Option<View>) -> Option<Box<dyn Widget>> {
    if view.is_some() {
        return None;
    }
    match page {
        6 => quick_settings::toolbar(),
        _ => None,
    }
}

/// The toolbar over the page: back and forward, the page's title, and any
/// button of the page's own at the trailing edge.
fn header(
    title: &str,
    can_back: bool,
    can_forward: bool,
    action: Option<Box<dyn Widget>>,
) -> Rectangle {
    let lead = Row::new(vec![
        boxed(
            MacSegmentedNav::new("settings.header.nav")
                .back_enabled(can_back)
                .forward_enabled(can_forward)
                .on_back(back)
                .on_forward(forward)
                .build(),
        ),
        boxed(
            Text::new(title)
                .style(mac::page_title_font())
                .color(mac::label_primary())
                .elide(),
        ),
    ])
    .gap(12.0)
    .align(Center);
    let mut parts: Vec<Box<dyn Widget>> = vec![boxed(lead)];
    if let Some(action) = action {
        parts.push(action);
    }
    Rectangle::new()
        .width(Size::Parent)
        .height(Size::Fixed(TOOLBAR))
        .fill(amane::Color::TRANSPARENT)
        .align_child(amane::Start, Center)
        .child(
            Row::new(parts)
                .width(Size::Parent)
                .justify(amane::SpaceBetween)
                .align(Center),
        )
}

/// The toolbar's height.
const TOOLBAR: f32 = 36.0;

/// The sidebar's width.
const SIDEBAR: f32 = 200.0;

/// The narrowest and widest a page's groups get.
const MIN_PAGE: f32 = 440.0;
const MAX_PAGE: f32 = 680.0;

/// How wide a page's groups are in a window `window` wide: what's left
/// beside the sidebar, kept between [`MIN_PAGE`] and [`MAX_PAGE`]; a wider
/// window centres them.
fn page_width_in(window: f32) -> f32 {
    if window <= 0.0 {
        return amane_kit::form::DEFAULT_WIDTH;
    }
    let room = window - 2.0 * sys::space::inset::l() - SIDEBAR - sys::space::gap::xl();
    room.clamp(MIN_PAGE, MAX_PAGE)
}

/// How wide a page's groups are in this window.
pub fn page_width() -> f32 {
    page_width_in(amane::window_size().0)
}

/// A line of explanation under a setting, in the caption's grey.
pub fn note(text: impl Into<String>) -> Text {
    Text::new(text)
        .style(sys::r#type::caption())
        .color(material::label(Level::Secondary))
}

/// A group of settings rows as wide as a page.
pub fn group() -> Group {
    Group::new().width(page_width())
}

/// A page: its groups, a little apart.
pub fn groups(groups: Vec<Group>) -> Column {
    Column::new(
        groups
            .into_iter()
            .filter(|group| !group.is_empty())
            .map(|group| boxed(group.build()))
            .collect(),
    )
    .gap(sys::space::gap::l())
}

/// A row with a switch at its trailing edge.
pub fn switch_row(id: &str, title: &str, on: bool, toggle: impl Fn(bool) + 'static) -> FormRow {
    FormRow::new(id, title).trailing(
        Switch::new(format!("{id}.switch"), on)
            .on_toggle(toggle)
            .build(),
    )
}

/// Something that went wrong, said plainly: a warning tile, what it means
/// for the page, and Details for what the tool itself said.
pub fn problem_row(id: &str, title: &str, meaning: &str, details: &str) -> FormRow {
    let shown = details.trim().to_string();
    let heading = title.to_string();
    let mut row = FormRow::new(id, title)
        .icon(amane_kit::icons::ICON_WARNING, Accent::Orange)
        .subtitle(meaning);
    if !shown.is_empty() {
        row = row.trailing(
            amane_kit::buttons::PushButton::new(format!("{id}.details"), "Details…")
                .on_press(move || {
                    Dialog::new("settings.problem-details", heading.clone())
                        .message(shown.clone())
                        .button("OK", || {})
                        .open("main");
                })
                .build(),
        );
    }
    row
}

/// How tall a column of `count` radios is, for a row's detail.
pub fn radios_height(count: usize) -> f32 {
    count as f32 * mac::control_height()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_and_forward_walk_the_places() {
        let mut nav = Navigation::new();
        assert!(!nav.can_back());
        nav.set_page(4);
        nav.open(View::Sub("clock"));
        assert_eq!(nav.here().view(), Some(View::Sub("clock")));
        nav.back();
        assert_eq!(nav.here(), &Place::page(4));
        assert!(nav.can_forward());
        nav.forward();
        assert_eq!(nav.here().view(), Some(View::Sub("clock")));
        nav.back();
        nav.back();
        assert_eq!(nav.here().page, 1);
        // going somewhere new forgets what was forward
        nav.set_page(5);
        assert!(!nav.can_forward());
        // the same place twice is one step
        nav.set_page(5);
        nav.back();
        assert_eq!(nav.here().page, 1);
    }

    #[test]
    fn a_page_fills_the_room_between_its_limits() {
        assert_eq!(page_width_in(0.0), amane_kit::form::DEFAULT_WIDTH);
        let room = 760.0 - 2.0 * sys::space::inset::l() - SIDEBAR - sys::space::gap::xl();
        assert_eq!(page_width_in(760.0), room.clamp(MIN_PAGE, MAX_PAGE));
        assert!(page_width_in(900.0) > page_width_in(760.0));
        assert_eq!(page_width_in(600.0), MIN_PAGE);
        assert_eq!(page_width_in(2000.0), MAX_PAGE);
    }
}
