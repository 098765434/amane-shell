//! The grid's geometry: `cols` columns by `rows` rows of square cells with
//! a gap between them, and items that span whole cells. What spans and
//! places are, how they map to pixels, and which cells are taken. The
//! sizes come from whoever owns the grid (Quick Settings' tokens, or the
//! desktop's), so this reads no tokens.
//!
//! amane has no grid widget (on purpose: its layout stays rows, columns and
//! stacks), so the grid is drawn as a stack of translated rectangles.

/// How many cells a control covers, across and down.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    pub cols: u8,
    pub rows: u8,
}

impl Span {
    pub const fn new(cols: u8, rows: u8) -> Self {
        Self { cols, rows }
    }

    pub fn cells(self) -> u16 {
        u16::from(self.cols) * u16::from(self.rows)
    }
}

/// A cell on the canvas, from the top-left.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Place {
    pub col: u8,
    pub row: u8,
}

impl Place {
    pub const fn new(col: u8, row: u8) -> Self {
        Self { col, row }
    }
}

/// The cells a control covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub at: Place,
    pub span: Span,
}

impl Rect {
    pub const fn new(at: Place, span: Span) -> Self {
        Self { at, span }
    }

    fn right(&self) -> u8 {
        self.at.col + self.span.cols
    }

    fn bottom(&self) -> u8 {
        self.at.row + self.span.rows
    }

    /// Every cell it covers, row by row.
    pub fn cells(&self) -> impl Iterator<Item = Place> + '_ {
        (self.at.row..self.bottom())
            .flat_map(move |row| (self.at.col..self.right()).map(move |col| Place { col, row }))
    }
}

/// The canvas: how many cells across and down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Canvas {
    pub cols: u8,
    pub rows: u8,
}

impl Canvas {
    /// The most whole cells of `px` that fit in `width` by `height`.
    pub fn fit(px: &Metrics, width: f32, height: f32) -> Self {
        let n = |side: f32| ((side + px.gap) / px.pitch()).floor().clamp(0.0, 255.0) as u8;
        Self {
            cols: n(width),
            rows: n(height),
        }
    }

    pub fn fits(&self, rect: &Rect) -> bool {
        rect.right() <= self.cols && rect.bottom() <= self.rows
    }

    /// `span` placed so its top-left is nearest (`col`, `row`) and it fits.
    pub fn clamp(&self, col: i32, row: i32, span: Span) -> Place {
        let col = col.clamp(0, i32::from(self.cols.saturating_sub(span.cols)));
        let row = row.clamp(0, i32::from(self.rows.saturating_sub(span.rows)));
        Place {
            col: col as u8,
            row: row as u8,
        }
    }

    pub fn cells(&self) -> impl Iterator<Item = Place> + '_ {
        (0..self.rows).flat_map(move |row| (0..self.cols).map(move |col| Place { col, row }))
    }
}

/// The canvas in pixels: a cell's side and the gap between cells.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub cell: f32,
    pub gap: f32,
}

/// Where a control is drawn, in the canvas's pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Metrics {
    pub const fn new(cell: f32, gap: f32) -> Self {
        Self { cell, gap }
    }

    /// `n` cells and the gaps between them.
    pub fn extent(&self, n: u8) -> f32 {
        if n == 0 {
            return 0.0;
        }
        self.cell * f32::from(n) + self.gap * f32::from(n - 1)
    }

    /// A cell's pitch: its side and one gap.
    pub fn pitch(&self) -> f32 {
        self.cell + self.gap
    }

    pub fn width(&self, span: Span) -> f32 {
        self.extent(span.cols)
    }

    pub fn height(&self, span: Span) -> f32 {
        self.extent(span.rows)
    }

    /// Where `canvas` starts in a `width` by `height` area it's centred
    /// in: what the whole cells leave over, split evenly.
    pub fn centred_origin(&self, canvas: &Canvas, width: f32, height: f32) -> (f32, f32) {
        (
            ((width - self.extent(canvas.cols)) / 2.0).max(0.0),
            ((height - self.extent(canvas.rows)) / 2.0).max(0.0),
        )
    }

    pub fn slot(&self, rect: Rect) -> Slot {
        Slot {
            x: f32::from(rect.at.col) * self.pitch(),
            y: f32::from(rect.at.row) * self.pitch(),
            width: self.width(rect.span),
            height: self.height(rect.span),
        }
    }

    /// The place a `span`-sized control with its top-left at (`x`, `y`)
    /// is nearest to, on `canvas`.
    pub fn nearest(&self, canvas: &Canvas, x: f32, y: f32, span: Span) -> Place {
        let col = (x / self.pitch()).round() as i32;
        let row = (y / self.pitch()).round() as i32;
        canvas.clamp(col, row, span)
    }

    /// [`nearest`](Self::nearest), unless `current` is set and the
    /// top-left hasn't left that cell by more than half a pitch and `band`
    /// pitches on that axis: a control settles into a cell and stays until
    /// it's clearly in the next, so a hand resting on a boundary doesn't
    /// make it flap.
    pub fn settle(
        &self,
        canvas: &Canvas,
        x: f32,
        y: f32,
        span: Span,
        current: Option<Place>,
        band: f32,
    ) -> Place {
        let nearest = self.nearest(canvas, x, y, span);
        let Some(current) = current else {
            return nearest;
        };
        let pitch = self.pitch();
        let holds = |v: f32, cell: u8| (v - f32::from(cell) * pitch).abs() <= pitch * (0.5 + band);
        let col = if holds(x, current.col) {
            current.col
        } else {
            nearest.col
        };
        let row = if holds(y, current.row) {
            current.row
        } else {
            nearest.row
        };
        canvas.clamp(i32::from(col), i32::from(row), span)
    }
}

