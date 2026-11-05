//! This computer's pointer and keyboard as the phone's: a left press and
//! drag is a finger on the screen, the right button is Back and the
//! middle one Home, a scroll scrolls where the pointer is, and keys type
//! (text as text, other keys as Android's keycodes).

use std::cell::Cell;

use amane::{Button, Key, Point, Scroll, Video};

use crate::protocol::{
    ACTION_DOWN, ACTION_MOVE, ACTION_UP, Control, KEYCODE_APP_SWITCH, KEYCODE_HOME, META_ALT,
    META_CTRL, META_SHIFT, POINTER_FINGER, Position,
};
use amane::Service;

use crate::session::{self, Mirror};

thread_local! {
    // a finger is down on the phone
    static PRESSED: Cell<bool> = const { Cell::new(false) };

    // where the pointer last was over the video, for scrolling there
    static POINTER: Cell<Point> = const { Cell::new(Point { x: 0.0, y: 0.0 }) };
}

/// The pointer moved over the video.
pub fn track(point: Point) {
    POINTER.set(point);
}

/// The area the video is drawn in, so a point maps onto the frame.
#[derive(Debug, Clone, Copy)]
pub struct Area {
    pub width: f32,
    pub height: f32,
}

fn position(area: Area, point: Point) -> Option<Position> {
    let (width, height) = Mirror::read().size?;
    let on_frame = Video::frame_point(area.width, area.height, (width, height), point)?;

    Some(Position {
        x: on_frame.x as u32,
        y: on_frame.y as u32,
        width: width as u16,
        height: height as u16,
    })
}

// a point clamped onto the frame, for a drag that slides off its edge
fn clamped(area: Area, point: Point) -> Option<Position> {
    let (width, height) = Mirror::read().size?;

    // the frame's scale in the area, as Video contains it
    let scale = (area.width / width as f32).min(area.height / height as f32);
    let left = (area.width - width as f32 * scale) / 2.0;
    let top = (area.height - height as f32 * scale) / 2.0;

    let x = ((point.x - left) / scale).clamp(0.0, width as f32 - 1.0);
    let y = ((point.y - top) / scale).clamp(0.0, height as f32 - 1.0);

    Some(Position {
        x: x as u32,
        y: y as u32,
        width: width as u16,
        height: height as u16,
    })
}

fn touch(action: u8, position: Position) {
    session::send(Control::Touch {
        action,
        pointer: POINTER_FINGER,
        position,
        pressure: if action == ACTION_UP { 0.0 } else { 1.0 },
        action_button: 0,
        buttons: 0,
    });
}

/// The left button pressed, or moved while pressed.
pub fn drag(area: Area, point: Point) {
    if PRESSED.get() {
        if let Some(position) = clamped(area, point) {
            touch(ACTION_MOVE, position);
        }
        return;
    }

    // a press on the bars beside the picture touches nothing
    if let Some(position) = position(area, point) {
        PRESSED.set(true);
        touch(ACTION_DOWN, position);
    }
}

pub fn release(area: Area, point: Point) {
    if !PRESSED.replace(false) {
        return;
    }
    if let Some(position) = clamped(area, point) {
        touch(ACTION_UP, position);
    }
}

pub fn click(button: Button) {
    match button {
        Button::Right => back(),
        Button::Middle => press(KEYCODE_HOME, 0),
        _ => {}
    }
}

pub fn scroll(area: Area, scroll: Scroll) {
    let Some(position) = position(area, POINTER.get()) else {
        return;
    };

    // a wheel's notch is a notch; fingers move in fractions of one
    session::send(Control::Scroll {
        position,
        horizontal: -scroll.x,
        vertical: -scroll.y,
        buttons: 0,
    });
}

/// Back, which also wakes the phone's screen.
pub fn back() {
    session::send(Control::BackOrScreenOn {
        action: ACTION_DOWN,
    });
    session::send(Control::BackOrScreenOn { action: ACTION_UP });
}

pub fn home() {
    press(KEYCODE_HOME, 0);
}

pub fn recents() {
    press(KEYCODE_APP_SWITCH, 0);
}

/// A key pressed and let go on the phone.
pub fn press(keycode: u32, meta: u32) {
    for action in [ACTION_DOWN, ACTION_UP] {
        session::send(Control::Key {
            action,
            keycode,
            repeat: 0,
            meta,
        });
    }
}

/// A key typed in the window.
pub fn key(key: Key) {
    let held = amane::modifiers();

    // the logo key is this computer's
    if held.logo {
        return;
    }

    let mut meta = 0;
    if held.shift {
        meta |= META_SHIFT;
    }
    if held.alt {
        meta |= META_ALT;
    }
    if held.ctrl {
        meta |= META_CTRL;
    }

    match key {
        // Ctrl+V pastes this computer's clipboard, as scrcpy does
        Key::Character('v' | 'V') if held.ctrl => session::paste(),

        // text is typed as text, so every layout and accent arrives as it was meant
        Key::Character(c) if !held.ctrl && !held.alt => session::send(Control::Text(c.to_string())),
        Key::Space if !held.ctrl && !held.alt => session::send(Control::Text(" ".into())),

        // with Ctrl or Alt it's a shortcut on the phone, like Ctrl+A
        Key::Character(c) => {
            if let Some(keycode) = character_keycode(c) {
                press(keycode, meta);
            }
        }

        other => {
            if let Some(keycode) = named_keycode(other) {
                press(keycode, meta);
            }
        }
    }
}

// Android's keycodes for letters and digits
fn character_keycode(c: char) -> Option<u32> {
    let c = c.to_ascii_lowercase();
    match c {
        'a'..='z' => Some(29 + (c as u32 - 'a' as u32)),
        '0'..='9' => Some(7 + (c as u32 - '0' as u32)),
        _ => None,
    }
}

fn named_keycode(key: Key) -> Option<u32> {
    Some(match key {
        Key::Enter => 66,
        Key::Escape => 111,
        Key::Tab => 61,
        Key::Backspace => 67,
        Key::Delete => 112,
        Key::Space => 62,
        Key::Up => 19,
        Key::Down => 20,
        Key::Left => 21,
        Key::Right => 22,
        Key::Home => 122,
        Key::End => 123,
        Key::PageUp => 92,
        Key::PageDown => 93,
        Key::Insert => 124,
        Key::F(n @ 1..=12) => 130 + u32::from(n),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_and_digits_have_androids_keycodes() {
        assert_eq!(character_keycode('a'), Some(29));
        assert_eq!(character_keycode('Z'), Some(54));
        assert_eq!(character_keycode('0'), Some(7));
        assert_eq!(character_keycode('9'), Some(16));
        assert_eq!(character_keycode('é'), None);
    }

    #[test]
    fn named_keys_have_androids_keycodes() {
        assert_eq!(named_keycode(Key::Enter), Some(66));
        assert_eq!(named_keycode(Key::Backspace), Some(67));
        assert_eq!(named_keycode(Key::F(1)), Some(131));
        assert_eq!(named_keycode(Key::F(12)), Some(142));
        assert_eq!(named_keycode(Key::F(13)), None);
    }
}
