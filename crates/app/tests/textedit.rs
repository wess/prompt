use super::*;

#[test]
fn new_places_cursor_at_end() {
    let e = TextEdit::new("abc");
    assert_eq!(e.split(), ("abc".into(), "".into()));
}

#[test]
fn insert_at_cursor() {
    let mut e = TextEdit::new("ac");
    e.left();
    e.insert("b");
    assert_eq!(e.text(), "abc");
    assert_eq!(e.split(), ("ab".into(), "c".into()));
}

#[test]
fn backspace_and_delete() {
    let mut e = TextEdit::new("abc");
    assert!(e.backspace()); // removes 'c'
    assert_eq!(e.text(), "ab");
    e.home();
    assert!(e.delete()); // removes 'a'
    assert_eq!(e.text(), "b");
    // Boundaries are no-ops.
    e.home();
    assert!(!e.backspace());
    e.end();
    assert!(!e.delete());
}

#[test]
fn cursor_movement_clamps() {
    let mut e = TextEdit::new("ab");
    e.home();
    e.left();
    assert_eq!(e.split(), ("".into(), "ab".into()));
    e.end();
    e.right();
    assert_eq!(e.split(), ("ab".into(), "".into()));
}

#[test]
fn handles_unicode() {
    let mut e = TextEdit::new("café");
    assert!(e.backspace()); // drops 'é' as one unit
    assert_eq!(e.text(), "caf");
    e.insert("é");
    assert_eq!(e.text(), "café");
}

#[test]
fn word_navigation() {
    let mut e = TextEdit::new("foo bar baz");
    e.word_left();
    assert_eq!(e.split(), ("foo bar ".into(), "baz".into()));
    e.word_left();
    assert_eq!(e.split(), ("foo ".into(), "bar baz".into()));
    e.home();
    e.word_right();
    assert_eq!(e.split(), ("foo".into(), " bar baz".into()));
}

#[test]
fn delete_word_back_and_forward() {
    let mut e = TextEdit::new("foo bar baz");
    assert!(e.delete_word_back()); // removes "baz"
    assert_eq!(e.text(), "foo bar ");
    e.home();
    assert!(e.delete_word_forward()); // removes "foo"
    assert_eq!(e.text(), " bar ");
}

#[test]
fn delete_to_line_ends() {
    let mut e = TextEdit::new("hello world");
    e.home();
    e.word_right(); // cursor after "hello"
    assert!(e.delete_to_start());
    assert_eq!(e.text(), " world");
    e.end();
    e.word_left(); // cursor before "world"
    assert!(e.delete_to_end());
    assert_eq!(e.text(), " ");
}
