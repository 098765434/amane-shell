//! Shell Settings: changes amane-shell's settings file, which the running
//! shell follows at once, and, under System, the NixOS configuration the
//! machine is built from. One copy runs; launching it again brings it up.

mod about;
mod appearance;
mod desktop;
mod displays;
mod dock;
mod drop;
mod launcher;
mod passwords;
mod previews;
mod quick_settings;
mod search;
mod status_bar;
mod system;

use amane::{
    Center, Column, Key, Rectangle, Row, ScrollArea, Service, Size, Text, TextInput, Widget, Window,
};
use amane_kit::app::{
    Dialog, Page, app, app_window, field_box, sidebar_with, standard_shortcut, text_field,
};
use amane_kit::buttons::Switch;
use amane_kit::color::Accent;
use amane_kit::form::{FormRow, Group};
use amane_kit::icons::{
    ICON_APPS, ICON_COMPUTER, ICON_DARK_MODE, ICON_DISPLAY, ICON_DNS, ICON_HISTORY, ICON_INFO,
    ICON_LAYERS, ICON_LOCK, ICON_MORE_HORIZ, ICON_PACKAGE, ICON_SEARCH, ICON_SETTINGS,
    ICON_STORAGE, ICON_TRAY, ICON_TUNE, ICON_UPDATE,
};
use amane_kit::layout::boxed;
use amane_kit::material::{self, Level};
use amane_kit::nav_buttons::NavButtons;
use amane_kit::style;
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
const APPEARANCE: usize = 1;
const DESKTOP: usize = 2;
const DISPLAYS: usize = 3;
const STATUS_BAR: usize = 4;
const DOCK: usize = 5;
const QUICK_SETTINGS: usize = 6;
const SPOTLIGHT: usize = 7;
const DROP: usize = 8;
const PASSWORDS: usize = 9;
const ABOUT_SHELL: usize = 10;
const SYS_ABOUT: usize = 12;
const SYS_GENERAL: usize = 13;
const SYS_APPS: usize = 14;
const SYS_DEFAULTS: usize = 15;
const SYS_SERVICES: usize = 16;
const SYS_POWER: usize = 17;
const SYS_STORAGE: usize = 18;
const SYS_HISTORY: usize = 19;
const SYS_UPDATE: usize = 20;

/// The page `--page <name>` opens: the sidebar's names, lowercased, or a
/// System page's short name.
pub fn page_named(name: &str) -> Option<usize> {
    Some(match name.trim().to_lowercase().as_str() {
        "appearance" => APPEARANCE,
        "desktop" => DESKTOP,
        "displays" | "display" | "monitors" => DISPLAYS,
        "status-bar" | "bar" => STATUS_BAR,
        "dock" => DOCK,
        "quick-settings" => QUICK_SETTINGS,
        "spotlight" | "launcher" => SPOTLIGHT, // lint-terms: allow (an old name, for keybinds)
        "drop" | "amane-drop" => DROP,
        "passwords" | "bitwarden" => PASSWORDS,
        "about-shell" => ABOUT_SHELL,
        "about" | "system" | "computer" => SYS_ABOUT,
        "general" => SYS_GENERAL,
        "apps" | "software" => SYS_APPS,
        "default-apps" | "defaults" | "open-with" => SYS_DEFAULTS,
        "services" => SYS_SERVICES,
        "power" | "lock" => SYS_POWER,
        "storage" => SYS_STORAGE,
        "history" => SYS_HISTORY,
        "update" | "updates" => SYS_UPDATE,
        _ => return None,
    })
}

/// Each page's `--page` name, in the sidebar's order: the one the search
/// index and `page_named` go by.
pub const PAGE_NAMES: [(&str, usize); 19] = [
    ("appearance", APPEARANCE),
    ("desktop", DESKTOP),
    ("displays", DISPLAYS),
    ("status-bar", STATUS_BAR),
    ("dock", DOCK),
    ("quick-settings", QUICK_SETTINGS),
    ("spotlight", SPOTLIGHT), // lint-terms: allow (an old name, for keybinds)
    ("drop", DROP),
    ("passwords", PASSWORDS),
    ("about-shell", ABOUT_SHELL),
    ("about", SYS_ABOUT),
    ("general", SYS_GENERAL),
    ("apps", SYS_APPS),
    ("default-apps", SYS_DEFAULTS),
    ("services", SYS_SERVICES),
    ("power", SYS_POWER),
    ("storage", SYS_STORAGE),
    ("history", SYS_HISTORY),
    ("update", SYS_UPDATE),
];

