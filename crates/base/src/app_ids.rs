//! The programs' ids, written once: each names itself with its own (its
//! log, its window, its files), and the shell finds them by it (the niri
//! window rules, the phone's apps, Software Update's link).

/// The shell; also the folder its settings and state live in.
pub const SHELL: &str = "emaki";
/// The assistant's daemon.
pub const ASSISTANT: &str = "emaki-assistant";
/// The pinentry the shell answers for.
pub const PINENTRY: &str = "emaki-pinentry";
/// The train watcher.
pub const TRAIN_WATCH: &str = "emaki-train-watch";

/// Emaki Settings.
pub const SETTINGS: &str = "emaki-settings";
/// Messages, the phone's texts.
pub const MESSAGES: &str = "emaki-messages";
/// The phone's screen; each of the phone's apps opened alone is
/// `emaki-mirror.<package>` ([`mirror_app`]).
pub const MIRROR: &str = "emaki-mirror";
/// Shader Lab.
pub const SHADER_LAB: &str = "emaki-shader-lab";
/// Drop.
pub const DROP: &str = "emaki-drop";
/// Drop's grid, a layer surface of its own.
pub const DROP_NS: &str = "emaki.drop";

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
