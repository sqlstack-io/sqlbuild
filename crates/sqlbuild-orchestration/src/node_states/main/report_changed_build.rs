//! A location reports what the default `sqb build --changed` runs; the server keeps the newest per target and serves it with the node states.

use serde_json::json;
use uuid::Uuid;

use crate::node_states::models::{ChangedBuildNodes, ChangedBuildReport};
use crate::store::errors::ControlError;
use crate::store::main::{commit::commit, query::query};
use crate::store::models::{Change, ControlStore, Parameter};
use crate::warehouse_metadata::_helpers::ingestion::begin_ingestion;

/// Only the registered reader of the current generation reports; each report replaces the target's last one.
pub async fn report_changed_build(
    store: &ControlStore,
    (location_id, target): (Uuid, &str),
    report: &ChangedBuildReport,
) -> Result<(), ControlError> {
    if report
        .changed
        .iter()
        .chain(&report.downstream)
        .any(|key| !key.contains(':'))
    {
        return Err(ControlError::Invalid(
            "changed build nodes must be kind:name keys".into(),
        ));
    }
    let mut transaction = begin_ingestion(
        store,
        (location_id, target),
        (report.session_id, report.generation),
    )
    .await?;
    let nodes = ChangedBuildNodes {
        changed: report.changed.clone(),
        downstream: report.downstream.clone(),
    };
    let now = transaction.now;
    (transaction, _) = query(
        transaction,
        "INSERT INTO sqb_changed_builds (location_id,target,generation,reported_at,report_json) \
         VALUES ($1,$2,$3,$4,$5) ON CONFLICT (location_id,target) DO UPDATE SET \
         generation=excluded.generation,reported_at=excluded.reported_at,report_json=excluded.report_json",
        vec![
            Parameter::Id(location_id),
            Parameter::Text(target.into()),
            Parameter::Integer(report.generation),
            Parameter::Time(now),
            Parameter::Json(json!(nodes)),
        ],
    )
    .await?;
    transaction
        .changes
        .insert(Change::NodeStatus(location_id, target.into()));
    commit(transaction).await
}
