//! Liquid glass — a look test, not shell code.
//!
//! Recreates the look of the "Жидкое стекло" CodePen cards with only what amane
//! already has, so we can judge it before anything goes into the shell: a
//! frosted backdrop, a faint tint, a bright rim lit from the top-left, and a
//! soft shadow. The pen's lens warp at the edges (an SVG displacement map) is
//! left out on purpose: amane shaders can't read what's behind them.
//!
//! The window draws its own wallpaper, since the blur only sees what's in the
//! window. Point it at another one with `GLASS_WALLPAPER=/path/to/image`.
//! `GLASS_THEME=dark` shows the dark version, over a dimmed wallpaper, and
//! `GLASS_THEME=dark-black` the same with black highlights.
//! The edge is lit by the shell's own shader, `src/components/glass.wgsl`,
//! with this test's stronger values; `GLASS_RIM=flat` swaps it for the pen's
//! plain 1px line, to compare. Run it from the repo root, the shader path is
//! relative to it.
//!
//! `GLASS_SHOW=wifi` swaps the cards for a copy of the bar's Wi-Fi panel: on
//! the left as it is today, on the right with the glass rim, glow and shadow.

use amane::{
    App, Center, Color, Column, End, Full, Image, LayerWindow, Mask, Padding, Parent, Rectangle,
    Row, Shadow, SpaceBetween, Stack, Text, Weight, Widget, children,
};

const WIDTH: f32 = 1150.0;
const HEIGHT: f32 = 600.0;

// the pen's card
const CARD: Shape = Shape {
    width: 320.0,
    height: 220.0,
    radius: 34.0,
};

// sys.size.popover.* and sys.radius.popover
const PANEL: Shape = Shape {
    width: 320.0,
    height: 380.0,
    radius: 10.0,
};

const WHITE: Color = Color::WHITE;

/// The numbers that change between light and dark.
struct Look {
    /// Laid over the wallpaper, to stand in for a dark one.
    dim: Color,
    tint: Color,
    /// The color of the glow, rim and bloom.
    highlight: Color,
    glow: f32,
    rim: f32,
    /// The light bounced back onto the bottom-right edge, against `rim`.
    rim_far: f32,
    /// The soft light just inside the lit edge.
    bloom: f32,
    shadow: f32,
    shadow_blur: f32,
    shadow_y: f32,
    text_shadow: Color,
    palette: &'static Palette,
}

/// The Wi-Fi panel's colors, copied from the design tokens for this theme.
struct Palette {
    /// `sys.color.surface.panel.bg`
    panel: Color,
    /// `sys.color.surface.panel.fg`
    fg: Color,
    /// `sys.color.content.secondary`
    secondary: Color,
    /// `sys.color.state.selected`
    selected: Color,
    /// `sys.color.surface.raised.bg`
    raised: Color,
}

const LIGHT_PALETTE: Palette = Palette {
    panel: Color::rgba(246, 246, 246, 153),
    fg: Color::rgba(0, 0, 0, 230),
    secondary: Color::rgba(0, 0, 0, 140),
    selected: Color::rgba(0, 0, 0, 36),
    raised: Color::rgba(0, 0, 0, 15),
};

const DARK_PALETTE: Palette = Palette {
    panel: Color::rgba(0, 0, 0, 140),
    fg: WHITE,
    secondary: Color::rgba(255, 255, 255, 102),
    selected: Color::rgba(255, 255, 255, 46),
    raised: Color::rgba(255, 255, 255, 20),
};

// the pen's own values
const LIGHT: Look = Look {
    dim: Color::TRANSPARENT,
    tint: Color::rgba(255, 255, 255, 38),
    highlight: WHITE,
    glow: 0.75,
    rim: 0.9,
    rim_far: 0.5,
    bloom: 0.3,
    shadow: 0.25,
    shadow_blur: 12.0,
    shadow_y: 6.0,
    text_shadow: Color::rgba(0, 0, 0, 64),
    palette: &LIGHT_PALETTE,
};

// a smoked tint, the rim and glow turned well down so they only catch a little light
const DARK: Look = Look {
    dim: Color::rgba(0, 0, 0, 115),
    // the panels' own tint, dark sys.color.surface.panel.bg (#0000008c)
    tint: Color::rgba(0, 0, 0, 140),
    highlight: WHITE,
    glow: 0.08,
    rim: 0.3,
    rim_far: 0.2,
    bloom: 0.05,
    shadow: 0.45,
    shadow_blur: 16.0,
    shadow_y: 8.0,
    text_shadow: Color::rgba(0, 0, 0, 110),
    palette: &DARK_PALETTE,
};

