//! The tiles and their pages, one file per setting. Each file has the
//! grid's cell for it (`None` when this machine hasn't got it) and the page
//! its tile opens.

pub mod airplane;
pub mod appearance;
pub mod background;
pub mod bluetooth;
pub mod keyboard;
pub mod media;
pub mod network;
pub mod notifications;
pub mod power;
pub mod session;
pub mod sound;

use amane::{Column, Widget};

use super::detail::DetailPage;
use super::grid::{Cell, grid as pack};
use super::system_row;
use crate::components::taskbar::close;
use crate::services::installed;

pub(super) type GridCell = Cell<Box<dyn Widget>>;

/// The grid, in GNOME's order, and its height.
pub(super) fn grid() -> (Column, f32) {
    let cells: Vec<Option<GridCell>> = vec![
        Some(system_row::cell()),
        media::cell(),
        sound::output_cell(),
        sound::input_cell(),
        appearance::brightness_cell(),
        network::wired_cell(),
        network::wifi_cell(),
        network::vpn_cell(),
        bluetooth::cell(),
        power::mode_cell(),
        appearance::night_light_cell(),
        appearance::dark_style_cell(),
        notifications::cell(),
        keyboard::cell(),
        airplane::cell(),
        background::cell(),
    ];
    pack(cells.into_iter().flatten().collect())
}

/// `page` with a last row opening `program`, the app that has the rest of
/// its settings, when that's installed.
pub(super) fn settings_link(
    page: DetailPage,
    label: &'static str,
    program: &'static str,
) -> DetailPage {
    if !installed(program) {
        return page;
    }
    page.footer(label, move || {
        close();
        amane::spawn(program);
    })
}

/// "On" or "Off", the subtitle of a tile with nothing more to say.
pub(super) fn on_off(on: bool) -> &'static str {
    if on { "On" } else { "Off" }
}
