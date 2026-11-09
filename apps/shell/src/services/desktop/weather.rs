//! Today's weather for a place, for the bar's weather item: from
//! Open-Meteo (no key), only while the bar's weather is switched on and a
//! place is set. The place is found once with Open-Meteo's geocoding; the
//! weather is read every half hour, and again when the place changes.
//! Nothing but the place name and its coordinates leave the machine.

use std::process::Command;
use std::time::{Duration, Instant, SystemTime};

use amane::{Config, Service};
use emaki_kit::settings::{StatusBar, WeatherLookup, remember_weather, status_bar};
use serde_json::Value;

use crate::services::Wake;
use emaki_kit::service::ServiceExt as _;

/// How often the weather is read again.
const EVERY: Duration = Duration::from_secs(30 * 60);

/// How soon it's tried again after Open-Meteo couldn't be reached.
const RETRY: Duration = Duration::from_secs(5 * 60);

// a change of the bar's settings wakes the listener
static WAKE: Wake = Wake::new();

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Weather {
    /// The place as Open-Meteo names it, like "Oslo, Norway".
    place: String,
    now: Option<Now>,
    /// The last reading, kept while Open-Meteo can't be reached.
    stale: bool,
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

        // the place asked for, when, and how long until it's asked again
        let mut asked: Option<(String, Instant, Duration)> = None;
        let mut found: Option<(String, (f64, f64, String))> = None;
        let mut shown = Weather::default();
        loop {
            let (on, place) = {
                let bar = status_bar();
                (bar.weather, bar.weather_place.trim().to_string())
            };
            let due = asked
                .as_ref()
                .is_none_or(|(asked_for, at, wait)| *asked_for != place || at.elapsed() >= *wait);
            if on && !place.is_empty() && due {
                let mut lookup = WeatherLookup::Found;
                if found.as_ref().is_none_or(|(name, _)| *name != place) {
                    found = None;
                    match geocode(&place) {
                        Ok(Some(spot)) => found = Some((place.clone(), spot)),
                        Ok(None) => lookup = WeatherLookup::NotFound,
                        Err(Unreachable) => lookup = WeatherLookup::Unreachable,
                    }
                }
                let reading = found.as_ref().map(|(_, (latitude, longitude, name))| {
                    (name, forecast(*latitude, *longitude))
                });
                let next = match reading {
                    Some((name, Some(now))) => Weather {
                        place: name.clone(),
                        now: Some(now),
                        stale: false,
                    },
                    Some((name, None)) => {
                        lookup = WeatherLookup::Unreachable;
                        kept(&shown, name)
                    }
                    None => Weather::default(),
                };
                let wait = match lookup {
                    WeatherLookup::Unreachable => RETRY,
                    _ => EVERY,
                };
                shown = next.clone();
                Weather::replace(next);
                remember_weather(&place, lookup);
                asked = Some((place, Instant::now(), wait));
            }
            // the next read is due when it said, while it's on; a setting may change sooner
            let next = asked
                .as_ref()
                .filter(|_| on)
                .map(|(_, at, wait)| SystemTime::now() + wait.saturating_sub(at.elapsed()));
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

    /// Whether the reading is an old one, kept while Open-Meteo can't be
    /// reached.
    pub fn stale(&self) -> bool {
        self.stale
    }
}

/// The reading `shown` kept, out of date, while Open-Meteo can't be
/// reached; nothing when it was for another place.
fn kept(shown: &Weather, place: &str) -> Weather {
    Weather {
        place: place.to_string(),
        now: shown.now.clone().filter(|_| shown.place == place),
        stale: true,
    }
}

/// Open-Meteo couldn't be reached, or answered with something unreadable.
#[derive(Debug, PartialEq)]
struct Unreachable;

/// `place`'s coordinates and the name Open-Meteo gives it, or nothing when
/// it knows no such place.
fn geocode(place: &str) -> Result<Option<(f64, f64, String)>, Unreachable> {
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?count=1&format=json&name={}",
        encode(place)
    );
    let text = fetch(&url).ok_or(Unreachable)?;
    read_place(&text)
}

/// The first place in a geocoding answer.
fn read_place(json: &str) -> Result<Option<(f64, f64, String)>, Unreachable> {
    let found: Value = serde_json::from_str(json).map_err(|_| Unreachable)?;
    let Some(spot) = found["results"].get(0) else {
        return Ok(None);
    };
    let name = [spot["name"].as_str(), spot["country"].as_str()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ");
    Ok(spot["latitude"]
        .as_f64()
        .zip(spot["longitude"].as_f64())
        .map(|(latitude, longitude)| (latitude, longitude, name)))
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
    fn a_place_nobody_knows_is_not_an_outage() {
        // Open-Meteo leaves `results` out for a place it doesn't know
        assert_eq!(read_place(r#"{"generationtime_ms":0.5}"#), Ok(None));
        assert_eq!(read_place("<html>502</html>"), Err(Unreachable));
        let oslo =
            r#"{"results":[{"name":"Oslo","country":"Norway","latitude":59.9,"longitude":10.7}]}"#;
        assert_eq!(
            read_place(oslo),
            Ok(Some((59.9, 10.7, String::from("Oslo, Norway"))))
        );
    }

    #[test]
    fn an_outage_keeps_the_last_reading_for_the_same_place_only() {
        let shown = Weather {
            place: String::from("Oslo, Norway"),
            now: read_forecast(
                r#"{"current":{"temperature_2m":7.4,"weather_code":61},
                "daily":{"temperature_2m_max":[9.1],"temperature_2m_min":[3.0]}}"#,
            ),
            stale: false,
        };
        let same = kept(&shown, "Oslo, Norway");
        assert!(same.stale && same.now.is_some());
        assert!(kept(&shown, "Bergen, Norway").now.is_none());
    }

    #[test]
    fn a_place_is_made_safe_for_a_url() {
        assert_eq!(encode("Tromsø, Norway"), "Troms%C3%B8%2C%20Norway");
    }
}
