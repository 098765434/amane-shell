use std::fmt;
use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::{budget, lenient};

/// The bar's well-known name on the session bus.
///
/// What a program and the bar say to each other over the session bus: every
/// payload is one JSON string, so a script can send it with busctl too.
pub const BUS_NAME: &str = "org.emaki.StatusBar1";

/// The object path the bar serves its interface at.
pub const PATH: &str = "/org/emaki/StatusBar";

/// The interface for registering items and starting activities, and the one
/// the bar's `ItemEvent` and `ActivityEvent` signals come from.
pub const INTERFACE: &str = "org.emaki.StatusBar1";

/// The protocol's level: every payload carries it as `v`, and a bar refuses one above the
/// level it serves. A level is served for two shell minor releases after the next one ships.
pub const LEVEL: u32 = 1;

/// The oldest level this bar still serves; a payload below it is refused.
pub const OLDEST_LEVEL: u32 = 1;

/// The old name of [`LEVEL`].
#[deprecated(note = "use LEVEL")]
pub const VERSION: u32 = LEVEL;

fn level() -> u32 {
    LEVEL
}

fn yes() -> bool {
    true
}

fn middling() -> f32 {
    0.5
}

/// A picture in the bar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemIcon {
    /// One of the bar's own symbols, by name, like "tray" or "timer".
    Glyph(String),

    /// An icon theme name, like "folder-download".
    Theme(String),

    /// A png or svg file.
    File(PathBuf),

    /// The first of these the bar can show, in order: a name, then a file to fall back on,
    /// like `{"first_of":[{"theme":"folder-download"},{"file":"/…/drop.svg"}]}`. Entries this
    /// bar doesn't know are left out, and a list inside a list is read as one list.
    #[serde(deserialize_with = "lenient::first_of")]
    FirstOf(Vec<ItemIcon>),
}

impl Default for ItemIcon {
    fn default() -> Self {
        Self::Glyph(String::from("dot"))
    }
}

impl ItemIcon {
    /// One of the bar's own symbols, by name.
    pub fn glyph(name: &str) -> Self {
        Self::Glyph(String::from(name))
    }

    /// An icon theme name.
    pub fn theme(name: &str) -> Self {
        Self::Theme(String::from(name))
    }

    /// A png or svg file.
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self::File(path.into())
    }

    /// This icon, or `next` when the bar can't show it: `ItemIcon::theme("x").or(ItemIcon::file(p))`.
    pub fn or(self, next: Self) -> Self {
        let mut list = match self {
            Self::FirstOf(list) => list,
            icon => vec![icon],
        };
        match next {
            Self::FirstOf(more) => list.extend(more),
            icon => list.push(icon),
        }
        Self::FirstOf(list)
    }
}

/// What can be dropped on an item.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accepts {
    /// Files, handed over as paths.
    #[serde(default)]
    pub files: bool,

    /// Dragged text.
    #[serde(default)]
    pub text: bool,
}

/// One line of an item's menu.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MenuEntry {
    /// Handed back when it's picked; a separator needs none.
    #[serde(default)]
    pub id: String,

    /// The words shown for it.
    #[serde(default)]
    pub label: String,

    /// A line between groups of entries instead of an entry.
    #[serde(default)]
    pub separator: bool,

    /// Whether it can be picked; on unless the payload says otherwise.
    #[serde(default = "yes")]
    pub enabled: bool,

    /// A tick before the label, none for an entry that isn't a switch.
    #[serde(default)]
    pub checked: Option<bool>,

    /// Drawn in the critical status color, for something that deletes or can't be undone.
    #[serde(default)]
    pub destructive: bool,

    /// A picture before the label.
    #[serde(default, deserialize_with = "lenient::or_none")]
    pub icon: Option<ItemIcon>,

    /// A submenu.
    #[serde(default, deserialize_with = "lenient::each")]
    pub children: Vec<MenuEntry>,
}