/// A page's title in the sidebar.
pub fn page_title(page: usize) -> String {
    pages()
        .get(page)
        .map_or_else(String::new, |page| page.name.clone())
}

/// The sub-pages a `--page` name can reach as `page/sub`, by page.
pub const SUBS: [(usize, &str); 8] = [
    (STATUS_BAR, "clock"),
    (STATUS_BAR, "weather"),
    (STATUS_BAR, "items"),
    (STATUS_BAR, "activities"),
    (DISPLAYS, "advanced"),
    (ABOUT_SHELL, "files"),
    (SYS_ABOUT, "advanced"),
    (SYS_DEFAULTS, "file-types"),
];

/// A place by name: a page's name, or `page/sub` for one of its sub-pages.
pub fn place_named(name: &str) -> Option<Place> {
    let (page, sub) = match name.split_once('/') {
        Some((page, sub)) => (page, Some(sub)),
        None => (name, None),
    };
    let mut place = Place::page(page_named(page)?);
    if let Some(sub) = sub {
        let (_, sub) = SUBS
            .iter()
            .copied()
            .find(|(on, known)| *on == place.page && known.eq_ignore_ascii_case(sub.trim()))?;
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
    amane_log::init("amane-settings");
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
        // launched again (the launcher, a notification) while running: show that page
        .on_activate(|args| {
            if let Some(place) = page_arg(args) {
                Navigation::write().go_to(place);
            }
            package_arg(args);
        })
        .run();
}

fn pages() -> [Page; 21] {
    [
        Page::section("Shell"),
        Page::new(ICON_DARK_MODE, "Appearance").accent(Accent::Blue),
        Page::new(ICON_APPS, "Desktop").accent(Accent::Cyan),
        Page::new(ICON_DISPLAY, "Displays").accent(Accent::Teal),
        Page::new(ICON_MORE_HORIZ, "Status Bar").accent(Accent::Gray),
        Page::new(ICON_LAYERS, "Dock").accent(Accent::Indigo),
        Page::new(ICON_TUNE, "Quick Settings").accent(Accent::Gray),
        Page::new(ICON_SEARCH, "Launcher").accent(Accent::Purple),
        Page::new(ICON_TRAY, "Drop").accent(Accent::Orange),
        Page::new(ICON_LOCK, "Passwords").accent(Accent::Gray),
        Page::new(ICON_COMPUTER, "About").accent(Accent::Gray),
        Page::section("System"),
        Page::new(ICON_INFO, "About This Computer").accent(Accent::Gray),
        Page::new(ICON_SETTINGS, "General").accent(Accent::Gray),
        Page::new(ICON_PACKAGE, "Apps").accent(Accent::Blue),
        Page::new(amane_kit::icons::ICON_OPEN_IN_NEW, "Default Apps").accent(Accent::Purple),
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
    // Ctrl+F searches
    if held.ctrl && !held.alt && key == Key::Character('f') {
        search::focus();
        drop(Navigation::write());
        return true;
    }
    // Escape clears a search, else leaves a view for the page under it
    if key == Key::Escape && !held.ctrl && !held.alt {
        if search::clear() {
            return true;
        }
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
        STATUS_BAR => status_bar::sub(name),
        SYS_DEFAULTS => system::defaults::sub(name),
        DISPLAYS => displays::sub(name),
        ABOUT_SHELL => about::sub(name),
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
                APPEARANCE => (
                    "Appearance",
                    "settings.scroll.appearance",
                    boxed(appearance::page()),
                ),
                DESKTOP => ("Desktop", "settings.scroll.desktop", boxed(desktop::page())),
                DISPLAYS => (
                    "Displays",
                    "settings.scroll.displays",
                    boxed(displays::page()),
                ),
                STATUS_BAR => (
                    "Status Bar",
                    "settings.scroll.status-bar",
                    boxed(status_bar::page()),
                ),
                DOCK => ("Dock", "settings.scroll.dock", boxed(dock::page())),
                QUICK_SETTINGS => (
                    "Quick Settings",
                    "settings.scroll.quick-settings",
                    boxed(quick_settings::page()),
                ),
                SPOTLIGHT => (
                    "Launcher",
                    "settings.scroll.spotlight",
                    boxed(launcher::page()),
                ),
                DROP => ("Drop", "settings.scroll.drop", boxed(drop::page())),
                PASSWORDS => (
                    "Passwords",
                    "settings.scroll.passwords",
                    boxed(passwords::page()),
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
                SYS_DEFAULTS => (
                    "Default Apps",
                    "settings.scroll.system-defaults",
                    boxed(system::defaults::page()),
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

    // a search shows what it finds in the page's place, and narrows the
    // sidebar to the pages with something found
    let query = TextInput::text(search::FIELD);
    let found = search::find(&query, &search::system_entries());
    let searching = !query.trim().is_empty();
    let shown: Vec<bool> = if searching {
        (0..pages.len())
            .map(|index| {
                found.iter().any(|found| {
                    let page = found.place.split('/').next().unwrap_or_default();
                    page_named(page) == Some(index)
                })
            })
            .collect()
    } else {
        Vec::new()
    };
    let nav = sidebar_with(
        "settings.nav",
        &pages,
        page,
        |page| {
            TextInput::set_text(search::FIELD, "");
            Navigation::write().set_page(page);
        },
        Some(search_field()),
        &shown,
    );

    amane_kit::form::highlight_row(search::lit_row());
    let (title, body) = if searching {
        (String::from("Search"), results(&query, &found))
    } else {
        content(page, view)
    };

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

/// The field over the sidebar's pages.
fn search_field() -> Box<dyn Widget> {
    let mut input = text_field(search::FIELD, "Search")
        .on_change(|_| drop(Navigation::write()))
        .on_submit(|text| {
            if let Some(first) = search::find(&text, &search::system_entries()).first() {
                search::open(first);
            }
        });
    if search::take_focus() {
        input = input.focused();
    }
    boxed(
        Rectangle::new()
            .width(Size::Parent)
            .height(Size::Fixed(style::control_height() + sys::space::gap::s()))
            .fill(amane::Color::TRANSPARENT)
            .align_child(amane::Start, amane::Start)
            .child(field_box(
                search::FIELD,
                input,
                SIDEBAR - 2.0 * sys::space::inset::s(),
            )),
    )
}

/// What a search found, each a row that opens it; Return opens the first.
fn results(query: &str, found: &[search::Found]) -> Box<dyn Widget> {
    let rows: Vec<FormRow> = found
        .iter()
        .enumerate()
        .map(|(index, found)| {
            let opened = found.clone();
            FormRow::new(
                format!("settings.search.result.{index}"),
                found.title.clone(),
            )
            .subtitle(found.path.clone())
            .on_press(move || search::open(&opened))
        })
        .collect();
    let list = if rows.is_empty() {
        group().row(
            FormRow::new(
                "settings.search.none",
                format!("Nothing found for “{}”", query.trim()),
            )
            .subtitle("Try another word, like dark mode, wifi or keyboard"),
        )
    } else {
        group().rows(rows).footer("Return opens the first.")
    };
    boxed(
        ScrollArea::new(
            "settings.scroll.search",
            Column::new(vec![boxed(groups(vec![list]))])
                .width(Size::Parent)
                .align(Center),
        )
        .width(Size::Parent)
        .height(Size::Parent),
    )
}

/// A page's own button in the toolbar, such as Quick Settings' Customize.
fn toolbar_action(page: usize, view: Option<View>) -> Option<Box<dyn Widget>> {
    if view.is_some() {
        return None;
    }
    match page {
        QUICK_SETTINGS => quick_settings::toolbar(),
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
            NavButtons::new("settings.header.nav")
                .back_enabled(can_back)
                .forward_enabled(can_forward)
                .on_back(back)
                .on_forward(forward)
                .build(),
        ),
        boxed(
            Text::new(title)
                .style(style::page_title_font())
                .color(style::label_primary())
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
    count as f32 * style::control_height()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_and_forward_walk_the_places() {
        let mut nav = Navigation::new();
        assert!(!nav.can_back());
        nav.set_page(STATUS_BAR);
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
