//! Each of the bar's status items: sound, Bluetooth, the network, Tailscale, Keep Awake, the
//! battery, the phone, Quick Settings' button, what's switched on, the bell, the clock, the
//! keyboard layout and the weather.

use super::*;
use emaki_kit::layout::frame;

pub(super) fn volume() -> Option<Box<dyn Widget>> {
    let audio = Audio::read();
    let muted = audio.muted();
    let volume = audio.volume();
    let kind = audio.output_kind();
    let output = audio
        .outputs()
        .iter()
        .find(|device| device.default())
        .map(|device| device.description().to_string());
    drop(audio);

    // what is playing through: speakers, headphones, a headset, the TV …
    let (glyph, title) = if muted {
        (ICON_VOLUME_OFF, String::from("Muted"))
    } else {
        (audio_glyph(kind, false), format!("Volume {volume}%"))
    };
    let tip = with_detail(title, output.as_deref());

    let icon = status_icon(
        "emaki.bar.volume",
        glyph,
        material::label(Level::Secondary),
        Page::Sound,
        tip,
    )
    .on_scroll(|scroll: Scroll| {
        // the volume as it is now: a burst of scroll steps comes before the
        // bar draws again, and each should step on from the last
        let now = Audio::read().volume();
        let next = slider::scrolled(now, scroll, slider::STEP);
        if next != now {
            Audio::set_volume(next);
        }
    })
    .on_middle(Audio::toggle_mute);
    Some(boxed(icon.build()))
}

/// Only with an adapter: off, on, or connected (in the accent).
pub(super) fn bluetooth() -> Option<Box<dyn Widget>> {
    let bluetooth = Bluetooth::read();
    if !bluetooth.available() {
        return None;
    }
    let powered = bluetooth.powered();
    let connected: Vec<String> = bluetooth
        .devices()
        .iter()
        .filter(|device| device.connected())
        // with its charge, where it tells
        .map(|device| match device.battery() {
            Some(charge) => format!("{} {charge}%", BluetoothDevice::name(device)),
            None => BluetoothDevice::name(device).to_string(),
        })
        .collect();
    drop(bluetooth);

    let (glyph, tip) = match (powered, connected.is_empty()) {
        (false, _) => (ICON_BLUETOOTH_OFF, String::from("Bluetooth off")),
        (true, true) => (ICON_BLUETOOTH, String::from("Bluetooth on")),
        (true, false) => (ICON_BLUETOOTH_CONNECTED, connected.join(", ")),
    };
    let color = status_color(!connected.is_empty());
    Some(boxed(
        status_icon("emaki.bar.bluetooth", glyph, color, Page::Bluetooth, tip).build(),
    ))
}

pub(super) fn network() -> Option<Box<dyn Widget>> {
    let net = Network::read();
    let link = net.link();
    let strength = net.strength();
    let wifi_on = net.wifi_enabled();
    let ssid = net.ssid().to_string();
    drop(net);

    let (glyph, tip) = match link {
        Link::Wifi => (
            wifi_glyph(strength),
            format!("{ssid} · {} signal", signal_word(strength)),
        ),
        Link::Wired => (ICON_CABLE, String::from("Wired")),
        // a VPN, a tether and the rest: no strength to show, and not Wi‑Fi
        Link::Other => (ICON_LAN, other_link()),
        Link::Offline if wifi_on => (ICON_WIFI_NONE, String::from("Offline")),
        Link::Offline => (ICON_WIFI_OFF, String::from("Wi‑Fi off")),
    };
    // grey like its neighbours: the glyph already shows the link
    Some(boxed(
        status_icon(
            "emaki.bar.network",
            glyph,
            material::label(Level::Secondary),
            Page::Wifi,
            tip,
        )
        .build(),
    ))
}

/// A Wi‑Fi signal in a word, by the wedges it fills.
fn signal_word(strength: u8) -> &'static str {
    match strength {
        67.. => "Strong",
        34..67 => "Good",
        _ => "Weak",
    }
}

