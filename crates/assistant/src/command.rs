//! What the assistant can ask the shell to do: the system commands the shell
//! already has (Quick Settings' switches, volume and brightness, apps,
//! what's playing, the session, a timer, the power mode), named by what the
//! shell knows them by, so running one is the shell's ordinary action.

use serde::{Deserialize, Serialize};

/// One thing the shell can be asked to do, as JSON tagged by `do`
/// (`{"do":"mute","state":"on"}`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "do", rename_all = "kebab-case")]
pub enum Command {
    /// One of Quick Settings' switches, by its control's name
    /// ("night-light", "wifi").
    Switch {
        /// The control's name.
        control: String,
        /// What to turn it to.
        state: State,
    },
    /// The volume or the screen's brightness.
    Level {
        /// Which level.
        level: Level,
        /// What to set it to.
        target: Target,
    },
    /// The speakers' mute.
    Mute {
        /// On mutes, off unmutes.
        state: State,
    },
    /// An app, by its desktop id.
    Open {
        /// The app's desktop id ("firefox").
        app: String,
    },
    /// What's playing.
    Media {
        /// What to do to it.
        media: Media,
    },
    /// The session or the computer: lock, log out, sleep, restart, power off.
    Power {
        /// Which of them.
        power: Power,
    },
    /// A timer for this many seconds.
    Timer {
        /// How long, in seconds.
        seconds: u64,
    },
    /// Stops the running timer.
    CancelTimer,
    /// A power profile, by the daemon's name for it ("power-saver").
    PowerMode {
        /// The profile's name, as the power profiles daemon knows it.
        profile: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// What to turn a switch or the mute to.
pub enum State {
    /// On.
    On,
    /// Off.
    Off,
    /// The other way from how it is.
    Toggle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// A level a [`Command::Level`] sets.
pub enum Level {
    /// The speakers' volume.
    Volume,
    /// The screen's brightness.
    Brightness,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Where a [`Command::Level`] takes its level.
pub enum Target {
    /// To this percent, 0 … 100.
    Percent(u8),
    /// A step up.
    Up,
    /// A step down.
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// What to do to what's playing.
pub enum Media {
    /// Play it, when it's paused.
    Play,
    /// Pause it, when it's playing.
    Pause,
    /// Skip to the next track.
    Next,
    /// Go back to the previous track.
    Previous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// What a [`Command::Power`] does to the session or the computer.
pub enum Power {
    /// Locks the screen.
    Lock,
    /// Ends the session.
    LogOut,
    /// Suspends the computer.
    Sleep,
    /// Hibernates the computer (saves RAM to disk, powers off).
    Hibernate,
    /// Restarts the computer.
    Restart,
    /// Shuts the computer down.
    PowerOff,
}

/// How much care running a command needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Risk {
    /// Undone as easily as done: a switch, a level, a song.
    Safe,
    /// Changes what's on screen: an app opening, the screen locking.
    Notable,
    /// Loses work or the session: asked about first, every time.
    Destructive,
}

impl Command {
    /// How much care running it needs: ending the session is destructive,
    /// opening an app or locking the screen notable, the rest safe.
    pub fn risk(&self) -> Risk {
        match self {
            Self::Switch { .. }
            | Self::Level { .. }
            | Self::Mute { .. }
            | Self::Media { .. }
            | Self::Timer { .. }
            | Self::CancelTimer
            | Self::PowerMode { .. } => Risk::Safe,
            Self::Open { .. } | Self::Power { power: Power::Lock } => Risk::Notable,
            Self::Power { .. } => Risk::Destructive,
        }
    }
}

impl Power {
    /// Every one, in the order the tool offers them.
    pub const ALL: [Power; 6] = [
        Self::Lock,
        Self::LogOut,
        Self::Sleep,
        Self::Hibernate,
        Self::Restart,
        Self::PowerOff,
    ];

    /// What people call it, as the tool offers it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Lock => "lock",
            Self::LogOut => "log out",
            Self::Sleep => "sleep",
            Self::Hibernate => "hibernate",
            Self::Restart => "restart",
            Self::PowerOff => "power off",
        }
    }

    /// The one with this [`name`](Self::name), as the tool gives it back.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|power| power.name() == name)
    }
}

impl Media {
    /// Every one, in the order the tool offers them.
    pub const ALL: [Media; 4] = [Self::Play, Self::Pause, Self::Next, Self::Previous];

    /// Its name, as the tool offers it: "play", "next".
    pub fn name(self) -> &'static str {
        match self {
            Self::Play => "play",
            Self::Pause => "pause",
            Self::Next => "next",
            Self::Previous => "previous",
        }
    }

    /// The one with this [`name`](Self::name), as the tool gives it back.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|media| media.name() == name)
    }
}

impl State {
    /// The one called "on", "off" or "toggle".
    pub fn named(name: &str) -> Option<Self> {
        match name {
            "on" => Some(Self::On),
            "off" => Some(Self::Off),
            "toggle" => Some(Self::Toggle),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ending_the_session_is_always_destructive() {
        for power in Power::ALL {
            let risk = Command::Power { power }.risk();
            if power == Power::Lock {
                assert_eq!(risk, Risk::Notable);
            } else {
                assert_eq!(risk, Risk::Destructive);
            }
        }
    }

    #[test]
    fn commands_round_trip_as_json() {
        let command = Command::Level {
            level: Level::Volume,
            target: Target::Percent(40),
        };
        let json = serde_json::to_string(&command).unwrap();
        assert_eq!(serde_json::from_str::<Command>(&json).unwrap(), command);
    }
}
