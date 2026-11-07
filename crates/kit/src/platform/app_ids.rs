//! The apps' ids, written once: each app names itself with its own, and
//! the shell finds them by it (the niri window rules, the phone's apps,
//! Software Update's link).

/// Shell Settings.
pub const SETTINGS: &str = "amane-settings";
/// Messages, the phone's texts.
pub const MESSAGES: &str = "amane-messages";
/// The phone's screen; each of the phone's apps opened alone is
/// `amane-mirror.<package>` ([`mirror_app`]).
pub const MIRROR: &str = "amane-mirror";
/// Shader Lab.
pub const SHADER_LAB: &str = "amane-shader-lab";
/// Drop.
pub const DROP: &str = "amane-drop";
/// Drop's grid, a layer surface of its own.
pub const DROP_NS: &str = "amane.drop";

/// Every app with a window of its own, by its folder under `apps/`.
pub const APPS: [(&str, &str); 5] = [
    ("settings", SETTINGS),
    ("messages", MESSAGES),
    ("mirror", MIRROR),
    ("shader-lab", SHADER_LAB),
    ("drop", DROP),
];

/// The id of one of the phone's apps opened alone.
pub fn mirror_app(package: &str) -> String {
    format!("{MIRROR}.{package}")
}
