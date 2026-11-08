//! How the dock draws: the card, the highlights, the icons and their tooltip, the frost.

use super::*;
use emaki_kit::reveal;

#[allow(clippy::too_many_arguments)]
pub(super) fn build(
    dock: &mut Dock,
    output: &str,
    sections: &[Section],
    entries: Vec<Entry>,
    spaces: &[Space],
    held: Option<Held>,
    hold: f32,
    at: Measures,
    motion: &DockMotion,
) -> View {
    let menu_open = context_menu::is_open_under(DOCK_NS)
        && MENU_ON.with_borrow(|on| on.as_deref() == Some(output));
    // a menu open, an icon held or files held keep it up wherever the
    // pointer goes
    dock.reveal.keep_out(
        menu_open || dock.drag.is_some() || dock.files.is_some() || !dock_settings().auto_hide,
    );
    let progress = dock.reveal.progress(hold);
    let out = reveal::showing(dock.reveal.wanted(), progress);
    if out && !dock.out {
        dock.swell = anim(&swell_anim(motion), 0.0);
    }
    dock.out = out;

    let laid = lay_out(dock, entries, at, motion, !out);
    spring_load(dock, &laid, spaces, motion);
    let sunk = at.sunk(progress);
    let top = at.top + sunk;

    // where the pointer is along the row at rest, while it's over the card
    // or the icons swelling above it
    // or nowhere while it holds an icon, which needs the row still
    let pointing = dock.pointer.filter(|_| held.is_none()).and_then(|(x, y)| {
        let over = y >= at.top - at.headroom && y <= at.top + at.card && progress > 0.5;
        over.then(|| along(at, laid.width, x))
    });
    dock.swell.to(if pointing.is_some() { 1.0 } else { 0.0 });
    if pointing.is_some() {
        dock.last_at = pointing;
    }
    let strength = dock.swell.value();
    let pointer_at = pointing.or(dock.last_at);

    let swollen: Swollen = magnify::swell(
        &laid.rests,
        at.side,
        at.pitch,
        pointer_at,
        strength,
        motion.magnify.as_ref(),
    );
    let card_x = at.card_left(laid.width) - swollen.before;
    let card_w = laid.width + swollen.before + swollen.after;
    let start = at.card_left(laid.width) + at.pad;

    let holding = held.is_some();
    let mut layers: Vec<Box<dyn Widget>> =
        vec![boxed(still_layer(output, &laid, spaces, at, holding))];
    let mut frost = Vec::new();

    if progress > 0.001 {
        // the card's glass
        let glass = Rectangle::new()
            .width(card_w.max(1.0))
            .height(at.card)
            .radius(RADIUS)
            .fill(Material::Control.fill());
        layers.push(place(
            Material::Control.lit(glass, RADIUS),
            card_x,
            top,
            progress,
        ));
        frost.push(
            Material::Control
                .shape(card_x, top, card_w, at.card, RADIUS, 1.0)
                .id(glass_id(output)),
        );

        // the workspace showing, lit behind its icons
        if let Some(active) = sections.iter().position(|section| section.active) {
            let span = laid
                .entries
                .iter()
                .zip(&swollen.icons)
                .filter(|(entry, _)| entry.section == active && entry.pointable())
                .fold(None, |span: Option<(f32, f32)>, (_, icon)| {
                    let (x0, x1) = (icon.x, icon.x + icon.side.max(at.side));
                    Some(span.map_or((x0, x1), |(a, b)| (a.min(x0), b.max(x1))))
                });
            if let Some((x0, x1)) = span {
                let inset = at.pad / 2.0;
                layers.push(place(
                    Rectangle::new()
                        .width(x1 - x0 + inset * 2.0)
                        .height(at.card - inset * 2.0)
                        .radius(RADIUS - inset)
                        .fill(sys::color::state::hover()),
                    start + x0 - inset,
                    top + inset,
                    progress,
                ));
            }
        }

        // the icon held files would open with, lit behind
        let opening = dock
            .files
            .as_ref()
            .filter(|files| files.opens)
            .and_then(|files| files.icon.as_ref())
            .and_then(|(key, _)| laid.entries.iter().position(|entry| entry.key == *key));
        if let Some(index) = opening {
            let icon = swollen.icons[index];
            let inset = at.pad / 2.0;
            layers.push(place(
                Rectangle::new()
                    .width(icon.side + inset * 2.0)
                    .height(icon.side + inset * 2.0)
                    .radius(icon.side / 4.0 + inset)
                    .fill(sys::color::state::selected()),
                start + icon.x - inset,
                top + at.pad + at.side - icon.side - icon.lift - inset,
                progress,
            ));
        }

        // the icons, the rules and the empty places
        let mut icons = Vec::new();
        for ((entry, icon), shown) in laid.entries.iter().zip(&swollen.icons).zip(&laid.shown) {
            let x = start + icon.x;
            match &entry.kind {
                // the held icon leaves its place empty
                Kind::Icon(_) if held.as_ref().is_some_and(|held| held.key == entry.key) => {}
                Kind::Icon(app) => icons.extend(icon_layers(app, icon, x, top, at, progress)),
                Kind::Rule => icons.push(place(
                    Divider::vertical(at.side),
                    x + sys::space::gap::s() - at.gap,
                    top + at.pad,
                    progress * shown,
                )),
                Kind::Empty if icon.side >= 1.0 => icons.push(place(
                    Rectangle::new()
                        .width(icon.side)
                        .height(icon.side)
                        .radius(icon.side / 4.0)
                        .fill(sys::color::state::hover()),
                    x,
                    top + at.pad + at.side - icon.side - icon.lift,
                    progress,
                )),
                Kind::Empty => {}
            }
        }
        // the icons slide down under the card's bottom edge and out
        layers.push(boxed(
            Rectangle::new()
                .width(Parent)
                .height((top + at.card).max(1.0))
                .align_child(Start, Start)
                .clip()
                .child(Stack::new(icons).width(Parent).height(Parent)),
        ));

        // the held icon, under the pointer; a pinned app pulled off fades,
        // as it comes out of the dock when let go there
        if let Some(held) = &held {
            let faded = if held.off && matches!(held.icon, Icon::Pin(_)) {
                0.5
            } else {
                1.0
            };
            layers.push(place(
                item::icon(&held.icon, at.side),
                held.x,
                held.y,
                progress * faded,
            ));
        }

        // the name of the icon under the pointer
        let under = pointing
            .and_then(|along| laid.under(along))
            .and_then(|index| match &laid.entries[index].kind {
                Kind::Icon(icon) => Some((index, icon)),
                _ => None,
            });
        match (motion.tooltip, under) {
            (Some(tooltip), Some((index, icon))) => {
                let key = &laid.entries[index].key;
                if dock.tip_for.as_ref() != Some(key) {
                    dock.tip_for = Some(key.clone());
                    dock.tip = anim(&tooltip, 0.0);
                }
                dock.tip.to(1.0);
                let placed = swollen.icons[index];
                let (label, width, height) = item::label(&item::name(icon));
                let x = (start + placed.x + placed.side / 2.0 - width / 2.0)
                    .clamp(0.0, (at.width - width).max(0.0));
                let y = top + at.pad + at.side
                    - placed.side
                    - placed.lift
                    - sys::space::gap::s()
                    - height;
                let shown = dock.tip.value() * progress;
                frost.push(Material::Control.shape(
                    x,
                    y,
                    width,
                    height,
                    sys::radius::raised(),
                    shown,
                ));
                layers.push(place(label, x, y, shown));
            }
            _ => dock.tip_for = None,
        }
    }

    // the frost comes and goes with the card, where the compositor can
    // fade it; elsewhere it shows whole while any of the card does
    let frost = if progress <= 0.001 {
        Vec::new()
    } else if amane::blur_fades() {
        glass_region::faded(frost, progress)
    } else {
        frost
    };

    let shown = InputArea {
        x: card_x.floor() as i32,
        y: 0,
        width: card_w.ceil().max(1.0) as i32,
        height: at.height as i32,
    };
    let input = dock
        .reveal
        .input_region(at.width, at.height, STRIP, progress, shown);

    View {
        root: Stack::new(layers).width(Parent).height(Parent),
        frost,
        input,
        tucked: progress <= 0.001 && !dock.reveal.wanted(),
    }
}

