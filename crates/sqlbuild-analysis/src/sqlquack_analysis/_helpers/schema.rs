//! Dialect profiles and relation schemas as SQLQuack's lineage reads them.

use std::collections::HashMap;

use sqlquack_semantics::dialects::constants::{DUCKDB, SNOWFLAKE};
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_semantics::lineage::models::SchemaColumn;
use sqlquack_semantics::lineage::types::LineageSchema;
use sqlquack_syntax::Dialect;

use crate::sqlquack_analysis::constants::{
    DUCKDB_DIALECT, QUOTE, RELATION_PART_SEPARATOR, SNOWFLAKE_DIALECT,
};

/// Known relations' columns by resolved name.
#[derive(Debug, Default)]
pub(crate) struct RelationColumns(pub(crate) HashMap<String, Vec<SchemaColumn>>);

impl LineageSchema for RelationColumns {
    fn columns(&self, relation: &str) -> Option<&[SchemaColumn]> {
        self.0.get(relation).map(Vec::as_slice)
    }
}

/// SQLQuack's dialect and profile for an analysis dialect it covers.
pub(crate) fn sqlquack_dialect(dialect: &str) -> Option<(Dialect, &'static dyn DialectProfile)> {
    match dialect {
        SNOWFLAKE_DIALECT => Some((Dialect::Snowflake, &SNOWFLAKE)),
        DUCKDB_DIALECT => Some((Dialect::DuckDB, &DUCKDB)),
        _ => None,
    }
}

/// A relation name, each dot-separated part resolved as written unquoted.
pub(crate) fn relation_key(name: &str, profile: &dyn DialectProfile) -> String {
    name.split(RELATION_PART_SEPARATOR)
        .map(|part| profile.resolved_identifier(part, false))
        .collect::<Vec<String>>()
        .join(RELATION_PART_SEPARATOR)
}

/// A relation's columns: a name in double quotes, or listed elsewhere in them, is quoted.
pub(crate) fn schema_columns<'a>(
    names: impl Iterator<Item = &'a str> + Clone,
) -> Vec<SchemaColumn> {
    let quoted: Vec<&str> = names.clone().filter_map(unquoted).collect();
    let mut columns: Vec<SchemaColumn> = Vec::new();
    for name in names {
        let column: SchemaColumn = match unquoted(name) {
            Some(inner) => SchemaColumn {
                name: inner.to_owned(),
                quoted: true,
            },
            None => SchemaColumn {
                name: name.to_owned(),
                quoted: quoted.contains(&name),
            },
        };
        if !columns.contains(&column) {
            columns.push(column);
        }
    }
    columns
}

fn unquoted(name: &str) -> Option<&str> {
    name.strip_prefix(QUOTE)?.strip_suffix(QUOTE)
}
