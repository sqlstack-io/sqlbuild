//! Insert native Recall rows and identify retry conflicts within a control transaction.

use std::collections::HashSet;

use chrono::{DateTime, SecondsFormat};
use sqlbuild_recall::records::models::{RecallRow, RecallValue};
use sqlbuild_recall::schema::main::postgres_partition_statement::postgres_partition_statement;
use sqlbuild_recall::schema::main::table_columns::table_columns;
use sqlbuild_recall::schema::main::table_spec::table_spec;
use sqlbuild_recall::schema::models::{ColumnType, Table};
use sqlbuild_recall::store::main::conflict_evidence::conflict_evidence;
use sqlbuild_recall::store::main::relational_insert::relational_insert;
use sqlbuild_recall::store::main::relational_lookup::relational_lookup;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{Connection, ControlTransaction, Parameter, RecallAppendOutcome};

fn parameters(values: &[(&RecallValue, ColumnType)]) -> Result<Vec<Parameter>, ControlError> {
    values
        .iter()
        .map(|(value, kind)| {
            Ok(match (value, kind) {
                (RecallValue::Null, kind) => Parameter::Null(*kind),
                (RecallValue::Integer(value), ColumnType::Boolean) => {
                    Parameter::Boolean(*value != 0)
                }
                (RecallValue::Integer(value), _) => Parameter::Integer(*value),
                (RecallValue::Real(value), _) => Parameter::Real(*value),
                (RecallValue::Text(value), ColumnType::Json) => Parameter::Json(
                    serde_json::from_str(value)
                        .map_err(|error| ControlError::Invalid(error.to_string()))?,
                ),
                (RecallValue::Text(value), ColumnType::Timestamp) => Parameter::Time(
                    DateTime::parse_from_rfc3339(value)
                        .map_err(|error| ControlError::Invalid(error.to_string()))?
                        .to_utc(),
                ),
                (RecallValue::Text(value), _) => Parameter::Text(value.clone()),
                (RecallValue::Blob(value), _) => Parameter::Bytes(value.clone()),
            })
        })
        .collect()
}

pub(crate) fn text<'a>(row: &'a RecallRow, name: &str) -> Result<&'a str, ControlError> {
    match row.value(name) {
        Some(RecallValue::Text(value)) => Ok(value),
        _ => Err(ControlError::Invalid(format!("Recall {name} must be text"))),
    }
}

pub(crate) async fn write_row(
    mut transaction: ControlTransaction,
    row: &RecallRow,
) -> Result<(ControlTransaction, RecallAppendOutcome), ControlError> {
    let postgres = matches!(transaction.connection, Some(Connection::Postgres(_)));
    if postgres && row.table != Table::Artifacts {
        let date = DateTime::parse_from_rfc3339(text(row, table_spec(row.table).expiry_column)?)
            .map_err(|error| ControlError::Invalid(error.to_string()))?
            .to_utc()
            .date_naive();
        let statement = postgres_partition_statement(row.table, date)
            .ok_or_else(|| ControlError::Invalid("Recall partition date is out of range".into()))?;
        (transaction, _) = query(transaction, &statement, vec![]).await?;
    }
    let ingested = RecallValue::Text(transaction.now.to_rfc3339_opts(SecondsFormat::Micros, true));
    let columns: HashSet<String> = table_columns(row.table)
        .into_iter()
        .map(|column| column.name.to_owned())
        .collect();
    let insert = relational_insert((row, &columns), &ingested, postgres);
    let returning = if row.table == Table::Events {
        "storage_id"
    } else {
        "1 AS inserted"
    };
    let (transaction, rows) = query(
        transaction,
        &format!("{} RETURNING {returning}", insert.sql),
        parameters(&insert.values)?,
    )
    .await?;
    if let Some(found) = rows.first() {
        return Ok((
            transaction,
            RecallAppendOutcome {
                inserted: true,
                storage_id: found.get("storage_id").and_then(serde_json::Value::as_i64),
                conflict: None,
            },
        ));
    }
    let lookup = relational_lookup(row, postgres, true);
    let (transaction, rows) = query(transaction, &lookup.sql, parameters(&lookup.values)?).await?;
    let found = rows.first().ok_or_else(|| {
        ControlError::Invalid("Recall duplicate has no matching stored key".into())
    })?;
    let existing = found
        .get("content_hash")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ControlError::Invalid("stored Recall row has no content hash".into()))?;
    Ok((
        transaction,
        RecallAppendOutcome {
            inserted: false,
            storage_id: found.get("storage_id").and_then(serde_json::Value::as_i64),
            conflict: conflict_evidence(row, existing),
        },
    ))
}
