//! The on-screen display: a small glass capsule above the bottom edge that
//! shows a level or a switch as it changes (volume, the microphone muted,
//! brightness, the keyboard light, a lock key, airplane mode, the power
//! mode, the sound moving to another device), then fades out, as macOS and
//! GNOME show the media keys.
//!
//! The keys come to the shell over IPC ([`keys`]), which changes the level
//! and shows it at once; changes made anywhere else (a script, a headset's
//! buttons, the keyboard light's own key) show too, noticed by the services
//! they change ([`watch`]). See `docs/shell/osd.md`.
//!
//! One surface, placed by niri on the focused output, as big as the
//! capsule and taking no input. Hidden, it's `visible(false)` and holds
//! nothing on the GPU; it draws only rectangles, text and glyphs.

mod keys;
mod watch;

use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use amane::{
    AudioKind, Center, Horizontal, Keyboard, LayerWindow, Margin, Rectangle, Row, Size, Start,
    Text, Vertical, Widget,
};

use crate::components::glass_region;
use crate::components::icons::{
    ICON_BRIGHTNESS_HIGH, ICON_BRIGHTNESS_LOW, ICON_KEYBOARD_LIGHT, ICON_MIC, ICON_MIC_OFF,
    volume_glyph,
};
use crate::components::layout::{blank, boxed, icon, pad, slot};
use crate::components::material::{self, FillLevel, Level, Material};
use crate::components::motion;
use crate::components::panel::overlay;
use crate::components::slider::Slider;
use crate::tokens::{Styled, osd, sys};

pub use keys::ipc;

amane_kit::marker! {
    /// The display showed, changed or ran out: only its window draws again.
    pub Osd
}

/// Layer-surface namespace; the niri `layer-rule` matches it.
pub const OSD_NS: &str = "amane.osd";

/// How long a reading stays before it fades.
const LINGER: Duration = Duration::from_millis(1500);

/// A device's name is longer to read than a level.
const DEVICE_LINGER: Duration = Duration::from_millis(2200);

/// Keyboard lights with this many steps or fewer show them as pips.
const MAX_PIPS: u32 = 4;

/// What the display shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Readout {
    /// The output's level, 0 … 100.
    Volume {
        level: u8,
        muted: bool,
        kind: AudioKind,
    },
    /// The microphone's level and whether it's muted.
    Mic { level: u8, muted: bool },
    /// The screen's brightness, 0 … 100.
    Brightness(u8),
    /// The keyboard light, in its steps.
    KeyboardLight { level: u32, max: u32 },
    /// Something switched or picked: a glyph and what it now is.
    Label { glyph: char, text: String },
}

impl Readout {
    fn linger(&self) -> Duration {
        match self {
            Self::Label { .. } => DEVICE_LINGER,
            _ => LINGER,
        }
    }
}

thread_local! {
    // what shows, kept while it fades out
    static SHOWN: RefCell<Option<Readout>> = const { RefCell::new(None) };
    // when it starts to fade
    static UNTIL: Cell<Option<Instant>> = const { Cell::new(None) };

    // how far it has faded in
    static FADE: RefCell<amane::Animation> = RefCell::new(motion::fade(0.0, motion::FADE));
    static OPENING: Cell<bool> = const { Cell::new(false) };
}

/// Shows `readout`, in place of whatever shows, for its linger from now.
/// Showing again while it's up changes the capsule in place.
pub fn show(readout: Readout) {
    let linger = readout.linger();
    SHOWN.set(Some(readout));
    UNTIL.set(Some(Instant::now() + linger));
    motion::redraw_after::<Osd>(linger);
    Osd::changed();
}

/// Steps the fade towards `open` and gives how far in it is.
fn faded_in(open: bool) -> f32 {
    FADE.with_borrow_mut(|anim| {
        if OPENING.replace(open) != open {
            anim.retime(if open { motion::FADE } else { motion::EXIT });
            anim.to(if open { 1.0 } else { 0.0 });
        }
        anim.value()
    })
}