/// `title`, then ` · detail` when there's one.
fn with_detail(title: String, detail: Option<&str>) -> String {
    match detail.filter(|detail| !detail.is_empty()) {
        Some(detail) => format!("{title} · {detail}"),
        None => title,
    }
}

/// What a link that's neither Wi‑Fi nor a cable is: the VPN up, if one is.
fn other_link() -> String {
    Connections::read()
        .vpns()
        .iter()
        .find(|vpn| vpn.active)
        .map_or_else(
            || String::from("Connected"),
            |vpn| format!("Connected · {}", vpn.name),
        )
}

/// Tailscale's mark while connected; accent while routing through an
/// exit node.
pub(super) fn tailscale() -> Option<Box<dyn Widget>> {
    let tailscale = Tailscale::read();
    if tailscale.state() != TailscaleState::Running {
        return None;
    }
    let routed = tailscale.exit_node().is_some();
    let tip = format!("Tailscale · {}", tailscale.describe());
    drop(tailscale);

    Some(boxed(
        status_icon(
            "emaki.bar.tailscale",
            Glyph::Tailscale,
            status_color(routed),
            Page::Tailscale,
            tip,
        )
        .build(),
    ))
}

/// While Keep Awake holds anything; accent, since the machine isn't
/// behaving as it normally would.
pub(super) fn keep_awake() -> Option<Box<dyn Widget>> {
    if !KeepAwake::read().on() {
        return None;
    }
    let tip = format!("Keep Awake · {}", quick_settings::keep_awake_status());

    Some(boxed(
        status_icon(
            "emaki.bar.keep-awake",
            ICON_KEEP_AWAKE,
            status_color(true),
            Page::KeepAwake,
            tip,
        )
        .build(),
    ))
}

pub(super) fn battery() -> Option<Box<dyn Widget>> {
    let charge = Battery::read();
    if !charge.present() {
        return None;
    }
    let pct = charge.percent();
    // charging, not merely plugged in: a full battery on AC isn't charging
    let charging = charge.charging();
    let low = battery::low(&charge);
    let detail = quick_settings::battery_detail(&charge);
    drop(charge);

    let color = if charging {
        sys::color::accent::bg()
    } else {
        battery::tint(low, material::label(Level::Secondary))
    };
    let tip = battery_tip(pct, &detail);

    Some(boxed(
        status_icon(
            "emaki.bar.battery",
            battery_glyph(pct, charging),
            color,
            Page::Power,
            tip,
        )
        .build(),
    ))
}

/// "Battery 42% · 3 h 5 min left", "Battery 100% · full", or the charge
/// alone when Quick Settings' detail has nothing to add.
fn battery_tip(percent: u8, detail: &str) -> String {
    if detail.is_empty() {
        format!("Battery {percent}%")
    } else {
        format!("Battery {percent}% · {detail}")
    }
}

/// The phone KDE Connect has, with its charge, while it's connected:
/// critical while low, accent while charging, like the battery beside it.
pub(super) fn phone() -> Option<Box<dyn Widget>> {
    let kdeconnect = KdeConnect::read();
    let phone = kdeconnect.primary()?;
    let name = phone.name().to_string();
    let charge = phone.battery()?;
    drop(kdeconnect);

    let (color, tip) = if charge.charging {
        (
            sys::color::accent::bg(),
            format!("{name} {}% · charging", charge.percent),
        )
    } else {
        (
            battery::color(charge.percent, false, material::label(Level::Secondary)),
            format!("{name} {}%", charge.percent),
        )
    };

    Some(boxed(
        status_icon("emaki.bar.phone", ICON_PHONE, color, Page::Phone, tip).build(),
    ))
}

