//! The phone's screen on the Phone page: a **Screen** row that opens
//! `emaki-mirror`, and before that **Set Up Screen Sharing…**, a page that
//! pairs adb with the phone's Wireless debugging by a QR code the phone
//! scans (or the code it shows).
//!
//! KDE Connect and adb complement each other here: KDE Connect knows where
//! the phone is on the network, and adb (through the fork's `Adb`
//! service) finds the phone's Wireless debugging there by mDNS, pairs and
//! connects. A phone set up once is remembered (`services::phone::screen`),
//! so its row stays Screen.

use std::cell::RefCell;
use std::net::IpAddr;

use amane::{Center, Column, Parent, Rectangle, Row, Service, Text, TextInput, Widget};
use emaki_phone::{Adb, AdbAction, AdbOperation, PhoneDevice, WirelessKind};
use qrcode::{Color as Module, EcLevel, QrCode};

use crate::services::installed;
use crate::services::phone::MIRROR_APP;
use crate::services::phone::screen::{open_screen, ready, remember, set_up};
use crate::services::phone::webcam::{Facing, Webcam, set_camera};
use crate::surfaces::quick_settings::QuickSettings;
use crate::surfaces::quick_settings::detail::{DetailPage, page_width};
use crate::tokens::{Styled, sys};
use emaki_kit::buttons::{PushButton, Style, Switch};
use emaki_kit::layout::{boxed, caption};
use emaki_kit::list::ListItem;
use emaki_kit::material::{self, Level};

/// `screen_share`, for the Screen row.
pub(crate) const ICON_SCREEN: char = '\u{e0e2}';

/// How big the QR code is drawn.
const QR_SIZE: f32 = 196.0;

/// What the pairing page is doing, for the phone it's open for.
struct Pairing {
    device: String,
    /// The service name and password the QR code carries.
    name: String,
    password: String,
    /// A pairing was asked for, so it isn't asked again.
    asked: bool,
    /// The code typed for "pair with code instead".
    code: String,
}

thread_local! {
    static PAIRING: RefCell<Option<Pairing>> = const { RefCell::new(None) };

}

/// The phone page's row for the screen, when the app and adb are here.
pub(crate) fn row(device: &PhoneDevice, chevron: impl Fn() -> Text) -> Option<ListItem> {
    if !installed(MIRROR_APP) || !Adb::read().installed() {
        return None;
    }
    let id = device.id().to_string();

    // set up, or adb has it anyway (over USB): the screen opens, and waits for the phone if need be
    if set_up(&id) || ready(device) {
        return Some(
            ListItem::new("emaki.qs.phone.device.screen", "Screen")
                .leading(ICON_SCREEN)
                .trailing(chevron())
                .on_press(move || open_screen(&id)),
        );
    }

    Some(
        ListItem::new(
            "emaki.qs.phone.device.screen-setup",
            "Set Up Screen Sharing…",
        )
        .leading(ICON_SCREEN)
        .trailing(chevron())
        .on_press(move || start(&id)),
    )
}

/// The webcam's rows, when the loopback device is there and adb has the phone.
pub(crate) fn camera_rows(device: &PhoneDevice) -> Vec<ListItem> {
    let webcam = *Webcam::read();
    if !installed(MIRROR_APP) || !webcam.loopback || !ready(device) {
        return Vec::new();
    }
    let running = webcam.running;
    let on = running.is_some();
    let id = device.id().to_string();

    let (toggled, pressed) = (id.clone(), id.clone());
    let mut rows = vec![
        ListItem::new("emaki.qs.phone.device.webcam", "Use as Webcam")
            .leading(ICON_CAMERA)
            .trailing(
                Switch::new("emaki.qs.phone.device.webcam.switch", on)
                    .on_toggle(move |on| set_camera(&toggled, on.then_some(Facing::Front)))
                    .build(),
            )
            .on_press(move || set_camera(&pressed, (!on).then_some(Facing::Front))),
    ];

    if let Some(facing) = running {
        let back = facing == Facing::Back;
        let (toggled, pressed) = (id.clone(), id);
        let pick = |back: bool| Some(if back { Facing::Back } else { Facing::Front });
        rows.push(
            ListItem::new("emaki.qs.phone.device.webcam.back", "Back Camera")
                .trailing(
                    Switch::new("emaki.qs.phone.device.webcam.back.switch", back)
                        .on_toggle(move |on| set_camera(&toggled, pick(on)))
                        .build(),
                )
                .on_press(move || set_camera(&pressed, pick(!back))),
        );
    }
    rows
}

/// `photo_camera`, for the webcam's row.
const ICON_CAMERA: char = '\u{e412}';

