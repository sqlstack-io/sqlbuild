//! Spans in the parsed text read back as positions in the rule's source, across marker names and `TABLE( )` wrappers.

use std::ops::Range;

use sqlquack_syntax::tokens::models::Span;

use crate::relation_markers::types::MarkerRanges;
use crate::sql_tree::_helpers::positions::{line_column, line_starts};
use crate::sql_tree::models::NodeExtent;

/// The parsed text's `parsed` characters stand where the earlier text's `source` characters were.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Rewrite {
    pub(crate) parsed: Range<usize>,
    pub(crate) source: Range<usize>,
}

/// The rule's source as the parser's text maps back to it.
pub(crate) struct SourceMap<'source> {
    /// Each marker call's source text, by marker index.
    pub(crate) calls: Vec<&'source str>,
    line_starts: Vec<usize>,
    /// Marker names over their calls, in order: named text to source.
    markers: Vec<Rewrite>,
    /// `TABLE( )` wrappers, in order: parsed text to named text.
    wrappers: Vec<Rewrite>,
}

impl<'source> SourceMap<'source> {
    /// The map of `source`, whose marker calls `named` holds as names at `ranges`, parsed after `wrappers`.
    pub(crate) fn new(
        source: &'source str,
        named: &str,
        ranges: &[MarkerRanges],
        wrappers: Vec<Rewrite>,
    ) -> Self {
        let calls: Vec<Range<usize>> = char_ranges(source, ranges.iter().map(|(call, _)| call));
        let names: Vec<Range<usize>> = char_ranges(named, ranges.iter().map(|(_, name)| name));
        Self {
            calls: ranges
                .iter()
                .map(|(call, _)| &source[call.clone()])
                .collect(),
            line_starts: line_starts(source),
            markers: names
                .into_iter()
                .zip(calls)
                .map(|(parsed, source)| Rewrite { parsed, source })
                .collect(),
            wrappers,
        }
    }

    /// The 1-based line and column of a parsed character offset where a node starts.
    pub(crate) fn start(&self, offset: usize) -> (usize, usize) {
        line_column(
            &self.line_starts,
            source_start(&self.markers, source_start(&self.wrappers, offset)),
        )
    }

    /// The source extent of a parsed span: a node starting or ending inside a marker name covers its whole call.
    pub(crate) fn extent(&self, span: Span) -> NodeExtent {
        let start: usize = source_start(&self.markers, source_start(&self.wrappers, span.start));
        let end: usize = source_end(&self.markers, source_end(&self.wrappers, span.end));
        let (end_line, end_column) = line_column(&self.line_starts, end);
        NodeExtent {
            end_line,
            end_column,
            span: (start, end),
        }
    }
}

/// The earlier text's offset for a start offset: inside a rewrite, the start of what it replaced.
fn source_start(rewrites: &[Rewrite], offset: usize) -> usize {
    let after: usize = rewrites.partition_point(|rewrite| rewrite.parsed.end <= offset);
    match rewrites.get(after) {
        Some(rewrite) if rewrite.parsed.start <= offset => rewrite.source.start,
        _ => shifted(rewrites[..after].last(), offset),
    }
}

/// The earlier text's offset for an exclusive end offset: inside or closing a rewrite, the end of what it replaced, and at a removal, where the removed text began.
fn source_end(rewrites: &[Rewrite], offset: usize) -> usize {
    let after: usize = rewrites.partition_point(|rewrite| rewrite.parsed.end < offset);
    match rewrites.get(after) {
        Some(rewrite) if rewrite.parsed.start < offset => rewrite.source.end,
        Some(rewrite) if rewrite.parsed.is_empty() && rewrite.parsed.start == offset => {
            rewrite.source.start
        }
        _ => shifted(rewrites[..after].last(), offset),
    }
}

/// `offset` past the last rewrite before it, moved as that rewrite moved the text after it.
fn shifted(previous: Option<&Rewrite>, offset: usize) -> usize {
    previous.map_or(offset, |rewrite| {
        rewrite.source.end + (offset - rewrite.parsed.end)
    })
}

/// Ascending byte `ranges` of `text` as character ranges, counted in one pass.
fn char_ranges<'a>(
    text: &str,
    ranges: impl Iterator<Item = &'a Range<usize>>,
) -> Vec<Range<usize>> {
    let mut byte: usize = 0;
    let mut chars: usize = 0;
    ranges
        .map(|range| {
            chars += text[byte..range.start].chars().count();
            let start: usize = chars;
            chars += text[range.clone()].chars().count();
            byte = range.end;
            start..chars
        })
        .collect()
}
