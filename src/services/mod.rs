//! The shell's own services: system state Quick Settings shows that the
//! amane library has no service for. Each is an amane [`Service`] like the
//! library's — live state a view reads, redrawing only when it changes, and
//! a sane "not present" when its daemon or device is missing — so a tile
//! reads it the same way it reads `Network` or `Bluetooth`.
//!
//! Power (`power/`):
//!
//! - [`power::session`] — lock, suspend, restart, power off and log out (logind).
//! - [`power::keep_awake`] — the screen held on and the lid ignored (inhibitors).
//! - [`power::keyboard_backlight`] — the keyboard's light (sysfs and logind).
//! - [`power::power_refresh`] — the displays' slowest refresh rate on battery.
//!
//! Network and radios (`network/`):
//!
//! - [`network::connections`] — wired, VPN and hotspot (NetworkManager's `nmcli`).
//! - [`network::rfkill`] — airplane mode: every radio blocked.
//! - [`network::tailscale`] — the tailnet, its exit nodes and settings (the CLI).
//! - [`network::bt_battery`] — a notification when a Bluetooth device runs low.
//!
//! Display (`display/`):
//!
//! - [`display::night_light`] — a warm screen, by running `wlsunset`.
//! - [`display::dark_style`] — the system's light/dark preference (dconf).
//!
//! The desktop (`desktop/`):
//!
//! - [`desktop::privacy`] — what's using the microphone, the camera or the screen.
//! - [`desktop::background_apps`] — apps running without a window (the portal).
//! - [`desktop::lock_keys`] — whether Caps Lock and Num Lock are on (the keyboards' LEDs).
//! - [`desktop::updates`] — what the last NixOS update check found.
//! - [`desktop::system_stats`] — processor, memory, network, temperature and
//!   disks, and their recent history, for the desktop's widgets.
//!
//! The launcher and the bar (`launcher/`):
//!
//! - [`launcher::clock`] — the wall-clock time and date the bar shows.
//! - [`launcher::timer`] — the launcher's countdown, which the bar shows while it runs.
//! - [`launcher::clipboard_history`] — what's been copied, for the launcher's
//!   Clipboard (wl-clipboard), once it's turned on.
//! - [`launcher::file_search`] — files by name and content for the launcher's
//!   Files (LocalSearch's index, or `fd`).
//!
//! Shared helpers: [`state_file`] and [`save_state`] (`state.rs`), and
//! [`run`], [`run_now`], [`installed`] and [`Latest`] (`process.rs`).
//!
//! See `docs/shell/quick-settings.md` and `docs/shell/widgets.md`.
//!
//! [`Service`]: amane::Service

pub mod desktop;
pub mod display;
pub mod launcher;
pub mod network;
pub mod power;

mod process;
mod state;

pub use process::{Latest, installed, run, run_now};
pub use state::{save_state, state_file};
