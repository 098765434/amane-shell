//! Menus: one item model ([`MenuItem`] in a [`ContextMenu`]), one drawing of
//! its rows ([`rows`]), one keyboard (`keys.rs`), and two hosts:
//!
//! - the popup host (`popup.rs`), for app windows: the menu a dropdown and a pop-up button
//!   open, and [`context_menu`], an app's right-click menu. Its own panel
//!   and shadow.
//! - the layer host ([`layer`]), for the shell: a glass card on a layer surface of its own,
//!   with submenus.
//!
//! The pointer, or Up, Down and Enter, picks a row; Right and Left go into
//! and out of a submenu; Escape or a click outside closes it.

mod keys;
pub mod layer;
mod model;
mod popup;
pub mod rows;
mod stack;

pub use model::{Check, ContextMenu, Entry, Icon, MenuItem};
pub use popup::{Parent, context_menu};
pub(crate) use popup::{register, register_on, toggle, track};
