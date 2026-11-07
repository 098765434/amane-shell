/*
 * this computer as the phone's trackpad and keyboard; the other way, the
 * phone driving this computer, kdeconnect does by itself through the
 * compositor and has nothing to draw
 */

use std::collections::BTreeMap;

use amane::Argument;

use super::KdeConnect;
use super::actions::input_call;

const REMOTE_CONTROL: &str = "org.kde.kdeconnect.device.remotecontrol";
const REMOTE_KEYBOARD: &str = "org.kde.kdeconnect.device.remotekeyboard";

/// A click on the phone's screen, where its pointer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhoneClick {
    /// A single tap.
    Left,
    /// A double tap.
    Double,
    /// A middle click.
    Middle,
    /// A right click, a long press on most phones.
    Right,
    /// A press held for dragging, until Release.
    Hold,
    /// Lets go of a Hold.
    Release,
}

impl PhoneClick {
    fn command(self) -> &'static str {
        match self {
            Self::Left => "singleclick",
            Self::Double => "doubleclick",
            Self::Middle => "middleclick",
            Self::Right => "rightclick",
            Self::Hold => "singlehold",
            Self::Release => "singlerelease",
        }
    }
}

/// The keys that aren't text, which the phone's keyboard plugin knows by qt's numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhoneKey {
    /// Deletes before the cursor.
    Backspace,
    /// Tab.
    Tab,
    /// Enter, or the field's send or go action.
    Enter,
    /// Escape, often going back.
    Escape,
    /// Deletes after the cursor.
    Delete,
    /// The left arrow.
    Left,
    /// The right arrow.
    Right,
    /// The up arrow.
    Up,
    /// The down arrow.
    Down,
    /// To the start of the line.
    Home,
    /// To the end of the line.
    End,
    /// A page up.
    PageUp,
    /// A page down.
    PageDown,
}

impl PhoneKey {
    fn qt(self) -> i32 {
        match self {
            Self::Escape => 0x0100_0000,
            Self::Tab => 0x0100_0001,
            Self::Backspace => 0x0100_0003,
            Self::Enter => 0x0100_0004,
            Self::Delete => 0x0100_0007,
            Self::Home => 0x0100_0010,
            Self::End => 0x0100_0011,
            Self::Left => 0x0100_0012,
            Self::Up => 0x0100_0013,
            Self::Right => 0x0100_0014,
            Self::Down => 0x0100_0015,
            Self::PageUp => 0x0100_0016,
            Self::PageDown => 0x0100_0017,
        }
    }
}

/// Held keys, as qt's modifier bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PhoneModifiers {
    /// Shift is held.
    pub shift: bool,
    /// Ctrl is held.
    pub ctrl: bool,
    /// Alt is held.
    pub alt: bool,
}

impl PhoneModifiers {
    fn qt(self) -> i32 {
        let mut bits = 0;

        if self.shift {
            bits |= 0x0200_0000;
        }

        if self.ctrl {
            bits |= 0x0400_0000;
        }

        if self.alt {
            bits |= 0x0800_0000;
        }

        bits
    }
}

impl KdeConnect {
    /// Moves the phone's pointer by this much, in the phone's pixels.
    pub fn move_pointer(device: &str, dx: i32, dy: i32) {
        if dx == 0 && dy == 0 {
            return;
        }

        // a QPoint, which d-bus carries as (ii)
        let point = Argument::Struct(vec![Argument::from(dx), Argument::from(dy)]);

        input_call(
            device,
            "/remotecontrol",
            REMOTE_CONTROL,
            "moveCursor",
            vec![point],
        );
    }

    /// Clicks where the phone's pointer is.
    pub fn click(device: &str, click: PhoneClick) {
        let command = BTreeMap::from([(click.command().to_string(), Argument::from(true))]);

        send_command(device, command);
    }

    /// Scrolls the phone by this much, in the remote control plugin's own steps;
    /// nothing is sent for zero.
    pub fn scroll(device: &str, dx: i32, dy: i32) {
        if dx == 0 && dy == 0 {
            return;
        }

        let command = BTreeMap::from([
            ("scroll".to_string(), Argument::from(true)),
            ("dx".to_string(), Argument::from(dx)),
            ("dy".to_string(), Argument::from(dy)),
        ]);

        send_command(device, command);
    }

    /// Typed text, into the field focused on the phone.
    pub fn type_text(device: &str, text: &str, modifiers: PhoneModifiers) {
        if text.is_empty() {
            return;
        }

        send_key_event(device, 0, text, modifiers);
    }

    /// Presses a key that isn't text, with these keys held, in the field focused on the phone.
    pub fn press_key(device: &str, key: PhoneKey, modifiers: PhoneModifiers) {
        send_key_event(device, key.qt(), "", modifiers);
    }
}

fn send_command(device: &str, command: BTreeMap<String, Argument>) {
    input_call(
        device,
        "/remotecontrol",
        REMOTE_CONTROL,
        "sendCommand",
        vec![Argument::Map(command)],
    );
}

// a serialized key event, as the phone's keyboard plugin reads it
fn send_key_event(device: &str, key: i32, text: &str, modifiers: PhoneModifiers) {
    let event = BTreeMap::from([
        ("key".to_string(), Argument::from(key)),
        ("modifiers".to_string(), Argument::from(modifiers.qt())),
        ("text".to_string(), Argument::from(text)),
    ]);

    input_call(
        device,
        "/remotekeyboard",
        REMOTE_KEYBOARD,
        "sendQKeyEvent",
        vec![Argument::Map(event)],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_are_qts_bits() {
        let all = PhoneModifiers {
            shift: true,
            ctrl: true,
            alt: true,
        };

        assert_eq!(all.qt(), 0x0E00_0000);
        assert_eq!(PhoneModifiers::default().qt(), 0);
        assert_eq!(PhoneKey::Backspace.qt(), 0x0100_0003);
    }
}