/// Which control each cell of the canvas holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Occupancy<T> {
    canvas: Canvas,
    cells: Vec<Option<T>>,
}

impl<T: Copy + PartialEq> Occupancy<T> {
    pub fn new(canvas: Canvas) -> Self {
        Self {
            canvas,
            cells: vec![None; usize::from(canvas.cols) * usize::from(canvas.rows)],
        }
    }

    pub fn of(canvas: Canvas, placed: impl IntoIterator<Item = (T, Rect)>) -> Self {
        let mut out = Self::new(canvas);
        for (id, rect) in placed {
            out.put(id, rect);
        }
        out
    }

    fn index(&self, place: Place) -> Option<usize> {
        (place.col < self.canvas.cols && place.row < self.canvas.rows).then(|| {
            usize::from(place.row) * usize::from(self.canvas.cols) + usize::from(place.col)
        })
    }

    pub fn put(&mut self, id: T, rect: Rect) {
        for place in rect.cells() {
            if let Some(i) = self.index(place) {
                self.cells[i] = Some(id);
            }
        }
    }

    pub fn at(&self, place: Place) -> Option<T> {
        self.index(place).and_then(|i| self.cells[i])
    }

    /// Whether every cell of `rect` is on the canvas and empty, or holds
    /// `ignoring`.
    pub fn free(&self, rect: &Rect, ignoring: Option<T>) -> bool {
        self.canvas.fits(rect)
            && rect
                .cells()
                .all(|place| self.at(place).is_none_or(|id| Some(id) == ignoring))
    }

    /// What sits under `rect`, each once, in canvas order.
    pub fn covered(&self, rect: &Rect, ignoring: Option<T>) -> Vec<T> {
        let mut out = Vec::new();
        for place in rect.cells() {
            if let Some(id) = self.at(place)
                && Some(id) != ignoring
                && !out.contains(&id)
            {
                out.push(id);
            }
        }
        out
    }

    /// The first place, row by row, where `span` fits.
    pub fn first_free(&self, span: Span) -> Option<Place> {
        self.canvas
            .cells()
            .map(|at| Rect::new(at, span))
            .find(|rect| self.free(rect, None))
            .map(|rect| rect.at)
    }

