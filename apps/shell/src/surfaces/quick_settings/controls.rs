//! The controls the grid can hold — one id per control, what sizes it
//! comes in, and how to build it. The layout (`layout.rs`) says where each
//! sits and at what size; a control whose hardware is missing builds to
//! nothing and its cells stay empty, as GNOME hides it.

use amane::{Audio, Battery, Bluetooth, Brightness, PowerProfile, Service, Widget};

use super::grid::Span;
use super::system_row;
use super::tile::{QuickToggle, Style};
use super::tiles::{
    airplane, appearance, background, bluetooth, keep_awake, keyboard, media, network,
    notifications, phone, power, software_update, sound, tailscale,
};
use crate::services::desktop::background_apps::BackgroundApps;
use crate::services::display::dark_style;
use crate::services::display::night_light::NightLight;
use crate::services::network::connections::Connections;
use crate::services::network::rfkill::Rfkill;
use crate::services::network::tailscale::Tailscale;
use crate::services::power::keep_awake::KeepAwake;
use crate::services::power::keyboard_backlight::KeyboardBacklight;
use crate::surfaces::quick_settings::grid::Sizes as _;
use amane_kit::grid::GridKey;
use amane_kit::layout::boxed;

/// A control, by its place in [`SPECS`]: the default order, then the ones
/// added since (a control's place keys its motion, so new ones go last).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ControlId {
    Wifi,
    NowPlaying,
    Bluetooth,
    Focus,
    DarkStyle,
    NightLight,
    Display,
    Sound,
    Battery,
    Airplane,
    PowerMode,
    Keyboard,
    BackgroundApps,
    Screenshot,
    Lock,
    PowerOff,
    Vpn,
    Wired,
    Tailscale,
    KeepAwake,
    SoftwareUpdate,
    Phone,
}

/// How a control is drawn: a toggle the grid sizes, or a cell of its own.
#[derive(Clone, Copy)]
pub enum Kind {
    /// A toggle; none when this machine hasn't got what it drives.
    Toggle(fn() -> Option<QuickToggle>),
    /// A cell drawn at a span, inert or not; none when this machine hasn't
    /// got what it drives.
    Cell(fn(Span, bool) -> Option<Box<dyn Widget>>),
}

/// What a control is, written once.
pub struct Spec {
    /// Which control.
    pub id: ControlId,
    /// The name kept in the layout file, and the IPC's.
    pub name: &'static str,
    /// What the Add Controls list calls it.
    pub title: &'static str,
    /// The sizes it comes in, smallest first.
    pub sizes: &'static [Span],
    /// The size it's added at.
    pub default: Span,
    /// Waits in the Add list until asked for.
    pub hidden: bool,
    /// How it's drawn.
    pub kind: Kind,
    /// Whether this machine has what it drives, read straight from the
    /// services (cheaper than building it to see).
    pub available: fn() -> bool,
}

// the sizes controls come in
const SMALL: &[Span] = &[Span::S, Span::M];
const TOGGLE: &[Span] = &[Span::M, Span::W3, Span::FULL];
const WIDE: &[Span] = &[Span::W3, Span::FULL];
const LARGE: &[Span] = &[Span::L, Span::FULL];

const fn always() -> bool {
    true
}

const fn spec(
    id: ControlId,
    name: &'static str,
    title: &'static str,
    sizes: &'static [Span],
    kind: Kind,
    available: fn() -> bool,
) -> Spec {
    Spec {
        id,
        name,
        title,
        sizes,
        default: sizes[0],
        hidden: false,
        kind,
        available,
    }
}

/// Added only when asked for.
const fn hidden(spec: Spec) -> Spec {
    Spec {
        hidden: true,
        ..spec
    }
}

