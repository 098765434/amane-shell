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
    notifications, power, software_update, sound, tailscale,
};
use crate::components::grid::GridKey;
use crate::components::layout::boxed;
use crate::services::desktop::background_apps::BackgroundApps;
use crate::services::network::connections::Connections;
use crate::services::display::dark_style;
use crate::services::power::keep_awake::KeepAwake;
use crate::services::power::keyboard_backlight::KeyboardBacklight;
use crate::services::display::night_light::NightLight;
use crate::services::network::rfkill::Rfkill;
use crate::services::network::tailscale::Tailscale;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ControlId {
    Wifi,
    Bluetooth,
    NowPlaying,
    Vpn,
    Wired,
    Focus,
    DarkStyle,
    NightLight,
    Display,
    Sound,
    Battery,
    PowerMode,
    Keyboard,
    Airplane,
    BackgroundApps,
    Screenshot,
    Lock,
    PowerOff,
    Tailscale,
    KeepAwake,
    SoftwareUpdate,
}

impl ControlId {
    /// Control Center's order, then the ones added since (a control's
    /// place here keys its motion).
    pub const ALL: [ControlId; 21] = [
        Self::Wifi,
        Self::NowPlaying,
        Self::Bluetooth,
        Self::Focus,
        Self::DarkStyle,
        Self::NightLight,
        Self::Display,
        Self::Sound,
        Self::Battery,
        Self::Airplane,
        Self::PowerMode,
        Self::Keyboard,
        Self::BackgroundApps,
        Self::Screenshot,
        Self::Lock,
        Self::PowerOff,
        Self::Vpn,
        Self::Wired,
        Self::Tailscale,
        Self::KeepAwake,
        Self::SoftwareUpdate,
    ];

    /// The name kept in the layout file.
    pub fn name(self) -> &'static str {
        match self {
            Self::Wifi => "wifi",
            Self::Bluetooth => "bluetooth",
            Self::NowPlaying => "now-playing",
            Self::Vpn => "vpn",
            Self::Wired => "wired",
            Self::Focus => "focus",
            Self::DarkStyle => "dark-style",
            Self::NightLight => "night-light",
            Self::Display => "display",
            Self::Sound => "sound",
            Self::Battery => "battery",
            Self::PowerMode => "power-mode",
            Self::Keyboard => "keyboard",
            Self::Airplane => "airplane",
            Self::BackgroundApps => "background-apps",
            Self::Screenshot => "screenshot",
            Self::Lock => "lock",
            Self::PowerOff => "power-off",
            Self::Tailscale => "tailscale",
            Self::KeepAwake => "keep-awake",
            Self::SoftwareUpdate => "software-update",
        }
    }

    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|id| id.name() == name)
    }

    /// What the Add Controls list calls it.
    pub fn title(self) -> &'static str {
        match self {
            Self::Wifi => "Wi‑Fi",
            Self::Bluetooth => "Bluetooth",
            Self::NowPlaying => "Now Playing",
            Self::Vpn => "VPN",
            Self::Wired => "Wired",
            Self::Focus => "Focus",
            Self::DarkStyle => "Dark Style",
            Self::NightLight => "Night Light",
            Self::Display => "Display",
            Self::Sound => "Sound",
            Self::Battery => "Battery",
            Self::PowerMode => "Power Mode",
            Self::Keyboard => "Keyboard Brightness",
            Self::Airplane => "Airplane Mode",
            Self::BackgroundApps => "Background Apps",
            Self::Screenshot => "Screenshot",
            Self::Lock => "Lock Screen",
            Self::PowerOff => "Power Off",
            Self::Tailscale => "Tailscale",
            Self::KeepAwake => "Keep Awake",
            Self::SoftwareUpdate => "Software Update",
        }
    }

    /// The sizes it comes in, smallest first.
    pub fn supported(self) -> &'static [Span] {
        match self {
            Self::NowPlaying => &[Span::L, Span::FULL],
            Self::Display | Self::Sound => &[Span::W3, Span::FULL],
            Self::Wifi
            | Self::Bluetooth
            | Self::Vpn
            | Self::Wired
            | Self::Tailscale
            | Self::KeepAwake
            | Self::SoftwareUpdate
            | Self::Focus
            | Self::Battery => &[Span::M, Span::W3, Span::FULL],
            _ => &[Span::S, Span::M],
        }
    }

    pub fn default_span(self) -> Span {
        match self {
            Self::NowPlaying => Span::L,
            Self::Display | Self::Sound => Span::FULL,
            Self::Wifi
            | Self::Bluetooth
            | Self::Vpn
            | Self::Wired
            | Self::Tailscale
            | Self::KeepAwake
            | Self::SoftwareUpdate
            | Self::Focus
            | Self::Battery => Span::M,
            _ => Span::S,
        }
    }

    /// Waits in the Add list until asked for.
    pub fn hidden_by_default(self) -> bool {
        matches!(
            self,
            Self::Vpn | Self::Wired | Self::Tailscale | Self::KeepAwake | Self::SoftwareUpdate
        )
    }

    /// The key its motion is kept under.
    pub fn key(self) -> u64 {
        Self::ALL.iter().position(|id| *id == self).unwrap_or(0) as u64
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
    let inert = ctx.inert;
    let toggle = |t: Option<QuickToggle>| t.map(|t| sized(t, span, inert));
    match id {
        ControlId::Wifi => toggle(network::wifi_toggle()),
        ControlId::Bluetooth => toggle(bluetooth::toggle()),
        ControlId::NowPlaying => Some(media::cell(span, inert)),
        ControlId::Vpn => toggle(network::vpn_toggle()),
        ControlId::Wired => toggle(network::wired_toggle()),
        ControlId::Airplane => toggle(airplane::toggle()),
        ControlId::Focus => toggle(Some(notifications::toggle())),
        ControlId::DarkStyle => toggle(appearance::dark_style_toggle()),
        ControlId::NightLight => toggle(appearance::night_light_toggle()),
        ControlId::Display => appearance::brightness_cell(span, inert),
        ControlId::Sound => sound::cell(span, inert),
        ControlId::Battery => toggle(system_row::battery_toggle()),
        ControlId::PowerMode => toggle(power::mode_toggle()),
        ControlId::Keyboard => toggle(keyboard::toggle()),
        ControlId::BackgroundApps => toggle(background::toggle()),
        ControlId::Screenshot => toggle(Some(system_row::screenshot_toggle())),
        ControlId::Lock => toggle(Some(system_row::lock_toggle())),
        ControlId::PowerOff => toggle(Some(system_row::power_off_toggle())),
        ControlId::Tailscale => toggle(tailscale::toggle()),
        ControlId::KeepAwake => toggle(keep_awake::toggle()),
        ControlId::SoftwareUpdate => toggle(software_update::toggle()),
    }
}

