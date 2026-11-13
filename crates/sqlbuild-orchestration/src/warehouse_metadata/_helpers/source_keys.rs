//! A state row's source identity comes from its key columns alone, so a key page and a full record agree.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

use crate::store::errors::ControlError;
use crate::warehouse_metadata::models::WarehouseHistoryKind;

#[derive(Deserialize)]
struct EventKey {
    event_id: String,
}

#[derive(Deserialize)]
struct AuditResultKey {
    result_id: String,
}

#[derive(Deserialize)]
struct ObservationKey {
    observation_id: String,
}

/// Node outcomes carry no event identity; their identity columns hash to one.
#[derive(Deserialize)]
struct NodeResultKey {
    node_type: Option<String>,
    node_name: Option<String>,
    target_database: Option<String>,
    target_schema: Option<String>,
    target_name: Option<String>,
    run_id: Option<String>,
    status: Option<String>,
    ts: DateTime<Utc>,
}

/// Consumed input versions carry no event identity; their identity columns hash to one.
#[derive(Deserialize)]
struct ConsumptionKey {
    execution_id: Option<String>,
    phase: Option<String>,
    input_kind: Option<String>,
    input_name: Option<String>,
    input_version: Option<String>,
    recorded_at: DateTime<Utc>,
}

fn key<T: DeserializeOwned>(kind: WarehouseHistoryKind, record: &Value) -> Result<T, ControlError> {
    T::deserialize(record)
        .map_err(|error| ControlError::Invalid(format!("invalid {} key: {error}", kind.as_str())))
}

/// The source id of a full record or its key projection; `None` for kinds that read whole rows.
pub(crate) fn source_id(
    kind: WarehouseHistoryKind,
    record: &Value,
) -> Result<Option<String>, ControlError> {
    Ok(Some(match kind {
        WarehouseHistoryKind::JanitorEvents
        | WarehouseHistoryKind::Migrations
        | WarehouseHistoryKind::OldNameViews
        | WarehouseHistoryKind::Microbatches
        | WarehouseHistoryKind::ColumnMigrations => key::<EventKey>(kind, record)?.event_id,
        WarehouseHistoryKind::AuditResults => key::<AuditResultKey>(kind, record)?.result_id,
        WarehouseHistoryKind::Observations => key::<ObservationKey>(kind, record)?.observation_id,
        WarehouseHistoryKind::NodeResults => {
            let key: NodeResultKey = key(kind, record)?;
            sha256_hex(
                json!([
                    key.node_type,
                    key.node_name,
                    key.target_database,
                    key.target_schema,
                    key.target_name,
                    key.run_id,
                    key.status,
                    key.ts,
                ])
                .to_string()
                .as_bytes(),
            )
        }
        WarehouseHistoryKind::Consumption => {
            let key: ConsumptionKey = key(kind, record)?;
            sha256_hex(
                json!([
                    key.execution_id,
                    key.phase,
                    key.input_kind,
                    key.input_name,
                    key.input_version,
                    key.recorded_at,
                ])
                .to_string()
                .as_bytes(),
            )
        }
        WarehouseHistoryKind::SourceFreshness
        | WarehouseHistoryKind::Fingerprints
        | WarehouseHistoryKind::Catalog
        | WarehouseHistoryKind::Statistics => return Ok(None),
    }))
}
