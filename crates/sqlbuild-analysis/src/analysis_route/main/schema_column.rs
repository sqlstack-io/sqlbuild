use sqlquack_semantics::lineage::models::{Nullability, SchemaColumn};

use crate::assembly::analysis_session::constants::QUOTED_IDENTIFIER_DELIMITER;

/// A stored column name as SQLQuack reads it: `"…"` is quoted, its inner `""` one quote.
#[must_use]
pub fn schema_column(
    name: &str,
    data_type: Option<String>,
    nullability: Nullability,
) -> SchemaColumn {
    let delimiter: String = QUOTED_IDENTIFIER_DELIMITER.to_string();
    let inner: Option<&str> = name
        .strip_prefix(QUOTED_IDENTIFIER_DELIMITER)
        .and_then(|rest| rest.strip_suffix(QUOTED_IDENTIFIER_DELIMITER));
    SchemaColumn {
        name: inner.map_or_else(
            || name.to_owned(),
            |inner| inner.replace(&delimiter.repeat(2), &delimiter),
        ),
        quoted: inner.is_some(),
        data_type,
        nullability,
    }
}
