//! Whether a type spelling names one of a dialect's own warehouse types.

use crate::analysis_route::_helpers::dialects::clickhouse_types::clickhouse_type;
use crate::analysis_route::constants::{ARRAY_SUFFIX, CLICKHOUSE_DIALECT, DIALECT_TYPE_NAMES};

/// Whether `spelling` is a type `dialect`'s warehouse knows: ClickHouse's as `toTypeName` reads it, others by base name.
#[must_use]
pub fn names_dialect_type(dialect: &str, spelling: &str) -> bool {
    if dialect == CLICKHOUSE_DIALECT {
        return clickhouse_type(spelling).is_some();
    }
    let base: String = base_name(spelling);
    DIALECT_TYPE_NAMES
        .iter()
        .find(|(name, _)| *name == dialect)
        .is_some_and(|(_, names)| names.contains(&base.as_str()))
}

/// The upper-cased words before a type's arguments (`(`, `<`) and array suffixes.
fn base_name(spelling: &str) -> String {
    let mut text: &str = spelling.trim();
    while let Some(element) = text.strip_suffix(ARRAY_SUFFIX) {
        text = element.trim_end();
    }
    let end: usize = text.find(['(', '<']).unwrap_or(text.len());
    text[..end]
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
        .to_ascii_uppercase()
}