/// Quick Settings' button in the bar.
pub(super) fn quick_settings_button() -> Box<dyn Widget> {
    boxed(
        bar_icon(
            "emaki.bar.quick-settings",
            ICON_TUNE,
            material::label(Level::Secondary),
        )
        .active(is_open(Panel::QuickSettings) && !quick_settings::alone())
        // over a status icon's page, it brings the whole card instead
        .on_press(|| {
            if quick_settings::alone() {
                quick_settings::open(Page::Grid);
            } else {
                panel::toggle(Panel::QuickSettings);
            }
        })
        .tooltip(BAR_NS, "Quick Settings")
        .build(),
    )
}

/// Settings shown in the bar only while they're on: night light, a VPN, a
/// power mode other than Balanced, airplane mode; each opens its own page.
pub(super) fn on_icons() -> Vec<Box<dyn Widget>> {
    let mut glyphs = Vec::new();

    if NightLight::read().on() {
        glyphs.push((
            "emaki.bar.night-light",
            ICON_NIGHT_LIGHT,
            "Night Light",
            Page::NightLight,
        ));
    }
    if Connections::read().vpns().iter().any(|vpn| vpn.active) {
        glyphs.push(("emaki.bar.vpn", ICON_VPN, "VPN", Page::Vpn));
    }
    let profile = PowerProfile::read();
    if profile.present() && profile.active() != "balanced" {
        let tip = if profile.active() == "performance" {
            "Performance Mode"
        } else {
            "Power Saver"
        };
        glyphs.push((
            "emaki.bar.power-mode",
            power_profile_glyph(profile.active()),
            tip,
            Page::PowerMode,
        ));
    }
    drop(profile);
    if Rfkill::read().airplane() {
        // airplane mode has no page of its own: its tile is in the grid
        glyphs.push((
            "emaki.bar.airplane",
            ICON_AIRPLANE,
            "Airplane Mode",
            Page::Grid,
        ));
    }

    let color = material::label(Level::Secondary);
    glyphs
        .into_iter()
        .map(|(id, glyph, tip, page)| boxed(status_icon(id, glyph, color, page, tip).build()))
        .collect()
}

/// A moon while Do Not Disturb is on, else a bell while something arrived
/// unseen; nothing otherwise. Click opens the center, right-click picks
/// how long Do Not Disturb lasts.
pub(super) fn notification_icon() -> Option<Box<dyn Widget>> {
    let until = DoNotDisturb::read().until();
    let unseen = notifications::unseen_count();

    let (glyph, color, tip) = match (until, unseen) {
        (Some(until), _) => (ICON_DND, material::label(Level::Primary), dnd_tip(until)),
        (None, 0) => return None,
        (None, 1) => (
            ICON_NOTIFICATIONS,
            sys::color::accent::bg(),
            String::from("1 new notification"),
        ),
        (None, many) => (
            ICON_NOTIFICATIONS,
            sys::color::accent::bg(),
            format!("{many} new notifications"),
        ),
    };

    Some(boxed(
        bar_icon("emaki.bar.notifications", glyph, color)
            .active(is_open(Panel::Notifications) || menu_showing("emaki.bar.notifications"))
            .on_press(|| panel::toggle(Panel::Notifications))
            .on_secondary(|| {
                opened_menu("emaki.bar.notifications", notifications::DND_MENU_KEY);
                notifications::dnd_menu(BAR_NS, menu_point());
            })
            .tooltip(BAR_NS, tip)
            .build(),
    ))
}

/// "Do Not Disturb", or "Do Not Disturb · until 14:00" when it ends by itself.
fn dnd_tip(until: crate::services::do_not_disturb::Until) -> String {
    use crate::services::do_not_disturb::Until;
    match until {
        Until::Forever => String::from("Do Not Disturb"),
        Until::At(end) => format!(
            "Do Not Disturb · until {}",
            crate::services::clock::local(end, "%H:%M")
        ),
    }
}

