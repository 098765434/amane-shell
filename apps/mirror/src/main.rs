//! Phone Screen: the phone's screen and sound, mirrored and driven from
//! here through scrcpy's server, drawn by amane like the rest of the
//! shell. KDE Connect finds the phone (its address picks the adb device)
//! and keeps the clipboard; adb carries the screen.
//!
//! `amane-mirror --device <kdeconnect id>` mirrors that phone; `--serial
//! <adb serial>` names the adb device instead. Without either it mirrors
//! KDE Connect's connected phone, or the one phone on USB.
//!
//! `--app <package> [--name <label>]` opens one of the phone's apps in a
//! window of its own instead: a virtual display that resizes with the
//! window, under its own app id (`amane-mirror.<package>`), so each app is
//! a window of its own. `--list-apps --serial <serial> --device <id>`
//! writes the phone's apps for the launcher and exits. `--camera front|back`
//! makes the phone's camera this computer's webcam, without a window.

mod apps;
mod audio;
mod camera;
mod decode;
mod input;
mod protocol;
mod server;
mod session;

use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::Mutex;

use amane::{Center, Column, Key, Rectangle, Row, Service, Size, Text, Video, Widget, Window};
use amane_kit::app::{app, standard_shortcut};
use amane_kit::buttons::IconButton;
use amane_kit::color::with_alpha;
use amane_kit::layout::boxed;
use amane_kit::style;
use amane_kit::tokens::{Styled, sys};
use amane_phone::{Adb, KdeConnect};

use crate::session::{Mirror, Phase};
use amane_kit::args::flag;
use amane_kit::sync::Locked as _;

/// The bar under the screen.
const BAR_HEIGHT: f32 = 44.0;

const ICON_BACK: char = '\u{e5c4}';
const ICON_HOME: char = '\u{e88a}';
const ICON_RECENTS: char = '\u{e3c6}';
const ICON_VOLUME_DOWN: char = '\u{e04d}';
const ICON_VOLUME_UP: char = '\u{e050}';
const ICON_ROTATE: char = '\u{e1c1}';
const ICON_NOTIFICATIONS: char = '\u{e7f4}';
const ICON_SCREEN_OFF: char = '\u{e201}';

/// Which phone, from the launch.
#[derive(Default)]
struct Target {
    device: Option<String>,
    serial: Option<String>,
    /// One app on a virtual display, by package, and its name.
    app: Option<(String, Option<String>)>,
}

static TARGET: Mutex<Target> = Mutex::new(Target {
    device: None,
    serial: None,
    app: None,
});

/// The virtual display's density: twice Android's baseline, so an app laid
/// out at the window's logical size has twice its pixels, sharp on a HiDPI
/// screen.
const APP_DPI: u32 = 320;

thread_local! {
    // the virtual display's size last asked for, in its pixels
    static DISPLAY_SIZE: std::cell::Cell<(u16, u16)> = const { std::cell::Cell::new((0, 0)) };
}

fn app_target() -> Option<(String, Option<String>)> {
    TARGET.locked().app.clone()
}

// the addresses already handed to adb to stay connected to
static WANTED: Mutex<Option<HashSet<IpAddr>>> = Mutex::new(None);

fn follow(args: &[String]) {
    let mut target = TARGET.locked();
    if let Some(device) = flag(args, "--device") {
        target.device = Some(device);
    }
    if let Some(serial) = flag(args, "--serial") {
        target.serial = Some(serial);
    }
    if let Some(app) = flag(args, "--app") {
        target.app = Some((app, flag(args, "--name")));
    }
}

/// What the session needs from KDE Connect: the phone's addresses, and
/// whether it already shares the clipboard.
struct Phone {
    addresses: Vec<IpAddr>,
    shares_clipboard: bool,
}

