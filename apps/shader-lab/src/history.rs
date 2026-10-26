//! Undo and redo over the lab's look. Changes to the same control close
//! together, like a slider being dragged, count as one step.

use std::time::{Duration, Instant};

use amane_kit::lab::config::Config;

/// Changes to one control this close together are one step.
const TOGETHER: Duration = Duration::from_millis(400);

/// The most steps kept.
const KEPT: usize = 100;

#[derive(Default)]
pub struct History {
    undo: Vec<Config>,
    redo: Vec<Config>,

    // the control changed last, and when
    last: Option<(String, Instant)>,
}

impl History {
    /// Keeps `before`, unless it's the same control as the last change and close to it.
    pub fn record(&mut self, key: &str, before: &Config) {
        let now = Instant::now();

        let same = self
            .last
            .as_ref()
            .is_some_and(|(last, at)| last == key && now.duration_since(*at) < TOGETHER);

        if !same {
            self.undo.push(before.clone());

            if self.undo.len() > KEPT {
                self.undo.remove(0);
            }

            self.redo.clear();
        }

        self.last = Some((key.to_owned(), now));
    }

    /// The look before `now`, if there's one; `now` can be had back with redo.
    pub fn undo(&mut self, now: &Config) -> Option<Config> {
        let before = self.undo.pop()?;

        self.redo.push(now.clone());
        self.last = None;

        Some(before)
    }

    pub fn redo(&mut self, now: &Config) -> Option<Config> {
        let after = self.redo.pop()?;

        self.undo.push(now.clone());
        self.last = None;

        Some(after)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(value: f32) -> Config {
        let mut config = Config::new("chrome");
        config.values.insert("depth".into(), value);
        config
    }

    #[test]
    fn a_dragged_control_is_one_step() {
        let mut history = History::default();

        // the slider moves 1 → 2 → 3, recording what was there before each move
        history.record("depth", &with(1.0));
        history.record("depth", &with(2.0));

        assert_eq!(history.undo(&with(3.0)), Some(with(1.0)));
        assert!(!history.can_undo());
    }

    #[test]
    fn another_control_is_another_step_and_redo_goes_forward() {
        let mut history = History::default();

        history.record("depth", &with(1.0));
        history.record("scale", &with(2.0));

        assert_eq!(history.undo(&with(3.0)), Some(with(2.0)));
        assert_eq!(history.undo(&with(2.0)), Some(with(1.0)));
        assert_eq!(history.redo(&with(1.0)), Some(with(2.0)));
        assert_eq!(history.redo(&with(2.0)), Some(with(3.0)));
        assert!(!history.can_redo());
    }

    #[test]
    fn a_new_change_drops_what_could_be_redone() {
        let mut history = History::default();

        history.record("depth", &with(1.0));
        history.undo(&with(2.0));
        history.record("scale", &with(1.0));

        assert!(!history.can_redo());
    }
}