/// The clock opens the notification center; its tooltip is
/// the date. `dated` says whether there's room for a date shown "when space
/// allows".
pub(super) fn clock(dated: bool) -> Box<dyn Widget> {
    let clock = Clock::read();
    let time = clock.bar_text(dated);
    let date = clock.date();
    let (hour, minute) = clock.hands();
    let full_time = clock.text();
    drop(clock);

    if status_bar().clock_style == ClockStyle::Analog {
        return boxed(
            item(DrawnButton::new("emaki.bar.clock", move |color| {
                analog(hour, minute, color)
            }))
            .width(metrics().item_width)
            .color(material::label(Level::Primary))
            .active(is_open(Panel::Notifications))
            .on_press(|| panel::toggle(Panel::Notifications))
            .tooltip(BAR_NS, format!("{full_time} · {date}"))
            .build(),
        );
    }

    // as wide as the time with its colons, so flashing them never moves it
    let width = metrics().item_width_for(clock_width(&time));

    boxed(
        item(PushButton::new("emaki.bar.clock", time))
            .text(metrics().numeric())
            .width(width)
            .active(is_open(Panel::Notifications))
            .on_press(|| panel::toggle(Panel::Notifications))
            .tooltip(BAR_NS, date)
            .build(),
    )
}

/// How wide the clock's text is with its colons, whether or not they show.
pub(super) fn clock_width(time: &str) -> f32 {
    text_width(&time.replace('\u{2008}', ":"), metrics().numeric())
}

/// A small clock face with its two hands, the bar's analog clock.
pub(super) fn analog(hour: u32, minute: u32, color: Color) -> Rectangle {
    let face = metrics().icon;
    let hand = |length: f32, thickness: f32, degrees: f32| -> Box<dyn Widget> {
        // a hand from the middle up, in a frame the size of the face that turns
        boxed(
            frame(face, face)
                .rotate(degrees)
                .align_child(Center, Start)
                .padding(amane::Padding {
                    top: face / 2.0 - length,
                    ..amane::Padding::default()
                })
                .child(
                    Rectangle::new()
                        .width(thickness)
                        .height(length)
                        .radius(thickness / 2.0)
                        .fill(color),
                ),
        )
    };
    let minutes = minute as f32 * 6.0;
    let hours = (hour % 12) as f32 * 30.0 + minute as f32 * 0.5;
    // a 16 px face reads clearer in finer lines; a bigger one keeps its weight
    let (rim, hour_hand, minute_hand) = if face <= 16.0 {
        (1.0, 1.25, 1.0)
    } else {
        (1.5, 1.5, 1.2)
    };
    Rectangle::new()
        .width(metrics().item_width)
        .height(Parent)
        .align_child(Center, Center)
        .child(
            Rectangle::new()
                .width(face)
                .height(face)
                .radius(face / 2.0)
                .border(rim, color)
                .child(amane::Stack::new(vec![
                    // drawn for a 16 px face, and grown with a bigger one
                    hand(4.0 * face / 16.0, hour_hand, hours),
                    hand(6.0 * face / 16.0, minute_hand, minutes),
                ])),
        )
}

