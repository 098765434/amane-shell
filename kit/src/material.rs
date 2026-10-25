//! Materials, as Apple's HIG has them: a piece of glass asks for the
//! material of its role (`Material::Control`, `Material::Sheet` …), never
//! for a colour, and the material decides the rest: which of niri's
//! materials frosts it, how thick its tint is, whether it turns light or
//! dark with its backdrop, and the vibrant labels, fills and separator
//! drawn on it. The glass setting and higher contrast change every
//! material at once, here.
//!
//! Two families, after Liquid Glass and Apple's standard materials:
//!
//! - **Glass** is the layer of controls and navigation floating over
//!   content, lensed by niri's `glass` material. Its Clear variant
//!   (`glass-clear`) is for media only, and dims over a bright backdrop.
//! - **Standard** materials frost without a lens (`frost`): the bar, tooltips
//!   and app windows.
//!
//! The thicker a material, the more opaque its tint: small glass is thin,
//! large glass that holds text is thick. The vibrant labels are one set for
//! every material, as Apple's are; the contrast floors check each against
//! each tint (`tokens::rules`). See `docs/materials.md`.

use std::cell::Cell;

use amane::{BlurShape, Color, Rectangle};

use crate::color::{faded, with_alpha};
use crate::glass;
use crate::glass_region::{self, see_through};
use crate::theme::{self, Mode};
use crate::tokens::{Key, sys, with_key};

/// What a piece of glass is for. Pick by role, not by how it looks: the
/// look changes with the settings (Apple: "choose materials by semantic
/// meaning").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Material {
    /// Small controls floating over content: Quick Settings modules, the
    /// dock. Turns light or dark with its backdrop.
    Control,
    /// A notification banner. Turns with its backdrop.
    Banner,
    /// The one primary action of a view, in stained glass of the accent.
    Prominent,
    /// A menu.
    Menu,
    /// A popover, dropdown, or a Quick Settings page.
    Popover,
    /// Large glass holding text: the launcher, the notification center, the
    /// Customize sheet.
    Sheet,
    /// A tooltip.
    ToolTip,
    /// The status bar: plain frost, no lens.
    Bar,
    /// Clear glass over media; dims over a bright backdrop.
    Media,
    /// An app window's background, frosting what's behind the window.
    Window,
    /// An app's sidebar.
    Sidebar,
    /// An app's header bar, frosting the page scrolling under it.
    Header,
    /// Grouped content inside a material: cards and lists.
    Content,
}

/// The two families of material.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// Liquid Glass: controls and navigation, lensed.
    Glass(Variant),
    /// Frost without a lens, inside the content layer.
    Standard,
}

/// Liquid Glass's variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// Blurs and evens out what's behind, for anything with text.
    Regular,
    /// Barely tinted, for media only.
    Clear,
}

/// How opaque a material's tint is, thinnest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Thickness {
    UltraThin,
    Thin,
    Regular,
    Thick,
    UltraThick,
    /// The status bar's.
    Bar,
}

/// What a material frosts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blending {
    /// What's behind the window: the compositor frosts it.
    BehindWindow,
    /// What the window itself drew below, such as a page scrolling under a
    /// header (amane's own canvas blur).
    WithinWindow,
}

/// What a [`Material`] is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spec {
    pub family: Family,
    pub thickness: Thickness,
    /// Turns light or dark with its backdrop, as Apple's small glass does.
    pub adaptive: bool,
    pub blending: Blending,
}

/// Label levels, most contrast first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Primary,
    Secondary,
    /// Inactive things, where legibility matters less.
    Tertiary,
    /// The least contrast. Not on thin or ultra-thin materials.
    Quaternary,
}

/// Fill levels, for controls and rows on a material.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillLevel {
    Primary,
    Secondary,
    Tertiary,
}

impl Thickness {
    /// The tint at this thickness, at the glass level
    /// ([`glass_region::level`]).
    pub fn tint(self) -> Color {
        glass_region::level(self.token())
    }

    /// The tint as the tokens have it.
    fn token(self) -> Color {
        match self {
            Self::UltraThin => sys::color::material::ultra_thin(),
            Self::Thin => sys::color::material::thin(),
            Self::Regular => sys::color::material::regular(),
            Self::Thick => sys::color::material::thick(),
            Self::UltraThick => sys::color::material::ultra_thick(),
            Self::Bar => sys::color::material::bar(),
        }
    }
}

