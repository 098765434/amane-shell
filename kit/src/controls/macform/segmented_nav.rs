//! MacSegmentedNav — the back/forward pill: two 28 px round buttons with
//! left and right chevrons, a 1×20 `fieldBorder` line between them, on a
//! `controlFill` capsule with a soft shadow. A disabled side greys its
//! chevron and takes no click.

use std::rc::Rc;

use amane::{Center, Color, Radius, Rectangle, Row, Shadow, Size};

use super::symbol::{Symbol, symbol};
use super::theme::{self, px};
use super::{crossfade, hover_fill, on_press};
use crate::buttons::interactive;
use crate::layout::boxed;

pub struct MacSegmentedNav {
    id: String,
    back_enabled: bool,
    forward_enabled: bool,
    on_back: Rc<dyn Fn()>,
    on_forward: Rc<dyn Fn()>,
}

impl MacSegmentedNav {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            back_enabled: true,
            forward_enabled: false,
            on_back: Rc::new(|| {}),
            on_forward: Rc::new(|| {}),
        }
    }

    pub fn back_enabled(mut self, enabled: bool) -> Self {
        self.back_enabled = enabled;
        self
    }

    pub fn forward_enabled(mut self, enabled: bool) -> Self {
        self.forward_enabled = enabled;
        self
    }

    pub fn on_back(mut self, back: impl Fn() + 'static) -> Self {
        self.on_back = Rc::new(back);
        self
    }

    pub fn on_forward(mut self, forward: impl Fn() + 'static) -> Self {
        self.on_forward = Rc::new(forward);
        self
    }

    pub fn build(self) -> Rectangle {
        let side = px(28.0);
        let button = |key: String, glyph: Symbol, enabled: bool, press: Rc<dyn Fn()>| {
            let fg = crossfade(
                &format!("{key}.fg"),
                enabled,
                theme::label_tertiary(),
                theme::label_primary(),
            );
            let base = Rectangle::new()
                .width(Size::Fixed(side))
                .height(Size::Fixed(side))
                .radius(Radius::Full)
                .fill(hover_fill(&key, Color::TRANSPARENT, fg, !enabled))
                .align_child(Center, Center)
                .child(symbol(glyph, px(13.0), fg));
            // a disabled side stays drawn at full strength, only greyed
            interactive(base, key, !enabled, on_press(move || press())).opacity(1.0)
        };

        let line = Rectangle::new()
            .width(Size::Fixed(1.0))
            .height(Size::Fixed(px(20.0)))
            .fill(theme::field_border());

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
            .width(Size::Fixed(side * 2.0 + 3.0 + px(8.0)))
            .height(Size::Fixed(px(36.0)))
            .radius(Radius::Full)
            // opaque, so the shadow under it doesn't show through and grey it
            .fill(crate::color::over(
                theme::control_fill(),
                crate::tokens::sys::color::window::bg(),
            ))
            .shadow(
                Shadow::drop(theme::shadow_color())
                    .blur(3.09)
                    .spread(-0.29)
                    .offset(0.0, 0.5),
            )
            .align_child(Center, Center)
            .child(row)
    }
}
