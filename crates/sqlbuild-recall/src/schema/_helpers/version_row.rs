//! The insert recording this build's Recall schema version.

use crate::schema::_helpers::rendering::clickhouse_table;
use crate::schema::constants::{
    DDL_SCRIPT_INVOCATION, META_SCHEMA_COMPONENT, META_SCHEMA_VERSION_KEY, RECALL_SCHEMA_VERSION,
};
use crate::schema::main::table_spec::table_spec;
use crate::schema::models::{DdlOptions, Dialect, Table};

/// The `recall_meta` row recording the schema version, so writers allowed only `SELECT` and `INSERT` find the store complete.
fn clickhouse_version_row(options: &DdlOptions) -> String {
    format!(
        "INSERT INTO {meta} (component, key, value, recorded_at, invocation_id, \
         invocation_started_at, schema_version, producer_version, ingested_at, content_hash)\n\
         SELECT '{META_SCHEMA_COMPONENT}', '{META_SCHEMA_VERSION_KEY}', \
         '{RECALL_SCHEMA_VERSION}', now64(6), '{DDL_SCRIPT_INVOCATION}', now64(6), \
         {RECALL_SCHEMA_VERSION}, '{version}', now64(6), ''",
        meta = clickhouse_table(table_spec(Table::Meta).name, options),
        version = options.producer_version,
    )
}

/// The SQLite `recall_meta` row recording the schema version.
fn sqlite_version_row(options: &DdlOptions) -> String {
    let now = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
    format!(
        "INSERT OR IGNORE INTO \"{meta}\" (component, key, value, recorded_at, invocation_id, \
         invocation_started_at, schema_version, producer_version, ingested_at, content_hash)\n\
         VALUES ('{META_SCHEMA_COMPONENT}', '{META_SCHEMA_VERSION_KEY}', \
         '{RECALL_SCHEMA_VERSION}', {now}, '{DDL_SCRIPT_INVOCATION}', {now}, \
         {RECALL_SCHEMA_VERSION}, '{version}', {now}, '')",
        meta = table_spec(Table::Meta).name,
        version = options.producer_version,
    )
}

/// The insert recording this build's schema version, which a writer allowed only `SELECT` and `INSERT` may also run.
#[must_use]
pub(crate) fn version_row_statement(dialect: Dialect, options: &DdlOptions) -> String {
    match dialect {
        Dialect::Sqlite => sqlite_version_row(options),
        Dialect::ClickHouse => clickhouse_version_row(options),
    }
}
