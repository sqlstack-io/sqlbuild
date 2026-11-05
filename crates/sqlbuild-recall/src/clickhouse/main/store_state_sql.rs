//! `store_state_sql`: what setup reads about a ClickHouse store.

use crate::clickhouse::_helpers::literals::string_literal;
use crate::clickhouse::models::StoreStateSql;
use crate::schema::constants::{
    ALL_TABLES, META_SCHEMA_COMPONENT, META_SCHEMA_VERSION_KEY, VIEW_NAMES,
};
use crate::schema::main::clickhouse_relation::clickhouse_relation;
use crate::schema::main::table_spec::table_spec;
use crate::schema::models::{DdlOptions, Table};

/// The relation, column and schema-version queries of the store in `database`.
#[must_use]
pub fn store_state_sql(database: &str) -> StoreStateSql {
    let mut names: Vec<String> = Vec::new();
    for table in ALL_TABLES {
        names.push(string_literal(table_spec(table).name));
    }
    let tables: String = names.join(", ");
    for view in VIEW_NAMES {
        names.push(string_literal(view));
    }
    let views: String = VIEW_NAMES
        .iter()
        .map(|view| string_literal(view))
        .collect::<Vec<String>>()
        .join(", ");
    let database_literal: String = string_literal(database);
    let options = DdlOptions {
        database: Some(database.to_owned()),
        retention_days: 0,
        cluster: None,
        producer_version: String::new(),
    };
    StoreStateSql {
        relations: format!(
            "SELECT name, toUInt8(engine = 'View') AS is_view, create_table_query \
             FROM system.tables WHERE database = {database_literal} AND name IN ({})",
            names.join(", ")
        ),
        columns: format!(
            "SELECT table, name, type FROM system.columns \
             WHERE database = {database_literal} AND table IN ({tables}) ORDER BY table, position"
        ),
        schema_version: format!(
            "SELECT toInt64(max(toInt64OrZero(value))) AS schema_version, \
             toInt64(dateDiff('second', max(recorded_at), now64(6))) AS age_seconds FROM {} \
             WHERE component = {} AND key = {}",
            clickhouse_relation(table_spec(Table::Meta).name, &options),
            string_literal(META_SCHEMA_COMPONENT),
            string_literal(META_SCHEMA_VERSION_KEY)
        ),
        view_columns: format!(
            "SELECT table, name FROM system.columns \
             WHERE database = {database_literal} AND table IN ({views})"
        ),
    }
}