// a test: the dark glass with the highlights in black, so the edge reads as a
// shadowed bevel instead of a lit one; stronger, as black barely shows on black
const DARK_BLACK: Look = Look {
    highlight: Color::BLACK,
    glow: 0.4,
    rim: 0.8,
    rim_far: 0.5,
    bloom: 0.3,
    ..DARK
};

fn main() {
    App::new().window(view).run();
}

fn view() -> LayerWindow {
    let wallpaper = std::env::var("GLASS_WALLPAPER")
        .unwrap_or_else(|_| "../amane/examples/wallpaper.png".into());

    let look = match std::env::var("GLASS_THEME").as_deref() {
        Ok("dark") => &DARK,
        Ok("dark-black") => &DARK_BLACK,
        _ => &LIGHT,
    };

    // the panel as it is today, next to the same panel in glass
    let row = if std::env::var("GLASS_SHOW").as_deref() == Ok("wifi") {
        Row::new(children![plain_panel(look), glass_panel(look)]).gap(80.0)
    } else {
        Row::new(children![card(look), card(look), card(look)]).gap(42.0)
    };

    LayerWindow::new().width(WIDTH).height(HEIGHT).child(
        Rectangle::new()
            .width(WIDTH)
            .height(HEIGHT)
            .fill(Image::cover(wallpaper))
            .child(
                Rectangle::new()
                    .width(Parent)
                    .height(Parent)
                    .fill(look.dim)
                    .align_child(Center, Center)
                    .child(row),
            ),
    )
}

/// The size and corners every layer of one glass surface shares.
#[derive(Clone, Copy)]
struct Shape {
    width: f32,
    height: f32,
    radius: f32,
}

impl Shape {
    fn rectangle(self) -> Rectangle {
        Rectangle::new()
            .width(self.width)
            .height(self.height)
            .radius(self.radius)
    }
}

/// One card: shadow, glass, rim and label, bottom to top.
fn card(look: &Look) -> Stack {
    Stack::new(children![
        shadow(look, CARD),
        glass(look, CARD, 10.0, look.tint),
        rim(look, CARD),
        label(look.text_shadow).translate(0.0, 1.0),
        label(WHITE),
    ])
}

/*
 * box-shadow 0 6px 12px rgba(0,0,0,.25). A drop shadow is painted after the
 * blur, so on the glass itself it would darken it; it sits on its own layer
 * instead, with the card's shape cut out of it so only the outside shows
 */
fn shadow(look: &Look, shape: Shape) -> Rectangle {
    shape
        .rectangle()
        .shadow(
            Shadow::drop(Color::BLACK)
                .opacity(look.shadow)
                .blur(look.shadow_blur)
                .offset(0.0, look.shadow_y),
        )
        .child(
            Rectangle::new()
                .width(Parent)
                .height(Parent)
                .radius(shape.radius)
                .fill(Mask),
        )
}

// backdrop-filter blur, the tint and the inset 0 0 5px glow
fn glass(look: &Look, shape: Shape, blur: f32, tint: Color) -> Rectangle {
    shape
        .rectangle()
        .blur(blur)
        .fill(tint)
        .shadow(Shadow::inner(look.highlight).opacity(look.glow).blur(5.0))
}

fn rim(look: &Look, shape: Shape) -> Rectangle {
    let rim = shape.rectangle();

    // the pen's inset 1px 1px 0: a hard line along the top and left edges
    if std::env::var("GLASS_RIM").as_deref() == Ok("flat") {
        return rim.shadow(Shadow::inner(look.highlight).opacity(0.75).offset(1.0, 1.0));
    }

    let channel = |value: u8| f32::from(value) / 255.0;
    let color = look.highlight;

    rim.shader("src/components/glass.wgsl").shader_values(vec![
        [shape.radius, 1.5, look.rim, look.bloom],
        [14.0, look.rim_far, 0.0, 0.0],
        [
            channel(color.red()),
            channel(color.green()),
            channel(color.blue()),
            0.0,
        ],
    ])
}

// the label laid over the card, centered; drawn twice for its text shadow
fn label(color: Color) -> Rectangle {
    Rectangle::new()
        .width(CARD.width)
        .height(CARD.height)
        .align_child(Center, Center)
        .child(
            Text::new("Жидкое стекло")
                .size(22.0)
                .weight(Weight::SemiBold)
                .color(color),
        )
}

// ───── the Wi-Fi panel ───────────────────────────────────────────────────────
//
// A lookalike of `src/components/taskbar/wifi.rs` at comfortable density, with
// made-up networks: the shell is a binary, so its components can't be used
// from here. The real one is frosted by niri; here amane's blur stands in.

