//! Markers — services that hold nothing, for state kept elsewhere (a
//! component's `thread_local!`, a static) that some windows show.
//!
//! amane draws a window again when a service it read changes, or when
//! something calls `mark_all`, which draws every window on every monitor.
//! A marker scopes that: the views that show the state call `watch()`,
//! and whatever changes it calls `changed()`, so only those windows draw
//! again. Input needs neither: the window the pointer or keyboard is on
//! draws again for its input by itself.
//!
//! ```ignore
//! amane_kit::marker! {
//!     /// The launcher's own state changed.
//!     pub Launcher
//! }
//!
//! fn view() -> LayerWindow {
//!     Launcher::watch();
//!     // …
//! }
//!
//! fn picked() {
//!     STATE.with_borrow_mut(|state| state.picked = true);
//!     Launcher::changed();
//! }
//! ```
//!
//! See `docs/performance.md`.

/// Declares a marker: a service holding nothing, with `watch()` for the
/// views that show some state and `changed()` for what changes it.
#[macro_export]
macro_rules! marker {
    ($(#[$meta:meta])* $vis:vis $name:ident) => {
        $(#[$meta])*
        $vis struct $name;

        impl ::amane::Service for $name {
            fn new() -> Self {
                Self
            }

            // only ever changed by hand, so nothing is polled
            fn listen() {}
        }

        #[allow(dead_code)]
        impl $name {
            /// Called while drawing a window: it draws again when this
            /// is changed.
            $vis fn watch() {
                drop(<Self as ::amane::Service>::read());
            }

            /// Draws again every window that watches this, from any
            /// thread.
            $vis fn changed() {
                drop(<Self as ::amane::Service>::write());
            }
        }
    };
}