/// A short random word from the clock and the process, enough for one pairing.
fn random(length: usize, alphabet: &[u8]) -> String {
    let mut seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |time| time.as_nanos() as u64)
        ^ (u64::from(std::process::id()) << 32);
    (0..length)
        .map(|_| {
            // xorshift
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            alphabet[(seed % alphabet.len() as u64) as usize] as char
        })
        .collect()
}

fn start(device: &str) {
    const LETTERS: &[u8] = b"abcdefghijkmnopqrstuvwxyz23456789";
    PAIRING.set(Some(Pairing {
        device: device.to_string(),
        name: format!("amane-{}", random(8, LETTERS)),
        password: random(10, LETTERS),
        asked: false,
        code: String::new(),
    }));
    Adb::look(true);
    QuickSettings::changed();
}

fn close() {
    PAIRING.set(None);
    Adb::look(false);
    QuickSettings::changed();
}

/// The phone page was left.
pub(crate) fn left() {
    if PAIRING.with_borrow(Option::is_some) {
        PAIRING.set(None);
        Adb::look(false);
    }
}

/// The pairing page, while it's open for `device`.
pub(crate) fn page(device: &PhoneDevice) -> Option<DetailPage> {
    let open = PAIRING.with_borrow(|pairing| {
        pairing
            .as_ref()
            .is_some_and(|pairing| pairing.device == device.id())
    });
    if !open {
        return None;
    }

    follow(device);

    let device = device.clone();
    Some(
        DetailPage::new("emaki.qs.phone.screen-setup", "Screen Sharing")
            .subtitle(device.name().to_string())
            .on_back(close)
            .body(move |_| body(&device)),
    )
}

/// Pairs once the phone offers the QR code's pairing, and stays connected
/// to it once paired.
fn follow(device: &PhoneDevice) {
    let addresses: Vec<IpAddr> = device.addresses().to_vec();

    if ready(device) {
        let id = device.id().to_string();
        remember(&id);
        return;
    }

    let adb = Adb::read();
    let offered = PAIRING.with_borrow(|pairing| {
        let pairing = pairing.as_ref()?;
        if pairing.asked {
            return None;
        }
        adb.found()
            .iter()
            .find(|service| service.kind == WirelessKind::Pairing && service.name == pairing.name)
            .map(|service| (service.address, service.port, pairing.password.clone()))
    });
    drop(adb);

    if let Some((address, port, password)) = offered {
        PAIRING.with_borrow_mut(|pairing| {
            if let Some(pairing) = pairing {
                pairing.asked = true;
            }
        });
        // writes to Adb, which this view reads
        std::thread::spawn(move || {
            Adb::pair(address, port, &password);
            Adb::want(address);
            for address in addresses {
                Adb::want(address);
            }
        });
    }
}

/// With the code the phone shows under "Pair device with pairing code",
/// to its pairing service at the phone's address.
fn pair_with_code(device: &PhoneDevice) {
    let code = PAIRING.with_borrow(|pairing| {
        pairing
            .as_ref()
            .map(|pairing| pairing.code.trim().to_string())
    });
    let Some(code) = code.filter(|code| !code.is_empty()) else {
        return;
    };
    let addresses: Vec<IpAddr> = device.addresses().to_vec();
    let target = Adb::read()
        .found()
        .iter()
        .find(|service| {
            service.kind == WirelessKind::Pairing && addresses.contains(&service.address)
        })
        .map(|service| (service.address, service.port));
    let Some((address, port)) = target else {
        return;
    };
    std::thread::spawn(move || {
        Adb::pair(address, port, &code);
        for address in addresses {
            Adb::want(address);
        }
    });
}

/// What adb is busy with, as the page says it.
fn doing(operation: &AdbOperation) -> String {
    let target = &operation.target;
    match operation.action {
        AdbAction::Pairing => format!("Pairing with {target}…"),
        AdbAction::Connecting => format!("Connecting to {target}…"),
        AdbAction::Disconnecting => format!("Disconnecting {target}…"),
    }
}

