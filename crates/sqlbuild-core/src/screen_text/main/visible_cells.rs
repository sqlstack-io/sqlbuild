//! How many terminal cells a styled row covers.

use crate::text::main::cell_len::cell_len;

/// The cells `row` covers once its colour and hyperlink escapes are left out.
#[must_use]
pub fn visible_cells(row: &str) -> usize {
    let mut text: String = String::with_capacity(row.len());
    let mut characters = row.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '\u{1b}' {
            text.push(character);
            continue;
        }
        match characters.next() {
            Some('[') => {
                for next in characters.by_ref() {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(next) = characters.next() {
                    if next == '\u{7}' {
                        break;
                    }
                    if next == '\u{1b}' && characters.peek() == Some(&'\\') {
                        characters.next();
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    cell_len(&text)
}