impl MenuEntry {
    /// An enabled entry with this id and label.
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: String::from(id),
            label: String::from(label),
            enabled: true,
            ..Self::default()
        }
    }

    /// A line between groups of entries.
    pub fn separator() -> Self {
        Self {
            separator: true,
            enabled: true,
            ..Self::default()
        }
    }
}

/// A program's item in the bar: its id is unique on
/// the desktop, like a reverse-DNS app id ("org.emaki.drop"), and the bar
/// keeps its place and whether it's shown by it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemSpec {
    /// The payload's level, [`LEVEL`] when left out.
    #[serde(default = "level")]
    pub v: u32,

    /// Unique on the desktop, like a reverse-DNS app id; it can't be empty.
    pub id: String,

    /// What the bar draws for it.
    #[serde(default, deserialize_with = "lenient::or_default")]
    pub icon: ItemIcon,

    /// Text after the icon, kept short by the bar.
    #[serde(default)]
    pub title: Option<String>,

    /// Shown while the pointer rests on it.
    #[serde(default)]
    pub tooltip: String,

    /// Shown on a right-click; a click is the program's own to answer.
    #[serde(default, deserialize_with = "lenient::each")]
    pub menu: Vec<MenuEntry>,

    /// What can be dropped on it.
    #[serde(default)]
    pub accepts: Accepts,

    /// Files dragged over it tell the program at once, so it can open a window to drop on.
    #[serde(default)]
    pub opens_on_drag: bool,

    /// Ringed, like a tray app that wants attention.
    #[serde(default)]
    pub attention: bool,

    /// A little count or mark on the icon.
    #[serde(default)]
    pub badge: Option<String>,
}

impl ItemSpec {
    /// An item with this id and icon and nothing else.
    pub fn new(id: &str, icon: ItemIcon) -> Self {
        Self {
            v: LEVEL,
            id: String::from(id),
            icon,
            title: None,
            tooltip: String::new(),
            menu: Vec::new(),
            accepts: Accepts::default(),
            opens_on_drag: false,
            attention: false,
            badge: None,
        }
    }
}

/// The mouse button an item was clicked with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemButton {
    /// The primary button.
    Left,
    /// The middle button or wheel press.
    Middle,
    /// The secondary button.
    Right,
}

/// The screen edge the bar is on, so a program opens its window from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BarEdge {
    /// Along the top of the screen.
    Top,
    /// Along the bottom of the screen.
    Bottom,
}

/// What happened to an item, sent to the program that registered it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemEvent {
    /// Clicked: `x` is the middle of the item on its monitor, in logical
    /// pixels, `output` the monitor's name, and `edge` where the bar is.
    Activated {
        /// The button it was clicked with.
        button: ItemButton,
        /// The middle of the item, in logical pixels from the monitor's left.
        x: f32,
        /// The monitor's name, like "DP-1".
        output: String,
        /// The edge the bar is on.
        edge: BarEdge,
    },

    /// A menu entry, by its id.
    MenuSelected(String),

    /// Files let go on it, by path.
    DroppedFiles(Vec<PathBuf>),

    /// Text let go on it.
    DroppedText(String),

    /// Files came over it (true) or went away (false), for an item that opens on drag.
    DragHover {
        /// True while files are over it, false once they left.
        over: bool,
        /// The middle of the item, in logical pixels from the monitor's left.
        x: f32,
        /// The monitor's name, like "DP-1".
        output: String,
        /// The edge the bar is on.
        edge: BarEdge,
    },

    /// The user took it out of the bar, or put it back; the program keeps running.
    Shown(bool),
}

/// A color from the bar's palette, so activities match the desktop's theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tint {
    /// The desktop's accent color.
    Accent,
    /// Red.
    Red,
    /// Orange.
    Orange,
    /// Yellow.
    Yellow,
    /// Green.
    Green,
    /// Mint, a pale blue-green.
    Mint,
    /// Teal, a deep blue-green.
    Teal,
    /// Cyan.
    Cyan,
    /// Blue.
    Blue,
    /// Indigo, a deep blue-violet.
    Indigo,
    /// Purple.
    Purple,
    /// Pink.
    Pink,
    /// Gray.
    Gray,
}

