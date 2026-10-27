//! A connected Bluetooth device running low: a notification at 20 % and
//! again at 10 %, once each per connection, as phones warn about a headset.
//! Read from the fork's `Bluetooth` service once a minute, as a battery
//! that reports itself changes slowly.

use std::collections::BTreeMap;
use std::sync::Once;
use std::time::Duration;

use amane::{Bluetooth, Notify, Service, Urgency};

/// The levels warned at, highest first.
const LEVELS: [u8; 2] = [20, 10];

/// Above this a device counts as charged again, and is warned anew.
const RESET_ABOVE: u8 = 25;

const EVERY: Duration = Duration::from_secs(60);

/// Starts watching, once.
pub fn watch() {
    static WATCH: Once = Once::new();
    WATCH.call_once(|| {
        std::thread::spawn(|| {
            // the lowest level each device was warned at, by address
            let mut warned: BTreeMap<String, u8> = BTreeMap::new();
            loop {
                std::thread::sleep(EVERY);
                check(&mut warned);
            }
        });
    });
}

fn check(warned: &mut BTreeMap<String, u8>) {
    let bluetooth = Bluetooth::read();
    let devices: Vec<(String, String, u8)> = bluetooth
        .devices()
        .iter()
        .filter(|device| device.connected())
        .filter_map(|device| {
            let percent = device.battery()?;
            Some((
                device.address().to_string(),
                device.name().to_string(),
                percent,
            ))
        })
        .collect();
    drop(bluetooth);

    // a device that disconnected is warned again on its next connection
    warned.retain(|address, _| devices.iter().any(|(a, _, _)| a == address));

    for (address, name, percent) in devices {
        let (now, warn) = warning(warned.get(&address).copied(), percent);
        match now {
            Some(level) => warned.insert(address, level),
            None => warned.remove(&address),
        };
        if let Some(level) = warn {
            let urgency = if level <= 10 {
                Urgency::Critical
            } else {
                Urgency::Normal
            };
            Notify::new(&format!("{name} Battery Low"))
                .body(&format!("{percent}% remaining"))
                .icon("battery-low")
                .urgency(urgency)
                .send();
        }
    }
}

/// From the level last warned at and the battery now: the level warned at
/// after this reading, and the one to warn about now, if any.
fn warning(warned: Option<u8>, percent: u8) -> (Option<u8>, Option<u8>) {
    if percent > RESET_ABOVE {
        return (None, None);
    }
    let due = LEVELS
        .iter()
        .copied()
        .filter(|level| percent <= *level)
        .min();
    match (warned, due) {
        (Some(warned), Some(due)) if due < warned => (Some(due), Some(due)),
        (None, Some(due)) => (Some(due), Some(due)),
        (warned, _) => (warned, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warns_once_at_each_level() {
        assert_eq!(warning(None, 50), (None, None));
        assert_eq!(warning(None, 20), (Some(20), Some(20)));
        assert_eq!(warning(Some(20), 15), (Some(20), None));
        assert_eq!(warning(Some(20), 9), (Some(10), Some(10)));
        assert_eq!(warning(Some(10), 5), (Some(10), None));
    }

    #[test]
    fn a_device_found_very_low_warns_once_at_the_lowest() {
        assert_eq!(warning(None, 4), (Some(10), Some(10)));
    }

    #[test]
    fn charging_past_the_reset_warns_again() {
        assert_eq!(warning(Some(10), 22), (Some(10), None));
        assert_eq!(warning(Some(10), 30), (None, None));
        assert_eq!(warning(None, 19), (Some(20), Some(20)));
    }
}