/// Every control, in [`ControlId`]'s order.
pub const SPECS: [Spec; 22] = {
    use ControlId as C;
    use Kind::{Cell, Toggle};
    [
        spec(
            C::Wifi,
            "wifi",
            "Wi‑Fi",
            TOGGLE,
            Toggle(network::wifi_toggle),
            network::wifi_available,
        ),
        spec(
            C::NowPlaying,
            "now-playing",
            "Now Playing",
            LARGE,
            Cell(|span, inert| Some(media::cell(span, inert))),
            always,
        ),
        spec(
            C::Bluetooth,
            "bluetooth",
            "Bluetooth",
            TOGGLE,
            Toggle(bluetooth::toggle),
            || Bluetooth::read().available(),
        ),
        spec(
            C::Focus,
            "focus",
            "Focus",
            TOGGLE,
            Toggle(|| Some(notifications::toggle())),
            always,
        ),
        spec(
            C::DarkStyle,
            "dark-style",
            "Dark Style",
            SMALL,
            Toggle(appearance::dark_style_toggle),
            dark_style::present,
        ),
        spec(
            C::NightLight,
            "night-light",
            "Night Light",
            SMALL,
            Toggle(appearance::night_light_toggle),
            || NightLight::read().present(),
        ),
        Spec {
            default: Span::FULL,
            ..spec(
                C::Display,
                "display",
                "Display",
                WIDE,
                Cell(appearance::brightness_cell),
                || Brightness::read().present(),
            )
        },
        Spec {
            default: Span::FULL,
            ..spec(C::Sound, "sound", "Sound", WIDE, Cell(sound::cell), || {
                !Audio::read().outputs().is_empty()
            })
        },
        spec(
            C::Battery,
            "battery",
            "Battery",
            TOGGLE,
            Toggle(system_row::battery_toggle),
            || Battery::read().present(),
        ),
        spec(
            C::Airplane,
            "airplane",
            "Airplane Mode",
            SMALL,
            Toggle(airplane::toggle),
            || Rfkill::read().present(),
        ),
        spec(
            C::PowerMode,
            "power-mode",
            "Power Mode",
            SMALL,
            Toggle(power::mode_toggle),
            || PowerProfile::read().present(),
        ),
        spec(
            C::Keyboard,
            "keyboard",
            "Keyboard Brightness",
            SMALL,
            Toggle(keyboard::toggle),
            || KeyboardBacklight::read().present(),
        ),
        spec(
            C::BackgroundApps,
            "background-apps",
            "Background Apps",
            SMALL,
            Toggle(background::toggle),
            || !BackgroundApps::read().apps().is_empty(),
        ),
        spec(
            C::Screenshot,
            "screenshot",
            "Screenshot",
            SMALL,
            Toggle(|| Some(system_row::screenshot_toggle())),
            always,
        ),
        spec(
            C::Lock,
            "lock",
            "Lock Screen",
            SMALL,
            Toggle(|| Some(system_row::lock_toggle())),
            always,
        ),
        spec(
            C::PowerOff,
            "power-off",
            "Power Off",
            SMALL,
            Toggle(|| Some(system_row::power_off_toggle())),
            always,
        ),
        hidden(spec(
            C::Vpn,
            "vpn",
            "VPN",
            TOGGLE,
            Toggle(network::vpn_toggle),
            network::vpn_available,
        )),
        hidden(spec(
            C::Wired,
            "wired",
            "Wired",
            TOGGLE,
            Toggle(network::wired_toggle),
            || Connections::read().wired().is_some(),
        )),
        hidden(spec(
            C::Tailscale,
            "tailscale",
            "Tailscale",
            TOGGLE,
            Toggle(tailscale::toggle),
            || Tailscale::read().present(),
        )),
        hidden(spec(
            C::KeepAwake,
            "keep-awake",
            "Keep Awake",
            TOGGLE,
            Toggle(keep_awake::toggle),
            || KeepAwake::read().present(),
        )),
        // addable with Shell Settings around; its cells stay empty until an update waits
        hidden(spec(
            C::SoftwareUpdate,
            "software-update",
            "Software Update",
            TOGGLE,
            Toggle(software_update::toggle),
            software_update::available,
        )),
        hidden(spec(
            C::Phone,
            "phone",
            "Phone",
            TOGGLE,
            Toggle(phone::toggle),
            phone::available,
        )),
    ]
};