/// The right of a compact activity.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trailing {
    /// Nothing on the right.
    #[default]
    None,

    /// A few words, kept short by the bar.
    Text(String),

    /// Time left until this moment, in milliseconds since 1970; the bar counts it down itself.
    Countdown(i64),

    /// Time since this moment, the same way, like a recording's length.
    CountUp(i64),

    /// Done, 0 to 1, drawn as a ring.
    Progress(f32),
}

/// A button in an activity's expanded view.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ActivityButton {
    /// Handed back in [`ActivityEvent::Action`] when it's pressed.
    pub id: String,

    /// The words on the button.
    #[serde(default)]
    pub label: String,

    /// A picture on the button.
    #[serde(default, deserialize_with = "lenient::or_none")]
    pub icon: Option<ItemIcon>,

    /// Drawn in the critical status color, for something like stop or cancel.
    #[serde(default)]
    pub destructive: bool,
}

impl ActivityButton {
    /// A button with this id and label.
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: String::from(id),
            label: String::from(label),
            ..Self::default()
        }
    }
}

/// An activity: something going on now, shown in the bar while it lasts,
/// like a timer, a song or an upload. Compact, the bar shows `leading`
/// and `trailing`; minimal, `leading` alone; expanded, everything.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivityContent {
    /// The payload's level, [`LEVEL`] when left out.
    #[serde(default = "level")]
    pub v: u32,

    /// The program's name for the permission list, like "Drop".
    #[serde(default)]
    pub app: String,

    /// The picture on the left, shown in every view.
    #[serde(default, deserialize_with = "lenient::or_default")]
    pub leading: ItemIcon,

    /// What's on the right of the compact view.
    #[serde(default, deserialize_with = "lenient::or_default")]
    pub trailing: Trailing,

    /// The main line of the expanded view.
    #[serde(default)]
    pub title: String,

    /// A second, smaller line under the title.
    #[serde(default)]
    pub subtitle: String,

    /// A bar under the text in the expanded view, 0 to 1.
    #[serde(default)]
    pub progress: Option<f32>,

    /// At most three are shown.
    #[serde(default, deserialize_with = "lenient::each")]
    pub buttons: Vec<ActivityButton>,

    /// The color it's drawn in, from the bar's palette.
    #[serde(default, deserialize_with = "lenient::or_none")]
    pub tint: Option<Tint>,

    /// 0 to 1: with several going on, the most relevant show compact first.
    #[serde(default = "middling")]
    pub relevance: f32,

    /// Seconds without an update before it shows as out of date.
    #[serde(default)]
    pub stale_after: Option<u64>,
}

impl ActivityContent {
    /// An activity for `app` with this picture and title, at middling relevance and nothing else.
    pub fn new(app: &str, leading: ItemIcon, title: &str) -> Self {
        Self {
            v: LEVEL,
            app: String::from(app),
            leading,
            trailing: Trailing::None,
            title: String::from(title),
            subtitle: String::new(),
            progress: None,
            buttons: Vec::new(),
            tint: None,
            relevance: middling(),
            stale_after: None,
        }
    }
}

/// How an ended activity leaves the bar.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dismiss {
    /// It leaves at once.
    #[default]
    Now,

    /// Its last state stays this many seconds.
    After(u32),
}

/// What happened to an activity, sent to the program that started it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityEvent {
    /// One of its buttons, by id.
    Action(String),

    /// It left the bar: ended, timed out, or the user turned the program's activities off.
    Dismissed,
}

