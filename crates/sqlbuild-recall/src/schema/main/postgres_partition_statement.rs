//! Monthly partition bounds are calculated from a typed date, never interpolated user SQL.

use chrono::{Datelike, Months, NaiveDate};
use sqlbuild_core::identifier_text::main::quote_identifier::quote_identifier as sqlite_identifier;

use crate::schema::main::table_spec::table_spec;
use crate::schema::models::Table;

/// A replay uses the same timestamp partition and therefore the same deterministic unique key.
#[must_use]
pub fn postgres_partition_statement(table: Table, date: NaiveDate) -> Option<String> {
    if table == Table::Artifacts {
        return None;
    }
    let first = date.with_day(1)?;
    let next = first.checked_add_months(Months::new(1))?;
    let spec = table_spec(table);
    let partition = format!("{}_{}", spec.name, first.format("%Y%m"));
    Some(format!(
        "CREATE TABLE IF NOT EXISTS {} PARTITION OF {} FOR VALUES FROM ('{first}T00:00:00Z') TO ('{next}T00:00:00Z')",
        sqlite_identifier(&partition),
        sqlite_identifier(spec.name)
    ))
}
