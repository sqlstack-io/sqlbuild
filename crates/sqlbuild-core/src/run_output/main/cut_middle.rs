//! Cut the middle of a long name so its start and its distinguishing end both stay.

use crate::run_output::constants::MIDDLE_ELLIPSIS;

/// `text` when it fits `width` characters, else its start, `...` and its end.
#[must_use]
pub fn cut_middle(text: &str, width: usize) -> String {
    let length: usize = text.chars().count();
    if length <= width {
        return text.to_owned();
    }
    if width <= MIDDLE_ELLIPSIS.len() {
        return text.chars().take(width).collect();
    }
    let kept: usize = width - MIDDLE_ELLIPSIS.len();
    let head: usize = kept / 2;
    let tail: usize = kept - head;
    let start: String = text.chars().take(head).collect();
    let end: String = text.chars().skip(length - tail).collect();
    format!("{start}{MIDDLE_ELLIPSIS}{end}")
}
