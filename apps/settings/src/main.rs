//! Emaki Settings: changes emaki's settings file, which the running
//! shell follows at once, and, under System, the NixOS configuration the
//! machine is built from. One copy runs; launching it again brings it up.

mod about;
mod appearance;
mod cursor;
mod desktop;
mod desktop_widgets;
mod displays;
mod dock;
mod drop;
mod input;
mod launcher;
mod network;
mod niri_files;
mod niri_settings;
mod notifications;
mod pages;
mod passwords;
mod previews;
mod quick_settings;
mod screenshots;
mod search;
mod sound;
mod status_bar;
mod system;
mod windows;

use amane::{
    Center, Column, Key, Rectangle, Row, ScrollArea, Service, Size, Text, TextInput, Widget, Window,
};
use emaki_kit::app::{Dialog, app, app_window, sidebar_with, standard_shortcut, text_field};
use emaki_kit::args::flag;
use emaki_kit::buttons::Switch;
use emaki_kit::color::Accent;
use emaki_kit::form::{FormRow, Group};
use emaki_kit::layout::boxed;
use emaki_kit::nav_buttons::NavButtons;
use emaki_kit::style;
use emaki_kit::tokens::{Styled, sys};

use system::apply_bar;
use system::job::BuildJob;

/// The window's name, which tooltips and dialogs hang under.
pub const WINDOW: &str = "main";

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
        // a setting's error is about the page it was made on
        if place.page != self.here().page {
            let mut feedback = system::controls::Feedback::write();
            if feedback.error.is_some() {
                feedback.error = None;
            } else {
                feedback.quiet();
            }
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

/// A page's title in the sidebar.
pub fn page_title(page: usize) -> String {
    pages::def(page).map_or_else(String::new, |def| String::from(def.title))
}

/// A place by name: a page's name, or `page/sub` for one of its sub-pages.
pub fn place_named(name: &str) -> Option<Place> {
    let name = pages::moved(name).unwrap_or(name);
    let (page, sub) = match name.split_once('/') {
        Some((page, sub)) => (page, Some(sub)),
        None => (name, None),
    };
    let mut place = Place::page(pages::named(page)?);
    if let Some(sub) = sub {
        let (subs, _) = pages::def(place.page)?.subs;
        let sub = subs
            .iter()
            .find(|known| known.eq_ignore_ascii_case(sub.trim()))?;
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

/// `--package <attr>`: the Apps page, with that nixpkgs package's details.
fn package_arg(args: &[String]) {
    if let Some(attr) = flag(args, "--package") {
        Navigation::write().set_page(pages::named("apps").unwrap_or_default());
        system::package::Details::open(&attr);
    }
}

fn main() {
    emaki_log::init(emaki_kit::app_ids::SETTINGS);
    emaki_kit::migrate::from_amane();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(place) = page_arg(&args) {
        *Navigation::write() = Navigation {
            history: vec![place],
            at: 0,
            selected_generation: None,
        };
    }
    package_arg(&args);
    app(emaki_kit::app_ids::SETTINGS)
        .normal_window("main", main_window)
        // icon-only buttons name themselves on hover
        .popup(emaki_kit::tooltip::popup)
        // launched again (the launcher, a notification) while running: show that page
        .on_activate(|args| {
            if let Some(place) = page_arg(args) {
                Navigation::write().go_to(place);
            }
            package_arg(args);
        })
        .run();
}

/// Closing while a build runs would end it (nix stops a build whose client
/// goes away), so ask first. While the built system is being activated it
/// can't be stopped halfway, so the window stays until it's done.
fn may_close() -> bool {
    let activating = {
        let job = BuildJob::read();
        if !job.running() {
            return true;
        }
        matches!(job.phase, system::job::Phase::Activating { .. })
    };
    if activating {
        Dialog::new("settings.close-dialog", "Applying the system")
            .message(
                "Emaki Settings can close once the system is applied. Stopping now could leave it half changed.",
            )
            .main_button("OK", || {})
            .open("main");
        return false;
    }
    Dialog::new("settings.close-dialog", "Stop the build?")
        .message("Closing Emaki Settings stops the build. Nothing has been applied yet.")
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
        Some(View::Log) => (String::from("Build log"), boxed(apply_bar::log_view())),
        Some(View::Package) => (
            system::package::title(),
            scrolled("settings.scroll.package", boxed(system::package::view())),
        ),
        Some(View::Sub(name)) => match pages::def(page).and_then(|def| (def.subs.1)(name)) {
            Some((title, body)) => (title, scrolled(sub_scroll(page, name), body)),
            None => content(page, None),
        },
        None => {
            // a page that's gone shows the first
            let Some(def) = pages::def(page).or_else(|| pages::all().next().map(|(_, def)| def))
            else {
                return (String::new(), emaki_kit::layout::nothing());
            };
            let scroll = emaki_kit::intern::intern(&format!("settings.scroll.{}", def.name));
            (
                String::from(def.title),
                scrolled(scroll, boxed((def.body)())),
            )
        }
    }
}

/// The scroll id a sub-page keeps its offset under: one per page and name.
fn sub_scroll(page: usize, name: &str) -> &'static str {
    let page = pages::def(page).map_or("", |def| def.name);
    emaki_kit::intern::intern(&format!("settings.scroll.{page}.{name}"))
}

fn main_window() -> Window {
    let (place, can_back, can_forward) = {
        let nav = Navigation::read();
        (nav.here().clone(), nav.can_back(), nav.can_forward())
    };
    let page = place.page;
    let view = place.view();

    let pages = pages::sidebar();
    // its poll rests while another page shows
    displays::showing(pages::is(page, "displays") && view.is_none());

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
                    pages::named(page) == Some(index)
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

    emaki_kit::form::highlight_row(search::lit_row());
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
    // what waits for Apply is the NixOS pages' business; a job under way
    // or just done shows wherever the window is
    let job_shown = BuildJob::read().busy();
    if (pages::nixos_page(page) || job_shown)
        && let Some(bar) = apply_bar::apply_bar()
    {
        column.push(bar);
    }

    let content = Row::new(vec![
        boxed(
            Column::new(vec![boxed(nav)])
                .width(SIDEBAR)
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

    app_window(emaki_kit::toast::over(content))
        .title("Emaki Settings")
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
            .height(style::control_height() + sys::space::gap::s())
            .align_child(amane::Start, amane::Start)
            .child(emaki_kit::app::search_box(
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

/// A page's own button in the toolbar, when it has one.
fn toolbar_action(page: usize, view: Option<View>) -> Option<Box<dyn Widget>> {
    if view.is_some() {
        return None;
    }
    pages::def(page)
        .and_then(|def| def.toolbar)
        .and_then(|toolbar| toolbar())
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
    .gap(sys::space::gap::xl())
    .align(Center);
    let mut parts: Vec<Box<dyn Widget>> = vec![boxed(lead)];
    if let Some(action) = action {
        parts.push(action);
    }
    Rectangle::new()
        .width(Size::Parent)
        .height(toolbar_height())
        .align_child(amane::Start, Center)
        .child(
            Row::new(parts)
                .width(Size::Parent)
                .justify(amane::SpaceBetween)
                .align(Center),
        )
}

/// The toolbar's height: a header bar's, 50 at the density's scale.
fn toolbar_height() -> f32 {
    style::px(50.0)
}

/// The sidebar's width.
const SIDEBAR: f32 = 200.0;

/// The widths a row's control takes, so menus and fields line up from page
/// to page: a stepper or tiny choice, a short menu, most menus, a menu of
/// long names (devices, modes), and a text field.
pub const COMPACT_W: f32 = 80.0;
pub const MENU_W_NARROW: f32 = 160.0;
pub const MENU_W: f32 = 200.0;
pub const MENU_W_WIDE: f32 = 260.0;
pub const FIELD_W: f32 = 240.0;

/// The narrowest and widest a page's groups get.
const MIN_PAGE: f32 = 440.0;
const MAX_PAGE: f32 = 680.0;

/// How wide a page's groups are in a window `window` wide: what's left
/// beside the sidebar, kept between [`MIN_PAGE`] and [`MAX_PAGE`]; a wider
/// window centres them.
fn page_width_in(window: f32) -> f32 {
    if window <= 0.0 {
        return emaki_kit::form::DEFAULT_WIDTH;
    }
    let room = window - 2.0 * sys::space::inset::l() - SIDEBAR - sys::space::gap::xl();
    room.clamp(MIN_PAGE, MAX_PAGE)
}

/// How wide a page's groups are in this window.
pub fn page_width() -> f32 {
    page_width_in(amane::window_size().0)
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
    .gap(style::section_gap())
}

/// A row with a switch at its trailing edge.
pub fn switch_row(id: &str, title: &str, on: bool, toggle: impl Fn(bool) + 'static) -> FormRow {
    FormRow::new(id, title).trailing(
        Switch::new(format!("{id}.switch"), on)
            .on_toggle(toggle)
            .build(),
    )
}

/// A switch that belongs to the row above: set in under it, and dimmed
/// and unclickable while that one is off (`enabled` false). A dimmed row
/// doesn't stop its controls taking clicks, so the switch is told too.
pub fn child_switch_row(
    id: &str,
    title: &str,
    on: bool,
    enabled: bool,
    toggle: impl Fn(bool) + 'static,
) -> FormRow {
    FormRow::new(id, title)
        .trailing(
            Switch::new(format!("{id}.switch"), on)
                .disabled(!enabled)
                .on_toggle(toggle)
                .build(),
        )
        .child()
        .disabled(!enabled)
}

/// Something that went wrong, said plainly: a warning tile, what it means
/// for the page, and Details for what the tool itself said.
pub fn problem_row(id: &str, title: &str, meaning: &str, details: &str) -> FormRow {
    let shown = details.trim().to_string();
    let heading = title.to_string();
    let mut row = FormRow::new(id, title)
        .icon(emaki_kit::icons::ICON_WARNING, Accent::Orange)
        .subtitle(meaning);
    if !shown.is_empty() {
        row = row.trailing(
            emaki_kit::buttons::PushButton::new(format!("{id}.details"), "Details…")
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

/// A change that didn't take: a [`problem_row`] with OK, which runs
/// `dismiss` to put it away.
pub fn dismissible_problem_row(
    id: &str,
    title: &str,
    meaning: &str,
    dismiss: impl Fn() + 'static,
) -> FormRow {
    problem_row(id, title, meaning, "").trailing(
        emaki_kit::buttons::PushButton::new(format!("{id}.dismiss"), "OK")
            .on_press(dismiss)
            .build(),
    )
}

/// A row whose value is still coming: a small spinner at its trailing
/// edge and the title in grey, the one way every page says it's reading.
pub fn loading_row(id: &str, title: &str) -> FormRow {
    FormRow::new(id, title).loading()
}

/// A row saying `who` isn't answering, what that means for the page, and
/// Try Again when `retry` is given: a warning tile, as [`problem_row`]'s.
pub fn unreachable_row(
    id: &str,
    who: &str,
    meaning: &str,
    retry: Option<Box<dyn Fn()>>,
) -> FormRow {
    let row = FormRow::new(id, format!("{who} isn't answering"))
        .icon(emaki_kit::icons::ICON_WARNING, Accent::Orange)
        .subtitle(meaning);
    match retry {
        Some(retry) => row.trailing(
            emaki_kit::buttons::PushButton::new(format!("{id}.retry"), "Try Again")
                .on_press(retry)
                .build(),
        ),
        None => row,
    }
}

/// A whole page that can't show yet, or has nothing to show: a status
/// page (a large faint glyph, a title, a line saying why, a button) in
/// the page's place, as wide as its groups.
pub fn status_page(page: emaki_kit::status_page::StatusPage) -> Column {
    Column::new(vec![boxed(page.width(page_width()).build())])
}

/// [`status_page`] for `who` not answering, with Try Again.
pub fn unreachable_page(id: &str, who: &str, meaning: &str, retry: impl Fn() + 'static) -> Column {
    status_page(
        emaki_kit::status_page::StatusPage::new(
            emaki_kit::icons::ICON_WARNING,
            format!("{who} isn't answering"),
        )
        .text(meaning)
        .action(
            emaki_kit::buttons::PushButton::new(format!("{id}.retry"), "Try Again")
                .on_press(retry)
                .build(),
        ),
    )
}

/// A list with nothing in it yet, said in one dim row inside its group.
pub fn empty_row(id: &str, text: impl Into<String>) -> FormRow {
    FormRow::new(id, text).disabled(true)
}

/// A row that keeps a long explanation out of a group's note: `title`
/// with Details…, which shows `text` under `heading` in a dialog.
pub fn details_row(id: &str, title: &str, heading: &str, text: &str) -> FormRow {
    let (heading, text) = (heading.to_string(), text.to_string());
    FormRow::new(id, title)
        .icon(emaki_kit::icons::ICON_INFO, Accent::Gray)
        .trailing(
            emaki_kit::buttons::PushButton::new(format!("{id}.details"), "Details…")
                .on_press(move || {
                    Dialog::new("settings.details-dialog", heading.clone())
                        .message(text.clone())
                        .button("OK", || {})
                        .open("main");
                })
                .build(),
        )
}

/// Something worth knowing that isn't wrong: an info tile in grey, so it
/// doesn't read as a warning.
pub fn info_row(id: &str, title: &str, text: &str) -> FormRow {
    FormRow::new(id, title)
        .icon(emaki_kit::icons::ICON_INFO, Accent::Gray)
        .subtitle(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_launcher_page_opens_by_its_old_name_too() {
        // the old name, which keybinds may still use
        assert_eq!(pages::named("spotlight"), pages::named("launcher")); // lint-terms: allow
    }

    #[test]
    fn back_and_forward_walk_the_places() {
        let mut nav = Navigation::new();
        assert!(!nav.can_back());
        let bar = pages::named("status-bar").unwrap();
        nav.set_page(bar);
        nav.open(View::Sub("clock"));
        assert_eq!(nav.here().view(), Some(View::Sub("clock")));
        nav.back();
        assert_eq!(nav.here(), &Place::page(bar));
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
        assert_eq!(page_width_in(0.0), emaki_kit::form::DEFAULT_WIDTH);
        let room = 760.0 - 2.0 * sys::space::inset::l() - SIDEBAR - sys::space::gap::xl();
        assert_eq!(page_width_in(760.0), room.clamp(MIN_PAGE, MAX_PAGE));
        assert!(page_width_in(900.0) > page_width_in(760.0));
        assert_eq!(page_width_in(600.0), MIN_PAGE);
        assert_eq!(page_width_in(2000.0), MAX_PAGE);
    }
}
