//! Find the file position an error message points at.

use crate::dev_events::constants::{POSITION_WRAPPERS, SOURCE_SUFFIXES};
use crate::dev_events::models::SourcePosition;

/// The first `path:line` or `path:line:column` in `message` whose path is a project source file.
#[must_use]
pub(crate) fn source_position(message: &str) -> Option<SourcePosition> {
    message.split_whitespace().find_map(word_position)
}

/// `word` as a position, its column `1` when it names none.
fn word_position(word: &str) -> Option<SourcePosition> {
    let word: &str = word.trim_matches(&POSITION_WRAPPERS[..]);
    let mut parts = word.split(':');
    let path: &str = parts.next()?;
    if !SOURCE_SUFFIXES.iter().any(|suffix| path.ends_with(suffix)) {
        return None;
    }
    let line: u32 = match parts.next()?.parse() {
        Ok(line) => line,
        Err(_) => return None,
    };
    let column: u32 = match parts.next().map(str::parse) {
        Some(Ok(column)) => column,
        Some(Err(_)) | None => 1,
    };
    Some(SourcePosition {
        path: path.to_owned(),
        line,
        column,
    })
}