fn phone() -> Option<Phone> {
    let asked = TARGET.locked().device.clone();
    let kdeconnect = KdeConnect::read();
    let device = match asked {
        Some(id) => kdeconnect.device(&id),
        None => kdeconnect.primary(),
    }?;
    Some(Phone {
        addresses: device.addresses().to_vec(),
        shares_clipboard: device.clipboard_auto_sync() == Some(true),
    })
}

/// The adb device to mirror: the one named, else KDE Connect's phone by
/// its address, else the only phone on USB.
fn serial(phone: Option<&Phone>) -> Option<String> {
    if let Some(serial) = TARGET.locked().serial.clone() {
        return Some(serial);
    }

    let adb = Adb::read();

    if let Some(device) = phone
        .into_iter()
        .flat_map(|phone| &phone.addresses)
        .find_map(|address| adb.ready_at(*address))
    {
        return Some(device.serial.clone());
    }

    let mut usb = adb.ready_usb();
    match (usb.next(), usb.next()) {
        (Some(only), None) => Some(only.serial.clone()),
        _ => None,
    }
}

// asks adb to stay connected to the phone's addresses, once each
fn want(phone: &Phone) {
    let mut wanted = WANTED.locked();
    let wanted = wanted.get_or_insert_with(HashSet::new);
    for address in &phone.addresses {
        if wanted.insert(*address) {
            let address = *address;
            // a write to Adb, which the view is reading
            std::thread::spawn(move || Adb::want(address));
        }
    }
}

fn main() {
    amane_log::init(amane_kit::app_ids::MIRROR);
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|arg| arg == "--list-apps") {
        std::process::exit(match apps::write(&args) {
            Ok(()) => 0,
            Err(why) => {
                log::warn!("{why}");
                1
            }
        });
    }

    if let Some(facing) = flag(&args, "--camera") {
        std::process::exit(match webcam(&args, &facing) {
            Ok(()) => 0,
            Err(why) => {
                log::warn!("camera: {why}");
                1
            }
        });
    }

    follow(&args);

    // each phone app is a window, and an app, of its own
    let id = match app_target() {
        Some((package, _)) => amane_kit::app_ids::mirror_app(&package),
        None => String::from(amane_kit::app_ids::MIRROR),
    };

    app(&id)
        .normal_window("main", main_window)
        .on_activate(follow)
        .run();
}