// TODO(token): `sys.material.clear-fade` once Penpot has it.
/// How much of its tint Clear glass keeps over a backdrop that isn't
/// bright. Over a bright one it keeps all of it: Apple's 35% dimming layer
/// (the ultra-thin tint is darker still).
const CLEAR_FADE: f32 = 0.7;

thread_local! {
    // the material what's being built sits on
    static CURRENT: Cell<Option<Material>> = const { Cell::new(None) };
    // whether the Clear glass being built is over a bright backdrop
    static BRIGHT: Cell<bool> = const { Cell::new(false) };
}

/// Builds `f` on `material`: labels and fills inside read
/// [`current`] without the material being handed down.
pub fn scope<T>(material: Material, f: impl FnOnce() -> T) -> T {
    let before = CURRENT.replace(Some(material));
    let out = f();
    CURRENT.set(before);
    out
}

/// The material what's being built sits on, if it's inside a [`scope`].
pub fn current() -> Option<Material> {
    CURRENT.get()
}

impl Material {
    pub const ALL: [Material; 13] = [
        Self::Control,
        Self::Banner,
        Self::Prominent,
        Self::Menu,
        Self::Popover,
        Self::Sheet,
        Self::ToolTip,
        Self::Bar,
        Self::Media,
        Self::Window,
        Self::Sidebar,
        Self::Header,
        Self::Content,
    ];

    pub fn spec(self) -> Spec {
        use Blending::*;
        use Family::*;
        use Thickness::*;

        let (family, thickness, adaptive, blending) = match self {
            Self::Control | Self::Banner => (Glass(Variant::Regular), Thin, true, BehindWindow),
            // stained with the accent, which stands in for its thickness
            Self::Prominent => (Glass(Variant::Regular), Thin, false, BehindWindow),
            Self::Menu | Self::Popover => (Glass(Variant::Regular), Regular, false, BehindWindow),
            Self::Sheet => (Glass(Variant::Regular), Thick, false, BehindWindow),
            Self::ToolTip => (Standard, Thick, false, BehindWindow),
            Self::Bar => (Standard, Bar, false, BehindWindow),
            Self::Media => (Glass(Variant::Clear), UltraThin, false, BehindWindow),
            Self::Window => (Standard, Thick, false, BehindWindow),
            Self::Sidebar => (Standard, Regular, false, BehindWindow),
            Self::Header => (Standard, Regular, false, WithinWindow),
            Self::Content => (Standard, UltraThick, false, BehindWindow),
        };

        Spec {
            family,
            thickness,
            adaptive,
            blending,
        }
    }

