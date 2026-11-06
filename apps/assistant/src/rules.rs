//! Commands read by their phrasing alone, with no model: the common ways of
//! saying the common things ("louder", "volume 40", "next song", "turn on
//! night light", "open firefox", "timer 5 minutes"). A rule matches the
//! whole query or not at all, and anything negated ("don't …") is left to
//! the model, so what a rule reads is certain.

use crate::command::{Command, Level, Media, Power, State, Target};
use crate::snapshot::Snapshot;
use crate::text;

/// Words at a query's start or end that don't change what it asks.
const POLITE_START: &[&str] = &[
    "please",
    "can you",
    "could you",
    "would you",
    "will you",
    "hey",
    "ok",
    "okay",
    "now",
    "just",
    "i want to",
    "i would like to",
    "id like to",
    "lets",
];
const POLITE_END: &[&str] = &["please", "for me", "now", "thanks", "thank you"];

/// Words that turn a command round; a query with one is the model's.
const NEGATIONS: &[&str] = &[
    "dont", "do not", "not", "never", "without", "no", "cant", "wont",
];

/// How a question starts, once what's polite is off ("can you" is, "can i"
/// isn't): asking about a thing isn't asking for it.
const QUESTIONS: &[&str] = &[
    "how", "what", "whats", "why", "when", "where", "who", "which", "is", "are", "does", "do i",
    "do you", "can i", "should", "will it",
];

/// Whether the model is worth asking about `query`, once no rule read it.
/// Not a word alone, nor the bare name of a switch or an app (Spotlight
/// already shows that thing), nor a question: each of those, asked, gets a
/// confident wrong call more often than a right one.
pub fn worth_asking(query: &str, snapshot: &Snapshot) -> bool {
    let q = plain(query);
    if !q.contains(' ') {
        return false;
    }
    let question = QUESTIONS.iter().any(|start| {
        q.strip_prefix(start)
            .is_some_and(|rest| rest.starts_with(' '))
    });
    !question && snapshot.switch(&q).is_none() && snapshot.app(&q).is_none()
}

/// What `query` asks, if a rule reads it.
pub fn read(query: &str, snapshot: &Snapshot) -> Option<Command> {
    let words = plain(query);
    if words.is_empty() || negated(&words) {
        return None;
    }
    let q = words.as_str();
    phrase(q, snapshot)
        .or_else(|| level(q, snapshot))
        .or_else(|| media(q))
        .or_else(|| switch(q, snapshot))
        .or_else(|| power(q, snapshot))
        .or_else(|| open(q, snapshot))
        .or_else(|| timer(q, snapshot))
        .or_else(|| power_mode(q, snapshot))
}

/// `query` normalized, with what's only polite taken off.
pub fn plain(query: &str) -> String {
    // a name with a negation in it isn't one
    let mut q = text::normalize(query).replace("do not disturb", "dnd");
    loop {
        let before = q.len();
        for start in POLITE_START {
            if let Some(rest) = q.strip_prefix(start).and_then(|r| r.strip_prefix(' ')) {
                q = rest.to_string();
            }
        }
        for end in POLITE_END {
            if let Some(rest) = q.strip_suffix(end).and_then(|r| r.strip_suffix(' ')) {
                q = rest.to_string();
            }
        }
        if q.len() == before {
            return q;
        }
    }
}

fn negated(words: &str) -> bool {
    let padded = format!(" {words} ");
    NEGATIONS
        .iter()
        .any(|negation| padded.contains(&format!(" {negation} ")))
}

