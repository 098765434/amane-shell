//! What the launcher shows and where: the kind of body, its layout, and the view of\nthe whole panel built once a frame.

use super::*;

/// What's under the search row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// Nothing: the glass is a pill.
    Idle,
    /// What a search found.
    Results,
    /// Applications: the grid, or the apps listed.
    Apps,
    /// Clipboard, before its history is turned on: asking to.
    Ask,
}

/// How the body lays its items out: the grid, or a list of rows, the first
/// `skip` items (the answer) drawn over it rather than in it, and a
/// section's heading over each row in `headings` (by its place in the list).
#[derive(Clone, PartialEq)]
pub(super) enum Layout {
    Grid,
    List { skip: usize, headings: Rc<[usize]> },
}

impl Layout {
    /// Where item `index` is down the body: its top and its height.
    pub(super) fn span(&self, index: usize) -> (f32, f32) {
        match self {
            Self::Grid => ((index / COLS) as f32 * CELL, CELL),
            Self::List { skip, .. } if index < *skip => (0.0, 0.0),
            Self::List { skip, headings } => {
                let row = index - skip;
                let above = headings.iter().filter(|at| **at <= row).count();
                (row as f32 * (ROW + ROW_GAP) + above as f32 * HEADING, ROW)
            }
        }
    }

    /// Where the highlight rests on item `index`.
    pub(super) fn highlight(&self, index: usize) -> (f32, f32) {
        match self {
            Self::Grid => (
                (index % COLS) as f32 * CELL + BOX_X,
                (index / COLS) as f32 * CELL + BOX_Y,
            ),
            Self::List { .. } => (0.0, self.span(index).0),
        }
    }

    /// How long `count` rows or apps are, top to bottom.
    pub(super) fn content(&self, count: usize) -> f32 {
        match self {
            Self::Grid => count.div_ceil(COLS).max(1) as f32 * CELL,
            Self::List { headings, .. } => {
                count as f32 * (ROW + ROW_GAP) + headings.len() as f32 * HEADING
            }
        }
    }

    /// How many items Up and Down move by.
    pub(super) fn row(&self) -> usize {
        match self {
            Self::Grid => COLS,
            Self::List { .. } => 1,
        }
    }

    /// How many items are drawn over the list rather than in it.
    pub(super) fn skip(&self) -> usize {
        match self {
            Self::Grid => 0,
            Self::List { skip, .. } => *skip,
        }
    }
}

/// What the launcher shows this moment, worked out once for the view, a
/// key or Enter.
pub(super) struct View<'a> {
    pub(super) all: &'a [DesktopApp],
    pub(super) field: Field,
    pub(super) mode: Mode,
    pub(super) scope: Option<Scope>,
    pub(super) kind: Kind,
    // what the search found, or Applications' apps as rows
    pub(super) found: Found,
    // the categories with apps under them, the one showing, and its apps
    // (the suggested ones first on the first page)
    pub(super) buckets: Vec<usize>,
    pub(super) page: usize,
    pub(super) suggested: Vec<&'a DesktopApp>,
    pub(super) shown: Vec<&'a DesktopApp>,
    pub(super) apps_layout: AppsLayout,
    pub(super) selected: usize,
    pub(super) focus: Zone,
}

impl<'a> View<'a> {
    pub(super) fn new(all: &'a [DesktopApp]) -> Self {
        let (field, mode, scope, selected, focus) = with(|state| {
            (
                state.field.clone(),
                state.mode,
                state.scope.clone(),
                state.selected,
                state.focus,
            )
        });
        let typed = !field.text().trim().is_empty();
        let settings = launcher();
        let apps_layout = settings.apps_layout;
        let kind = match mode {
            _ if scope.is_some() => Kind::Results,
            Mode::Home if typed => Kind::Results,
            Mode::Home => Kind::Idle,
            Mode::Apps if typed => Kind::Results,
            Mode::Apps => Kind::Apps,
            Mode::Actions => Kind::Results,
            Mode::Clipboard if settings.clipboard.enabled => Kind::Results,
            Mode::Clipboard => Kind::Ask,
            Mode::Files => Kind::Results,
            Mode::Vault => Kind::Results,
        };

        let buckets = model::active_buckets(
            all.iter()
                .map(|app| model::mask(app.categories(), app.exec())),
        );
        let page = PAGER
            .with_borrow(Pager::page)
            .min(buckets.len().saturating_sub(1));
        let mut found = Found::default();
        let mut suggested = Vec::new();
        let mut shown = Vec::new();
        match kind {
            Kind::Results => {
                let query = Query::new(field.text(), mode, all);
                found = provider::search(&query, scope.as_ref(), &settings.excluded_providers);
                // Files offers its kinds as chips, to narrow the search to one
                if mode == Mode::Files && scope.is_none() && !field.text().starts_with('/') {
                    found.scopes = FILE_CHIPS.into_iter().map(Scope::Kind).collect();
                }
            }
            Kind::Apps => {
                suggested = suggestions(all);
                shown = apps_on(all, &buckets, page, &suggested);
                if apps_layout == AppsLayout::List {
                    found.rows = shown.iter().map(|app| Entry::app(app)).collect();
                }
            }
            Kind::Idle | Kind::Ask => {}
        }
        drop(settings);

        let mut view = Self {
            all,
            field,
            mode,
            scope,
            kind,
            found,
            buckets,
            page,
            suggested,
            shown,
            apps_layout,
            selected,
            focus,
        };
        view.selected = selected.min(view.count().saturating_sub(1));
        view
    }

