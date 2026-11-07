//! Each of the bar's status items: sound, Bluetooth, the network, Tailscale, Keep Awake, the
//! battery, the phone, Quick Settings' button, what's switched on, the bell, the clock, the
//! keyboard layout and the weather.

use super::*;

pub(super) fn volume() -> Option<Box<dyn Widget>> {
    let audio = Audio::read();
    let muted = audio.muted();
    let volume = audio.volume();
    let kind = audio.output_kind();
    drop(audio);

    // what is playing through: speakers, headphones, a headset, the TV …
    let (glyph, tip) = if muted {
        (ICON_VOLUME_OFF, String::from("Muted"))
    } else {
        (audio_glyph(kind, false), format!("Volume {volume}%"))
    };

    let icon = status_icon(
        "amane.bar.volume",
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
        .map(|device| BluetoothDevice::name(device).to_string())
        .collect();
    drop(bluetooth);

    let (glyph, tip) = match (powered, connected.is_empty()) {
        (false, _) => (ICON_BLUETOOTH_OFF, String::from("Bluetooth off")),
        (true, true) => (ICON_BLUETOOTH, String::from("Bluetooth on")),
        (true, false) => (ICON_BLUETOOTH_CONNECTED, connected.join(", ")),
    };
    let color = status_color(!connected.is_empty());
    Some(boxed(
        status_icon("amane.bar.bluetooth", glyph, color, Page::Bluetooth, tip).build(),
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
        Link::Wifi => (wifi_glyph(strength), ssid),
        Link::Wired => (ICON_CABLE, String::from("Wired")),
        // vpn and the rest carry no strength to show
        Link::Other => (ICON_WIFI, String::from("Connected")),
        Link::Offline if wifi_on => (ICON_WIFI_NONE, String::from("Offline")),
        Link::Offline => (ICON_WIFI_OFF, String::from("Wi‑Fi off")),
    };
    // grey like its neighbours: the glyph already shows the link
    Some(boxed(
        status_icon(
            "amane.bar.network",
            glyph,
            material::label(Level::Secondary),
            Page::Wifi,
            tip,
        )
        .build(),
    ))
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
            "amane.bar.tailscale",
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
            "amane.bar.keep-awake",
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
    let on_ac = charge.on_ac();
    let low = battery::low(&charge);
    drop(charge);

    let (color, tip) = if on_ac {
        (
            sys::color::accent::bg(),
            format!("Battery {pct}% · charging"),
        )
    } else {
        (
            battery::tint(low, material::label(Level::Secondary)),
            format!("Battery {pct}%"),
        )
    };

    Some(boxed(
        status_icon(
            "amane.bar.battery",
            battery_glyph(pct, on_ac),
            color,
            Page::Power,
            tip,
        )
        .build(),
    ))
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
        status_icon("amane.bar.phone", ICON_PHONE, color, Page::Phone, tip).build(),
    ))
}

