//! When a battery reads low, and the colour that says so, shared by the
//! status bar and Quick Settings so they agree.

use amane::{Battery, Color, WarningLevel};

use crate::tokens::sys;

/// At or under this charge a battery reads low, where UPower gives no
/// warning level of its own (sysfs, peripherals).
pub(crate) const LOW: u8 = 15;

/// Whether the machine's battery is low: UPower's warning level, or the
/// threshold where the sysfs fallback has none.
pub(crate) fn low(battery: &Battery) -> bool {
    battery.warning() >= WarningLevel::Low || (battery.percent() <= LOW && !battery.on_ac())
}

/// `normal`, or the critical colour while a battery at `percent` is low and
/// not charging.
pub(crate) fn color(percent: u8, charging: bool, normal: Color) -> Color {
    tint(percent <= LOW && !charging, normal)
}

/// The critical colour while `low`, else `normal`.
pub(crate) fn tint(low: bool, normal: Color) -> Color {
    if low {
        sys::color::status::critical::bg()
    } else {
        normal
    }
}