/// `id` as a toggle, for the controls that are one, or `None` for the
/// cells (Now Playing, Display, Sound) and what this machine hasn't got.
pub fn quick_toggle(id: ControlId) -> Option<QuickToggle> {
    match id {
        ControlId::Wifi => network::wifi_toggle(),
        ControlId::Bluetooth => bluetooth::toggle(),
        ControlId::Vpn => network::vpn_toggle(),
        ControlId::Wired => network::wired_toggle(),
        ControlId::Airplane => airplane::toggle(),
        ControlId::Focus => Some(notifications::toggle()),
        ControlId::DarkStyle => appearance::dark_style_toggle(),
        ControlId::NightLight => appearance::night_light_toggle(),
        ControlId::Battery => system_row::battery_toggle(),
        ControlId::PowerMode => power::mode_toggle(),
        ControlId::Keyboard => keyboard::toggle(),
        ControlId::BackgroundApps => background::toggle(),
        ControlId::Screenshot => Some(system_row::screenshot_toggle()),
        ControlId::Lock => Some(system_row::lock_toggle()),
        ControlId::PowerOff => Some(system_row::power_off_toggle()),
        ControlId::Tailscale => tailscale::toggle(),
        ControlId::KeepAwake => keep_awake::toggle(),
        ControlId::SoftwareUpdate => software_update::toggle(),
        ControlId::NowPlaying | ControlId::Display | ControlId::Sound => None,
    }
}

/// Whether `id` would build: for the Add Controls list and the default layout.
pub fn present(id: ControlId, _ctx: &Ctx) -> bool {
    available(id)
}

/// Whether this machine has what `id` drives, read straight from the
/// services (cheaper than building the control to see).
pub fn available(id: ControlId) -> bool {
    match id {
        ControlId::Wifi => network::wifi_available(),
        ControlId::Bluetooth => Bluetooth::read().available(),
        ControlId::NowPlaying
        | ControlId::Focus
        | ControlId::Screenshot
        | ControlId::Lock
        | ControlId::PowerOff => true,
        ControlId::Vpn => network::vpn_available(),
        ControlId::Wired => Connections::read().wired().is_some(),
        ControlId::Airplane => Rfkill::read().present(),
        ControlId::DarkStyle => dark_style::present(),
        ControlId::NightLight => NightLight::read().present(),
        ControlId::Display => Brightness::read().present(),
        ControlId::Sound => !Audio::read().outputs().is_empty(),
        ControlId::Battery => Battery::read().present(),
        ControlId::PowerMode => PowerProfile::read().present(),
        ControlId::Keyboard => KeyboardBacklight::read().present(),
        ControlId::BackgroundApps => !BackgroundApps::read().apps().is_empty(),
        ControlId::Tailscale => Tailscale::read().present(),
        ControlId::KeepAwake => KeepAwake::read().present(),
        // addable with Shell Settings around; its cells stay empty until an update waits
        ControlId::SoftwareUpdate => software_update::available(),
    }
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
