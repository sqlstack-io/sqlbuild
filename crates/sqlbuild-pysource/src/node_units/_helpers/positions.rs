//! Character positions in the newline-translated source the tree was parsed from.

use ruff_text_size::TextSize;

use crate::cpython_tree::main::source_position::source_position;
use crate::cpython_tree::models::SourceLines;
use crate::node_units::models::UnitPosition;

/// The parsed text and its line starts.
#[derive(Debug)]
pub(crate) struct UnitText<'a> {
    pub(crate) text: &'a str,
    pub(crate) lines: &'a SourceLines,
}

/// The 1-based line and character column of `offset`.
pub(crate) fn unit_position(source: &UnitText<'_>, offset: TextSize) -> UnitPosition {
    let (line, byte_column) = source_position(source.lines, offset);
    let end: usize = offset.to_usize();
    let start: usize = end - byte_column;
    UnitPosition {
        line,
        column: source
            .text
            .get(start..end)
            .map_or(byte_column, |prefix| prefix.chars().count())
            + 1,
    }
}
