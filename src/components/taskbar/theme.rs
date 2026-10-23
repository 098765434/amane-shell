pub use super::tokens::ThemeTokens as Theme;

use super::tokens::tokens;

impl Theme {
    /// The colour roles for the current mode.
    pub fn auto() -> Self {
        tokens().theme
    }
}
