//! One schema template, rendered only at the database type boundary.

use semver::Version;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

use crate::store::_helpers::recall_schema::recall_schema;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::store::types::Row;

pub(crate) const SCHEMA: &str = include_str!("../schema.sql");
pub(crate) const LOCATION_INSTANCES: &str = include_str!("../location_instances.sql");
pub(crate) const LOCATION_WORK: &str = include_str!("../location_work.sql");
pub(crate) const PLATFORM_SINK: &str = include_str!("../platform_sink.sql");
pub(crate) const SENSORS: &str = include_str!("../sensors.sql");
pub(crate) const RUN_SENSORS: &str = include_str!("../run_sensors.sql");
pub(crate) const EXECUTED_NODES: &str = include_str!("../executed_nodes.sql");
pub(crate) const NODE_SENSORS: &str = include_str!("../node_sensors.sql");
pub(crate) const WAREHOUSE_FRESHNESS: &str = include_str!("../warehouse_freshness.sql");
pub(crate) const WAREHOUSE_FINGERPRINTS: &str = include_str!("../warehouse_fingerprints.sql");
pub(crate) const DEVELOPMENT_LOCATIONS: &str = include_str!("../development_locations.sql");
pub(crate) const NODE_TRANSITIONS: &str = include_str!("../node_transitions.sql");
pub(crate) const PASSWORD_CREDENTIALS: &str = include_str!("../password_credentials.sql");
pub(crate) const PASSWORD_SESSIONS: &str = include_str!("../password_sessions.sql");
pub(crate) const AUTHENTICATION_REVISION: &str = include_str!("../authentication_revision.sql");
pub(crate) const OIDC_LOGIN_FLOWS: &str = include_str!("../oidc_login_flows.sql");
pub(crate) const ADMINISTRATOR_INITIALIZATION: &str =
    include_str!("../administrator_initialization.sql");
pub(crate) const EXTERNAL_SESSIONS: &str = include_str!("../external_sessions.sql");
pub(crate) const WAREHOUSE_RECONCILIATION: &str = include_str!("../warehouse_reconciliation.sql");
pub(crate) const AUDIT_STORAGE_ORDER: &str = include_str!("../audit_storage_order.sql");
pub(crate) const WAREHOUSE_AUDIT_RESULTS: &str = include_str!("../warehouse_audit_results.sql");
pub(crate) const WAREHOUSE_JANITOR_EVENTS: &str = include_str!("../warehouse_janitor_events.sql");
pub(crate) const WAREHOUSE_MIGRATIONS: &str = include_str!("../warehouse_migrations.sql");
pub(crate) const WAREHOUSE_MICROBATCHES: &str = include_str!("../warehouse_microbatches.sql");
pub(crate) const SOURCE_SENSORS: &str = include_str!("../source_sensors.sql");
pub(crate) const SOURCE_STALENESS: &str = include_str!("../source_staleness.sql");
pub(crate) const SECURITY_AUDIT_SINK: &str = include_str!("../security_audit_sink.sql");
pub(crate) const WAREHOUSE_CATALOG: &str = include_str!("../warehouse_catalog.sql");
pub(crate) const FINGERPRINT_LOOKUP: &str = include_str!("../fingerprint_lookup.sql");
pub(crate) const WAREHOUSE_STATISTICS: &str = include_str!("../warehouse_statistics.sql");
pub(crate) const CHECK_OBSERVATIONS: &str = include_str!("../check_observations.sql");
pub(crate) const TRIGGER_PROGRESS: &str = include_str!("../trigger_progress.sql");
pub(crate) const CLI_LOGIN_FLOWS: &str = include_str!("../cli_login_flows.sql");
pub(crate) const RETENTION_INDEXES: &str = include_str!("../retention_indexes.sql");
pub(crate) const CUSTOM_ROLES: &str = include_str!("../custom_roles.sql");
pub(crate) const WAREHOUSE_CONSUMPTION: &str = include_str!("../warehouse_consumption.sql");

pub(crate) const ADDITIONS: &[(&str, &str)] = &[
    ("location_instances", LOCATION_INSTANCES),
    ("location_work", LOCATION_WORK),
    ("platform_sink", PLATFORM_SINK),
    ("sensors", SENSORS),
    ("run_sensors", RUN_SENSORS),
    ("executed_nodes", EXECUTED_NODES),
    ("node_sensors", NODE_SENSORS),
    ("warehouse_freshness", WAREHOUSE_FRESHNESS),
    ("warehouse_fingerprints", WAREHOUSE_FINGERPRINTS),
    ("development_locations", DEVELOPMENT_LOCATIONS),
    ("node_transitions", NODE_TRANSITIONS),
    ("password_credentials", PASSWORD_CREDENTIALS),
    ("password_sessions", PASSWORD_SESSIONS),
    ("authentication_revision", AUTHENTICATION_REVISION),
    ("oidc_login_flows", OIDC_LOGIN_FLOWS),
    ("administrator_initialization", ADMINISTRATOR_INITIALIZATION),
    ("external_sessions", EXTERNAL_SESSIONS),
    ("warehouse_reconciliation", WAREHOUSE_RECONCILIATION),
    ("audit_storage_order", AUDIT_STORAGE_ORDER),
    ("warehouse_audit_results", WAREHOUSE_AUDIT_RESULTS),
    ("warehouse_janitor_events", WAREHOUSE_JANITOR_EVENTS),
    ("warehouse_migrations", WAREHOUSE_MIGRATIONS),
    ("warehouse_microbatches", WAREHOUSE_MICROBATCHES),
    ("source_sensors", SOURCE_SENSORS),
    ("source_staleness", SOURCE_STALENESS),
    ("security_audit_sink", SECURITY_AUDIT_SINK),
    ("warehouse_catalog", WAREHOUSE_CATALOG),
    ("fingerprint_lookup", FINGERPRINT_LOOKUP),
    ("warehouse_statistics", WAREHOUSE_STATISTICS),
    ("check_observations", CHECK_OBSERVATIONS),
    ("trigger_progress", TRIGGER_PROGRESS),
    ("cli_login_flows", CLI_LOGIN_FLOWS),
    ("retention_indexes", RETENTION_INDEXES),
    ("custom_roles", CUSTOM_ROLES),
    ("warehouse_consumption", WAREHOUSE_CONSUMPTION),
];

pub(crate) fn migration_digests(store: &ControlStore) -> Vec<String> {
    let digest = sha256_hex(format!("{SCHEMA}\n{}", recall_schema(store).join(";\n")).as_bytes());
    let mut digests = vec![digest];
    digests.extend(ADDITIONS.iter().map(|(_, sql)| sha256_hex(sql.as_bytes())));
    digests
}

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