fn main_window() -> Window {
    let phone = phone();
    if let Some(phone) = &phone {
        want(phone);
    }

    let mirror = Mirror::read();
    let phase = mirror.phase.clone();
    let name = mirror.name.clone();
    let screen_off = mirror.screen_off;
    drop(mirror);

    if phase == Phase::Idle
        && !session::running()
        && let Some(serial) = serial(phone.as_ref())
    {
        let mut options = server::Options::screen(&serial);
        // KDE Connect already shares it; both would echo each change back
        options.clipboard_autosync = !phone.as_ref().is_some_and(|phone| phone.shares_clipboard);
        if let Some((package, _)) = app_target() {
            let (width, height) = display_size();
            options.new_display = Some(format!("{width}x{height}/{APP_DPI}"));
            options.start_app = Some(package);
            // the phone's sound comes with its screen's window, not each app's
            options.audio = false;
            DISPLAY_SIZE.set((width, height));
        }
        session::start(options);
    }

    let (width, height) = amane::window_size();
    let area = input::Area {
        width,
        height: (height - BAR_HEIGHT).max(1.0),
    };

    // an app's display follows its window
    if app_target().is_some() && phase == Phase::Live {
        let wanted = display_size();
        if wanted != DISPLAY_SIZE.get() && wanted.0 > 0 && wanted.1 > 0 {
            DISPLAY_SIZE.set(wanted);
            session::send(protocol::Control::ResizeDisplay {
                width: wanted.0,
                height: wanted.1,
            });
        }
    }

    let screen: Box<dyn Widget> = match &phase {
        Phase::Live => boxed(Video::new(&session::FEED).radius(sys::radius::raised())),
        Phase::Idle => boxed(status(
            "Waiting for the phone",
            "Turn on Wireless debugging on the phone, or plug it in with USB debugging on.",
            false,
        )),
        Phase::Starting => boxed(status("Connecting…", "", false)),
        Phase::Ended(None) => boxed(status("The phone stopped sharing its screen.", "", true)),
        Phase::Ended(Some(why)) => boxed(status("The screen couldn't be shown.", why, true)),
    };

    let video = Rectangle::new()
        .width(Size::Parent)
        .height(area.height)
        .child(
            Column::new(vec![screen])
                .width(Size::Parent)
                .height(Size::Parent),
        )
        .on_drag(move |point| input::drag(area, point))
        .on_release(move |point| input::release(area, point))
        .on_click(input::click)
        .on_scroll(move |scroll| input::scroll(area, scroll))
        .on_track(input::track)
        .on_drop(push_files);

    let bar = if app_target().is_some() {
        app_bar()
    } else {
        bar(screen_off)
    };

    let layout = Column::new(vec![boxed(video), boxed(bar)])
        .width(Size::Parent)
        .height(Size::Parent);

    let title = match app_target() {
        Some((package, label)) => label.unwrap_or(package),
        None if name.is_empty() => "Phone Screen".to_string(),
        None => name,
    };
    let (initial_width, initial_height) = if app_target().is_some() {
        (480.0, 800.0)
    } else {
        (420.0, 860.0)
    };

    Window::new()
        .child(
            Rectangle::new()
                .width(Size::Parent)
                .height(Size::Parent)
                .fill(amane::Color::rgb(0x10, 0x10, 0x12))
                .child(layout),
        )
        .title(&title)
        .size(initial_width, initial_height)
        .min_size(280.0, 480.0)
        .on_shortcut(standard_shortcut)
        .on_key(|key: Key| input::key(key))
}

