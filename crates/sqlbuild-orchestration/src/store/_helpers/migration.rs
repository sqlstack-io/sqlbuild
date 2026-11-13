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

/// Accept only the exact schema this server creates, or say how to start a new store.
pub(crate) fn verify_schema(
    store: &ControlStore,
    rows: &[Row],
    digest: &str,
) -> Result<(), ControlError> {
    if rows
        .iter()
        .all(|row| row.get("checksum").and_then(|value| value.as_str()) == Some(digest))
    {
        return Ok(());
    }
    let quote = |text: &str| format!("'{}'", text.replace('\'', r"'\''"));
    let command = match store {
        ControlStore::Sqlite(path) => {
            let path = path.to_string_lossy();
            format!(
                "rm -f {} {} {}",
                quote(&path),
                quote(&format!("{path}-wal")),
                quote(&format!("{path}-shm"))
            )
        }
        ControlStore::Postgres(pool) => {
            let options = pool.connect_options();
            let target = format!(
                "-h {} -p {} -U {} {}",
                quote(options.get_host()),
                options.get_port(),
                quote(options.get_username()),
                quote(options.get_database().unwrap_or(options.get_username()))
            );
            format!("dropdb {target} && createdb {target}")
        }
    };
    Err(ControlError::Schema(format!(
        "this control store was created by an earlier sqb release and cannot be upgraded; \
         to start a new, empty store, which discards its run history, stop every server \
         using it and run: {command}"
    )))
}

pub(crate) fn render(store: &ControlStore, sql: &str) -> String {
    let postgres = matches!(store, ControlStore::Postgres(_));
    sql.replace("{uuid}", if postgres { "UUID" } else { "TEXT" })
        .replace("{time}", if postgres { "TIMESTAMPTZ" } else { "TEXT" })
        .replace("{binary}", if postgres { "BYTEA" } else { "BLOB" })
        .replace("{json}", if postgres { "JSONB" } else { "JSON" })
}