impl ControlId {
    /// Every control, in their order.
    pub const ALL: [ControlId; 22] = {
        let mut all = [ControlId::Wifi; 22];
        let mut at = 0;
        while at < all.len() {
            all[at] = SPECS[at].id;
            at += 1;
        }
        all
    };

    /// What the control is.
    pub fn spec(self) -> &'static Spec {
        &SPECS[self as usize]
    }

    /// The name kept in the layout file.
    pub fn name(self) -> &'static str {
        self.spec().name
    }

    pub fn named(name: &str) -> Option<Self> {
        SPECS
            .iter()
            .find(|spec| spec.name == name)
            .map(|spec| spec.id)
    }

    /// What the Add Controls list calls it.
    pub fn title(self) -> &'static str {
        self.spec().title
    }

    /// The sizes it comes in, smallest first.
    pub fn supported(self) -> &'static [Span] {
        self.spec().sizes
    }

    pub fn default_span(self) -> Span {
        self.spec().default
    }

    /// Waits in the Add list until asked for.
    pub fn hidden_by_default(self) -> bool {
        self.spec().hidden
    }

    /// The key its motion is kept under.
    pub fn key(self) -> u64 {
        self as u64
    }
}

impl GridKey for ControlId {
    fn supported(&self) -> &'static [Span] {
        ControlId::supported(*self)
    }
}

/// What this frame knows that every control shares.
pub struct Ctx {
    /// Edit mode: controls draw but take nothing.
    pub inert: bool,
}

impl Ctx {
    pub fn new(inert: bool) -> Self {
        Self { inert }
    }
}

/// Builds `id` at `span`, or nothing when this machine hasn't got it.
pub fn build(id: ControlId, span: Span, ctx: &Ctx) -> Option<Box<dyn Widget>> {
    let span = id.clamp(span);
    match id.spec().kind {
        Kind::Toggle(toggle) => toggle().map(|t| sized(t, span, ctx.inert)),
        Kind::Cell(cell) => cell(span, ctx.inert),
    }
}

/// `id` as a toggle, for the controls that are one, or `None` for the
/// cells (Now Playing, Display, Sound) and what this machine hasn't got.
pub fn quick_toggle(id: ControlId) -> Option<QuickToggle> {
    match id.spec().kind {
        Kind::Toggle(toggle) => toggle(),
        Kind::Cell(_) => None,
    }
}

/// Whether `id` would build: for the Add Controls list and the default layout.
pub fn present(id: ControlId, _ctx: &Ctx) -> bool {
    available(id)
}

/// Whether this machine has what `id` drives.
pub fn available(id: ControlId) -> bool {
    (id.spec().available)()
}

/// `toggle` in the shape `span` asks for.
fn sized(toggle: QuickToggle, span: Span, inert: bool) -> Box<dyn Widget> {
    let toggle = toggle.inert(inert);
    if span == Span::S {
        boxed(toggle.style(Style::Round).build())
    } else {
        boxed(toggle.style(Style::Card).span(span).build())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_control_is_at_its_own_place() {
        for (at, spec) in SPECS.iter().enumerate() {
            assert_eq!(spec.id as usize, at, "{} is out of place", spec.name);
        }
    }

    #[test]
    fn every_control_has_a_unique_name_and_key() {
        let mut names = std::collections::HashSet::new();
        let mut keys = std::collections::HashSet::new();
        for id in ControlId::ALL {
            assert!(names.insert(id.name()));
            assert!(keys.insert(id.key()));
            assert_eq!(ControlId::named(id.name()), Some(id));
            assert!(id.supported().contains(&id.default_span()));
        }
        assert_eq!(ControlId::named("nope"), None);
    }

    #[test]
    fn a_size_it_lacks_snaps_to_the_nearest() {
        assert_eq!(ControlId::Display.clamp(Span::S), Span::W3);
        assert_eq!(ControlId::Lock.clamp(Span::FULL), Span::M);
        assert_eq!(ControlId::Focus.clamp(Span::S), Span::M);
        assert_eq!(ControlId::Focus.clamp(Span::L), Span::FULL);
        assert_eq!(ControlId::NowPlaying.clamp(Span::M), Span::L);
    }
}