fn body(device: &PhoneDevice) -> Vec<Box<dyn Widget>> {
    let (busy, error) = {
        let adb = Adb::read();
        (
            adb.operation().map(doing),
            adb.failure().map(|failure| failure.message.clone()),
        )
    };

    if ready(device) {
        let id = device.id().to_string();
        let open = PushButton::new("emaki.qs.phone.screen-setup.open", "Show Screen")
            .style(Style::Filled)
            .on_press(move || {
                open_screen(&id);
                close();
            })
            .build();
        return vec![
            boxed(
                caption("Paired. From now on the screen opens whenever Wireless debugging is on.")
                    .wrap(),
            ),
            boxed(open),
        ];
    }

    let Some((name, password)) = PAIRING.with_borrow(|pairing| {
        pairing
            .as_ref()
            .map(|pairing| (pairing.name.clone(), pairing.password.clone()))
    }) else {
        return Vec::new();
    };

    let mut body: Vec<Box<dyn Widget>> = vec![boxed(
        caption(
            "On the phone: Settings › Developer options › Wireless debugging › Pair device \
             with QR code, then scan this.",
        )
        .wrap(),
    )];

    body.push(boxed(
        Rectangle::new()
            .width(Parent)
            .height(QR_SIZE + 24.0)
            .align_child(Center, Center)
            .child(qr(&format!("WIFI:T:ADB;S:{name};P:{password};;"))),
    ));

    let status = match (&busy, &error) {
        (Some(busy), _) => busy.clone(),
        (None, Some(error)) => error.clone(),
        (None, None) => String::from("Waiting for the phone…"),
    };
    let status_color = if busy.is_none() && error.is_some() {
        sys::color::status::critical::text()
    } else {
        material::label(Level::Secondary)
    };
    body.push(boxed(
        Text::new(status)
            .style(sys::r#type::caption())
            .color(status_color)
            .wrap(),
    ));

    // or the six-digit code, for a phone that can't scan
    let pair = PushButton::new("emaki.qs.phone.screen-setup.pair", "Pair")
        .style(Style::Tonal)
        .on_press({
            let device = device.clone();
            move || pair_with_code(&device)
        })
        .build();
    let field_width = (page_width() - 80.0).max(1.0);
    let submitted = device.clone();
    let field = TextInput::new("emaki.qs.phone.screen-setup.code")
        .placeholder("Or the pairing code it shows")
        .size(sys::r#type::label().size)
        .color(material::label(Level::Primary))
        .width(field_width)
        .on_change(|text| {
            PAIRING.with_borrow_mut(|pairing| {
                if let Some(pairing) = pairing {
                    pairing.code = text.chars().filter(char::is_ascii_digit).take(6).collect();
                }
            });
        })
        .on_submit(move |_| pair_with_code(&submitted));
    body.push(boxed(
        Row::new(vec![boxed(field), boxed(pair)])
            .gap(sys::space::gap::s())
            .width(Parent)
            .align(Center),
    ));

    body.push(boxed(
        caption("Or plug it in with USB debugging on: the screen opens over the cable.").wrap(),
    ));

    vec![boxed(
        Column::new(body).gap(sys::space::gap::l()).width(Parent),
    )]
}

/// The code as squares, a row of dark runs at a time, on white with a
/// quiet zone, so a phone's camera reads it in either theme.
fn qr(text: &str) -> Rectangle {
    let Ok(code) = QrCode::with_error_correction_level(text, EcLevel::M) else {
        return Rectangle::new().width(QR_SIZE).height(QR_SIZE);
    };
    let width = code.width();
    let colors = code.to_colors();
    let module = (QR_SIZE / (width + 8) as f32).floor().max(1.0);
    let quiet = module * 4.0;
    let side = module * width as f32 + quiet * 2.0;

    let rows: Vec<Box<dyn Widget>> = colors
        .chunks(width)
        .map(|row| {
            let mut cells: Vec<Box<dyn Widget>> = Vec::new();
            let mut index = 0;
            while index < row.len() {
                let dark = row[index] == Module::Dark;
                let run = row[index..]
                    .iter()
                    .take_while(|cell| (**cell == Module::Dark) == dark)
                    .count();
                let cell = Rectangle::new().width(module * run as f32).height(module);
                cells.push(boxed(if dark {
                    cell.fill(amane::Color::BLACK)
                } else {
                    cell
                }));
                index += run;
            }
            boxed(Row::new(cells).height(module))
        })
        .collect();

    Rectangle::new()
        .width(side)
        .height(side)
        .fill(amane::Color::WHITE)
        .radius(sys::radius::raised())
        .padding(amane::Padding {
            top: quiet,
            right: quiet,
            bottom: quiet,
            left: quiet,
        })
        .child(Column::new(rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_words_use_the_alphabet() {
        let word = random(12, b"ab");
        assert_eq!(word.len(), 12);
        assert!(word.chars().all(|c| c == 'a' || c == 'b'));
    }

    #[test]
    fn the_pairing_text_makes_a_code() {
        let code = QrCode::with_error_correction_level(
            "WIFI:T:ADB;S:emaki-abcdefgh;P:abcdefghij;;",
            EcLevel::M,
        )
        .expect("a QR code");
        assert!(code.width() >= 21);
    }
}
