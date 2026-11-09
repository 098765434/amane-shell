//! scrcpy's wire format, as scrcpy 4.1's client speaks it
//! (`app/src/control_msg.c`, `device_msg.c`, `demuxer.c`): every number
//! big-endian.

/// A media socket's first word: which codec follows. 0 means the phone
/// turned the stream off (no audio before Android 11), 1 that it failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    H264,
    H265,
    Av1,
    Opus,
    Aac,
    Flac,
    Raw,
    Disabled,
    Failed,
    Unknown(u32),
}

impl Codec {
    pub fn from_id(id: u32) -> Self {
        match id {
            0 => Self::Disabled,
            1 => Self::Failed,
            0x6832_3634 => Self::H264,
            0x6832_3635 => Self::H265,
            0x0061_7631 => Self::Av1,
            0x6f70_7573 => Self::Opus,
            0x0061_6163 => Self::Aac,
            0x666c_6163 => Self::Flac,
            0x0072_6177 => Self::Raw,
            other => Self::Unknown(other),
        }
    }
}

/// The 12 bytes before each packet on a media socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Header {
    /// A new capture session, after a rotation or a fold: the frame's size.
    Session { width: u32, height: u32 },
    /// A packet of `size` bytes. A config packet carries the codec's setup
    /// (H.264's SPS and PPS) and goes in front of the next one.
    Packet {
        size: u32,
        config: bool,
        key: bool,
        pts: u64,
    },
}

pub const HEADER_SIZE: usize = 12;

/// The biggest media packet read: a 4K key frame is a few MB. The size comes from the other
/// end, so a bigger one ends the stream rather than being allocated.
pub const MAX_PACKET: u32 = 16 * 1024 * 1024;

/// The biggest clipboard text taken from the phone.
pub const MAX_DEVICE_CLIPBOARD: usize = 1024 * 1024;

const CONFIG: u64 = 1 << 62;
const KEY_FRAME: u64 = 1 << 61;
const PTS: u64 = KEY_FRAME - 1;

pub fn header(bytes: &[u8; HEADER_SIZE]) -> Header {
    if bytes[0] & 0x80 != 0 {
        return Header::Session {
            width: be32(&bytes[4..8]),
            height: be32(&bytes[8..12]),
        };
    }

    let flags = u64::from_be_bytes(bytes[0..8].try_into().expect("eight bytes"));

    Header::Packet {
        size: be32(&bytes[8..12]),
        config: flags & CONFIG != 0,
        key: flags & KEY_FRAME != 0,
        pts: flags & PTS,
    }
}

fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes.try_into().expect("four bytes"))
}

/// The phone's name, the first thing on the first socket: 64 bytes, the
/// name's UTF-8 then zeros.
pub fn device_name(bytes: &[u8; 64]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

// Android's KeyEvent and MotionEvent numbers

pub const ACTION_DOWN: u8 = 0;
pub const ACTION_UP: u8 = 1;
pub const ACTION_MOVE: u8 = 2;

pub const KEYCODE_HOME: u32 = 3;
#[allow(dead_code, reason = "part of the protocol, not sent yet")]
pub const KEYCODE_BACK: u32 = 4;
pub const KEYCODE_VOLUME_UP: u32 = 24;
pub const KEYCODE_VOLUME_DOWN: u32 = 25;
#[allow(dead_code, reason = "part of the protocol, not sent yet")]
pub const KEYCODE_POWER: u32 = 26;
pub const KEYCODE_APP_SWITCH: u32 = 187;

pub const META_SHIFT: u32 = 0x1 | 0x40;
pub const META_ALT: u32 = 0x2 | 0x10;
pub const META_CTRL: u32 = 0x1000 | 0x2000;

/// A finger, as opposed to the mouse (-1) or a virtual one (-3).
pub const POINTER_FINGER: u64 = -2i64 as u64;

/// Where on the phone's frame, with the frame's size so the phone can
/// scale a point from an older session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub x: u32,
    pub y: u32,
    pub width: u16,
    pub height: u16,
}

impl Position {
    fn write(self, out: &mut Vec<u8>) {
        out.extend(self.x.to_be_bytes());
        out.extend(self.y.to_be_bytes());
        out.extend(self.width.to_be_bytes());
        out.extend(self.height.to_be_bytes());
    }
}