/// `q` with any of `articles` taken off the front of each word run: "the",
/// "my", "this".
fn bare(q: &str) -> String {
    q.split(' ')
        .filter(|word| !matches!(*word, "the" | "my" | "this" | "a" | "an" | "it" | "its"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A level like "40", "40%", "40 percent".
fn percent(text: &str) -> Option<u8> {
    let text = text.trim();
    let text = text
        .strip_suffix(" percent")
        .or_else(|| text.strip_suffix('%'))
        .unwrap_or(text)
        .trim();
    let value: u16 = text.parse().ok()?;
    (value <= 100).then_some(value as u8)
}

/// Whole phrases for a switch: "keep the screen on", "take a screenshot".
fn phrase(q: &str, snapshot: &Snapshot) -> Option<Command> {
    let (control, state) = match bare(q).as_str() {
        "keep screen on" | "keep screen awake" | "stay awake" | "keep awake" => {
            ("keep-awake", State::On)
        }
        "take screenshot" | "screenshot" | "capture screen" => ("screenshot", State::Toggle),
        _ => return None,
    };
    let switch = snapshot.switches.iter().find(|s| s.control == control)?;
    Some(switch_command(switch, state))
}

fn level(q: &str, snapshot: &Snapshot) -> Option<Command> {
    let q = bare(q);
    let q = q.as_str();
    let volume = |target| Command::Level {
        level: Level::Volume,
        target,
    };
    let brightness = |target| {
        snapshot.brightness.then_some(Command::Level {
            level: Level::Brightness,
            target,
        })
    };
    match q {
        "louder" | "make louder" | "make sound louder" | "volume up" | "turn up"
        | "turn up volume" | "turn volume up" | "turn sound up" | "turn up sound"
        | "increase volume" | "raise volume" | "more volume" | "sound up" => {
            return Some(volume(Target::Up));
        }
        "quieter" | "make quieter" | "make softer" | "softer" | "volume down" | "turn down"
        | "turn down volume" | "turn volume down" | "turn sound down" | "turn down sound"
        | "lower volume" | "decrease volume" | "less volume" | "sound down" | "too loud" => {
            return Some(volume(Target::Down));
        }
        "max volume" | "full volume" | "maximum volume" | "volume max" => {
            return Some(volume(Target::Percent(100)));
        }
        "max brightness" | "full brightness" | "maximum brightness" | "brightness max" => {
            return brightness(Target::Percent(100));
        }
        "mute" | "mute sound" | "mute volume" | "mute audio" | "mute speakers" | "silence" => {
            return Some(Command::Mute { state: State::On });
        }
        "unmute" | "unmute sound" | "unmute volume" | "unmute audio" | "unmute speakers" => {
            return Some(Command::Mute { state: State::Off });
        }
        "brighter"
        | "make brighter"
        | "make screen brighter"
        | "brightness up"
        | "increase brightness"
        | "raise brightness"
        | "brighten screen"
        | "turn up brightness"
        | "turn brightness up"
        | "too dark" => {
            return brightness(Target::Up);
        }
        "dimmer"
        | "make dimmer"
        | "make screen dimmer"
        | "dim"
        | "dim screen"
        | "dim display"
        | "brightness down"
        | "decrease brightness"
        | "lower brightness"
        | "turn down brightness"
        | "turn brightness down"
        | "too bright"
        | "screen too bright"
        | "screen is too bright" => {
            return brightness(Target::Down);
        }
        _ => {}
    }
    // "volume 40", "set volume to 40%", "brightness at 70 percent"
    let q = q.strip_prefix("set ").unwrap_or(q);
    for (name, level) in [
        ("volume", Level::Volume),
        ("sound", Level::Volume),
        ("brightness", Level::Brightness),
        ("screen brightness", Level::Brightness),
    ] {
        let Some(rest) = q.strip_prefix(name).and_then(|r| r.strip_prefix(' ')) else {
            continue;
        };
        let rest = ["to ", "at ", "="]
            .iter()
            .find_map(|joiner| rest.strip_prefix(joiner))
            .unwrap_or(rest);
        let value = percent(rest)?;
        return match level {
            Level::Volume => Some(volume(Target::Percent(value))),
            Level::Brightness => brightness(Target::Percent(value)),
        };
    }
    None
}

fn media(q: &str) -> Option<Command> {
    let q = bare(q);
    let media = match q.as_str() {
        "next" | "next song" | "next track" | "skip" | "skip song" | "skip track"
        | "play next song" | "play next track" | "next one" => Media::Next,
        "previous"
        | "previous song"
        | "previous track"
        | "last song"
        | "last track"
        | "go back song"
        | "back song"
        | "play previous song"
        | "play previous track"
        | "go back to previous song"
        | "go back to previous track" => Media::Previous,
        "pause" | "pause music" | "pause song" | "pause video" | "pause playback"
        | "stop music" | "stop playback" | "stop song" => Media::Pause,
        "play" | "resume" | "play music" | "resume music" | "resume playback" | "unpause" => {
            Media::Play
        }
        _ => return None,
    };
    Some(Command::Media { media })
}

fn switch(q: &str, snapshot: &Snapshot) -> Option<Command> {
    let q = bare(q);
    let q = q.as_str();
    // "turn on X", "enable X", "toggle X"
    let starts: [(&str, State); 13] = [
        ("switch to ", State::On),
        ("connect to ", State::On),
        ("disconnect from ", State::Off),
        ("turn on ", State::On),
        ("turn off ", State::Off),
        ("switch on ", State::On),
        ("switch off ", State::Off),
        ("enable ", State::On),
        ("disable ", State::Off),
        ("activate ", State::On),
        ("deactivate ", State::Off),
        ("toggle ", State::Toggle),
        ("start ", State::On),
    ];
    for (start, state) in starts {
        if let Some(name) = q.strip_prefix(start)
            && let Some(found) = snapshot.switch(name)
        {
            return Some(switch_command(found, state));
        }
    }
    // "turn X on", "switch X off", "X on"
    for verb in ["turn ", "switch ", "put ", ""] {
        let Some(rest) = q.strip_prefix(verb) else {
            continue;
        };
        for (end, state) in [(" on", State::On), (" off", State::Off)] {
            if let Some(name) = rest.strip_suffix(end)
                && let Some(found) = snapshot.switch(name)
            {
                return Some(switch_command(found, state));
            }
        }
    }
    None
}

fn switch_command(switch: &crate::snapshot::Switch, state: State) -> Command {
    // a button (Screenshot) only presses
    let state = if switch.on.is_none() {
        State::Toggle
    } else {
        state
    };
    Command::Switch {
        control: switch.control.clone(),
        state,
    }
}

fn power(q: &str, snapshot: &Snapshot) -> Option<Command> {
    let q = bare(q);
    let power = match q.as_str() {
        "lock" | "lock screen" | "lock computer" | "lock laptop" | "lock session" => Power::Lock,
        "log out" | "logout" | "sign out" | "log off" => Power::LogOut,
        "sleep"
        | "suspend"
        | "go to sleep"
        | "put computer to sleep"
        | "put laptop to sleep"
        | "suspend computer"
        | "suspend laptop" => Power::Sleep,
        "restart" | "reboot" | "restart computer" | "reboot computer" | "restart laptop"
        | "reboot laptop" => Power::Restart,
        "shut down" | "shutdown" | "power off" | "turn off computer" | "turn off laptop"
        | "shut down computer" | "shut down laptop" | "power down" | "turn computer off"
        | "turn laptop off" | "power off computer" => Power::PowerOff,
        _ => return None,
    };
    snapshot
        .powers
        .contains(&power)
        .then_some(Command::Power { power })
}

fn open(q: &str, snapshot: &Snapshot) -> Option<Command> {
    for verb in ["open ", "launch ", "start ", "run ", "show "] {
        if let Some(name) = q.strip_prefix(verb) {
            let name = name.strip_prefix("the ").unwrap_or(name);
            let name = name.strip_suffix(" app").unwrap_or(name);
            if let Some(app) = snapshot.app(name) {
                return Some(Command::Open {
                    app: app.id.clone(),
                });
            }
        }
    }
    None
}

/// "timer 5 minutes", "set a timer for 1 hour 30 minutes", "5 minute
/// timer", "cancel the timer".
fn timer(q: &str, snapshot: &Snapshot) -> Option<Command> {
    let q = bare(q);
    let q = q.as_str();
    if matches!(
        q,
        "cancel timer" | "stop timer" | "end timer" | "clear timer" | "delete timer"
    ) {
        return snapshot.timer_running.then_some(Command::CancelTimer);
    }
    let rest = [
        "set timer for ",
        "start timer for ",
        "timer for ",
        "set timer ",
        "start timer ",
        "timer ",
    ]
    .iter()
    .find_map(|start| q.strip_prefix(start))
    .or_else(|| q.strip_suffix(" timer"))?;
    let seconds = duration(rest)?;
    (seconds > 0).then_some(Command::Timer { seconds })
}

/// "5 minutes", "1 hour 30 minutes", "90 seconds", "5m", "1h30m", "1:30".
pub fn duration(text: &str) -> Option<u64> {
    let text = text.trim();
    if let Some((minutes, seconds)) = text.split_once(':') {
        let minutes: u64 = minutes.parse().ok()?;
        let seconds: u64 = seconds.parse().ok()?;
        return (seconds < 60).then_some(minutes * 60 + seconds);
    }
    let mut total = 0u64;
    let mut number: Option<u64> = None;
    let mut any = false;
    // split "1h30m" and "5 minutes" alike into numbers and units
    let mut tokens: Vec<String> = Vec::new();
    for word in text.split(' ') {
        let mut current = String::new();
        let mut digits = None;
        for c in word.chars() {
            let is_digit = c.is_ascii_digit();
            if digits.is_some_and(|d| d != is_digit) {
                tokens.push(std::mem::take(&mut current));
            }
            digits = Some(is_digit);
            current.push(c);
        }
        if !current.is_empty() {
            tokens.push(current);
        }
    }
    for token in tokens {
        if let Ok(value) = token.parse::<u64>() {
            if number.is_some() {
                return None;
            }
            number = Some(value);
            continue;
        }
        let unit = match token.as_str() {
            "h" | "hr" | "hrs" | "hour" | "hours" => 3600,
            "m" | "min" | "mins" | "minute" | "minutes" => 60,
            "s" | "sec" | "secs" | "second" | "seconds" => 1,
            "and" => continue,
            _ => return None,
        };
        total += number.take()? * unit;
        any = true;
    }
    if number.is_some() || !any {
        return None;
    }
    Some(total)
}

fn power_mode(q: &str, snapshot: &Snapshot) -> Option<Command> {
    let q = bare(q);
    let name = [
        "switch to ",
        "set power mode to ",
        "power mode ",
        "use ",
        "set ",
    ]
    .iter()
    .find_map(|start| q.strip_prefix(start))
    .unwrap_or(&q);
    let name = name.strip_suffix(" power mode").unwrap_or(name);
    if !name.ends_with(" mode") && !q.starts_with("power mode") && !q.starts_with("set power mode")
    {
        return None;
    }
    snapshot.power_mode(name).map(|profile| Command::PowerMode {
        profile: profile.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn questions_names_and_single_words_arent_asked() {
        let snapshot = crate::snapshot::sample();
        for query in [
            "how do I turn on night light",
            "what is bluetooth",
            "can i use bluetooth on a plane",
            "night light",
            "dark mode",
            "firefox",
            "spotify",
            "volume",
        ] {
            assert!(!worth_asking(query, &snapshot), "{query}");
        }
        for query in [
            "can you make the screen warmer",
            "I can't hear anything",
            "go back to the light theme",
            "don't let the computer go to sleep",
        ] {
            assert!(worth_asking(query, &snapshot), "{query}");
        }
    }
    use crate::snapshot::sample;

    fn reads(query: &str) -> Option<Command> {
        read(query, &sample())
    }

    fn sw(control: &str, state: State) -> Option<Command> {
        Some(Command::Switch {
            control: control.into(),
            state,
        })
    }

    #[test]
    fn levels() {
        let volume = |target| {
            Some(Command::Level {
                level: Level::Volume,
                target,
            })
        };
        assert_eq!(reads("louder"), volume(Target::Up));
        assert_eq!(reads("Make it louder."), volume(Target::Up));
        assert_eq!(reads("turn the volume up please"), volume(Target::Up));
        assert_eq!(reads("volume 40"), volume(Target::Percent(40)));
        assert_eq!(reads("Set the volume to 40%."), volume(Target::Percent(40)));
        assert_eq!(reads("volume 140"), None);
        assert_eq!(
            reads("brightness to 70 percent"),
            Some(Command::Level {
                level: Level::Brightness,
                target: Target::Percent(70)
            })
        );
        assert_eq!(
            reads("the screen is too bright"),
            Some(Command::Level {
                level: Level::Brightness,
                target: Target::Down
            })
        );
        assert_eq!(reads("mute"), Some(Command::Mute { state: State::On }));
    }

    #[test]
    fn media() {
        assert_eq!(
            reads("skip this song"),
            Some(Command::Media { media: Media::Next })
        );
        assert_eq!(
            reads("next track"),
            Some(Command::Media { media: Media::Next })
        );
        assert_eq!(
            reads("pause the music"),
            Some(Command::Media {
                media: Media::Pause
            })
        );
        assert_eq!(reads("play"), Some(Command::Media { media: Media::Play }));
    }

    #[test]
    fn switches() {
        assert_eq!(reads("turn on night light"), sw("night-light", State::On));
        assert_eq!(reads("Turn on Night Light."), sw("night-light", State::On));
        assert_eq!(reads("night light off"), sw("night-light", State::Off));
        assert_eq!(reads("turn the wifi off"), sw("wifi", State::Off));
        assert_eq!(reads("enable do not disturb"), sw("focus", State::On));
        assert_eq!(reads("disable bluetooth"), sw("bluetooth", State::Off));
        assert_eq!(reads("toggle dark mode"), sw("dark-style", State::Toggle));
        assert_eq!(reads("switch to dark mode"), sw("dark-style", State::On));
        assert_eq!(
            reads("disconnect from the internet"),
            sw("wifi", State::Off)
        );
        assert_eq!(reads("screenshot on"), sw("screenshot", State::Toggle));
        assert_eq!(reads("turn on teleporter"), None);
    }

    #[test]
    fn power() {
        assert_eq!(
            reads("lock the screen"),
            Some(Command::Power { power: Power::Lock })
        );
        assert_eq!(
            reads("shut down"),
            Some(Command::Power {
                power: Power::PowerOff
            })
        );
        assert_eq!(
            reads("put the laptop to sleep"),
            Some(Command::Power {
                power: Power::Sleep
            })
        );
        assert_eq!(reads("don't let the computer go to sleep"), None);
        assert_eq!(reads("do not shut down"), None);
    }

    #[test]
    fn apps() {
        assert_eq!(
            reads("open firefox"),
            Some(Command::Open {
                app: "firefox".into()
            })
        );
        assert_eq!(
            reads("launch the terminal"),
            Some(Command::Open {
                app: "com.mitchellh.ghostty".into()
            })
        );
        assert_eq!(
            reads("open visual studio"),
            Some(Command::Open { app: "code".into() })
        );
        assert_eq!(reads("open the pod bay doors"), None);
        assert_eq!(reads("firefox"), None, "a bare name is Spotlight's");
    }

    #[test]
    fn timers() {
        assert_eq!(
            reads("timer 5 minutes"),
            Some(Command::Timer { seconds: 300 })
        );
        assert_eq!(
            reads("set a timer for 1 hour 30 minutes"),
            Some(Command::Timer { seconds: 5400 })
        );
        assert_eq!(
            reads("5 minute timer"),
            Some(Command::Timer { seconds: 300 })
        );
        assert_eq!(reads("timer 1h30m"), Some(Command::Timer { seconds: 5400 }));
        assert_eq!(reads("timer 1:30"), Some(Command::Timer { seconds: 90 }));
        assert_eq!(reads("timer soon"), None);
        assert_eq!(reads("cancel the timer"), None, "none runs");
    }

    #[test]
    fn power_modes() {
        assert_eq!(
            reads("switch to power saver mode"),
            Some(Command::PowerMode {
                profile: "power-saver".into()
            })
        );
        assert_eq!(
            reads("performance mode"),
            Some(Command::PowerMode {
                profile: "performance".into()
            })
        );
        assert_eq!(reads("balanced"), None);
    }

    #[test]
    fn questions_are_left_alone() {
        for query in [
            "what's the weather in Lyon",
            "how tall is the eiffel tower",
            "tell me a joke",
            "12 * 7",
            "recipes for lasagna",
        ] {
            assert_eq!(reads(query), None, "{query}");
        }
    }
}
