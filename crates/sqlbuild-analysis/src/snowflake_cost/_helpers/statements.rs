//! Query-type prefixes, Python whitespace and CREATE statement parsing.

use polyglot_sql::{Dialect, DialectType, Expression, ParseOptions};

use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::models::AnalysisRoute;
use crate::snowflake_cost::constants::{
    CREATE_VARIANT_PREFIX, DYNAMIC_TABLE_MODIFIER, SNOWFLAKE_DIALECT,
};

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

/// Whether a CREATE statement's text parses as a definition that needs no warehouse; false, the
/// warehouse-billed reading, when the parser cannot read it whole.
pub(crate) fn metadata_only_create(query_text: &str) -> bool {
    if let AnalysisRoute::Native(native) = analysis_route(Some(SNOWFLAKE_DIALECT)) {
        return native
            .read_whole(query_text)
            .is_ok_and(|statement| native_metadata_only(&statement));
    }
    let options: ParseOptions = ParseOptions {
        complexity_guard: None,
    };
    let Ok(mut statements) =
        Dialect::get(DialectType::Snowflake).parse_with_options(query_text, &options)
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

/// Whether a statement SQLQuack read is a CREATE that needs no warehouse.
fn native_metadata_only(statement: &sqlquack_syntax::Expression) -> bool {
    match statement {
        sqlquack_syntax::Expression::CreateTable(table) => {
            !(table.as_select.is_some()
                || table.clone_source.is_some()
                || table.using_template.is_some()
                || table.table_modifier.as_deref().unwrap_or("").to_uppercase()
                    == DYNAMIC_TABLE_MODIFIER)
        }
        sqlquack_syntax::Expression::CreateView(view) if view.materialized => false,
        statement => statement.variant_name().starts_with(CREATE_VARIANT_PREFIX),
    }
}