/// The keyboard layout as an input menu shows it, a short code, with
/// two or more; a click lists them to switch.
pub(super) fn keyboard() -> Option<Box<dyn Widget>> {
    use crate::services::desktop::keyboard_layout::{self, KeyboardLayout, short};
    let layouts = KeyboardLayout::read();
    if layouts.names().len() < 2 {
        return None;
    }
    let name = layouts.current().unwrap_or_default().to_string();
    drop(layouts);
    let code = short(&name);
    let width = metrics().item_width_for(text_width(&code, metrics().label()));
    Some(boxed(
        item(PushButton::new("emaki.bar.keyboard", code))
            .text(metrics().label())
            .width(width)
            .active(menu_showing("emaki.bar.keyboard"))
            // scrolling goes round the layouts, as the menu lists them
            .on_scroll(|scroll: Scroll| {
                let step = LAYOUT_SCROLLED.with(|scrolled| scroll_steps(scrolled, &scroll));
                let layouts = KeyboardLayout::read();
                let next = cycled(layouts.current_index(), layouts.names().len(), step);
                drop(layouts);
                if let Some(next) = next {
                    keyboard_layout::switch(next);
                }
            })
            .on_press(|| {
                let layouts = KeyboardLayout::read();
                let current = layouts.current_index();
                let mut menu = emaki_kit::menu::ContextMenu::new();
                for (index, name) in layouts.names().iter().enumerate() {
                    let row = emaki_kit::menu::MenuItem::label(name.clone())
                        .check(emaki_kit::menu::Check::Radio(index == current))
                        .on_select(move || keyboard_layout::switch(index));
                    menu = menu.item(row);
                }
                drop(layouts);
                show_menu(
                    "emaki.bar.keyboard",
                    "emaki.bar.keyboard",
                    menu,
                    menu_point(),
                );
            })
            .tooltip(BAR_NS, name)
            .build(),
    ))
}

/// The index `step` round from `current` among `len`, going round past
/// either end; nothing when that's where it is.
fn cycled(current: usize, len: usize, step: i32) -> Option<usize> {
    if len < 2 || step == 0 {
        return None;
    }
    let len = len as i64;
    Some((current as i64 + i64::from(step)).rem_euclid(len) as usize)
}

/// The weather where `weather_place` is, once it's been read: its glyph
/// and temperature; the tooltip says the place, the day's high and low.
pub(super) fn weather() -> Option<Box<dyn Widget>> {
    use crate::services::desktop::weather::{Weather, describe};
    if !status_bar().weather {
        return None;
    }
    let weather = Weather::read();
    let now = weather.now()?.clone();
    let place = weather.place().to_string();
    let stale = weather.stale();
    drop(weather);
    let (glyph, words) = describe(now.code, now.day);
    // an old reading, kept while Open-Meteo can't be reached, shows dimmed
    let (color, note) = if stale {
        (material::label(Level::Tertiary), " · Not up to date")
    } else {
        (material::label(Level::Primary), "")
    };
    let text = format!("{:.0}°", now.temperature);
    let metrics = metrics();
    let width = metrics
        .item_width_for(metrics.icon + metrics.inner_gap + text_width(&text, metrics.numeric()));
    Some(boxed(
        item(PushButton::new("emaki.bar.weather", text))
            .icon(glyph)
            .icon_size(metrics.icon)
            .text(metrics.numeric())
            .width(width)
            .color(color)
            // beside the clock, it opens what the clock does
            .active(is_open(Panel::Notifications))
            .on_press(|| panel::toggle(Panel::Notifications))
            .tooltip(
                BAR_NS,
                format!(
                    "{place} · {words} · H:{:.0}° L:{:.0}°{note}",
                    now.high, now.low
                ),
            )
            .build(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrolling_goes_round_the_layouts() {
        assert_eq!(cycled(0, 3, 1), Some(1));
        assert_eq!(cycled(2, 3, 1), Some(0));
        assert_eq!(cycled(0, 3, -1), Some(2));
        assert_eq!(cycled(0, 1, 1), None);
        assert_eq!(cycled(1, 3, 0), None);
    }

    #[test]
    fn a_tooltip_adds_its_detail_when_there_is_one() {
        assert_eq!(
            with_detail(String::from("Muted"), Some("Headphones")),
            "Muted · Headphones"
        );
        assert_eq!(with_detail(String::from("Muted"), Some("")), "Muted");
        assert_eq!(with_detail(String::from("Muted"), None), "Muted");
        assert_eq!(
            battery_tip(42, "3 h 5 min left"),
            "Battery 42% · 3 h 5 min left"
        );
        assert_eq!(battery_tip(80, ""), "Battery 80%");
        assert_eq!(signal_word(90), "Strong");
        assert_eq!(signal_word(50), "Good");
        assert_eq!(signal_word(10), "Weak");
    }
}