/// Why a payload from the bus was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum ProtocolError {
    /// The JSON didn't read, with serde's reason.
    Malformed(String),

    /// A level this bar doesn't serve: above [`LEVEL`] or below [`OLDEST_LEVEL`].
    Level(u32),

    /// More bytes than [`budget::MAX_PAYLOAD`](crate::budget::MAX_PAYLOAD).
    TooLarge(usize),

    /// An empty id.
    NoId,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(reason) => write!(f, "malformed payload: {reason}"),
            Self::Level(v) => write!(
                f,
                "payload level {v} isn't served: this bar serves {OLDEST_LEVEL} to {LEVEL}"
            ),
            Self::TooLarge(bytes) => write!(
                f,
                "payload of {bytes} bytes is over the {} allowed",
                budget::MAX_PAYLOAD
            ),
            Self::NoId => write!(f, "an item needs an id"),
        }
    }
}

impl std::error::Error for ProtocolError {}

/// Writes a payload as the JSON string that goes over the bus.
pub fn encode<T: Serialize>(payload: &T) -> String {
    serde_json::to_string(payload).expect("failed to write a status bar payload")
}

fn read<T: DeserializeOwned>(json: &str) -> Result<T, ProtocolError> {
    if json.len() > budget::MAX_PAYLOAD {
        return Err(ProtocolError::TooLarge(json.len()));
    }

    serde_json::from_str(json).map_err(|error| ProtocolError::Malformed(error.to_string()))
}

/// Refuses a level this bar doesn't serve.
fn served(v: u32) -> Result<(), ProtocolError> {
    if (OLDEST_LEVEL..=LEVEL).contains(&v) {
        Ok(())
    } else {
        Err(ProtocolError::Level(v))
    }
}

/// Cuts text to at most `max` characters.
fn clip(text: &mut String, max: usize) {
    if let Some((cut, _)) = text.char_indices().nth(max) {
        text.truncate(cut);
    }
}

/// Keeps a menu within [`budget::MAX_MENU_ENTRIES`] a level and [`budget::MAX_MENU_DEPTH`] deep.
fn clip_menu(menu: &mut Vec<MenuEntry>, depth: usize) {
    menu.truncate(budget::MAX_MENU_ENTRIES);

    for entry in menu {
        clip(&mut entry.label, budget::MAX_TEXT);

        if depth + 1 >= budget::MAX_MENU_DEPTH {
            entry.children.clear();
        } else {
            clip_menu(&mut entry.children, depth + 1);
        }
    }
}

impl ItemSpec {
    /// Reads an item from the bus, refusing a level this bar doesn't serve or no id; text and
    /// menus past the [`budget`](crate::budget) are cut.
    pub fn decode(json: &str) -> Result<Self, ProtocolError> {
        let mut spec: Self = read(json)?;

        served(spec.v)?;

        if spec.id.trim().is_empty() {
            return Err(ProtocolError::NoId);
        }

        if let Some(title) = &mut spec.title {
            clip(title, budget::MAX_TEXT);
        }
        if let Some(badge) = &mut spec.badge {
            clip(badge, budget::MAX_TEXT);
        }
        clip(&mut spec.tooltip, budget::MAX_TOOLTIP);
        clip_menu(&mut spec.menu, 0);

        Ok(spec)
    }
}

impl ActivityContent {
    /// Reads an activity from the bus, refusing a level this bar doesn't serve; relevance and
    /// progress outside 0 to 1 are brought back into it, and text and buttons past the
    /// [`budget`](crate::budget) are cut.
    pub fn decode(json: &str) -> Result<Self, ProtocolError> {
        let mut content: Self = read(json)?;

        served(content.v)?;

        for text in [&mut content.app, &mut content.title, &mut content.subtitle] {
            clip(text, budget::MAX_TEXT);
        }
        if let Trailing::Text(text) = &mut content.trailing {
            clip(text, budget::MAX_TEXT);
        }
        content.buttons.truncate(budget::MAX_ACTIVITY_BUTTONS);
        for button in &mut content.buttons {
            clip(&mut button.label, budget::MAX_TEXT);
        }

        content.relevance = content.relevance.clamp(0.0, 1.0);
        content.progress = content.progress.map(|done| done.clamp(0.0, 1.0));

        if let Trailing::Progress(done) = &mut content.trailing {
            *done = done.clamp(0.0, 1.0);
        }

        Ok(content)
    }
}

