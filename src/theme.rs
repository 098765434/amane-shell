//! The runtime inputs the token system may vary on top of Penpot's values
//! (manual §2): light/dark mode and density select theme sets.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Mode {
    Light,
    Dark,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Density {
    Comfortable,
    Compact,
}

impl Mode {
    pub const ALL: [Mode; 2] = [Mode::Light, Mode::Dark];

    /// The theme name in Penpot's `mode` group.
    pub fn theme(self) -> &'static str {
        match self {
            Mode::Light => "light",
            Mode::Dark => "dark",
        }
    }
}

impl Density {
    pub const ALL: [Density; 2] = [Density::Comfortable, Density::Compact];

    /// The theme name in Penpot's `density` group.
    pub fn theme(self) -> &'static str {
        match self {
            Density::Comfortable => "comfortable",
            Density::Compact => "compact",
        }
    }
}
