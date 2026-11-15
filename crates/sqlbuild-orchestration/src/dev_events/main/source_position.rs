//! Find the file position an error message points at.

use crate::dev_events::models::SourcePosition;

const SOURCE_SUFFIXES: [&str; 5] = [".sql", ".py", ".yml", ".yaml", ".toml"];

/// The first `path:line` or `path:line:column` in `message` whose path is a project source file.
#[must_use]
pub fn source_position(message: &str) -> Option<SourcePosition> {
    message.split_whitespace().find_map(|word| {
        let word: &str = word.trim_matches(|character: char| {
            matches!(
                character,
                '(' | ')' | '[' | ']' | ',' | ';' | '\'' | '"' | '`'
            )
        });
        let mut parts = word.split(':');
        let path: &str = parts.next()?;
        if !SOURCE_SUFFIXES.iter().any(|suffix| path.ends_with(suffix)) {
            return None;
        }
        let line: u32 = parts.next()?.parse().ok()?;
        let column: u32 = parts.next().and_then(|text| text.parse().ok()).unwrap_or(1);
        Some(SourcePosition {
            path: path.to_owned(),
            line,
            column,
        })
    })
}
