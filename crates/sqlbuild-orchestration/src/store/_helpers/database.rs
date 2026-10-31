//! Typed binding and row decoding for the two control-store engines.

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::types::{Value as SqliteValue, ValueRef};
use serde_json::Value;
use sqlbuild_recall::schema::models::ColumnType;
use sqlx_core::{column::Column, row::Row as _, type_info::TypeInfo, value::ValueRef as _};
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::models::Parameter;
use crate::store::types::Row;

pub(crate) fn sqlite_value(parameter: &Parameter) -> SqliteValue {
    match parameter {
        Parameter::Null(_) => SqliteValue::Null,
        Parameter::Text(value) => SqliteValue::Text(value.clone()),
        Parameter::OptionalText(value) => {
            value.clone().map_or(SqliteValue::Null, SqliteValue::Text)
        }
        Parameter::Bytes(value) => SqliteValue::Blob(value.clone()),
        Parameter::Integer(value) => SqliteValue::Integer(*value),
        Parameter::Real(value) => SqliteValue::Real(*value),
        Parameter::Json(value) => SqliteValue::Text(value.to_string()),
        Parameter::Boolean(value) => SqliteValue::Integer(i64::from(*value)),
        Parameter::Id(value) => SqliteValue::Text(value.to_string()),
        Parameter::OptionalId(value) => value.map_or(SqliteValue::Null, |value| {
            SqliteValue::Text(value.to_string())
        }),
        Parameter::Time(value) => {
            SqliteValue::Text(value.to_rfc3339_opts(SecondsFormat::Micros, true))
        }
        Parameter::OptionalTime(value) => value.map_or(SqliteValue::Null, |value| {
            SqliteValue::Text(value.to_rfc3339_opts(SecondsFormat::Micros, true))
        }),
    }
}

pub(crate) fn sqlite_query(
    connection: &rusqlite::Connection,
    sql: &str,
    parameters: &[Parameter],
) -> Result<Vec<Row>, ControlError> {
    let values: Vec<SqliteValue> = parameters.iter().map(sqlite_value).collect();
    let mut statement = connection.prepare(sql)?;
    let columns: Vec<String> = statement
        .column_names()
        .into_iter()
        .map(str::to_owned)
        .collect();
    let declared_types: Vec<Option<String>> = statement
        .columns()
        .into_iter()
        .map(|column| column.decl_type().map(str::to_ascii_uppercase))
        .collect();
    for (index, value) in values.into_iter().enumerate() {
        let name = format!("${}", index + 1);
        let position = statement
            .parameter_index(&name)?
            .ok_or_else(|| ControlError::Invalid(format!("missing SQL parameter {name}")))?;
        statement.raw_bind_parameter(position, value)?;
    }
    let mut query = statement.raw_query();
    let mut result: Vec<Row> = Vec::new();
    while let Some(row) = query.next()? {
        let mut mapped = Row::new();
        for (index, name) in columns.iter().enumerate() {
            let value = match row.get_ref(index)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(value) if declared_types[index].as_deref() == Some("BOOLEAN") => {
                    Value::Bool(value != 0)
                }
                ValueRef::Integer(value) => Value::from(value),
                ValueRef::Real(value) => Value::from(value),
                ValueRef::Text(value) => Value::from(String::from_utf8_lossy(value).into_owned()),
                ValueRef::Blob(value) => Value::from(value.to_vec()),
            };
            let value = if declared_types[index].as_deref() == Some("JSON") {
                value.as_str().map_or_else(
                    || Ok(value.clone()),
                    |text| {
                        serde_json::from_str(text)
                            .map_err(|error| ControlError::Invalid(error.to_string()))
                    },
                )?
            } else {
                value
            };
            mapped.insert(name.clone(), value);
        }
        result.push(mapped);
    }
    Ok(result)
}

pub(crate) async fn postgres_query(
    transaction: &mut sqlx_core::transaction::Transaction<'static, sqlx_postgres::Postgres>,
    sql: &str,
    parameters: Vec<Parameter>,
) -> Result<Vec<Row>, ControlError> {
    let mut query = sqlx_core::query::query(sql);
    for parameter in parameters {
        query = match parameter {
            Parameter::Null(kind) => match kind {
                ColumnType::Text => query.bind(None::<String>),
                ColumnType::Timestamp => query.bind(None::<DateTime<Utc>>),
                ColumnType::Json => query.bind(None::<Value>),
                ColumnType::Integer => query.bind(None::<i64>),
                ColumnType::Boolean => query.bind(None::<bool>),
                ColumnType::Real => query.bind(None::<f64>),
                ColumnType::Blob => query.bind(None::<Vec<u8>>),
            },
            Parameter::Text(value) => query.bind(value),
            Parameter::OptionalText(value) => query.bind(value),
            Parameter::Bytes(value) => query.bind(value),
            Parameter::Integer(value) => query.bind(value),
            Parameter::Real(value) => query.bind(value),
            Parameter::Json(value) => query.bind(value),
            Parameter::Boolean(value) => query.bind(value),
            Parameter::Id(value) => query.bind(value),
            Parameter::OptionalId(value) => query.bind(value),
            Parameter::Time(value) => query.bind(value),
            Parameter::OptionalTime(value) => query.bind(value),
        };
    }
    let mut result: Vec<Row> = Vec::new();
    for row in query.fetch_all(&mut **transaction).await? {
        let mut mapped = Row::new();
        for (index, column) in row.columns().iter().enumerate() {
            let raw = row.try_get_raw(index)?;
            let value = if raw.is_null() {
                Value::Null
            } else {
                match column.type_info().name() {
                    "BOOL" => Value::from(row.try_get::<bool, _>(index)?),
                    "INT8" => Value::from(row.try_get::<i64, _>(index)?),
                    "INT4" => Value::from(row.try_get::<i32, _>(index)?),
                    "FLOAT8" => Value::from(row.try_get::<f64, _>(index)?),
                    "UUID" => Value::from(row.try_get::<Uuid, _>(index)?.to_string()),
                    "TIMESTAMPTZ" => Value::from(
                        row.try_get::<DateTime<Utc>, _>(index)?
                            .to_rfc3339_opts(SecondsFormat::Micros, true),
                    ),
                    "JSONB" | "JSON" => row.try_get::<Value, _>(index)?,
                    "BYTEA" => Value::from(row.try_get::<Vec<u8>, _>(index)?),
                    _ => Value::from(row.try_get::<String, _>(index)?),
                }
            };
            mapped.insert(column.name().to_owned(), value);
        }
        result.push(mapped);
    }
    Ok(result)
}