/// `sys.effect.backdrop-blur`
const BACKDROP_BLUR: f32 = 24.0;
/// `sys.space.inset.l`, `sys.space.gap.l`
const INSET: f32 = 10.0;
const ICON_FONT: &str = "Material Symbols Rounded";
const ICON_WIFI_LEVELS: [char; 5] = ['\u{ebe4}', '\u{ebd6}', '\u{ebe1}', '\u{e1ba}', '\u{e1d8}'];
const ICON_LOCK: char = '\u{e897}';
const ICON_CHECK: char = '\u{e5ca}';
const ICON_REFRESH: char = '\u{e5d5}';

// name, signal strength, secured, connected
const NETWORKS: [(&str, u8, bool, bool); 5] = [
    ("Home", 100, true, true),
    ("Home-5G", 80, true, false),
    ("Café Libre", 55, false, false),
    ("Neighbour", 35, true, false),
    ("Printer-Direct", 20, false, false),
];

fn plain_panel(look: &Look) -> Rectangle {
    PANEL
        .rectangle()
        .blur(BACKDROP_BLUR)
        .fill(look.palette.panel)
        .padding(INSET)
        .child(wifi(look.palette))
}

fn glass_panel(look: &Look) -> Stack {
    Stack::new(children![
        shadow(look, PANEL),
        glass(look, PANEL, BACKDROP_BLUR, look.palette.panel),
        rim(look, PANEL),
        PANEL.rectangle().padding(INSET).child(wifi(look.palette)),
    ])
}

fn wifi(palette: &Palette) -> Column {
    let rows = NETWORKS
        .iter()
        .map(|&(name, strength, secured, active)| {
            Box::new(network(palette, name, strength, secured, active)) as Box<dyn Widget>
        })
        .collect();

    Column::new(children![
        header(palette),
        Column::new(rows).gap(4.0).width(Parent)
    ])
    .gap(INSET)
    .width(Parent)
}

fn header(palette: &Palette) -> Row {
    let title = Row::new(children![
        icon(ICON_WIFI_LEVELS[4], palette.fg),
        Text::new("Wi-Fi")
            .size(13.0)
            .weight(Weight::Medium)
            .color(palette.fg),
    ])
    .gap(6.0)
    .align(Center);

    // the switch, on: sys.size.switch on sys.color.accent.bg
    let switch = Rectangle::new()
        .width(44.0)
        .height(22.0)
        .radius(Full)
        .fill(Color::rgb(10, 132, 255))
        .padding(2.0)
        .align_child(End, Center)
        .child(
            Rectangle::new()
                .width(18.0)
                .height(18.0)
                .radius(Full)
                .fill(WHITE),
        );

    let refresh = Rectangle::new()
        .width(28.0)
        .height(22.0)
        .radius(6.0)
        .fill(palette.raised)
        .align_child(Center, Center)
        .child(icon(ICON_REFRESH, palette.fg));

    Row::new(children![
        title,
        Row::new(children![switch, refresh]).gap(6.0).align(Center),
    ])
    .justify(SpaceBetween)
    .width(Parent)
    .align(Center)
}

fn network(palette: &Palette, name: &str, strength: u8, secured: bool, active: bool) -> Rectangle {
    let color = if active {
        palette.fg
    } else {
        palette.secondary
    };
    let fill = if active {
        palette.selected
    } else {
        palette.raised
    };

    let level = (usize::from(strength) * 5 + 50) / 100;
    let leading = Row::new(children![
        icon(ICON_WIFI_LEVELS[level.clamp(1, 5) - 1], color),
        Text::new(name).size(13.0).color(color),
    ])
    .gap(8.0)
    .align(Center);

    let mut trailing: Vec<Box<dyn Widget>> = Vec::new();
    if secured {
        trailing.push(Box::new(icon(ICON_LOCK, palette.secondary).size(12.0)));
    }
    if active {
        trailing.push(Box::new(icon(ICON_CHECK, palette.fg)));
    }

    Rectangle::new()
        .width(Parent)
        .height(40.0)
        .radius(6.0)
        .fill(fill)
        .padding(Padding {
            top: 0.0,
            right: INSET,
            bottom: 0.0,
            left: INSET,
        })
        .align_child(Center, Center)
        .child(
            Row::new(children![
                leading,
                Row::new(trailing).gap(6.0).align(Center)
            ])
            .justify(SpaceBetween)
            .width(Parent)
            .align(Center),
        )
}

fn icon(glyph: char, color: Color) -> Text {
    Text::new(glyph.to_string())
        .font(ICON_FONT)
        .size(16.0)
        .color(color)
}
