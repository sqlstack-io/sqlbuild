//! One logical schema and index list for SQLite and PostgreSQL.

use sqlbuild_core::identifier_text::main::quote_identifier::quote_identifier as sqlite_identifier;

use crate::schema::_helpers::rendering::{all_columns, sqlite_type};
use crate::schema::constants::{ARTIFACTS_TABLE, EVENTS_TABLE};
use crate::schema::models::{ColumnType, TableSpec};

pub(crate) fn relational_table(spec: &TableSpec, postgres: bool, control: bool) -> Vec<String> {
    let mut definitions: Vec<String> = all_columns(spec)
        .iter()
        .map(|column| {
            let not_null = if column.nullable { "" } else { " NOT NULL" };
            let kind = if postgres {
                column.column_type.postgres_type()
            } else if control && column.column_type == ColumnType::Json {
                "JSON"
            } else {
                sqlite_type(column.column_type)
            };
            format!("    {} {kind}{not_null}", sqlite_identifier(column.name))
        })
        .collect();
    if control && spec.name == EVENTS_TABLE.name {
        definitions.push(if postgres {
            "    storage_id BIGINT GENERATED ALWAYS AS IDENTITY".into()
        } else {
            "    storage_id INTEGER PRIMARY KEY AUTOINCREMENT".into()
        });
    }
    let mut key: Vec<&str> = spec.key.to_vec();
    let partition = if postgres && spec.name != ARTIFACTS_TABLE.name {
        if !key.contains(&spec.expiry_column) {
            key.push(spec.expiry_column);
        }
        format!(
            " PARTITION BY RANGE ({})",
            sqlite_identifier(spec.expiry_column)
        )
    } else {
        String::new()
    };
    let key: Vec<String> = key.into_iter().map(sqlite_identifier).collect();
    definitions.push(format!("    UNIQUE ({})", key.join(", ")));
    let mut statements = vec![format!(
        "CREATE TABLE IF NOT EXISTS {} (\n{}\n){partition}",
        sqlite_identifier(spec.name),
        definitions.join(",\n")
    )];
    for (suffix, columns) in spec.sqlite_indexes {
        let columns: Vec<String> = columns.iter().map(|name| sqlite_identifier(name)).collect();
        statements.push(format!(
            "CREATE INDEX IF NOT EXISTS {} ON {} ({})",
            sqlite_identifier(&format!("{}_{suffix}", spec.name)),
            sqlite_identifier(spec.name),
            columns.join(", ")
        ));
    }
    statements
}
