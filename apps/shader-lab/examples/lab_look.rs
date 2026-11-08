//! A look saved in the Shader Lab, put on a pill with one call: the
//! `dock-glass` config (apps/shader-lab/configs/dock-glass.json), over the
//! wallpaper so the glass has something to bend.
//!
//! `cargo run -p emaki-shader-lab --example lab_look`

use std::path::PathBuf;

use amane::{Center, Image, Rectangle, Size, Stack, Window};
use emaki_kit::app::{app, app_window};
use emaki_kit::lab;
use emaki_kit::layout::boxed;

const WIDTH: f32 = 330.0;
const HEIGHT: f32 = 105.0;

/// The lab's effects and saved configs, as the lab embeds them.
mod files {
    include!(concat!(env!("OUT_DIR"), "/lab_files.rs"));
}

fn main() {
    lab::register(files::FILES);
    app("emaki-kit-lab-look")
        .normal_window("main", window)
        .run();
}

fn window() -> Window {
    let pill = Rectangle::new()
        .width(WIDTH)
        .height(HEIGHT)
        .radius(HEIGHT / 2.0);

    // red if the look isn't saved, so a missing one shows
    let styled: Stack = match lab::look("dock-glass", WIDTH, HEIGHT, HEIGHT / 2.0) {
        Some(look) => look.apply(pill),
        None => Stack::new(vec![boxed(pill.fill("#ff000080"))]),
    };

    let wallpaper = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("Pictures/overview.jpg");

    // the clip holds the wallpaper too, so the glass reads it as its backdrop
    let content = Rectangle::new()
        .width(Size::Parent)
        .height(Size::Parent)
        .clip()
        .child(
            Rectangle::new()
                .width(Size::Parent)
                .height(Size::Parent)
                .fill(Image::cover(wallpaper))
                .align_child(Center, Center)
                .child(styled),
        );

    app_window(content).title("A saved look").size(700.0, 440.0)
}
