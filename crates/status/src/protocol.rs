use std::fmt;
use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// The bar's well-known name on the session bus.
///
/// What a program and the bar say to each other over the session bus: every
/// payload is one JSON string, so a script can send it with busctl too.
pub const BUS_NAME: &str = "org.amane.StatusBar1";

/// The object path the bar serves its interface at.
pub const PATH: &str = "/org/amane/StatusBar";

/// The interface for registering items and starting activities, and the one
/// the bar's `ItemEvent` and `ActivityEvent` signals come from.
pub const INTERFACE: &str = "org.amane.StatusBar1";

/// The payloads' version; a bar refuses one newer than it knows.
pub const VERSION: u32 = 1;

fn version() -> u32 {
    VERSION
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
}

impl Default for ItemIcon {
    fn default() -> Self {
        Self::Glyph(String::from("dot"))
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
    #[serde(default)]
    pub icon: Option<ItemIcon>,

    /// A submenu.
    #[serde(default)]
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
/// the desktop, like a reverse-DNS app id ("org.amane.drop"), and the bar
/// keeps its place and whether it's shown by it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemSpec {
    /// The payload's version, [`VERSION`] when left out.
    #[serde(default = "version")]
    pub v: u32,

    /// Unique on the desktop, like a reverse-DNS app id; it can't be empty.
    pub id: String,

    /// What the bar draws for it.
    #[serde(default)]
    pub icon: ItemIcon,

    /// Text after the icon, kept short by the bar.
    #[serde(default)]
    pub title: Option<String>,

    /// Shown while the pointer rests on it.
    #[serde(default)]
    pub tooltip: String,

    /// Shown on a right-click; a click is the program's own to answer.
    #[serde(default)]
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
            v: VERSION,
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
    #[serde(default)]
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
    /// The payload's version, [`VERSION`] when left out.
    #[serde(default = "version")]
    pub v: u32,

    /// The program's name for the permission list, like "Drop".
    #[serde(default)]
    pub app: String,

    /// The picture on the left, shown in every view.
    #[serde(default)]
    pub leading: ItemIcon,

    /// What's on the right of the compact view.
    #[serde(default)]
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
    #[serde(default)]
    pub buttons: Vec<ActivityButton>,

    /// The color it's drawn in, from the bar's palette.
    #[serde(default)]
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
            v: VERSION,
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

    /// Newer than this bar knows.
    Version(u32),

    /// An empty id.
    NoId,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(reason) => write!(f, "malformed payload: {reason}"),
            Self::Version(v) => write!(f, "payload version {v} is newer than {VERSION}"),
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
    serde_json::from_str(json).map_err(|error| ProtocolError::Malformed(error.to_string()))
}

impl ItemSpec {
    /// Reads an item from the bus, refusing one newer than [`VERSION`] or without an id.
    pub fn decode(json: &str) -> Result<Self, ProtocolError> {
        let spec: Self = read(json)?;

        if spec.v > VERSION {
            return Err(ProtocolError::Version(spec.v));
        }

        if spec.id.trim().is_empty() {
            return Err(ProtocolError::NoId);
        }

        Ok(spec)
    }
}

impl ActivityContent {
    /// Reads an activity from the bus, refusing one newer than [`VERSION`]; relevance and
    /// progress outside 0 to 1 are brought back into it.
    pub fn decode(json: &str) -> Result<Self, ProtocolError> {
        let mut content: Self = read(json)?;

        if content.v > VERSION {
            return Err(ProtocolError::Version(content.v));
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
        let mut spec = ItemSpec::new("org.amane.drop", ItemIcon::Glyph(String::from("tray")));

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

        assert_eq!(spec.v, VERSION);
        assert_eq!(spec.icon, ItemIcon::default());
        assert!(!spec.accepts.files);

        let content = ActivityContent::decode(r#"{"title":"Tea","trailing":{"countdown":1}}"#)
            .expect("failed to read a short activity");

        assert_eq!(content.trailing, Trailing::Countdown(1));
        assert_eq!(content.relevance, 0.5);
    }

    #[test]
    fn refuses_newer_versions_and_missing_ids() {
        assert_eq!(
            ItemSpec::decode(r#"{"v":2,"id":"demo"}"#),
            Err(ProtocolError::Version(2))
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
}
