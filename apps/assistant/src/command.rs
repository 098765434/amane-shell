//! What the assistant can ask the shell to do: the system commands the shell
//! already has (Control Center's switches, volume and brightness, apps,
//! what's playing, the session, a timer, the power mode), named by what the
//! shell knows them by, so running one is the shell's ordinary action.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "do", rename_all = "kebab-case")]
pub enum Command {
    /// One of Control Center's switches, by its control's name
    /// ("night-light", "wifi").
    Switch {
        control: String,
        state: State,
    },
    /// The volume or the screen's brightness.
    Level {
        level: Level,
        target: Target,
    },
    /// The speakers' mute.
    Mute {
        state: State,
    },
    /// An app, by its desktop id.
    Open {
        app: String,
    },
    Media {
        media: Media,
    },
    Power {
        power: Power,
    },
    /// A timer for this many seconds.
    Timer {
        seconds: u64,
    },
    CancelTimer,
    /// A power profile, by the daemon's name for it ("power-saver").
    PowerMode {
        profile: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    On,
    Off,
    Toggle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    Volume,
    Brightness,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Target {
    Percent(u8),
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Media {
    Play,
    Pause,
    Next,
    Previous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Power {
    Lock,
    LogOut,
    Sleep,
    Restart,
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
    pub const ALL: [Power; 5] = [
        Self::Lock,
        Self::LogOut,
        Self::Sleep,
        Self::Restart,
        Self::PowerOff,
    ];

    /// What people call it, as the tool offers it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Lock => "lock",
            Self::LogOut => "log out",
            Self::Sleep => "sleep",
            Self::Restart => "restart",
            Self::PowerOff => "power off",
        }
    }

    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|power| power.name() == name)
    }
}

impl Media {
    pub const ALL: [Media; 4] = [Self::Play, Self::Pause, Self::Next, Self::Previous];

    pub fn name(self) -> &'static str {
        match self {
            Self::Play => "play",
            Self::Pause => "pause",
            Self::Next => "next",
            Self::Previous => "previous",
        }
    }

    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|media| media.name() == name)
    }
}

impl State {
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
