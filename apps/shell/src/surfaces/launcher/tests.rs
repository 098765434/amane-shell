use super::*;

#[test]
fn tab_goes_round_the_zones() {
    assert_eq!(Zone::Body.next(false, true), Zone::Search);
    assert_eq!(Zone::Search.next(false, true), Zone::Pills);
    assert_eq!(Zone::Pills.next(false, true), Zone::Body);
    assert_eq!(Zone::Search.next(true, true), Zone::Body);
}

#[test]
fn without_pills_tab_goes_between_the_field_and_the_list() {
    assert_eq!(Zone::Search.next(false, false), Zone::Body);
    assert_eq!(Zone::Body.next(false, false), Zone::Search);
    // the pills, focused before the search began, count as the field
    assert_eq!(Zone::Pills.next(false, false), Zone::Body);
}

#[test]
fn the_layouts_put_items_where_they_draw() {
    assert_eq!(Layout::Grid.span(COLS + 1), (CELL, CELL));
    assert_eq!(
        Layout::Grid.highlight(COLS + 1),
        (CELL + BOX_X, CELL + BOX_Y)
    );
    let list = Layout::List {
        skip: 0,
        headings: Rc::from([]),
    };
    assert_eq!(list.span(2), (2.0 * (ROW + ROW_GAP), ROW));
    assert_eq!(Layout::Grid.content(0), CELL);
    assert_eq!(Layout::Grid.content(COLS + 1), 2.0 * CELL);
}

#[test]
fn headings_push_the_rows_under_them_down() {
    let list = Layout::List {
        skip: 0,
        headings: Rc::from([0, 2]),
    };
    assert_eq!(list.span(0), (HEADING, ROW));
    assert_eq!(list.span(1), (ROW + ROW_GAP + HEADING, ROW));
    assert_eq!(list.span(2), (2.0 * (ROW + ROW_GAP) + 2.0 * HEADING, ROW));
    assert_eq!(list.content(3), 3.0 * (ROW + ROW_GAP) + 2.0 * HEADING);
}

#[test]
fn a_section_starts_where_its_name_changes() {
    let row = |section: Option<&str>| {
        let mut entry = Entry::new("k", "t", entries::Icon::Glyph(' '), entries::Action::Hint);
        entry.section = section.map(String::from);
        entry
    };
    let rows = [
        row(Some("Suggestions")),
        row(Some("Suggestions")),
        row(Some("System")),
        row(None),
        row(Some("Firefox")),
    ];
    assert_eq!(headings(&rows), vec![0, 2, 4]);
}

#[test]
fn an_answer_over_the_list_isnt_in_it() {
    let list = Layout::List {
        skip: 1,
        headings: Rc::from([]),
    };
    assert_eq!(list.span(0), (0.0, 0.0));
    assert_eq!(list.span(1), (0.0, ROW));
    assert_eq!(list.span(3), (2.0 * (ROW + ROW_GAP), ROW));
}
