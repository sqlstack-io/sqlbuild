//! Maintenance reads over the migration, old-name view and janitor mirrors.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use sqlbuild_state::events::main::decoded_grants::decoded_grants;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::query_records::query_records;
use crate::store::models::{ControlTransaction, Parameter};
use crate::warehouse_history::models::{MaintenanceHistory, MaintenanceOldNameView};
use crate::warehouse_metadata::models::{
    WarehouseJanitorRecord, WarehouseMigrationRecord, WarehouseOldNameRecord,
};

#[derive(Deserialize)]
struct Recorded<T> {
    record: T,
}

/// The `record` column of every row `sql` selects.
pub(crate) async fn records<T: DeserializeOwned>(
    transaction: ControlTransaction,
    sql: &str,
    parameters: Vec<Parameter>,
) -> Result<(ControlTransaction, Vec<T>), ControlError> {
    let (transaction, rows): (ControlTransaction, Vec<Recorded<T>>) =
        query_records(transaction, sql, parameters).await?;
    Ok((
        transaction,
        rows.into_iter().map(|row| row.record).collect(),
    ))
}

/// The newest moves, optionally into one model, with every step of their old-name views.
pub(crate) async fn maintenance_history(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    (node, limit): (Option<&str>, u32),
) -> Result<(ControlTransaction, MaintenanceHistory), ControlError> {
    let limit = Parameter::Integer(i64::from(limit.clamp(1, 1000)));
    let scope = vec![Parameter::Id(location_id), Parameter::Text(target.into())];
    let (transaction, migrations): (ControlTransaction, Vec<WarehouseMigrationRecord>) =
        match node {
            Some(node) => {
                records(transaction,
                    "SELECT record_json AS record FROM sqb_warehouse_migrations WHERE location_id=$1 AND target=$2 \
                     AND destination_model=$3 ORDER BY observed_at DESC,event_id DESC LIMIT $4",
                    [scope.clone(), vec![Parameter::Text(node.into()), limit.clone()]].concat()).await?
            }
            None => {
                records(transaction,
                    "SELECT record_json AS record FROM sqb_warehouse_migrations WHERE location_id=$1 AND target=$2 \
                     ORDER BY observed_at DESC,event_id DESC LIMIT $3",
                    [scope.clone(), vec![limit.clone()]].concat()).await?
            }
        };
    let mut transaction = transaction;
    let mut stored_views: Vec<WarehouseOldNameRecord> = Vec::new();
    if !migrations.is_empty() {
        let mut parameters = scope.clone();
        parameters.extend(
            migrations
                .iter()
                .map(|migration| Parameter::Text(migration.event_id.clone())),
        );
        let placeholders = Parameter::placeholders(3..=parameters.len());
        (transaction, stored_views) = records(transaction,
            &format!("SELECT record_json AS record FROM sqb_warehouse_old_name_views WHERE location_id=$1 AND target=$2 \
                      AND migration_event_id IN ({placeholders}) ORDER BY observed_at,event_id"),
            parameters).await?;
    }
    let mut janitor_events: Vec<WarehouseJanitorRecord> = Vec::new();
    if node.is_none() {
        (transaction, janitor_events) = records(transaction,
            "SELECT record_json AS record FROM sqb_warehouse_janitor_events WHERE location_id=$1 AND target=$2 \
             ORDER BY observed_at DESC,event_id DESC LIMIT $3",
            [scope, vec![limit]].concat()).await?;
    }
    Ok((
        transaction,
        MaintenanceHistory {
            migrations,
            old_name_views: stored_views.into_iter().map(old_name_view).collect(),
            janitor_events,
        },
    ))
}

/// The stored step with its grant statements reduced to their number, read the way local `sqb` reads them.
fn old_name_view(record: WarehouseOldNameRecord) -> MaintenanceOldNameView {
    MaintenanceOldNameView {
        grants_copied: decoded_grants(record.grants_copied.as_deref())
            .map(|statements| statements.len() as u64),
        state_database: record.state_database,
        state_schema: record.state_schema,
        event_id: record.event_id,
        target_name: record.target_name,
        event_type: record.event_type,
        migration_event_id: record.migration_event_id,
        destination_model: record.destination_model,
        old_database: record.old_database,
        old_schema: record.old_schema,
        old_name: record.old_name,
        new_database: record.new_database,
        new_schema: record.new_schema,
        new_name: record.new_name,
        view_retention: record.view_retention,
        archive_name: record.archive_name,
        column_aliases: record.column_aliases,
        view_sql: record.view_sql,
        expires_at: record.expires_at,
        drop_reason: record.drop_reason,
        run_id: record.run_id,
        created_at: record.created_at,
    }
}