    /// The niri material this one is drawn in, by name, as the generated
    /// config defines it (`material.*` tokens, `layer_rules`).
    pub fn niri_name(self) -> &'static str {
        match self.spec().family {
            Family::Glass(Variant::Regular) => "glass",
            Family::Glass(Variant::Clear) => "glass-clear",
            Family::Standard => "frost",
        }
    }

    /// The niri material asked for, by name, and its stand-ins: the one
    /// without the lens building up under reduced motion first, then
    /// liquid glass, which every config with materials has.
    fn wanted(self, still: bool) -> &'static [&'static str] {
        match (self.spec().family, still) {
            (Family::Glass(Variant::Regular), false) => &["glass"],
            (Family::Glass(Variant::Regular), true) => &["glass-still", "glass"],
            (Family::Glass(Variant::Clear), false) => &["glass-clear", "glass"],
            (Family::Glass(Variant::Clear), true) => {
                &["glass-clear-still", "glass-clear", "glass-still", "glass"]
            }
            (Family::Standard, false) => &["frost", "glass"],
            (Family::Standard, true) => &["frost-still", "frost", "glass-still", "glass"],
        }
    }

    /// The niri material that frosts this one, or none where the glass
    /// isn't see-through or the compositor has no materials.
    pub fn niri(self) -> Option<&'static str> {
        if !see_through() {
            return None;
        }
        self.wanted(theme::reduced_motion())
            .iter()
            .copied()
            .find(|name| amane::material_available(name))
    }

    /// Whether the compositor draws this material's tint and lit edge
    /// itself, with the frost: the surface then leaves both out.
    pub fn compositor_draws(self) -> bool {
        self.niri().is_some()
    }

    /// The tint, at the glass level.
    pub fn tint(self) -> Color {
        match self {
            Self::Prominent => glass_region::level(sys::color::accent::bg()),
            Self::Media => {
                let tint = with_key(Self::media_key(), || self.spec().thickness.tint());
                if !see_through() || BRIGHT.get() {
                    tint
                } else {
                    faded(tint, CLEAR_FADE)
                }
            }
            _ => self.spec().thickness.tint(),
        }
    }

    /// The frost behind a piece of this material drawn at (`x`, `y`),
    /// `width` by `height`, with `radius` corners, at `opacity`.
    pub fn shape(
        self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
        opacity: f32,
    ) -> BlurShape {
        self.shape_tinted(x, y, width, height, radius, opacity, self.tint())
    }

    /// [`shape`](Self::shape) with `tint` in place of the material's own,
    /// for glass turning from one material into another.
    #[allow(clippy::too_many_arguments)]
    pub fn shape_tinted(
        self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
        opacity: f32,
        tint: Color,
    ) -> BlurShape {
        let shape = BlurShape::new(x, y, width, height, radius).tint(tint);
        if !see_through() {
            return shape.alpha(0.0);
        }
        shape
            .alpha(opacity)
            .material(self.niri().unwrap_or(glass_region::MATERIAL))
    }

    /// What the glass drawn in the surface is filled with: the tint,
    /// unless the compositor lays it on with the frost.
    pub fn fill(self) -> Color {
        if self.compositor_draws() {
            Color::TRANSPARENT
        } else {
            self.tint()
        }
    }

    /// The hairline round a piece of this material, `stroke`: none where
    /// the compositor draws the edge, so it's back for solid glass and
    /// higher contrast.
    pub fn stroke(self, stroke: Color) -> Color {
        if self.compositor_draws() {
            Color::TRANSPARENT
        } else {
            stroke
        }
    }

    /// `surface`, a piece of this material with `radius` corners, with the
    /// lit edge the compositor would otherwise draw.
    pub fn lit(self, surface: Rectangle, radius: f32) -> Rectangle {
        if self.compositor_draws() {
            surface
        } else {
            glass::apply(surface, radius)
        }
    }

    /// Builds `f`, a piece of this material named by shape `id` and what's
    /// on it, inside its [`scope`]. Adaptive glass takes the mode its
    /// backdrop asks for (`glass_region::adapted`); Clear glass is always
    /// dark, with white labels, and dims over a bright backdrop.
    pub fn adapted<T>(self, id: u32, f: impl FnOnce() -> T) -> T {
        if self.spec().adaptive {
            return glass_region::adapted(id, || scope(self, f));
        }
        if self == Self::Media {
            let before = BRIGHT.replace(glass_region::bright(id));
            let out = with_key(Self::media_key(), || scope(self, f));
            BRIGHT.set(before);
            return out;
        }
        scope(self, f)
    }

    /// Clear glass is mode-less: dark, whatever the theme.
    fn media_key() -> Key {
        Key {
            mode: Mode::Dark,
            ..Key::current()
        }
    }

    /// A label on this material. Higher contrast makes every level
    /// stronger.
    pub fn label(self, level: Level) -> Color {
        debug_assert!(
            !(level == Level::Quaternary
                && matches!(
                    self.spec().thickness,
                    Thickness::UltraThin | Thickness::Thin
                )),
            "quaternary labels don't read on thin materials ({self:?})"
        );
        label(level)
    }

    /// A fill on this material: a control's track, a row, a chip.
    pub fn fill_role(self, level: FillLevel) -> Color {
        fill_role(level)
    }

    /// The separator between groups on this material.
    pub fn separator(self) -> Color {
        separator()
    }
}

/// A label on whatever material is current, or on none.
pub fn label(level: Level) -> Color {
    let color = match level {
        Level::Primary => sys::color::vibrant::label::primary(),
        Level::Secondary => sys::color::vibrant::label::secondary(),
        Level::Tertiary => sys::color::vibrant::label::tertiary(),
        Level::Quaternary => sys::color::vibrant::label::quaternary(),
    };
    if !theme::high_contrast() {
        return color;
    }
    // Apple's increased contrast: each level well clear of the next
    let floor = match level {
        Level::Primary => 1.0,
        Level::Secondary => 0.85,
        Level::Tertiary => 0.7,
        Level::Quaternary => 0.6,
    };
    let alpha = f32::from(color.alpha()) / 255.0;
    with_alpha(color, alpha.max(floor))
}

