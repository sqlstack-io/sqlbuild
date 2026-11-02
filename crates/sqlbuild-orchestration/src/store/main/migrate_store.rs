//! Forward-only migration under the same lock used by all other control-plane writes.

use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::store::_helpers::migration::{
    ADMINISTRATOR_INITIALIZATION, AUDIT_STORAGE_ORDER, AUTHENTICATION_REVISION,
    DEVELOPMENT_LOCATIONS, EXECUTED_NODES, EXTERNAL_SESSIONS, LOCATION_INSTANCES, LOCATION_WORK,
    NODE_SENSORS, NODE_TRANSITIONS, OIDC_LOGIN_FLOWS, PASSWORD_CREDENTIALS, PASSWORD_SESSIONS,
    PLATFORM_SINK, RUN_SENSORS, SCHEMA, SENSORS, WAREHOUSE_AUDIT_RESULTS, WAREHOUSE_FINGERPRINTS,
    WAREHOUSE_FRESHNESS, WAREHOUSE_JANITOR_EVENTS, WAREHOUSE_MICROBATCHES, WAREHOUSE_MIGRATIONS,
    WAREHOUSE_RECONCILIATION, render, verify_migrations,
};
use crate::store::_helpers::recall_schema::recall_schema;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Create the schema once, or reject drift and schemas newer than this server supports.
pub async fn migrate_store(store: &ControlStore) -> Result<(), ControlError> {
    let mut transaction = begin(store, TransactionMode::Write).await?;
    (transaction, _) = query(transaction, &render(store,
        "CREATE TABLE IF NOT EXISTS sqb_schema_migrations (version BIGINT PRIMARY KEY, \
         name TEXT NOT NULL, checksum TEXT NOT NULL, min_server_version TEXT NOT NULL, applied_at {time} NOT NULL)"), vec![]).await?;
    let (mut transaction, rows) = query(
        transaction,
        "SELECT version, checksum, min_server_version FROM sqb_schema_migrations ORDER BY version",
        vec![],
    )
    .await?;
    let recall_statements = recall_schema(store);
    let digest = sha256_hex(format!("{SCHEMA}\n{}", recall_statements.join(";\n")).as_bytes());
    let additions = [
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
    ];
    let mut digests = vec![digest.clone()];
    digests.extend(additions.iter().map(|(_, sql)| sha256_hex(sql.as_bytes())));
    let current = verify_migrations(&rows, &digests)?;
    if rows.is_empty() {
        for statement in SCHEMA
            .split(';')
            .map(str::trim)
            .filter(|statement| !statement.is_empty())
        {
            (transaction, _) = query(transaction, &render(store, statement), vec![]).await?;
        }
        for statement in recall_statements {
            (transaction, _) = query(transaction, &statement, vec![]).await?;
        }
        let now = transaction.now;
        (transaction, _) = query(transaction,
            "INSERT INTO sqb_installation (singleton, installation_id, created_at) VALUES (1, $1, $2)",
            vec![Parameter::Id(Uuid::now_v7()), Parameter::Time(now)]).await?;
        (transaction, _) = query(transaction,
            "INSERT INTO sqb_schema_migrations (version, name, checksum, min_server_version, applied_at) \
             VALUES (1, 'control_store', $1, $2, $3)",
            vec![Parameter::Text(digest), Parameter::Text(current.to_string()), Parameter::Time(now)]).await?;
    }
    for (index, (name, sql)) in additions.into_iter().enumerate() {
        if index + 1 < rows.len() {
            continue;
        }
        for statement in sql
            .split(';')
            .map(str::trim)
            .filter(|statement| !statement.is_empty())
        {
            (transaction, _) = query(transaction, &render(store, statement), vec![]).await?;
        }
        let now = transaction.now;
        (transaction, _) = query(transaction,
            "INSERT INTO sqb_schema_migrations (version, name, checksum, min_server_version, applied_at) \
             VALUES ($1, $2, $3, $4, $5)",
            vec![Parameter::Integer(index as i64 + 2), Parameter::Text(name.into()),
                Parameter::Text(digests[index + 1].clone()), Parameter::Text(current.to_string()), Parameter::Time(now)]).await?;
    }
    commit(transaction).await
}
