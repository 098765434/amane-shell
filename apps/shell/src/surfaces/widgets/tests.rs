use super::*;

#[test]
fn a_screen_is_whole_cells_inside_the_margin_and_over_the_bar() {
    let geometry = Geometry::of(1920.0, 1080.0);
    let px = metrics();
    // the bar is wherever the settings put it, top or bottom
    let bar = bar::height();
    let (above, below) = match bar::edge() {
        Vertical::Top => (bar, 0.0),
        _ => (0.0, bar),
    };
    assert!(px.extent(geometry.canvas.cols) <= 1920.0 - 2.0 * margin());
    assert!(px.extent(geometry.canvas.cols + 1) > 1920.0 - 2.0 * margin());
    assert!(geometry.origin.1 >= above + margin() - 0.01);
    assert!(
        geometry.origin.1 + px.extent(geometry.canvas.rows) <= 1080.0 - below - margin() + 0.01
    );
    // centred: as much room left as right
    let right = 1920.0 - geometry.origin.0 - px.extent(geometry.canvas.cols);
    assert!((right - geometry.origin.0).abs() < 0.01);
}

#[test]
fn the_background_surface_is_the_cards_and_their_shadows_within_the_screen() {
    let geometry = Geometry::of(1920.0, 1080.0);
    let px = metrics();
    // two cards: a 2×2 at the top left and a 4×2 further along
    let extent = (0.0, 0.0, 5.0 * px.pitch() + px.extent(4), px.extent(2));
    let on_surface = geometry.on_surface_around(extent);
    let (left, top, width, height) = on_surface.surface;
    assert!(left <= geometry.origin.0 && top <= geometry.origin.1);
    assert!(left + width >= geometry.origin.0 + extent.2);
    assert!(top + height >= geometry.origin.1 + extent.3);
    assert!(left >= 0.0 && top >= 0.0 && left + width <= 1920.0 && top + height <= 1080.0);
    // far smaller than the screen
    assert!(width < 1920.0 / 2.0 && height < 1080.0 / 3.0);
    // the same point on the grid, whichever surface it is measured on
    let screen = Point { x: 500.0, y: 300.0 };
    let surface = Point {
        x: screen.x - left,
        y: screen.y - top,
    };
    assert_eq!(geometry.grid(screen), on_surface.grid(surface));
    assert_eq!((on_surface.width, on_surface.height), (width, height));
    // the whole canvas, while a card is dragged, stays within the screen too
    let whole = geometry.on_surface_around(geometry.whole());
    let (l, t, w, h) = whole.surface;
    assert!(l >= 0.0 && t >= 0.0 && l + w <= 1920.0 && t + h <= 1080.0);
    assert!(w > width && h > height);
}