/// Quick Settings' button in the bar.
pub(super) fn quick_settings_button() -> Box<dyn Widget> {
    boxed(
        bar_icon(
            "amane.bar.quick-settings",
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

/// Settings GNOME shows in the bar only while they're on: night light, a
/// VPN, a power mode other than Balanced, airplane mode.
pub(super) fn on_icons() -> Vec<Box<dyn Widget>> {
    let mut glyphs = Vec::new();

    if NightLight::read().on() {
        glyphs.push(("amane.bar.night-light", ICON_NIGHT_LIGHT, "Night Light"));
    }
    if Connections::read().vpns().iter().any(|vpn| vpn.active) {
        glyphs.push(("amane.bar.vpn", ICON_VPN, "VPN"));
    }
    let profile = PowerProfile::read();
    if profile.present() && profile.active() != "balanced" {
        let tip = if profile.active() == "performance" {
            "Performance Mode"
        } else {
            "Power Saver"
        };
        glyphs.push((
            "amane.bar.power-mode",
            power_profile_glyph(profile.active()),
            tip,
        ));
    }
    drop(profile);
    if Rfkill::read().airplane() {
        glyphs.push(("amane.bar.airplane", ICON_AIRPLANE, "Airplane Mode"));
    }

    grid_icons(glyphs, material::label(Level::Secondary))
}

/// Status glyphs in `color`, each opening Quick Settings' grid.
pub(super) fn grid_icons(
    glyphs: Vec<(&'static str, char, impl Into<String>)>,
    color: Color,
) -> Vec<Box<dyn Widget>> {
    glyphs
        .into_iter()
        .map(|(id, glyph, tip)| boxed(status_icon(id, glyph, color, Page::Grid, tip).build()))
        .collect()
}

/// A moon while Do Not Disturb is on, else a bell while something arrived
/// unseen; nothing otherwise. Click opens the center, right-click picks
/// how long Do Not Disturb lasts.
pub(super) fn notification_icon() -> Option<Box<dyn Widget>> {
    let dnd = DoNotDisturb::read().on();
    let unseen = notifications::unseen();

    let (glyph, color, tip) = match (dnd, unseen) {
        (true, _) => (ICON_DND, material::label(Level::Primary), "Do Not Disturb"),
        (false, true) => (
            ICON_NOTIFICATIONS,
            sys::color::accent::bg(),
            "New notifications",
        ),
        (false, false) => return None,
    };

    Some(boxed(
        bar_icon("amane.bar.notifications", glyph, color)
            .active(is_open(Panel::Notifications))
            .on_press(|| panel::toggle(Panel::Notifications))
            .on_secondary(|| notifications::dnd_menu(BAR_NS, (cursor().0, menu_edge())))
            .tooltip(BAR_NS, tip)
            .build(),
    ))
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
            item(DrawnButton::new("amane.bar.clock", move |color| {
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
    let widest = clock_width(&time);
    let width = widest + sys::space::inset::s() * 2.0;

    boxed(
        item(PushButton::new("amane.bar.clock", time))
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
            Rectangle::new()
                .width(face)
                .height(face)
                .fill(Color::TRANSPARENT)
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
    Rectangle::new()
        .width(metrics().item_width)
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .align_child(Center, Center)
        .child(
            Rectangle::new()
                .width(face)
                .height(face)
                .radius(face / 2.0)
                .border(1.5, color)
                .child(amane::Stack::new(vec![
                    // drawn for a 16 px face, and grown with a bigger one
                    hand(4.0 * face / 16.0, 1.5, hours),
                    hand(6.0 * face / 16.0, 1.2, minutes),
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
    let width = text_width(&code, metrics().label()) + sys::space::inset::s() * 2.0;
    Some(boxed(
        item(PushButton::new("amane.bar.keyboard", code))
            .text(metrics().label())
            .width(width.max(metrics().item_width))
            .on_press(|| {
                let layouts = KeyboardLayout::read();
                let current = layouts.current_index();
                let mut menu = amane_kit::menu::ContextMenu::new();
                for (index, name) in layouts.names().iter().enumerate() {
                    let row = amane_kit::menu::MenuItem::label(name.clone())
                        .check(amane_kit::menu::Check::Radio(index == current))
                        .on_select(move || keyboard_layout::switch(index));
                    menu = menu.item(row);
                }
                drop(layouts);
                amane_kit::menu::layer::show_under(
                    "amane.bar.keyboard",
                    menu,
                    BAR_NS,
                    (cursor().0 as i32, menu_edge()),
                );
            })
            .tooltip(BAR_NS, name)
            .build(),
    ))
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
    drop(weather);
    let (glyph, words) = describe(now.code, now.day);
    let text = format!("{:.0}°", now.temperature);
    let width =
        text_width(&text, metrics().numeric()) + metrics().item_width + sys::space::inset::s();
    Some(boxed(
        item(PushButton::new("amane.bar.weather", text))
            .icon(glyph)
            .icon_size(metrics().icon)
            .text(metrics().numeric())
            .width(width)
            .tooltip(
                BAR_NS,
                format!("{place} · {words} · H:{:.0}° L:{:.0}°", now.high, now.low),
            )
            .build(),
    ))
}
