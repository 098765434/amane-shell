//! Material Symbols codepoints (the font stack is token `sys.type.icon`),
//! and the pickers that choose one for a device or a reading, so the bar,
//! Quick Settings and the notifications all agree. Draw them with
//! [`layout::icon`](super::layout::icon), or, where a control can also show
//! a mark the font hasn't got, as a [`Glyph`] with [`glyph`].

use amane::{
    AudioKind, BatteryState, Center, Color, Column, DeviceKind, PowerDevice, Radius, Rectangle,
    Row, Size,
};

use super::layout::{boxed, fixed_width, icon};

pub(crate) const ICON_APPS: char = '\u{e5c3}';
pub(crate) const ICON_VOLUME_UP: char = '\u{e050}';
pub(crate) const ICON_VOLUME_OFF: char = '\u{e04f}';
pub(crate) const ICON_VOLUME_DOWN: char = '\u{e04d}';
pub(crate) const ICON_LOCK: char = '\u{e897}';
pub(crate) const ICON_CHECK: char = '\u{e5ca}';
pub(crate) const ICON_CLOSE: char = '\u{e5cd}';
pub(crate) const ICON_REMOVE: char = '\u{e15b}';
pub(crate) const ICON_ADD: char = '\u{e145}';
pub(crate) const ICON_RESIZE: char = '\u{f1ce}';
pub(crate) const ICON_MOUSE: char = '\u{e323}';
pub(crate) const ICON_KEYBOARD: char = '\u{e312}';
pub(crate) const ICON_HEADPHONES: char = '\u{e310}';
pub(crate) const ICON_GAMEPAD: char = '\u{ea28}';
pub(crate) const ICON_MIC_OFF: char = '\u{e02b}';
pub(crate) const ICON_SPEAKER: char = '\u{e32d}';
pub(crate) const ICON_PHONE: char = '\u{e32c}';
pub(crate) const ICON_COMPUTER: char = '\u{e30a}';
pub(crate) const ICON_DEVICE_OTHER: char = '\u{e337}';
pub(crate) const ICON_BLUETOOTH: char = '\u{e1a7}';
pub(crate) const ICON_BLUETOOTH_CONNECTED: char = '\u{e1a8}';
pub(crate) const ICON_BLUETOOTH_OFF: char = '\u{e1a9}';
pub(crate) const ICON_NOTIFICATIONS: char = '\u{e7f4}';
pub(crate) const ICON_DND: char = '\u{ef44}';
pub(crate) const ICON_CHEVRON_LEFT: char = '\u{e5cb}';
pub(crate) const ICON_CHEVRON_RIGHT: char = '\u{e5cc}';
pub(crate) const ICON_TUNE: char = '\u{e429}';
pub(crate) const ICON_POWER: char = '\u{f8c7}';
pub(crate) const ICON_SCREENSHOT: char = '\u{f7d2}';
pub(crate) const ICON_BRIGHTNESS_LOW: char = '\u{e1ad}';
pub(crate) const ICON_BRIGHTNESS_HIGH: char = '\u{e1ac}';
pub(crate) const ICON_NIGHT_LIGHT: char = '\u{e1c6}';
pub(crate) const ICON_DARK_MODE: char = '\u{eb37}';
pub(crate) const ICON_AIRPLANE: char = '\u{e53d}';
pub(crate) const ICON_VPN: char = '\u{e0da}';
pub(crate) const ICON_HOTSPOT: char = '\u{e1e2}';
pub(crate) const ICON_WIRED: char = '\u{eb2f}';
pub(crate) const ICON_KEYBOARD_LIGHT: char = '\u{f7ed}';
pub(crate) const ICON_CAMERA: char = '\u{e04b}';
pub(crate) const ICON_SCREEN_SHARE: char = '\u{e0e2}';
pub(crate) const ICON_MUSIC: char = '\u{e405}';
pub(crate) const ICON_PLAY: char = '\u{e037}';
pub(crate) const ICON_PAUSE: char = '\u{e034}';
pub(crate) const ICON_SKIP_NEXT: char = '\u{e044}';
pub(crate) const ICON_SKIP_PREVIOUS: char = '\u{e045}';
pub(crate) const ICON_SLEEP: char = '\u{f037}';
pub(crate) const ICON_RESTART: char = '\u{f053}';
pub(crate) const ICON_LOG_OUT: char = '\u{e9ba}';
pub(crate) const ICON_BACKGROUND_APPS: char = '\u{e1bd}';
pub(crate) const ICON_MIC: char = '\u{e029}';
pub(crate) const ICON_KEEP_AWAKE: char = '\u{efef}';
pub(crate) const ICON_SCREEN_OFF: char = '\u{e99d}';
pub(crate) const ICON_SEARCH: char = '\u{e8b6}';
pub(crate) const ICON_CALCULATE: char = '\u{ea5f}';
pub(crate) const ICON_OPEN_IN_NEW: char = '\u{e89e}';
const ICON_MIC_EXTERNAL: char = '\u{ef5a}';
const ICON_HEADSET_MIC: char = '\u{e311}';
const ICON_TV: char = '\u{e333}';
const ICON_CAR: char = '\u{e531}';
const ICON_WEBCAM: char = '\u{e04b}';

