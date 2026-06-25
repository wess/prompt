use super::*;

fn grid_with_letters(rows: usize) -> Grid {
    let mut g = Grid::new(4, rows, 100);
    for r in 0..rows {
        g.cell_mut(r, 0).ch = (b'a' + r as u8) as char;
    }
    g
}

#[test]
fn new_grid_dimensions() {
    let g = Grid::new(80, 24, 10);
    assert_eq!(g.cols(), 80);
    assert_eq!(g.rows(), 24);
    assert_eq!(g.row(0).len(), 80);
}

#[test]
fn scroll_up_moves_rows_and_blanks_bottom() {
    let mut g = grid_with_letters(3);
    g.scroll_up(0, 2, 1, false, Cell::default());
    assert_eq!(g.row(0).text(), "b");
    assert_eq!(g.row(1).text(), "c");
    assert_eq!(g.row(2).text(), "");
}

#[test]
fn scroll_up_saves_to_scrollback() {
    let mut g = grid_with_letters(3);
    g.scroll_up(0, 2, 2, true, Cell::default());
    assert_eq!(g.scrollback().len(), 2);
    assert_eq!(g.scrollback().get(0).unwrap().text(), "a");
    assert_eq!(g.scrollback().get(1).unwrap().text(), "b");
    assert_eq!(g.row(0).text(), "c");
}

#[test]
fn scroll_up_region_only() {
    let mut g = grid_with_letters(4);
    g.scroll_up(1, 2, 1, false, Cell::default());
    assert_eq!(g.row(0).text(), "a");
    assert_eq!(g.row(1).text(), "c");
    assert_eq!(g.row(2).text(), "");
    assert_eq!(g.row(3).text(), "d");
}

#[test]
fn scroll_down_moves_rows_and_blanks_top() {
    let mut g = grid_with_letters(3);
    g.scroll_down(0, 2, 1, Cell::default());
    assert_eq!(g.row(0).text(), "");
    assert_eq!(g.row(1).text(), "a");
    assert_eq!(g.row(2).text(), "b");
}

#[test]
fn scroll_clamps_oversized_count() {
    let mut g = grid_with_letters(3);
    g.scroll_up(0, 2, 99, false, Cell::default());
    assert_eq!(g.row(0).text(), "");
    assert_eq!(g.row(2).text(), "");
}

#[test]
fn resize_grows_and_shrinks() {
    let mut g = grid_with_letters(3);
    g.resize(8, 5, (0, 0));
    assert_eq!(g.cols(), 8);
    assert_eq!(g.rows(), 5);
    assert_eq!(g.row(0).text(), "a");
    assert_eq!(g.row(4).text(), "");
    // Shrinking to 2 rows pushes the oldest reflowed line ("a") to scrollback.
    g.resize(2, 2, (0, 0));
    assert_eq!(g.cols(), 2);
    assert_eq!(g.rows(), 2);
    assert_eq!(g.row(0).text(), "b");
    assert_eq!(g.row(1).text(), "c");
}

#[test]
fn reflow_rejoins_and_rewraps_wrapped_line() {
    // Two rows forming one logical line "abcdef" wrapped at width 3.
    let mut g = Grid::new(3, 4, 100);
    for (c, ch) in "abc".chars().enumerate() {
        g.cell_mut(0, c).ch = ch;
    }
    g.row_mut(0).wrapped = true;
    for (c, ch) in "def".chars().enumerate() {
        g.cell_mut(1, c).ch = ch;
    }
    // Widen to 6: the logical line now fits on one row.
    g.resize(6, 4, (1, 0));
    assert_eq!(g.cols(), 6);
    assert_eq!(g.row(0).text(), "abcdef");
    assert!(!g.row(0).wrapped);
    // Narrow to 2: it re-wraps into three rows abc/def -> "ab","cd","ef".
    g.resize(2, 4, (0, 0));
    assert_eq!(g.row(0).text(), "ab");
    assert_eq!(g.row(1).text(), "cd");
    assert_eq!(g.row(2).text(), "ef");
    assert!(g.row(0).wrapped);
    assert!(g.row(1).wrapped);
    assert!(!g.row(2).wrapped);
}

#[test]
fn reflow_follows_the_cursor() {
    // Logical line "abcdef" wrapped at 3; cursor on the 'e' (row 1, col 1).
    let mut g = Grid::new(3, 4, 100);
    for (c, ch) in "abc".chars().enumerate() {
        g.cell_mut(0, c).ch = ch;
    }
    g.row_mut(0).wrapped = true;
    for (c, ch) in "def".chars().enumerate() {
        g.cell_mut(1, c).ch = ch;
    }
    // Widen to 6: 'e' is the 5th char (offset 4) -> row 0, col 4.
    let cursor = g.resize(6, 4, (1, 1));
    assert_eq!(cursor, (0, 4));
}

#[test]
fn reflow_preserves_prompt_mark_on_first_segment() {
    let mut g = Grid::new(6, 4, 100);
    for (c, ch) in "abcdef".chars().enumerate() {
        g.cell_mut(0, c).ch = ch;
    }
    g.row_mut(0).prompt = true;
    g.resize(3, 4, (0, 0));
    // Splits into "abc"/"def"; the prompt mark rides the first segment only.
    assert!(g.row(0).prompt);
    assert!(!g.row(1).prompt);
}

#[test]
fn resize_clamps_to_one() {
    let mut g = Grid::new(4, 4, 0);
    g.resize(0, 0, (0, 0));
    assert_eq!(g.cols(), 1);
    assert_eq!(g.rows(), 1);
}

#[test]
fn fresh_grid_is_fully_damaged() {
    let mut g = Grid::new(4, 4, 0);
    assert_eq!(g.take_damage(), Damage::Full);
    assert_eq!(g.take_damage(), Damage::Rows(vec![]));
}

#[test]
fn cell_and_row_mutation_mark_rows() {
    let mut g = Grid::new(4, 4, 0);
    g.take_damage();
    g.cell_mut(2, 1).ch = 'x';
    g.row_mut(0).fill(Cell::default());
    assert_eq!(g.take_damage(), Damage::Rows(vec![0, 2]));
    assert_eq!(g.take_damage(), Damage::Rows(vec![]));
}

#[test]
fn scroll_escalates_to_full() {
    let mut g = grid_with_letters(3);
    g.take_damage();
    g.scroll_up(0, 2, 1, false, Cell::default());
    assert_eq!(g.take_damage(), Damage::Full);
    g.scroll_down(0, 2, 1, Cell::default());
    assert_eq!(g.take_damage(), Damage::Full);
}

#[test]
fn resize_and_scrollback_clear_escalate_to_full() {
    let mut g = grid_with_letters(3);
    g.take_damage();
    g.resize(8, 5, (0, 0));
    assert_eq!(g.take_damage(), Damage::Full);
    g.clear_scrollback();
    assert_eq!(g.take_damage(), Damage::Full);
}
