//! Today's weather for a place, for the bar's weather item: from
//! Open-Meteo (no key), only while the bar's weather is switched on and a
//! place is set. The place is found once with Open-Meteo's geocoding; the
//! weather is read every half hour, and again when the place changes.
//! Nothing but the place name and its coordinates leave the machine.

use std::process::Command;
use std::time::{Duration, Instant, SystemTime};

use amane::{Config, Service};
use emaki_kit::settings::{StatusBar, status_bar};
use serde_json::Value;

use crate::services::Wake;
use emaki_kit::service::ServiceExt as _;

/// How often the weather is read again.
const EVERY: Duration = Duration::from_secs(30 * 60);

// a change of the bar's settings wakes the listener
static WAKE: Wake = Wake::new();

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Weather {
    /// The place as Open-Meteo names it, like "Oslo, Norway".
    place: String,
    now: Option<Now>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Now {
    pub temperature: f64,
    pub code: u32,
    pub high: f64,
    pub low: f64,
    /// Daylight where the place is, so a clear night shows a moon.
    pub day: bool,
}

impl Service for Weather {
    fn new() -> Self {
        Self::default()
    }

    fn listen() {
        Config::<StatusBar>::subscribe(|_| WAKE.nudge()).forever();

        let mut asked: Option<(String, Instant)> = None;
        let mut found: Option<(String, (f64, f64, String))> = None;
        loop {
            let (on, place) = {
                let bar = status_bar();
                (bar.weather, bar.weather_place.trim().to_string())
            };
            let due = asked
                .as_ref()
                .is_none_or(|(asked_for, at)| *asked_for != place || at.elapsed() >= EVERY);
            if on && !place.is_empty() && due {
                if found.as_ref().is_none_or(|(name, _)| *name != place) {
                    found = geocode(&place).map(|spot| (place.clone(), spot));
                }
                let next = match &found {
                    Some((_, (latitude, longitude, name))) => Weather {
                        place: name.clone(),
                        now: forecast(*latitude, *longitude),
                    },
                    None => Weather::default(),
                };
                Weather::replace(next);
                asked = Some((place, Instant::now()));
            }
            // the next read is due in half an hour, while it's on; a setting may change sooner
            let next = asked
                .as_ref()
                .filter(|_| on)
                .map(|(_, at)| SystemTime::now() + EVERY.saturating_sub(at.elapsed()));
            WAKE.wait(next);
        }
    }
}

impl Weather {
    pub fn place(&self) -> &str {
        &self.place
    }

    pub fn now(&self) -> Option<&Now> {
        self.now.as_ref()
    }
}

/// `place`'s coordinates and the name Open-Meteo gives it.
fn geocode(place: &str) -> Option<(f64, f64, String)> {
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?count=1&format=json&name={}",
        encode(place)
    );
    let found: Value = serde_json::from_str(&fetch(&url)?).ok()?;
    let spot = found["results"].get(0)?;
    let name = [spot["name"].as_str(), spot["country"].as_str()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ");
    Some((
        spot["latitude"].as_f64()?,
        spot["longitude"].as_f64()?,
        name,
    ))
}

fn forecast(latitude: f64, longitude: f64) -> Option<Now> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={latitude}&longitude={longitude}\
         &current=temperature_2m,weather_code,is_day&daily=temperature_2m_max,temperature_2m_min\
         &forecast_days=1&timezone=auto"
    );
    read_forecast(&fetch(&url)?)
}

fn read_forecast(json: &str) -> Option<Now> {
    let forecast: Value = serde_json::from_str(json).ok()?;
    let current = &forecast["current"];
    let daily = &forecast["daily"];
    Some(Now {
        temperature: current["temperature_2m"].as_f64()?,
        code: current["weather_code"].as_u64()? as u32,
        high: daily["temperature_2m_max"].get(0)?.as_f64()?,
        low: daily["temperature_2m_min"].get(0)?.as_f64()?,
        day: current["is_day"].as_u64().is_none_or(|day| day == 1),
    })
}

fn fetch(url: &str) -> Option<String> {
    let output = Command::new("curl")
        .args(["-fsSL", "--max-time", "15", url])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// A place name made safe for a URL.
fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

/// A Material Symbols glyph and a few words for a WMO weather code, by
/// day or by night.
pub fn describe(code: u32, day: bool) -> (char, &'static str) {
    match code {
        0 if day => ('\u{e81a}', "Clear"),
        0 => ('\u{f159}', "Clear"),
        1 | 2 if day => ('\u{f172}', "Partly Cloudy"),
        1 | 2 => ('\u{f174}', "Partly Cloudy"),
        3 => ('\u{e2bd}', "Cloudy"),
        45 | 48 => ('\u{e818}', "Fog"),
        51..=57 => ('\u{f176}', "Drizzle"),
        61..=67 | 80..=82 => ('\u{f176}', "Rain"),
        71..=77 | 85 | 86 => ('\u{e2cd}', "Snow"),
        95..=99 => ('\u{ebdb}', "Thunderstorms"),
        _ => ('\u{e2bd}', "Cloudy"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_forecast() {
        let json = r#"{"current":{"temperature_2m":7.4,"weather_code":61},
            "daily":{"temperature_2m_max":[9.1],"temperature_2m_min":[3.0]}}"#;
        assert_eq!(
            read_forecast(json),
            Some(Now {
                temperature: 7.4,
                code: 61,
                high: 9.1,
                low: 3.0,
                day: true,
            })
        );
        assert_eq!(read_forecast("{}"), None);
        assert_eq!(describe(61, true).1, "Rain");
        assert_ne!(describe(0, true).0, describe(0, false).0);
    }

    #[test]
    fn a_place_is_made_safe_for_a_url() {
        assert_eq!(encode("Tromsø, Norway"), "Troms%C3%B8%2C%20Norway");
    }
}
