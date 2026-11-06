//! `upgrade_statements`: what an administrator runs to bring an observed store up to this build's schema.

use sqlbuild_core::identifier_text::main::quote_identifier::quote_identifier as sqlite_identifier;

use crate::schema::_helpers::version_row::version_row_statement;
use crate::schema::constants::{ALL_TABLES, RECALL_SCHEMA_VERSION};
use crate::schema::main::add_column_statement::add_column_statement;
use crate::schema::main::ddl_statements::ddl_statements;
use crate::schema::main::table_columns::table_columns;
use crate::schema::main::table_spec::table_spec;
use crate::schema::main::table_statements::table_statements;
use crate::schema::main::view_statements::view_statements;
use crate::schema::models::{DdlOptions, Dialect, ObservedSchema};

/// The additive statements `observed` needs, in order: missing tables, missing columns, views, then the schema version row. Empty when the store is current; the whole creation script when it has no Recall tables.
#[must_use]
pub fn upgrade_statements(
    dialect: Dialect,
    options: &DdlOptions,
    observed: &ObservedSchema,
) -> Vec<String> {
    if observed.tables.is_empty() {
        return ddl_statements(dialect, options);
    }
    let mut statements: Vec<String> = Vec::new();
    if ALL_TABLES
        .iter()
        .any(|table| !observed.tables.contains_key(table))
    {
        statements.extend(table_statements(dialect, options));
    }
    for (table, present) in &observed.tables {
        for column in table_columns(*table) {
            if !present.contains(column.name) {
                statements.push(add_column_statement(
                    dialect,
                    options,
                    (table_spec(*table), &column),
                ));
            }
        }
    }
    let upgrade: bool = observed
        .version
        .is_none_or(|version| version < RECALL_SCHEMA_VERSION);
    for (name, statement) in view_statements(dialect, options) {
        match (dialect, upgrade) {
            (Dialect::Sqlite, true) => {
                statements.push(format!("DROP VIEW IF EXISTS {}", sqlite_identifier(name)));
                statements.push(statement);
            }
            (Dialect::ClickHouse, true) => statements.push(statement),
            (_, false) if observed.views.contains(name) => {}
            (Dialect::Sqlite, false) => statements.push(statement),
            (Dialect::ClickHouse, false) => statements.push(statement.replacen(
                "CREATE OR REPLACE VIEW",
                "CREATE VIEW IF NOT EXISTS",
                1,
            )),
        }
    }
    if upgrade {
        statements.push(version_row_statement(dialect, options));
    }
    statements
}