impl ItemEvent {
    /// Reads an item's event from the bus.
    pub fn decode(json: &str) -> Result<Self, ProtocolError> {
        read(json)
    }
}

impl ActivityEvent {
    /// Reads an activity's event from the bus.
    pub fn decode(json: &str) -> Result<Self, ProtocolError> {
        read(json)
    }
}

impl Dismiss {
    /// Reads how an activity leaves from the bus, as sent with `EndActivity`.
    pub fn decode(json: &str) -> Result<Self, ProtocolError> {
        read(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_item_comes_back_the_same() {
        let mut spec = ItemSpec::new("org.emaki.drop", ItemIcon::Glyph(String::from("tray")));

        spec.title = Some(String::from("Drop"));
        spec.accepts = Accepts {
            files: true,
            text: true,
        };
        spec.menu = vec![
            MenuEntry::new("grid", "Open Grid"),
            MenuEntry::separator(),
            MenuEntry::new("quit", "Quit"),
        ];

        assert_eq!(ItemSpec::decode(&encode(&spec)), Ok(spec));
    }

    #[test]
    fn a_script_can_leave_out_almost_everything() {
        let spec = ItemSpec::decode(r#"{"id":"demo"}"#).expect("failed to read a short item");

        assert_eq!(spec.v, LEVEL);
        assert_eq!(spec.icon, ItemIcon::default());
        assert!(!spec.accepts.files);

        let content = ActivityContent::decode(r#"{"title":"Tea","trailing":{"countdown":1}}"#)
            .expect("failed to read a short activity");

        assert_eq!(content.trailing, Trailing::Countdown(1));
        assert_eq!(content.relevance, 0.5);
    }

    #[test]
    fn refuses_levels_not_served_and_missing_ids() {
        assert_eq!(
            ItemSpec::decode(r#"{"v":2,"id":"demo"}"#),
            Err(ProtocolError::Level(2))
        );
        assert_eq!(
            ItemSpec::decode(r#"{"v":0,"id":"demo"}"#),
            Err(ProtocolError::Level(0))
        );

        assert_eq!(ItemSpec::decode(r#"{"id":"  "}"#), Err(ProtocolError::NoId));

        assert!(matches!(
            ItemSpec::decode("not json"),
            Err(ProtocolError::Malformed(_))
        ));
    }

    #[test]
    fn clamps_what_is_out_of_range() {
        let content =
            ActivityContent::decode(r#"{"relevance":4,"progress":-1,"trailing":{"progress":2}}"#)
                .expect("failed to read an activity");

        assert_eq!(content.relevance, 1.0);
        assert_eq!(content.progress, Some(0.0));
        assert_eq!(content.trailing, Trailing::Progress(1.0));
    }

    #[test]
    fn events_come_back_the_same() {
        let events = [
            ItemEvent::Activated {
                button: ItemButton::Left,
                x: 1200.5,
                output: String::from("DP-1"),
                edge: BarEdge::Bottom,
            },
            ItemEvent::MenuSelected(String::from("quit")),
            ItemEvent::DroppedFiles(vec![PathBuf::from("/tmp/a b.png")]),
            ItemEvent::DroppedText(String::from("hello")),
            ItemEvent::Shown(false),
        ];

        for event in events {
            assert_eq!(ItemEvent::decode(&encode(&event)), Ok(event));
        }

        assert_eq!(Dismiss::decode(r#"{"after":5}"#), Ok(Dismiss::After(5)));
        assert_eq!(Dismiss::decode(r#""now""#), Ok(Dismiss::Now));
    }

    #[test]
    fn drops_what_it_does_not_know_and_keeps_the_rest() {
        let spec = ItemSpec::decode(
            r#"{"id":"demo","title":"Kept","icon":{"pixmap":[1,2]},"later":true,
                "menu":[{"id":"a","label":"A"},{"id":"b","icon":{"hologram":1}},"nonsense",
                        {"id":"c","label":"C"}]}"#,
        )
        .expect("failed to read an item with newer parts");

        assert_eq!(spec.title.as_deref(), Some("Kept"));
        assert_eq!(spec.icon, ItemIcon::default());
        let ids: Vec<&str> = spec.menu.iter().map(|entry| entry.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c"]);
        assert_eq!(spec.menu[1].icon, None);

        let content = ActivityContent::decode(
            r#"{"title":"Tea","trailing":{"sparkline":[1]},"tint":"ultraviolet",
                "buttons":[{"id":"stop","label":"Stop"},{"label":"no id"}]}"#,
        )
        .expect("failed to read an activity with newer parts");

        assert_eq!(content.title, "Tea");
        assert_eq!(content.trailing, Trailing::None);
        assert_eq!(content.tint, None);
        assert_eq!(content.buttons, [ActivityButton::new("stop", "Stop")]);
    }

    #[test]
    fn refuses_a_payload_over_the_budget() {
        let tooltip = "x".repeat(budget::MAX_PAYLOAD);
        let json = format!(r#"{{"id":"demo","tooltip":"{tooltip}"}}"#);

        assert_eq!(
            ItemSpec::decode(&json),
            Err(ProtocolError::TooLarge(json.len()))
        );
    }

    #[test]
    fn cuts_text_buttons_and_menus_to_the_budget() {
        let mut spec = ItemSpec::new("demo", ItemIcon::default());
        spec.title = Some("é".repeat(budget::MAX_TEXT + 10));
        spec.menu = (0..budget::MAX_MENU_ENTRIES + 5)
            .map(|n| MenuEntry::new(&n.to_string(), "entry"))
            .collect();

        let mut deep = MenuEntry::new("deep", "deep");
        for _ in 0..budget::MAX_MENU_DEPTH + 2 {
            let mut parent = MenuEntry::new("deep", "deep");
            parent.children = vec![deep];
            deep = parent;
        }
        spec.menu[0] = deep;

        let spec = ItemSpec::decode(&encode(&spec)).expect("failed to read a long item");

        assert_eq!(
            spec.title.map(|title| title.chars().count()),
            Some(budget::MAX_TEXT)
        );
        assert_eq!(spec.menu.len(), budget::MAX_MENU_ENTRIES);

        let mut depth = 1;
        let mut entry = &spec.menu[0];
        while let Some(child) = entry.children.first() {
            depth += 1;
            entry = child;
        }
        assert_eq!(depth, budget::MAX_MENU_DEPTH);

        let mut content = ActivityContent::new("Demo", ItemIcon::default(), "Tea");
        content.buttons = (0..5)
            .map(|n| ActivityButton::new(&n.to_string(), "Go"))
            .collect();

        let content = ActivityContent::decode(&encode(&content)).expect("failed to read");
        assert_eq!(content.buttons.len(), budget::MAX_ACTIVITY_BUTTONS);
    }

    #[test]
    fn an_icon_falls_back_in_order() {
        let icon = ItemIcon::theme("folder-download")
            .or(ItemIcon::glyph("tray"))
            .or(ItemIcon::file("/drop.svg"));
        let spec = ItemSpec::new("demo", icon.clone());

        assert_eq!(
            ItemSpec::decode(&encode(&spec)).map(|spec| spec.icon),
            Ok(icon)
        );

        let spec = ItemSpec::decode(
            r#"{"id":"demo","icon":{"first_of":[{"pixmap":[]},{"first_of":[{"theme":"a"}]},
                {"file":"/b.png"}]}}"#,
        )
        .expect("failed to read an icon list");

        assert_eq!(
            spec.icon,
            ItemIcon::FirstOf(vec![ItemIcon::theme("a"), ItemIcon::file("/b.png")])
        );
    }
}
