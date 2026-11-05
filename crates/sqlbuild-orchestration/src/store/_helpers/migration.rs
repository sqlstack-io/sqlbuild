//! One schema template, rendered only at the database type boundary.

use semver::Version;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

use crate::store::_helpers::recall_schema::recall_schema;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::store::types::Row;

pub(crate) const SCHEMA: &str = include_str!("../schema.sql");

pub(crate) fn schema_digest(store: &ControlStore) -> String {
    sha256_hex(format!("{}\n{SCHEMA}", recall_schema(store).join(";\n")).as_bytes())
}

/// Accept only the exact schema this server creates, recorded by a server no newer than this one.
pub(crate) fn verify_schema(rows: &[Row], digest: &str) -> Result<Version, ControlError> {
    let current = Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|error| ControlError::Schema(error.to_string()))?;
    for row in rows {
        let minimum = row
            .get("min_server_version")
            .and_then(|value| value.as_str())
            .ok_or_else(|| ControlError::Schema("schema lacks a minimum server version".into()))?;
        let minimum =
            Version::parse(minimum).map_err(|error| ControlError::Schema(error.to_string()))?;
        if minimum > current || row.get("checksum").and_then(|value| value.as_str()) != Some(digest)
        {
            return Err(ControlError::Schema(
                "control store schema checksum mismatch or minimum server version not met".into(),
            ));
        }
    }
    Ok(current)
}

pub(crate) fn render(store: &ControlStore, sql: &str) -> String {
    let postgres = matches!(store, ControlStore::Postgres(_));
    sql.replace("{uuid}", if postgres { "UUID" } else { "TEXT" })
        .replace("{time}", if postgres { "TIMESTAMPTZ" } else { "TEXT" })
        .replace("{binary}", if postgres { "BYTEA" } else { "BLOB" })
        .replace("{json}", if postgres { "JSONB" } else { "JSON" })
}