/// The glyph for what a sound goes to (or, with `input`, comes from), so the
/// bar, the Sound card and page and its device rows all agree. Built-in
/// devices show the plain speaker / microphone.
pub(crate) fn audio_glyph(kind: AudioKind, input: bool) -> char {
    match kind {
        AudioKind::Internal if input => ICON_MIC,
        AudioKind::Internal => ICON_VOLUME_UP,
        AudioKind::Speaker => ICON_SPEAKER,
        AudioKind::Headphones => ICON_HEADPHONES,
        AudioKind::Headset => ICON_HEADSET_MIC,
        AudioKind::Display => ICON_TV,
        AudioKind::Car => ICON_CAR,
        AudioKind::Phone => ICON_PHONE,
        AudioKind::Webcam => ICON_WEBCAM,
        AudioKind::Microphone => ICON_MIC_EXTERNAL,
    }
}

/// Horizontal batteries (`battery_android_0` … `_6`, `_full`, `_bolt`), the
/// macOS shape; the fill shows the charge.
const ICON_BATTERY_LEVELS: [char; 7] = [
    '\u{f30d}', '\u{f30c}', '\u{f30b}', '\u{f30a}', '\u{f309}', '\u{f308}', '\u{f307}',
];
const ICON_BATTERY_FULL: char = '\u{f304}';
const ICON_BATTERY_BOLT: char = '\u{f305}';

/// The battery glyph for a charge: a bolt while charging, otherwise filled to
/// the nearest of the glyph's six bars (full from 95 %).
pub(crate) fn battery_glyph(percent: u8, charging: bool) -> char {
    if charging {
        ICON_BATTERY_BOLT
    } else if percent >= 95 {
        ICON_BATTERY_FULL
    } else {
        let bars = (usize::from(percent) * 6 + 50) / 100;
        ICON_BATTERY_LEVELS[bars.min(6)]
    }
}

/// Signal wedges (`network_wifi_1_bar` … `_3_bar`, `network_wifi`, then the
/// filled `signal_wifi_4_bar`), drawn like the battery glyphs: the filled part
/// shows the signal.
const ICON_WIFI_LEVELS: [char; 5] = ['\u{ebe4}', '\u{ebd6}', '\u{ebe1}', '\u{e1ba}', '\u{e1d8}'];
pub(crate) const ICON_WIFI: char = ICON_WIFI_LEVELS[4];
/// The empty wedge (`signal_wifi_0_bar`): the radio is on but nothing is joined.
pub(crate) const ICON_WIFI_NONE: char = '\u{f0b0}';
pub(crate) const ICON_WIFI_OFF: char = '\u{e1da}';
pub(crate) const ICON_CABLE: char = '\u{efe6}';

/// The wedge for a signal strength (0–100): the nearest of its five steps,
/// never fewer than one, since a network you can see has some signal.
pub(crate) fn wifi_glyph(strength: u8) -> char {
    let steps = (usize::from(strength) * 5 + 50) / 100;
    ICON_WIFI_LEVELS[steps.clamp(1, 5) - 1]
}

/// The glyph for BlueZ's freedesktop icon name (`audio-headset`, `input-mouse` …).
pub(crate) fn device_glyph(icon: &str) -> char {
    match icon {
        i if i.starts_with("audio-head") => ICON_HEADPHONES,
        i if i.starts_with("audio") => ICON_SPEAKER,
        "input-mouse" | "input-tablet" => ICON_MOUSE,
        "input-keyboard" => ICON_KEYBOARD,
        "input-gaming" => ICON_GAMEPAD,
        "phone" => ICON_PHONE,
        "computer" => ICON_COMPUTER,
        _ => ICON_DEVICE_OTHER,
    }
}

const ICON_POWER_SAVER: char = '\u{ea35}';
const ICON_BALANCED: char = '\u{eaf6}';
const ICON_PERFORMANCE: char = '\u{e9e4}';

/// The glyph for a power-profiles-daemon profile.
pub(crate) fn power_profile_glyph(profile: &str) -> char {
    match profile {
        "power-saver" => ICON_POWER_SAVER,
        "performance" => ICON_PERFORMANCE,
        _ => ICON_BALANCED,
    }
}

/// The glyph for a UPower peripheral: what it is, else its battery.
pub(crate) fn peripheral_glyph(device: &PowerDevice) -> char {
    match device.kind() {
        DeviceKind::Mouse => ICON_MOUSE,
        DeviceKind::Keyboard => ICON_KEYBOARD,
        DeviceKind::Headset => ICON_HEADPHONES,
        DeviceKind::GamingInput => ICON_GAMEPAD,
        _ => battery_glyph(device.percent(), device.state() == BatteryState::Charging),
    }
}

/// The glyph for a Tailscale peer's `OS`: a computer or a phone.
pub(crate) fn os_glyph(os: &str) -> char {
    match os.to_ascii_lowercase().as_str() {
        "linux" | "windows" | "macos" | "freebsd" | "openbsd" => ICON_COMPUTER,
        "ios" | "android" => ICON_PHONE,
        _ => ICON_DEVICE_OTHER,
    }
}

