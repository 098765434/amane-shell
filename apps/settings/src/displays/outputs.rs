//! niri's outputs as the page sees them, through amane's niri client: what
//! each one runs at, and the changes niri makes at once but keeps only until
//! its config is read again. What only this page asks of an output (its
//! name in the config, whether it's built in, its size, a turn's label) is
//! here.

use std::collections::BTreeMap;

pub use amane::niri::{Mode, Output, OutputChange as Change, Transform};

/// What the page asks of an output beyond what niri says.
pub trait OutputExt {
    /// The name niri matches a config block by when the connector could
    /// change: `make model serial`.
    fn identity(&self) -> String;
    /// A built-in panel: a laptop's, on an embedded connector.
    fn built_in(&self) -> bool;
    /// The diagonal in inches, when the size is known.
    fn inches(&self) -> Option<f32>;
}

impl OutputExt for Output {
    fn identity(&self) -> String {
        format!(
            "{} {} {}",
            self.make,
            self.model,
            self.serial.as_deref().unwrap_or("Unknown")
        )
    }

    fn built_in(&self) -> bool {
        ["eDP", "LVDS", "DSI"]
            .iter()
            .any(|kind| self.name.starts_with(kind))
    }

    fn inches(&self) -> Option<f32> {
        let (w, h) = self.physical_size.filter(|(w, h)| *w > 0 && *h > 0)?;
        Some((w as f32).hypot(h as f32) / 25.4)
    }
}

/// A turn's words on the page and in the config.
pub trait TransformExt {
    /// Its label in the Rotation menu.
    fn label(self) -> &'static str;
    /// Its name in the config (`flipped-90`).
    fn config(self) -> &'static str;
}

impl TransformExt for Transform {
    fn label(self) -> &'static str {
        match self {
            Transform::Normal => "Standard",
            Transform::_90 => "90°",
            Transform::_180 => "180°",
            Transform::_270 => "270°",
            Transform::Flipped => "Flipped",
            Transform::Flipped90 => "Flipped, 90°",
            Transform::Flipped180 => "Flipped, 180°",
            Transform::Flipped270 => "Flipped, 270°",
        }
    }

    fn config(self) -> &'static str {
        match self {
            Transform::Normal => "normal",
            Transform::_90 => "90",
            Transform::_180 => "180",
            Transform::_270 => "270",
            Transform::Flipped => "flipped",
            Transform::Flipped90 => "flipped-90",
            Transform::Flipped180 => "flipped-180",
            Transform::Flipped270 => "flipped-270",
        }
    }
}

/// Every output niri knows, by connector; `None` when niri isn't answering.
pub fn all() -> Option<BTreeMap<String, Output>> {
    match amane::niri::outputs() {
        Ok(list) => Some(
            list.into_iter()
                .map(|output| (output.name.clone(), output))
                .collect(),
        ),
        Err(error) => {
            log::warn!("niri's outputs: {error}");
            None
        }
    }
}

/// The app ids of the windows open now, each once, sorted.
pub fn window_app_ids() -> Vec<String> {
    let windows = amane::niri::windows().unwrap_or_else(|error| {
        log::warn!("niri's windows: {error}");
        Vec::new()
    });
    app_ids(windows.into_iter().filter_map(|window| window.app_id))
}

fn app_ids(ids: impl Iterator<Item = String>) -> Vec<String> {
    let mut ids: Vec<String> = ids.filter(|id| !id.is_empty()).collect();
    ids.sort();
    ids.dedup();
    ids
}

/// Makes `change` to the output named `output` until the config is read
/// again.
pub fn apply(output: &str, change: &Change) -> Result<(), String> {
    amane::niri::set_output(output, *change).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_windows_give_their_app_ids_once() {
        let ids = ["steam_app_1", "firefox", "firefox", ""].map(String::from);
        assert_eq!(app_ids(ids.into_iter()), ["firefox", "steam_app_1"]);
    }

    #[test]
    fn the_page_reads_an_output() {
        let panel = Output {
            name: String::from("eDP-1"),
            make: String::from("Lenovo Group Limited"),
            model: String::from("ATNA40HQ10-0 "),
            serial: None,
            physical_size: Some((300, 190)),
            logical: None,
            hdr_supported: false,
            hdr_enabled: false,
            sdr_brightness: None,
            peak_luminance: None,
            modes: Vec::new(),
            current_mode: None,
            is_custom_mode: false,
            vrr_supported: true,
            vrr_enabled: false,
        };
        assert!(panel.built_in());
        assert_eq!(panel.inches().unwrap().round(), 14.0);
        assert_eq!(
            panel.identity(),
            "Lenovo Group Limited ATNA40HQ10-0  Unknown"
        );
        assert_eq!(Transform::Flipped90.config(), "flipped-90");
    }
}
