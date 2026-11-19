//! Query-type prefixes, Python whitespace and CREATE statement parsing.

use polyglot_sql::{Dialect, DialectType, Expression};

use crate::parse_guard::main::polyglot_parse_options::polyglot_parse_options;
use crate::snowflake_cost::constants::{CREATE_VARIANT_PREFIX, DYNAMIC_TABLE_MODIFIER};

pub(crate) fn of_kind(query_type: &str, kind: &str) -> bool {
    query_type == kind
        || query_type
            .strip_prefix(kind)
            .is_some_and(|rest| rest.starts_with('_'))
}

/// Python's `str.isspace` for one character.
pub(crate) fn python_space(character: char) -> bool {
    character.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&character)
}

/// Whether a CREATE statement's text parses as a definition that needs no warehouse.
pub(crate) fn metadata_only_create(query_text: &str) -> bool {
    let Ok(mut statements) = Dialect::get(DialectType::Snowflake)
        .parse_with_options(query_text, &polyglot_parse_options())
    else {
        return false;
    };
    if statements.len() != 1 {
        return false;
    }
    match statements.remove(0) {
        Expression::CreateTable(table) => {
            !(table.as_select.is_some()
                || table.clone_source.is_some()
                || table.using_template.is_some()
                || table.table_modifier.as_deref().unwrap_or("").to_uppercase()
                    == DYNAMIC_TABLE_MODIFIER)
        }
        Expression::CreateView(view) if view.materialized => false,
        statement => statement.variant_name().starts_with(CREATE_VARIANT_PREFIX),
    }
}