/// What a control shows: a font glyph, or a mark drawn from shapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Glyph {
    Font(char),
    /// Tailscale's nine dots.
    Tailscale,
}

impl From<char> for Glyph {
    fn from(glyph: char) -> Self {
        Self::Font(glyph)
    }
}

/// `glyph` at `size` in `color`, as a box exactly its size.
pub(crate) fn glyph(glyph: Glyph, size: f32, color: Color) -> Rectangle {
    match glyph {
        Glyph::Font(ch) => {
            let text = icon(ch, size, color);
            let height = match amane::Widget::height(&text) {
                Size::Fixed(height) => height,
                Size::Parent => size,
            };
            Rectangle::new()
                .width(Size::Fixed(fixed_width(&text)))
                .height(Size::Fixed(height))
                .fill(Color::TRANSPARENT)
                .child(text)
        }
        Glyph::Tailscale => tailscale_mark(size, color),
    }
}

/// Tailscale's mark, rebuilt from its official geometry on a 16-unit grid:
/// dots of radius 5/3 five units apart (upstream's 3:1 spacing to radius),
/// the middle row and the bottom centre solid, the other five at 40 %.
fn tailscale_mark(size: f32, color: Color) -> Rectangle {
    const SOLID: [[bool; 3]; 3] = [
        [false, false, false],
        [true, true, true],
        [false, true, false],
    ];
    let unit = size / 16.0;
    let dot = unit * 10.0 / 3.0;
    let gap = unit * 5.0 - dot;
    let rows = SOLID
        .iter()
        .map(|row| {
            let dots = row
                .iter()
                .map(|solid| {
                    boxed(
                        Rectangle::new()
                            .width(Size::Fixed(dot))
                            .height(Size::Fixed(dot))
                            .radius(Radius::Full)
                            .fill(color)
                            .opacity(if *solid { 1.0 } else { 0.4 }),
                    )
                })
                .collect();
            boxed(Row::new(dots).gap(gap))
        })
        .collect();
    Rectangle::new()
        .width(Size::Fixed(size))
        .height(Size::Fixed(size))
        .fill(Color::TRANSPARENT)
        .align_child(Center, Center)
        .child(Column::new(rows).gap(gap))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battery_glyph_follows_the_charge() {
        assert_eq!(battery_glyph(0, false), ICON_BATTERY_LEVELS[0]);
        assert_eq!(battery_glyph(18, false), ICON_BATTERY_LEVELS[1]);
        assert_eq!(battery_glyph(50, false), ICON_BATTERY_LEVELS[3]);
        assert_eq!(battery_glyph(94, false), ICON_BATTERY_LEVELS[6]);
        assert_eq!(battery_glyph(95, false), ICON_BATTERY_FULL);
        assert_eq!(battery_glyph(100, false), ICON_BATTERY_FULL);
        assert_eq!(battery_glyph(18, true), ICON_BATTERY_BOLT);
    }

    #[test]
    fn wifi_glyph_follows_the_signal() {
        assert_eq!(wifi_glyph(0), ICON_WIFI_LEVELS[0]);
        assert_eq!(wifi_glyph(29), ICON_WIFI_LEVELS[0]);
        assert_eq!(wifi_glyph(30), ICON_WIFI_LEVELS[1]);
        assert_eq!(wifi_glyph(50), ICON_WIFI_LEVELS[2]);
        assert_eq!(wifi_glyph(70), ICON_WIFI_LEVELS[3]);
        assert_eq!(wifi_glyph(89), ICON_WIFI_LEVELS[3]);
        assert_eq!(wifi_glyph(90), ICON_WIFI);
        assert_eq!(wifi_glyph(100), ICON_WIFI);
    }

    #[test]
    fn audio_glyph_shows_what_sound_goes_through() {
        assert_eq!(audio_glyph(AudioKind::Internal, false), ICON_VOLUME_UP);
        assert_eq!(audio_glyph(AudioKind::Internal, true), ICON_MIC);
        assert_eq!(audio_glyph(AudioKind::Headphones, false), ICON_HEADPHONES);
        assert_eq!(audio_glyph(AudioKind::Headset, true), ICON_HEADSET_MIC);
        assert_eq!(audio_glyph(AudioKind::Display, false), ICON_TV);
        assert_eq!(audio_glyph(AudioKind::Speaker, false), ICON_SPEAKER);
    }

    #[test]
    fn device_glyph_follows_the_bluez_icon() {
        assert_eq!(device_glyph("audio-headset"), ICON_HEADPHONES);
        assert_eq!(device_glyph("audio-headphones"), ICON_HEADPHONES);
        assert_eq!(device_glyph("audio-card"), ICON_SPEAKER);
        assert_eq!(device_glyph("input-mouse"), ICON_MOUSE);
        assert_eq!(device_glyph("phone"), ICON_PHONE);
        assert_eq!(device_glyph(""), ICON_DEVICE_OTHER);
    }
}
