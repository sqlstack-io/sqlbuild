//! One schema template, rendered only at the database type boundary.

use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

use crate::store::_helpers::recall_schema::recall_schema;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::store::types::Row;

pub(crate) const SCHEMA: &str = include_str!("../schema.sql");

pub(crate) fn schema_digest(store: &ControlStore) -> String {
    sha256_hex(format!("{}\n{SCHEMA}", recall_schema(store).join(";\n")).as_bytes())
}

/// Accept only the exact schema this server creates.
pub(crate) fn verify_schema(rows: &[Row], digest: &str) -> Result<(), ControlError> {
    if rows
        .iter()
        .any(|row| row.get("checksum").and_then(|value| value.as_str()) != Some(digest))
    {
        return Err(ControlError::Schema(
            "control store schema checksum mismatch; recreate the control store, because \
             pre-release schemas are not upgraded"
                .into(),
        ));
    }
    Ok(())
}

pub(crate) fn render(store: &ControlStore, sql: &str) -> String {
    let postgres = matches!(store, ControlStore::Postgres(_));
    sql.replace("{uuid}", if postgres { "UUID" } else { "TEXT" })
        .replace("{time}", if postgres { "TIMESTAMPTZ" } else { "TEXT" })
        .replace("{binary}", if postgres { "BYTEA" } else { "BLOB" })
        .replace("{json}", if postgres { "JSONB" } else { "JSON" })
}