    /// Whether the body is the grid of apps.
    pub(super) fn grid(&self) -> bool {
        self.kind == Kind::Apps && self.apps_layout == AppsLayout::Grid
    }

    pub(super) fn layout(&self) -> Layout {
        if self.grid() {
            Layout::Grid
        } else {
            Layout::List {
                skip: usize::from(self.found.answer.is_some()),
                headings: headings(&self.found.rows).into(),
            }
        }
    }

    pub(super) fn count(&self) -> usize {
        match self.kind {
            _ if self.grid() => self.shown.len(),
            Kind::Results | Kind::Apps => {
                usize::from(self.found.answer.is_some()) + self.found.rows.len()
            }
            Kind::Idle | Kind::Ask => 0,
        }
    }

    /// The entry item `index` is: the answer, then the rows.
    pub(super) fn entry(&self, index: usize) -> Option<&Entry> {
        match &self.found.answer {
            Some(answer) if index == 0 => Some(answer),
            Some(_) => self.found.rows.get(index - 1),
            None => self.found.rows.get(index),
        }
    }

    /// Whether the panel has the category pills: Applications, unsearched.
    pub(super) fn pills(&self) -> bool {
        self.kind == Kind::Apps
    }

    /// How wide the panel is, and what's on it inside its inset.
    pub(super) fn width(&self) -> f32 {
        if matches!(self.mode, Mode::Apps | Mode::Clipboard | Mode::Files) {
            APPS_W
        } else {
            PILL_W
        }
    }

    /// Whether a preview of the row picked shows beside the list: a copy
    /// in Clipboard, a file in Files.
    pub(super) fn previews(&self) -> bool {
        matches!(self.mode, Mode::Clipboard | Mode::Files)
            && self.found.rows.iter().any(|row| row.preview.is_some())
    }

    /// How wide the list is: Clipboard and Files share their width with a
    /// preview of the row picked.
    pub(super) fn list_width(&self) -> f32 {
        if self.previews() {
            CLIP_LIST_W
        } else {
            self.inner()
        }
    }

    pub(super) fn inner(&self) -> f32 {
        self.width() - INSET * 2.0
    }

    /// How tall the list of results is in view.
    pub(super) fn list_height(&self) -> f32 {
        // Applications and Actions keep one size, however much they list
        if self.kind == Kind::Apps {
            return BODY;
        }
        if matches!(self.mode, Mode::Actions | Mode::Clipboard | Mode::Files) {
            return MAX_ROWS as f32 * (ROW + ROW_GAP) - ROW_GAP;
        }
        let rows = self.found.rows.len().clamp(1, MAX_ROWS);
        rows as f32 * (ROW + ROW_GAP) - ROW_GAP
    }

    /// How tall the panel is: the search row, then what's under it.
    pub(super) fn height(&self) -> f32 {
        let under = match self.kind {
            Kind::Idle => return PILL_H,
            Kind::Ask => EMPTY_H,
            Kind::Apps => PILLS + GAP + BODY + INSET * 2.0,
            Kind::Results => {
                let answer = if self.found.answer.is_some() {
                    ANSWER_H + GAP
                } else {
                    0.0
                };
                let chips = if self.found.scopes.is_empty() {
                    0.0
                } else {
                    CHIP_H + GAP
                };
                answer + chips + self.list_height() + INSET * 2.0
            }
        };
        PILL_H + Divider::height() + under
    }
}