/// What the computer sends the phone, on the control socket.
#[derive(Debug, Clone, PartialEq)]
#[allow(
    dead_code,
    reason = "the whole control protocol; some messages aren't sent yet"
)]
pub enum Control {
    Key {
        action: u8,
        keycode: u32,
        repeat: u32,
        meta: u32,
    },
    /// Up to 300 bytes are typed; the rest is cut at a character.
    Text(String),
    Touch {
        action: u8,
        pointer: u64,
        position: Position,
        /// 0 to 1.
        pressure: f32,
        action_button: u32,
        buttons: u32,
    },
    /// Notches, -16 to 16 each way; positive scrolls up and left.
    Scroll {
        position: Position,
        horizontal: f32,
        vertical: f32,
        buttons: u32,
    },
    /// Back, or wakes the screen when it's off.
    BackOrScreenOn {
        action: u8,
    },
    ExpandNotifications,
    ExpandSettings,
    CollapsePanels,
    /// Sets the phone's clipboard, and pastes it when `paste`.
    SetClipboard {
        sequence: u64,
        text: String,
        paste: bool,
    },
    /// Turns the phone's own screen off or on while mirroring goes on.
    DisplayPower {
        on: bool,
    },
    Rotate,
    /// By package; `+` first force-stops it, `?` first finds it by name.
    StartApp(String),
    ResetVideo,
    CameraTorch {
        on: bool,
    },
    CameraZoomIn,
    CameraZoomOut,
    /// A virtual display's new size, as its window resizes.
    ResizeDisplay {
        width: u16,
        height: u16,
    },
    /// Tells the phone's media scanner about a pushed file.
    ScanFile(String),
}

const TEXT_MAX: usize = 300;
const CLIPBOARD_MAX: usize = (1 << 18) - 14;
const PATH_MAX: usize = 256;

impl Control {
    pub fn serialize(&self) -> Vec<u8> {
        let mut out = vec![self.kind()];

        match self {
            Self::Key {
                action,
                keycode,
                repeat,
                meta,
            } => {
                out.push(*action);
                out.extend(keycode.to_be_bytes());
                out.extend(repeat.to_be_bytes());
                out.extend(meta.to_be_bytes());
            }
            Self::Text(text) => string(&mut out, text, TEXT_MAX),
            Self::Touch {
                action,
                pointer,
                position,
                pressure,
                action_button,
                buttons,
            } => {
                out.push(*action);
                out.extend(pointer.to_be_bytes());
                position.write(&mut out);
                out.extend(unsigned_fixed(*pressure).to_be_bytes());
                out.extend(action_button.to_be_bytes());
                out.extend(buttons.to_be_bytes());
            }
            Self::Scroll {
                position,
                horizontal,
                vertical,
                buttons,
            } => {
                position.write(&mut out);
                out.extend(signed_fixed(horizontal / 16.0).to_be_bytes());
                out.extend(signed_fixed(vertical / 16.0).to_be_bytes());
                out.extend(buttons.to_be_bytes());
            }
            Self::BackOrScreenOn { action } => out.push(*action),
            Self::SetClipboard {
                sequence,
                text,
                paste,
            } => {
                out.extend(sequence.to_be_bytes());
                out.push(u8::from(*paste));
                string(&mut out, text, CLIPBOARD_MAX);
            }
            Self::DisplayPower { on } | Self::CameraTorch { on } => out.push(u8::from(*on)),
            Self::StartApp(name) => {
                let name = truncated(name, 255);
                out.push(name.len() as u8);
                out.extend(name.as_bytes());
            }
            Self::ResizeDisplay { width, height } => {
                out.extend(width.to_be_bytes());
                out.extend(height.to_be_bytes());
            }
            Self::ScanFile(path) => string(&mut out, path, PATH_MAX),
            Self::ExpandNotifications
            | Self::ExpandSettings
            | Self::CollapsePanels
            | Self::Rotate
            | Self::ResetVideo
            | Self::CameraZoomIn
            | Self::CameraZoomOut => {}
        }

        out
    }