/// A fill on whatever material is current, or on none.
pub fn fill_role(level: FillLevel) -> Color {
    let color = match level {
        FillLevel::Primary => sys::color::vibrant::fill::primary(),
        FillLevel::Secondary => sys::color::vibrant::fill::secondary(),
        FillLevel::Tertiary => sys::color::vibrant::fill::tertiary(),
    };
    if theme::high_contrast() {
        // over an opaque material a fill has to carry the shape alone
        let alpha = f32::from(color.alpha()) / 255.0;
        with_alpha(color, (alpha * 2.0).min(1.0))
    } else {
        color
    }
}

/// The separator on whatever material is current, or on none.
pub fn separator() -> Color {
    let color = sys::color::vibrant::separator();
    if theme::high_contrast() {
        with_alpha(color, 0.4)
    } else {
        color
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Density;

    #[test]
    fn small_glass_is_thin_and_adapts_large_glass_is_thick() {
        for small in [Material::Control, Material::Banner] {
            assert_eq!(small.spec().thickness, Thickness::Thin);
            assert!(small.spec().adaptive);
        }
        assert_eq!(Material::Sheet.spec().thickness, Thickness::Thick);
        for m in Material::ALL {
            if !matches!(m, Material::Control | Material::Banner) {
                assert!(!m.spec().adaptive, "{m:?} shouldn't adapt");
            }
        }
    }

    #[test]
    fn standard_materials_frost_without_a_lens() {
        for m in [Material::Bar, Material::ToolTip, Material::Window] {
            assert_eq!(m.spec().family, Family::Standard);
            assert_eq!(m.wanted(false)[0], "frost");
        }
        assert_eq!(Material::Menu.wanted(false), &["glass"]);
        assert_eq!(Material::Media.wanted(false)[0], "glass-clear");
    }

    #[test]
    fn every_stand_in_ends_in_liquid_glass() {
        for m in Material::ALL {
            assert_eq!(m.wanted(false).last(), Some(&"glass"), "{m:?}");
            assert_eq!(m.wanted(false)[0], m.niri_name(), "{m:?}");
            assert_eq!(
                m.wanted(true)[0],
                format!("{}-still", m.niri_name()),
                "{m:?}"
            );
        }
    }

    #[test]
    fn only_the_header_blends_within_the_window() {
        for m in Material::ALL {
            assert_eq!(
                m.spec().blending == Blending::WithinWindow,
                m == Material::Header,
                "{m:?}"
            );
        }
    }

    #[test]
    fn thicker_is_more_opaque() {
        for mode in Mode::ALL {
            with_key(Key::new(mode, Density::Comfortable), || {
                let alphas: Vec<u8> = [
                    Thickness::UltraThin,
                    Thickness::Thin,
                    Thickness::Regular,
                    Thickness::Thick,
                    Thickness::UltraThick,
                ]
                .map(|t| t.token().alpha())
                .to_vec();
                assert!(alphas.is_sorted(), "{mode:?}: {alphas:?}");
            });
        }
    }

    #[test]
    fn clear_glass_is_dark_whatever_the_theme() {
        let light = Key::new(Mode::Light, Density::Comfortable);
        let (dark_tint, media_tint) = with_key(light, || {
            (
                with_key(Key::new(Mode::Dark, Density::Comfortable), || {
                    Thickness::UltraThin.token()
                }),
                with_key(Material::media_key(), || Thickness::UltraThin.token()),
            )
        });
        assert_eq!(dark_tint, media_tint);
    }

    #[test]
    fn scope_nests_and_restores() {
        assert_eq!(current(), None);
        scope(Material::Sheet, || {
            assert_eq!(current(), Some(Material::Sheet));
            scope(Material::Content, || {
                assert_eq!(current(), Some(Material::Content))
            });
            assert_eq!(current(), Some(Material::Sheet));
        });
        assert_eq!(current(), None);
    }

    #[test]
    #[should_panic(expected = "quaternary")]
    #[cfg(debug_assertions)]
    fn quaternary_is_refused_on_thin_glass() {
        with_key(Key::new(Mode::Dark, Density::Comfortable), || {
            Material::Control.label(Level::Quaternary)
        });
    }

    #[test]
    fn labels_step_down_in_contrast() {
        for mode in Mode::ALL {
            with_key(Key::new(mode, Density::Comfortable), || {
                let alphas = [
                    Level::Primary,
                    Level::Secondary,
                    Level::Tertiary,
                    Level::Quaternary,
                ]
                .map(|l| label(l).alpha());
                assert!(
                    alphas.windows(2).all(|w| w[0] > w[1]),
                    "{mode:?}: {alphas:?}"
                );
            });
        }
    }
}
