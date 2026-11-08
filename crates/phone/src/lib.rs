//! Phones, for emaki and its apps: KDE Connect (pairing, battery,
//! ringing, files, the clipboard, run commands, remote input, notifications
//! and text messages) over `kdeconnectd`'s session-bus interface, and adb
//! (USB and Wireless debugging, found over mDNS) for mirroring.
//!
//! Each is an amane [`Service`](amane::Service): read it in a view, and it
//! follows its source on a thread of its own.

#![deny(missing_docs)]

mod adb;
mod kdeconnect;
pub mod shared;

pub use adb::run as adb;
pub use adb::{Adb, AdbAction, AdbDevice, AdbOperation, AdbState, WirelessKind, WirelessService};
pub use kdeconnect::{KdeConnect, RunCommand, digits, same_phone_number};
pub use kdeconnect::{
    PairState as PhonePairState, PhoneBattery, PhoneClick, PhoneDevice, PhoneKey, PhoneMessages,
    PhoneModifiers, PhoneNotification, SMS_PAGE, SmsAttachment, SmsKind, SmsMessage, SmsState,
    SmsThread,
};