    // the message's number, its place in scrcpy's enum
    fn kind(&self) -> u8 {
        match self {
            Self::Key { .. } => 0,
            Self::Text(_) => 1,
            Self::Touch { .. } => 2,
            Self::Scroll { .. } => 3,
            Self::BackOrScreenOn { .. } => 4,
            Self::ExpandNotifications => 5,
            Self::ExpandSettings => 6,
            Self::CollapsePanels => 7,
            // 8 asks for the phone's clipboard, which arrives by itself here
            Self::SetClipboard { .. } => 9,
            Self::DisplayPower { .. } => 10,
            Self::Rotate => 11,
            // 12 to 15: HID devices and the keyboard's settings
            Self::StartApp(_) => 16,
            Self::ResetVideo => 17,
            Self::CameraTorch { .. } => 18,
            Self::CameraZoomIn => 19,
            Self::CameraZoomOut => 20,
            Self::ResizeDisplay { .. } => 21,
            Self::ScanFile(_) => 22,
        }
    }
}

// a u32 length, then the UTF-8, cut at a character to fit `max`
fn string(out: &mut Vec<u8>, text: &str, max: usize) {
    let text = truncated(text, max);
    out.extend((text.len() as u32).to_be_bytes());
    out.extend(text.as_bytes());
}

fn truncated(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

// 0..1 as 16 bits, 1 being 0xffff
fn unsigned_fixed(value: f32) -> u16 {
    let scaled = (value.clamp(0.0, 1.0) * 65536.0) as u32;
    scaled.min(0xffff) as u16
}

// -1..1 as 16 bits
fn signed_fixed(value: f32) -> i16 {
    let scaled = (value.clamp(-1.0, 1.0) * 32768.0) as i32;
    scaled.clamp(-0x8000, 0x7fff) as i16
}

/// What the phone sends back on the control socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceMessage {
    /// The phone's clipboard changed.
    Clipboard(String),
    /// The phone set the clipboard that `SetClipboard` with this sequence sent.
    AckClipboard(u64),
    UhidOutput {
        id: u16,
        data: Vec<u8>,
    },
}

/// Reads one message from the front of `bytes`: it and how many bytes it
/// took, none while it isn't all there yet, an error for one scrcpy
/// doesn't send.
pub fn device_message(bytes: &[u8]) -> Result<Option<(DeviceMessage, usize)>, u8> {
    let Some(&kind) = bytes.first() else {
        return Ok(None);
    };

    match kind {
        0 => {
            if bytes.len() < 5 {
                return Ok(None);
            }
            let len = be32(&bytes[1..5]) as usize;
            // too big to keep reading towards: the stream can't be read in step any more
            if len > MAX_DEVICE_CLIPBOARD {
                return Err(kind);
            }
            if bytes.len() < 5 + len {
                return Ok(None);
            }
            let text = String::from_utf8_lossy(&bytes[5..5 + len]).into_owned();
            Ok(Some((DeviceMessage::Clipboard(text), 5 + len)))
        }
        1 => {
            if bytes.len() < 9 {
                return Ok(None);
            }
            let sequence = u64::from_be_bytes(bytes[1..9].try_into().expect("eight bytes"));
            Ok(Some((DeviceMessage::AckClipboard(sequence), 9)))
        }
        2 => {
            if bytes.len() < 5 {
                return Ok(None);
            }
            let id = u16::from_be_bytes([bytes[1], bytes[2]]);
            let size = u16::from_be_bytes([bytes[3], bytes[4]]) as usize;
            if bytes.len() < 5 + size {
                return Ok(None);
            }
            let data = bytes[5..5 + size].to_vec();
            Ok(Some((DeviceMessage::UhidOutput { id, data }, 5 + size)))
        }
        other => Err(other),
    }
}

/// One app from the server's `list_apps=true`, which it logs as
/// ` * Name<padding> package` (`*` a system app, `-` one installed), the
/// package on the next line when the name is 30 characters or longer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    pub name: String,
    pub package: String,
    pub system: bool,
}

