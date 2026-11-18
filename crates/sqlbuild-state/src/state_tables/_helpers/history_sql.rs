//! The SQL pieces a keyset history page is built from: byte-wise comparisons, the newest versions and the cursor filter.

use crate::state_tables::constants::{NEWEST_FIRST_COLUMN, NEWEST_VERSION_COLUMN};
use crate::state_tables::models::NewestVersions;

/// Session and column collations must not merge case, accents or trailing spaces at page boundaries.
pub(crate) fn binary(value: &str, adapter: &str) -> Result<String, String> {
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
                "warehouse history does not support adapter '{adapter}'"
            ));
        }
    })
}

/// The table a read pages through, or each partition's newest observations and their filter.
pub(crate) fn versioned_source(
    qualified_name: &str,
    timestamp_column: &str,
    newest: Option<NewestVersions<'_>>,
) -> (String, Option<String>) {
    let Some(newest) = newest else {
        return (qualified_name.to_owned(), None);
    };
    let partition = newest.partition.join(", ");
    let mut order = vec![format!("{NEWEST_FIRST_COLUMN} DESC")];
    order.extend(
        newest
            .observation
            .iter()
            .map(|column| format!("{column} DESC")),
    );
    (
        format!(
            "(SELECT *, DENSE_RANK() OVER (PARTITION BY {partition} ORDER BY {}) AS {NEWEST_VERSION_COLUMN} \
             FROM (SELECT *, MIN({timestamp_column}) OVER (PARTITION BY {partition}, {}) AS {NEWEST_FIRST_COLUMN} \
             FROM {qualified_name}) AS sqb_observations) AS sqb_versions",
            order.join(", "),
            newest.observation.join(", ")
        ),
        Some(format!("{NEWEST_VERSION_COLUMN} <= {}", newest.versions)),
    )
}

/// The rows after a page's last cursor values, in key order; `None` for an empty cursor.
pub(crate) fn after_filter(
    (columns, ordered): (&[&str], &[String]),
    after: &[Option<String>],
    (timestamp_column, adapter): (&str, &str),
) -> Result<Option<String>, String> {
    let mut greater: Vec<String> = Vec::new();
    let mut equal: Vec<String> = Vec::new();
    for ((column, ordered), value) in columns.iter().zip(ordered).zip(after) {
        let literal = value
            .as_ref()
            .map(|value| {
                if *column == timestamp_column {
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
    Ok((!greater.is_empty()).then(|| format!("({})", greater.join(" OR "))))
}
