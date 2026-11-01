//! Dialect profiles and relation schemas as SQLQuack's lineage reads them.

use std::collections::HashMap;

use sqlquack_semantics::dialects::constants::{DUCKDB, SNOWFLAKE};
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_semantics::lineage::models::{Nullability, NullabilityRule, SchemaColumn};
use sqlquack_semantics::lineage::types::LineageSchema;
use sqlquack_syntax::Dialect;

use crate::sqlquack_analysis::constants::{
    CONDITIONAL_RESULT_RULE, DUCKDB_DIALECT, FIRST_ARGUMENT_RULE, QUOTE, RELATION_PART_SEPARATOR,
    SAFE_CAST_RULE, SNOWFLAKE_DIALECT,
};
use crate::sqlquack_analysis::models::CompileFunctions;

/// Known relations' columns by resolved name.
#[derive(Debug, Default)]
pub(crate) struct RelationColumns(pub(crate) HashMap<String, Vec<SchemaColumn>>);

impl LineageSchema for RelationColumns {
    fn columns(&self, relation: &str) -> Option<&[SchemaColumn]> {
        self.0.get(relation).map(Vec::as_slice)
    }
}

/// Relations with every column known, the typed columns of relations not fully known, and functions.
#[derive(Debug, Default)]
pub(crate) struct TypedRelations<'a> {
    pub(crate) closed: HashMap<String, Vec<SchemaColumn>>,
    pub(crate) open: HashMap<String, Vec<SchemaColumn>>,
    pub(crate) functions: CompileFunctions<'a>,
}

impl LineageSchema for TypedRelations<'_> {
    fn columns(&self, relation: &str) -> Option<&[SchemaColumn]> {
        self.closed.get(relation).map(Vec::as_slice)
    }

    fn open_columns(&self, relation: &str) -> Option<&[SchemaColumn]> {
        self.open.get(relation).map(Vec::as_slice)
    }

    fn function_type(&self, name: &str) -> Option<&str> {
        named(self.functions.return_types, name)
    }

    fn function_nullability(&self, name: &str) -> Option<NullabilityRule> {
        match named(self.functions.nullability_rules, name)? {
            FIRST_ARGUMENT_RULE => Some(NullabilityRule::FirstArgument),
            CONDITIONAL_RESULT_RULE => Some(NullabilityRule::ConditionalResult),
            SAFE_CAST_RULE => Some(NullabilityRule::SafeCast),
            _ => None,
        }
    }
}

/// The value `pairs` give the function `name`, matched case-insensitively.
fn named<'a>(pairs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|(function, _)| function.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
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
                data_type: None,
                nullability: Nullability::Unknown,
            },
            None => SchemaColumn {
                name: name.to_owned(),
                quoted: quoted.contains(&name),
                data_type: None,
                nullability: Nullability::Unknown,
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