    /// How many rows down the last taken cell is.
    pub fn rows_used(&self) -> u8 {
        self.canvas
            .cells()
            .filter(|place| self.at(*place).is_some())
            .map(|place| place.row + 1)
            .max()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANVAS: Canvas = Canvas { cols: 4, rows: 8 };
    const PX: Metrics = Metrics::new(74.0, 16.0);

    const S: Span = Span::new(1, 1);
    const M: Span = Span::new(2, 1);
    const L: Span = Span::new(2, 2);
    const W3: Span = Span::new(3, 1);
    const FULL: Span = Span::new(4, 1);

    #[test]
    fn spans_measure_as_the_reference() {
        assert_eq!(PX.width(S), 74.0);
        assert_eq!(PX.width(M), 164.0);
        assert_eq!(PX.width(L), 164.0);
        assert_eq!(PX.height(L), 164.0);
        assert_eq!(PX.width(W3), 254.0);
        assert_eq!(PX.width(FULL), 344.0);
        assert_eq!(PX.extent(CANVAS.rows), 704.0);
        assert_eq!(PX.extent(0), 0.0);
    }

    #[test]
    fn a_rect_lands_where_its_cells_are() {
        let slot = PX.slot(Rect::new(Place::new(2, 3), M));
        assert_eq!(slot.x, 180.0);
        assert_eq!(slot.y, 270.0);
        assert_eq!(slot.width, 164.0);
        assert_eq!(slot.height, 74.0);
        assert_eq!(Rect::new(Place::new(0, 0), L).cells().count(), 4);
        assert!(CANVAS.fits(&Rect::new(Place::new(2, 0), L)));
        assert!(!CANVAS.fits(&Rect::new(Place::new(3, 0), M)));
        assert!(!CANVAS.fits(&Rect::new(Place::new(0, 7), L)));
    }

    #[test]
    fn the_nearest_place_snaps_and_stays_on_the_canvas() {
        assert_eq!(PX.nearest(&CANVAS, 40.0, 50.0, S), Place::new(0, 1));
        assert_eq!(PX.nearest(&CANVAS, 300.0, 0.0, M), Place::new(2, 0));
        // pulled off the right edge, a wide span stays inside
        assert_eq!(PX.nearest(&CANVAS, 900.0, -50.0, W3), Place::new(1, 0));
        assert_eq!(PX.nearest(&CANVAS, 0.0, 9000.0, L), Place::new(0, 6));
    }

    #[test]
    fn settling_keeps_the_cell_until_the_pointer_is_well_past_the_middle() {
        let pitch = 90.0;
        let from = Some(Place::new(0, 0));
        // without a cell in hand it's `nearest`
        assert_eq!(
            PX.settle(&CANVAS, 0.55 * pitch, 0.0, S, None, 0.1),
            Place::new(1, 0)
        );
        // a hair past the middle holds, well past it goes
        assert_eq!(
            PX.settle(&CANVAS, 0.55 * pitch, 0.0, S, from, 0.1),
            Place::new(0, 0)
        );
        assert_eq!(
            PX.settle(&CANVAS, 0.65 * pitch, 0.0, S, from, 0.1),
            Place::new(1, 0)
        );
        // and back the same way, each axis on its own
        let next = Some(Place::new(1, 0));
        assert_eq!(
            PX.settle(&CANVAS, 0.45 * pitch, 0.0, S, next, 0.1),
            Place::new(1, 0)
        );
        assert_eq!(
            PX.settle(&CANVAS, 0.35 * pitch, 0.65 * pitch, S, next, 0.1),
            Place::new(0, 1)
        );
        // held off the canvas, a wide span still stays inside
        assert_eq!(
            PX.settle(&CANVAS, 900.0, 0.0, W3, from, 0.1),
            Place::new(1, 0)
        );
    }

    #[test]
    fn occupancy_finds_free_room_row_by_row() {
        let mut occ = Occupancy::of(
            CANVAS,
            [
                ("conn", Rect::new(Place::new(0, 0), L)),
                ("play", Rect::new(Place::new(2, 0), L)),
                ("focus", Rect::new(Place::new(0, 2), M)),
            ],
        );
        assert_eq!(occ.first_free(S), Some(Place::new(2, 2)));
        assert_eq!(occ.first_free(M), Some(Place::new(2, 2)));
        assert_eq!(occ.first_free(W3), Some(Place::new(0, 3)));
        assert_eq!(occ.rows_used(), 3);
        assert!(occ.free(&Rect::new(Place::new(0, 2), M), Some("focus")));
        assert!(!occ.free(&Rect::new(Place::new(0, 2), M), None));
        assert_eq!(
            occ.covered(&Rect::new(Place::new(1, 1), L), None),
            ["conn", "play", "focus"]
        );
        occ.put("x", Rect::new(Place::new(0, 7), FULL));
        assert_eq!(occ.rows_used(), 8);
        assert_eq!(occ.first_free(FULL), Some(Place::new(0, 3)));
        // nothing two rows tall fits in a full canvas's last row
        let full = Occupancy::of(
            CANVAS,
            [(
                "all",
                Rect::new(Place::new(0, 0), Span { cols: 4, rows: 8 }),
            )],
        );
        assert_eq!(full.first_free(S), None);
    }
}

#[cfg(test)]
mod fit {
    use super::*;

    #[test]
    fn a_screen_fits_whole_cells_centred() {
        let px = Metrics::new(74.0, 16.0);
        // 1920 × 1080 less a 40 px bar
        let canvas = Canvas::fit(&px, 1920.0, 1040.0);
        assert_eq!(canvas, Canvas { cols: 21, rows: 11 });
        let (x, y) = px.centred_origin(&canvas, 1920.0, 1040.0);
        assert_eq!(x, (1920.0 - px.extent(21)) / 2.0);
        assert_eq!(y, (1040.0 - px.extent(11)) / 2.0);
        // exactly one cell, and less than one
        assert_eq!(Canvas::fit(&px, 74.0, 74.0), Canvas { cols: 1, rows: 1 });
        assert_eq!(Canvas::fit(&px, 73.0, 0.0), Canvas { cols: 0, rows: 0 });
    }
}
