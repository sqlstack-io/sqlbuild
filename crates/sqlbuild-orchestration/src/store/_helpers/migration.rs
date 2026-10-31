//! One schema template, rendered only at the database type boundary.

use semver::Version;

use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::store::types::Row;

pub(crate) const SCHEMA: &str = include_str!("../schema.sql");
pub(crate) const LOCATION_INSTANCES: &str = include_str!("../location_instances.sql");
pub(crate) const LOCATION_WORK: &str = include_str!("../location_work.sql");

pub(crate) fn verify_migrations(rows: &[Row], digests: &[String]) -> Result<Version, ControlError> {
    let current = Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|error| ControlError::Schema(error.to_string()))?;
    for (index, row) in rows.iter().enumerate() {
        let minimum = row
            .get("min_server_version")
            .and_then(|value| value.as_str())
            .ok_or_else(|| {
                ControlError::Schema("migration lacks a minimum server version".into())
            })?;
        let minimum =
            Version::parse(minimum).map_err(|error| ControlError::Schema(error.to_string()))?;
        if row.get("version").and_then(|value| value.as_u64()) != Some(index as u64 + 1)
            || minimum > current
            || row.get("checksum").and_then(|value| value.as_str())
                != digests.get(index).map(String::as_str)
            || index >= digests.len()
        {
            return Err(ControlError::Schema(
                "unknown migration, checksum mismatch, or minimum server version not met".into(),
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