/// The swell's motion, or one that's never used without magnification.
pub(super) fn swell_anim(motion: &DockMotion) -> motion::Anim {
    motion
        .magnify
        .map_or(motion.reveal, |magnify| magnify.hover)
}

/// One icon as drawn, with its running dot: `x` is its left edge on the
/// surface, `top` the card's top, `shown` how much of it shows.
pub(super) fn icon_layers(
    app: &Icon,
    icon: &magnify::Placed,
    x: f32,
    top: f32,
    at: Measures,
    shown: f32,
) -> Vec<Box<dyn Widget>> {
    if icon.side < 1.0 {
        return Vec::new();
    }
    // swollen icons grow up from the card's bottom padding
    let y = top + at.pad + at.side - icon.side - icon.lift;
    let mut layers = vec![place(item::icon(app, icon.side), x, y, shown)];

    let (running, width) = item::running(app);
    if width > 0.0 {
        let dot_y = top + at.card - at.pad / 2.0 - item::RUNNING / 2.0;
        layers.push(place(
            item::frame(width, item::RUNNING).child(Row::new(vec![running])),
            x + icon.side / 2.0 - width / 2.0,
            dot_y,
            shown,
        ));
    }
    layers
}

/// The id of monitor `output`'s dock glass, for niri's readings of its
/// backdrop.
pub(super) fn glass_id(output: &str) -> u32 {
    glass_region::shape_id(&format!("emaki.dock.{output}"))
}
