//! Where a native parser's gap is, and why the parser cannot read the SQL there.

use crate::analysis_route::models::{GapPosition, GapSite, LocatedGap, ParserGap};
use crate::compiled_project::main::text_position::text_position;
use crate::compiled_project::models::TextPosition;

/// The gap at its line and column in `sql`, and its offset in `source` when the text holds.
pub(crate) fn located(sql: &str, gap: ParserGap, source: Option<&str>) -> LocatedGap {
    let before: &str = sql.get(..gap.offset).unwrap_or(sql);
    let offset: usize = before.chars().count();
    let position: TextPosition = text_position(sql, offset);
    LocatedGap {
        construct: gap.construct,
        message: gap.message,
        line: position.line,
        column: position.column,
        syntax: gap.syntax,
        source_offset: source.and_then(|source| aligned(sql, offset, source)),
    }
}

/// The character of `source` that `derived`'s `offset` came from, when the text before or after holds.
pub(crate) fn aligned(derived: &str, offset: usize, source: &str) -> Option<usize> {
    let derived: Vec<char> = derived.chars().collect();
    let source: Vec<char> = source.chars().collect();
    if offset > derived.len() {
        return None;
    }
    if source.starts_with(&derived[..offset]) {
        return Some(offset);
    }
    source
        .ends_with(&derived[offset..])
        .then(|| source.len() - (derived.len() - offset))
}

/// Why the dialect's native parser cannot read SQL, as P026 words it.
pub(crate) fn unreadable_reason(dialect: &str, gap: &LocatedGap) -> String {
    unreadable_at(dialect, gap, (gap.line, gap.column, ""))
}

/// P026's text for a gap at its site: the hook and file, then the parser's reason there.
pub(crate) fn sited_reason(dialect: &str, gap: &LocatedGap, site: &GapSite<'_>) -> String {
    let label: String = site
        .label
        .map_or_else(String::new, |label| format!(" {label}"));
    let at: (usize, usize, &str) = match site.position {
        GapPosition::File(position) => (position.line, position.column, ""),
        GapPosition::Statement(position) => (position.line, position.column, " of its SQL"),
        GapPosition::Analysed => (gap.line, gap.column, " of the analysed SQL"),
    };
    format!(
        "{label} ({}): {}",
        site.path,
        unreadable_at(dialect, gap, at)
    )
}

fn unreadable_at(dialect: &str, gap: &LocatedGap, at: (usize, usize, &str)) -> String {
    let (line, column, within) = at;
    let detail: String = if gap.message.is_empty() || gap.message == gap.construct {
        String::new()
    } else {
        format!(": {}", gap.message)
    };
    format!(
        "the {dialect} parser cannot read {} at line {line}, column {column}{within}{detail}",
        gap.construct
    )
}