/// The surface: hidden while nothing shows, else the capsule.
pub fn surface() -> LayerWindow {
    // reading the services here wakes this view when one changes
    watch::changes();
    Osd::watch();

    let open = UNTIL.get().is_some_and(|until| Instant::now() < until);
    let shown = faded_in(open);

    let readout = SHOWN.with_borrow(Clone::clone);
    let Some(readout) = readout.filter(|_| open || shown > 0.001) else {
        if !open {
            SHOWN.set(None);
            UNTIL.set(None);
        }
        return window()
            .visible(false)
            .blur_shapes(Vec::new())
            .child(blank());
    };

    let (width, height, radius) = (osd::width(), osd::height(), osd::radius());

    // a compositor that can't fade frost only shows or hides it, so there
    // the capsule arrives whole rather than fading in over frost at full strength
    let fades = amane::blur_fades();
    let opacity = if open && !fades {
        1.0
    } else {
        shown.clamp(0.0, 1.0)
    };

    // small glass, light or dark with its backdrop
    let glass = glass_region::shape_id(OSD_NS);
    let capsule = Material::Control
        .adapted(glass, || {
            let face = Rectangle::new()
                .width(width)
                .height(height)
                .radius(radius)
                .fill(Material::Control.fill())
                .padding(pad(osd::pad(), 0.0))
                .align_child(Start, Center)
                .child(content(&readout, width - osd::pad() * 2.0));
            Material::Control.lit(face, radius)
        })
        .opacity(opacity);

    let frost = if open || fades {
        let shape = Material::Control.adapted(glass, || {
            Material::Control.shape(0.0, 0.0, width, height, radius, opacity)
        });
        vec![shape.id(glass)]
    } else {
        Vec::new()
    };

    window().visible(true).blur_shapes(frost).child(capsule)
}

/// What's on the capsule, `width` wide: a glyph, then a level or a label.
fn content(readout: &Readout, width: f32) -> Row {
    let gap = sys::space::gap::m();
    let side = sys::size::control::height();
    let rest = (width - side - gap).max(1.0);
    let label = material::label(Level::Primary);

    let (glyph, after): (char, Box<dyn Widget>) = match readout {
        Readout::Volume { level, muted, kind } => (
            volume_glyph(*muted, *level, *kind),
            boxed(meter(*level, *muted, rest)),
        ),
        Readout::Mic { level, muted } => (
            if *muted { ICON_MIC_OFF } else { ICON_MIC },
            boxed(meter(*level, *muted, rest)),
        ),
        Readout::Brightness(level) => (
            if *level < 50 {
                ICON_BRIGHTNESS_LOW
            } else {
                ICON_BRIGHTNESS_HIGH
            },
            boxed(meter(*level, false, rest)),
        ),
        Readout::KeyboardLight { level, max } if *max <= MAX_PIPS => {
            (ICON_KEYBOARD_LIGHT, boxed(pips(*level, *max, rest)))
        }
        Readout::KeyboardLight { level, max } => {
            let percent = (level * 100 / (*max).max(1)).min(100) as u8;
            (ICON_KEYBOARD_LIGHT, boxed(meter(percent, false, rest)))
        }
        Readout::Label { glyph, text } => (
            *glyph,
            boxed(
                Text::new(text.clone())
                    .style(osd::label())
                    .color(label)
                    .elide()
                    .max_lines(1),
            ),
        ),
    };

    let glyph = slot(side).child(icon(glyph, sys::size::icon::m(), label));
    Row::new(vec![boxed(glyph), after])
        .width(Size::Fixed(width))
        .gap(gap)
        .align(Center)
}

/// A level as Quick Settings' thin track draws it, greyed while muted.
fn meter(level: u8, muted: bool, width: f32) -> Rectangle {
    Slider::new(level, width)
        .inert(true)
        .dimmed(muted)
        .fill(material::label(Level::Primary))
        .thickness(osd::track())
        .build()
}

/// A light with few steps, one pip a step, the lit ones filled.
fn pips(level: u32, max: u32, width: f32) -> Row {
    let gap = sys::space::gap::s();
    let track = osd::track();
    let each = ((width - gap * (max - 1) as f32) / max as f32).max(1.0);
    let pips = (0..max)
        .map(|step| {
            let fill = if step < level {
                material::label(Level::Primary)
            } else {
                material::fill_role(FillLevel::Secondary)
            };
            boxed(
                Rectangle::new()
                    .width(Size::Fixed(each))
                    .height(Size::Fixed(track))
                    .radius(track / 2.0)
                    .fill(fill),
            )
        })
        .collect();
    Row::new(pips).gap(gap).align(Center)
}

/// The surface, as big as the capsule, `osd.offset` above the bottom edge
/// (clear of the bar there, which reserves its zone), in the middle, taking
/// neither the keyboard nor the pointer.
fn window() -> LayerWindow {
    let base = LayerWindow::new()
        .width(osd::width())
        .height(osd::height())
        .margin(Margin {
            top: 0,
            right: 0,
            bottom: osd::offset().round() as i32,
            left: 0,
        })
        .keyboard(Keyboard::None)
        .click_through();
    overlay(base, OSD_NS, Vertical::Bottom, Horizontal::Middle)
}
