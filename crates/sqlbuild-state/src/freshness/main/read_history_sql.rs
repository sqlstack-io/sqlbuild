//! Keyset pages preserve distinct observations without rereading or ranking all preceding rows.

use crate::freshness::constants::{
    OBSERVED_AT_COLUMN, SOURCE_FRESHNESS_COLUMN_TYPES, SOURCE_FRESHNESS_EVENT_ID, SQLSERVER_ADAPTER,
};

/// Cursor values are already rendered by the adapter; `None` sorts before every SQL value.
pub fn read_history_sql(
    qualified_name: &str,
    after: Option<&[Option<String>; 11]>,
    observed_since: Option<&str>,
    (limit, adapter, has_event_id): (usize, &str, bool),
) -> Result<String, String> {
    let mut columns: Vec<&str> = SOURCE_FRESHNESS_COLUMN_TYPES
        .iter()
        .map(|(name, _)| *name)
        .collect();
    let mut projection = columns.join(", ");
    projection.push_str(if has_event_id {
        ", event_id"
    } else {
        ", NULL AS event_id"
    });
    columns.push(if has_event_id {
        SOURCE_FRESHNESS_EVENT_ID
    } else {
        "NULL"
    });
    let ordered = columns
        .iter()
        .map(|column| {
            if *column == OBSERVED_AT_COLUMN {
                Ok((*column).to_owned())
            } else {
                binary(column, adapter)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut greater: Vec<String> = Vec::new();
    let mut equal: Vec<String> = Vec::new();
    if let Some(after) = after {
        for ((column, ordered), value) in columns.iter().zip(&ordered).zip(after) {
            let literal = value
                .as_ref()
                .map(|value| {
                    if *column == OBSERVED_AT_COLUMN {
                        Ok(value.clone())
                    } else {
                        binary(value, adapter)
                    }
                })
                .transpose()?;
            let comparison = literal.as_ref().map_or_else(
                || format!("{column} IS NOT NULL"),
                |literal| format!("{ordered} > {literal}"),
            );
            let prefix = if equal.is_empty() {
                String::new()
            } else {
                format!("{} AND ", equal.join(" AND "))
            };
            greater.push(format!("({prefix}{comparison})"));
            equal.push(literal.as_ref().map_or_else(
                || format!("{column} IS NULL"),
                |literal| format!("{ordered} = {literal}"),
            ));
        }
    }
    let mut filters: Vec<String> = Vec::new();
    if !greater.is_empty() {
        filters.push(format!("({})", greater.join(" OR ")));
    }
    if let Some(since) = observed_since {
        filters.push(format!("{OBSERVED_AT_COLUMN} >= {since}"));
    }
    let filter = if filters.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", filters.join(" AND "))
    };
    let order: Vec<String> = columns
        .iter()
        .zip(&ordered)
        .map(|(column, ordered)| format!("CASE WHEN {column} IS NULL THEN 0 ELSE 1 END, {ordered}"))
        .collect();
    let top = adapter == SQLSERVER_ADAPTER;
    let head = if top {
        format!("TOP {limit} ")
    } else {
        String::new()
    };
    let tail = if top {
        String::new()
    } else {
        format!(" LIMIT {limit}")
    };
    Ok(format!(
        "SELECT {head}{projection} FROM {qualified_name}{filter} ORDER BY {}{tail}",
        order.join(", ")
    ))
}

/// Session and column collations must not merge case, accents or trailing spaces at page boundaries.
fn binary(value: &str, adapter: &str) -> Result<String, String> {
    Ok(match adapter {
        "duckdb" | "motherduck" => format!("encode({value})"),
        "sqlite" => format!("CAST({value} AS BLOB)"),
        "postgres" => format!("convert_to({value}, 'UTF8')"),
        "snowflake" => format!("TO_BINARY({value}, 'UTF-8')"),
        "bigquery" => format!("CAST({value} AS BYTES)"),
        "databricks" => format!("CAST({value} AS BINARY)"),
        "sqlserver" => format!("CONVERT(VARBINARY(MAX), CONVERT(NVARCHAR(MAX), {value}))"),
        "clickhouse" => value.to_owned(),
        _ => {
            return Err(format!(
                "source freshness history does not support adapter '{adapter}'"
            ));
        }
    })
}