/// `--camera front|back`: the phone's camera into the "Phone Camera"
/// device until the phone stops it. The phone comes from `--serial`, or
/// is waited for: KDE Connect's phone at its address, or the one on USB.
fn webcam(args: &[String], facing: &str) -> Result<(), String> {
    let facing = match facing {
        "front" => server::Facing::Front,
        "back" => server::Facing::Back,
        other => return Err(format!("--camera takes front or back, not {other}")),
    };

    follow(args);

    // the services fill in on their own threads; give them a moment to find the phone
    for _ in 0..50 {
        let phone = phone();
        if let Some(phone) = &phone {
            want(phone);
        }
        if let Some(serial) = serial(phone.as_ref()) {
            return camera::run(&serial, facing);
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    Err("adb doesn't have the phone.".into())
}

/// Where the screen would be: what's happening, and a Retry once it's over.
fn status(title: &str, detail: &str, retry: bool) -> Rectangle {
    let mut lines: Vec<Box<dyn Widget>> = vec![boxed(
        Text::new(title)
            .style(style::page_title_font())
            .color(amane::Color::WHITE),
    )];
    if !detail.is_empty() {
        lines.push(boxed(
            Rectangle::new()
                .width(Size::Fixed(300.0))
                .height(Size::Parent)
                .child(
                    Text::new(detail)
                        .style(sys::r#type::label())
                        .color(with_alpha(amane::Color::WHITE, 0.6))
                        .wrap(),
                ),
        ));
    }
    if retry {
        lines.push(boxed(
            amane_kit::buttons::PushButton::new("amane.mirror.retry", "Try Again")
                .on_press(|| Mirror::write().phase = Phase::Idle)
                .build(),
        ));
    }

    Rectangle::new()
        .width(Size::Parent)
        .height(Size::Parent)
        .align_child(Center, Center)
        .child(
            Column::new(lines)
                .gap(sys::space::gap::m())
                .justify(Center)
                .align(Center)
                .width(Size::Parent)
                .height(Size::Parent),
        )
}

/// The virtual display's size for the window's video area, in its pixels:
/// twice the logical size, even, as encoders want.
fn display_size() -> (u16, u16) {
    let (width, height) = amane::window_size();
    let (width, height) = if width <= 0.0 {
        (480.0, 800.0)
    } else {
        (width, height)
    };
    let even = |value: f32| ((value * 2.0) as u16) & !1;
    (even(width), even((height - BAR_HEIGHT).max(100.0)))
}

/// An app's bar: Back, the one key an app needs that a click can't give.
fn app_bar() -> Rectangle {
    Rectangle::new()
        .width(Size::Parent)
        .height(BAR_HEIGHT)
        .align_child(Center, Center)
        .child(
            IconButton::new("amane.mirror.back", ICON_BACK)
                .on_press(input::back)
                .tooltip("main", "Back")
                .build(),
        )
}

fn bar(screen_off: bool) -> Rectangle {
    let button = |id: &str, glyph: char, tip: &'static str, press: fn()| {
        boxed(
            IconButton::new(format!("amane.mirror.{id}"), glyph)
                .on_press(press)
                .tooltip("main", tip)
                .build(),
        )
    };

    let left = Row::new(vec![
        button("volume-down", ICON_VOLUME_DOWN, "Volume Down", || {
            input::press(protocol::KEYCODE_VOLUME_DOWN, 0)
        }),
        button("volume-up", ICON_VOLUME_UP, "Volume Up", || {
            input::press(protocol::KEYCODE_VOLUME_UP, 0)
        }),
        button("rotate", ICON_ROTATE, "Rotate", || {
            session::send(protocol::Control::Rotate)
        }),
    ]);

    let middle = Row::new(vec![
        button("back", ICON_BACK, "Back", input::back),
        button("home", ICON_HOME, "Home", input::home),
        button("recents", ICON_RECENTS, "Recent Apps", input::recents),
    ])
    .gap(sys::space::gap::l());

    let screen = IconButton::new("amane.mirror.screen-off", ICON_SCREEN_OFF)
        .selected(screen_off)
        .on_press(move || {
            let off = !Mirror::read().screen_off;
            session::send(protocol::Control::DisplayPower { on: !off });
            Mirror::write().screen_off = off;
        })
        .tooltip("main", "Phone's Screen Off")
        .build();

    let right = Row::new(vec![
        button("notifications", ICON_NOTIFICATIONS, "Notifications", || {
            session::send(protocol::Control::ExpandNotifications)
        }),
        boxed(screen),
    ]);

    Rectangle::new()
        .width(Size::Parent)
        .height(BAR_HEIGHT)
        .padding(amane_kit::layout::pad_x(
            sys::space::inset::s(),
            sys::space::inset::s(),
        ))
        .child(
            Row::new(vec![boxed(left), boxed(middle), boxed(right)])
                .justify(amane::SpaceBetween)
                .align(Center)
                .width(Size::Parent)
                .height(Size::Parent),
        )
}

/// Files dropped on the screen: APKs installed, the rest into Downloads.
fn push_files(paths: Vec<std::path::PathBuf>) {
    let Some(serial) = serial(phone().as_ref()) else {
        return;
    };
    std::thread::spawn(move || {
        for path in paths {
            let local = path.to_string_lossy().to_string();
            let is_apk = path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("apk"));
            if is_apk {
                if let Err(why) = amane_phone::adb(Some(&serial), &["install", "-r", &local]) {
                    log::warn!("install {local}: {why}");
                }
                continue;
            }
            let Some(name) = path.file_name() else {
                continue;
            };
            let remote = format!("/sdcard/Download/{}", name.to_string_lossy());
            match amane_phone::adb(Some(&serial), &["push", &local, &remote]) {
                Ok(_) => session::send(protocol::Control::ScanFile(remote)),
                Err(why) => log::warn!("push {local}: {why}"),
            }
        }
    });
}
