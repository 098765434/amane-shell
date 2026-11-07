//! NavButtons — the back/forward pill: two 28 px round buttons with
//! left and right chevrons, a 1×20 `fieldBorder` line between them, on a
//! `controlFill` capsule with a soft shadow. A disabled side greys its
//! chevron and takes no click.

use std::rc::Rc;

use crate::buttons::Actions;

use amane::{Center, Color, Radius, Rectangle, Row, Shadow, Size};

use crate::buttons::interactive;
use crate::layout::boxed;
use crate::look::hover::{crossfade, hover_fill};
use crate::style::{self, px};
use crate::symbol::{Symbol, symbol};

/// The back and forward pair of a window's toolbar, in one pill.
pub struct NavButtons {
    id: String,
    back_enabled: bool,
    forward_enabled: bool,
    on_back: Rc<dyn Fn()>,
    on_forward: Rc<dyn Fn()>,
}

impl NavButtons {
    /// Both halves disabled until said otherwise; `id` keys their hover.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            back_enabled: true,
            forward_enabled: false,
            on_back: Rc::new(|| {}),
            on_forward: Rc::new(|| {}),
        }
    }

    /// Whether there's somewhere to go back to.
    pub fn back_enabled(mut self, enabled: bool) -> Self {
        self.back_enabled = enabled;
        self
    }

    /// Whether there's somewhere to go forward to.
    pub fn forward_enabled(mut self, enabled: bool) -> Self {
        self.forward_enabled = enabled;
        self
    }

    /// What the back half does.
    pub fn on_back(mut self, back: impl Fn() + 'static) -> Self {
        self.on_back = Rc::new(back);
        self
    }

    /// What the forward half does.
    pub fn on_forward(mut self, forward: impl Fn() + 'static) -> Self {
        self.on_forward = Rc::new(forward);
        self
    }

    /// The pill.
    pub fn build(self) -> Rectangle {
        let side = px(28.0);
        let button = |key: String, glyph: Symbol, enabled: bool, press: Rc<dyn Fn()>| {
            let fg = crossfade(
                &format!("{key}.fg"),
                enabled,
                style::label_tertiary(),
                style::label_primary(),
            );
            let base = Rectangle::new()
                .width(side)
                .height(side)
                .radius(Radius::Full)
                .fill(hover_fill(&key, Color::TRANSPARENT, fg, !enabled))
                .align_child(Center, Center)
                .child(symbol(glyph, px(13.0), fg));
            // a disabled side stays drawn at full strength, only greyed
            if enabled {
                interactive(base, key, false, Actions::press(move || press()))
            } else {
                base
            }
        };

        let line = Rectangle::new()
            .width(Size::Fixed(1.0))
            .height(px(20.0))
            .fill(style::field_border());

        let row = Row::new(vec![
            boxed(button(
                format!("{}.back", self.id),
                Symbol::ChevronLeft,
                self.back_enabled,
                self.on_back,
            )),
            boxed(line),
            boxed(button(
                format!("{}.forward", self.id),
                Symbol::ChevronRight,
                self.forward_enabled,
                self.on_forward,
            )),
        ])
        .gap(1.0)
        .align(Center);

        Rectangle::new()
            .width(side * 2.0 + 3.0 + px(8.0))
            .height(px(36.0))
            .radius(Radius::Full)
            // opaque, so the shadow under it doesn't show through and grey it
            .fill(crate::color::over(
                style::control_fill(),
                crate::tokens::sys::color::window::bg(),
            ))
            .shadow(
                Shadow::drop(style::shadow_color())
                    .blur(3.09)
                    .spread(-0.29)
                    .offset(0.0, 0.5),
            )
            .align_child(Center, Center)
            .child(row)
    }
}