pub fn apps(log: &str) -> Vec<App> {
    let mut apps = Vec::new();
    let mut waiting: Option<(String, bool)> = None;

    for line in log.lines() {
        // the server's log prefix, when the lines come straight from it
        let line = line
            .strip_prefix("[server] INFO: ")
            .unwrap_or(line)
            .trim_end();

        let marked = line
            .strip_prefix(" * ")
            .map(|rest| (rest, true))
            .or_else(|| line.strip_prefix(" - ").map(|rest| (rest, false)));

        if let Some((rest, system)) = marked {
            // the package is the last word, after the padding; a long name stands alone
            match rest.rsplit_once(char::is_whitespace) {
                Some((name, package)) if is_package(package) && name.ends_with(' ') => {
                    apps.push(App {
                        name: name.trim().to_string(),
                        package: package.to_string(),
                        system,
                    });
                    waiting = None;
                }
                _ => waiting = Some((rest.trim().to_string(), system)),
            }
            continue;
        }

        if let Some((name, system)) = waiting.take() {
            let package = line.trim();
            if is_package(package) {
                apps.push(App {
                    name,
                    package: package.to_string(),
                    system,
                });
            }
        }
    }

    apps
}

fn is_package(word: &str) -> bool {
    word.contains('.')
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clipboard_too_big_ends_the_stream() {
        let mut huge = vec![0u8];
        huge.extend(((MAX_DEVICE_CLIPBOARD + 1) as u32).to_be_bytes());
        assert_eq!(device_message(&huge).map(|_| ()), Err(0));

        let mut small = vec![0u8];
        small.extend(2u32.to_be_bytes());
        small.extend(b"hi");
        assert!(matches!(
            device_message(&small),
            Ok(Some((DeviceMessage::Clipboard(text), 7))) if text == "hi"
        ));
    }

    // the byte layouts from scrcpy's app/tests/test_control_msg_serialize.c

    #[test]
    fn serializes_a_keycode() {
        let message = Control::Key {
            action: ACTION_UP,
            keycode: 66, // ENTER
            repeat: 5,
            meta: 0x41, // shift on, shift left
        };
        assert_eq!(
            message.serialize(),
            [0, 1, 0, 0, 0, 66, 0, 0, 0, 5, 0, 0, 0, 0x41]
        );
    }

    #[test]
    fn serializes_text() {
        let mut expected = vec![1, 0, 0, 0, 5];
        expected.extend(b"hello");
        assert_eq!(Control::Text("hello".into()).serialize(), expected);
    }

    #[test]
    fn cuts_long_text_at_a_character() {
        let text = "é".repeat(200); // 400 bytes
        let bytes = Control::Text(text).serialize();
        assert_eq!(bytes.len(), 5 + 300);
        assert_eq!(&bytes[1..5], &300u32.to_be_bytes());
    }

    #[test]
    fn serializes_a_touch() {
        let message = Control::Touch {
            action: ACTION_DOWN,
            pointer: 0x1234_5678_8765_4321,
            position: Position {
                x: 100,
                y: 200,
                width: 1080,
                height: 1920,
            },
            pressure: 1.0,
            action_button: 1, // primary
            buttons: 1,
        };
        assert_eq!(
            message.serialize(),
            [
                2, 0, 0x12, 0x34, 0x56, 0x78, 0x87, 0x65, 0x43, 0x21, 0, 0, 0, 100, 0, 0, 0, 200,
                0x04, 0x38, 0x07, 0x80, 0xff, 0xff, 0, 0, 0, 1, 0, 0, 0, 1,
            ]
        );
    }

    #[test]
    fn serializes_a_scroll() {
        let message = Control::Scroll {
            position: Position {
                x: 260,
                y: 1026,
                width: 1080,
                height: 1920,
            },
            horizontal: 16.0,
            vertical: -16.0,
            buttons: 1,
        };
        assert_eq!(
            message.serialize(),
            [
                3, 0, 0, 0x01, 0x04, 0, 0, 0x04, 0x02, 0x04, 0x38, 0x07, 0x80, 0x7f, 0xff, 0x80,
                0x00, 0, 0, 0, 1,
            ]
        );
    }

    #[test]
    fn serializes_the_clipboard() {
        let message = Control::SetClipboard {
            sequence: 0x0102_0304_0506_0708,
            text: "hello, world!".into(),
            paste: true,
        };
        let mut expected = vec![9, 1, 2, 3, 4, 5, 6, 7, 8, 1, 0, 0, 0, 13];
        expected.extend(b"hello, world!");
        assert_eq!(message.serialize(), expected);
    }

    #[test]
    fn serializes_the_short_ones() {
        assert_eq!(Control::BackOrScreenOn { action: 1 }.serialize(), [4, 1]);
        assert_eq!(Control::ExpandNotifications.serialize(), [5]);
        assert_eq!(Control::ExpandSettings.serialize(), [6]);
        assert_eq!(Control::CollapsePanels.serialize(), [7]);
        assert_eq!(Control::DisplayPower { on: true }.serialize(), [10, 1]);
        assert_eq!(Control::Rotate.serialize(), [11]);
        assert_eq!(Control::ResetVideo.serialize(), [17]);
        assert_eq!(
            Control::ResizeDisplay {
                width: 0x0438,
                height: 0x0780
            }
            .serialize(),
            [21, 0x04, 0x38, 0x07, 0x80]
        );
    }

    #[test]
    fn serializes_start_app() {
        let mut expected = vec![16, 15];
        expected.extend(b"firefox.example");
        assert_eq!(
            Control::StartApp("firefox.example".into()).serialize(),
            expected
        );
    }

    // and from app/tests/test_device_msg_deserialize.c

    #[test]
    fn reads_the_phones_clipboard() {
        let bytes = [0, 0, 0, 0, 3, b'A', b'B', b'C'];
        assert_eq!(
            device_message(&bytes),
            Ok(Some((DeviceMessage::Clipboard("ABC".into()), 8)))
        );
        assert_eq!(device_message(&bytes[..6]), Ok(None));
    }

    #[test]
    fn reads_a_clipboard_ack() {
        let bytes = [1, 1, 2, 3, 4, 5, 6, 7, 8];
        assert_eq!(
            device_message(&bytes),
            Ok(Some((
                DeviceMessage::AckClipboard(0x0102_0304_0506_0708),
                9
            )))
        );
    }

    #[test]
    fn reads_uhid_output() {
        let bytes = [2, 0, 42, 0, 2, 0xab, 0xcd];
        assert_eq!(
            device_message(&bytes),
            Ok(Some((
                DeviceMessage::UhidOutput {
                    id: 42,
                    data: vec![0xab, 0xcd]
                },
                7
            )))
        );
    }

    #[test]
    fn reads_media_headers() {
        let session = [0x80, 0, 0, 0, 0, 0, 0x04, 0x38, 0, 0, 0x09, 0x60];
        assert_eq!(
            header(&session),
            Header::Session {
                width: 1080,
                height: 2400
            }
        );

        let mut packet = [0u8; 12];
        packet[0] = 0x60; // config and key frame
        packet[7] = 42;
        packet[8..12].copy_from_slice(&1000u32.to_be_bytes());
        assert_eq!(
            header(&packet),
            Header::Packet {
                size: 1000,
                config: true,
                key: true,
                pts: 42
            }
        );
    }

    #[test]
    fn names_codecs() {
        assert_eq!(Codec::from_id(0x6832_3634), Codec::H264);
        assert_eq!(Codec::from_id(0x6f70_7573), Codec::Opus);
        assert_eq!(Codec::from_id(0), Codec::Disabled);
    }

    #[test]
    fn reads_the_device_name() {
        let mut bytes = [0u8; 64];
        bytes[..10].copy_from_slice(b"Galaxy S24");
        assert_eq!(device_name(&bytes), "Galaxy S24");
    }

    #[test]
    fn reads_the_app_list() {
        let log = "[server] INFO: List of apps:\n \
 * Camera                         com.sec.android.app.camera\n \
 - Signal                         org.thoughtcrime.securesms\n \
 - A name that is far too long for the column\n                                 com.example.long\n";
        let apps = apps(log);
        assert_eq!(apps.len(), 3);
        assert_eq!(apps[0].name, "Camera");
        assert!(apps[0].system);
        assert_eq!(apps[1].package, "org.thoughtcrime.securesms");
        assert!(!apps[1].system);
        assert_eq!(apps[2].name, "A name that is far too long for the column");
        assert_eq!(apps[2].package, "com.example.long");
    }
}
