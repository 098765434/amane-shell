//! `sound.json`: the sound a notification plays.

use super::*;

/// The sounds the shell plays itself.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct Sound {
    /// The sound theme's sound a notification's banner plays, by name
    /// (see [`crate::sounds`]); none plays nothing.
    pub notification: Option<String>,

    /// How loud it plays, 0 to 100.
    pub volume: u8,
}

impl Default for Sound {
    fn default() -> Self {
        Self {
            notification: Some(String::from("message-new-instant")),
            volume: 60,
        }
    }
}

settings_file!(Sound, crate::app_ids::SHELL, "sound", sound, change_sound);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_field_is_its_default_and_none_stays_none() {
        let sound: Sound = serde_json::from_str(r#"{"volume": 30}"#).expect("reads");
        assert_eq!(sound.notification.as_deref(), Some("message-new-instant"));
        assert_eq!(sound.volume, 30);
        let off: Sound = serde_json::from_str(r#"{"notification": null}"#).expect("reads");
        assert_eq!(off.notification, None);
    }
}
