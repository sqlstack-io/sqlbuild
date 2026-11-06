//! Physical observations and planner-local definitions remain independent evidence dimensions.

use sqlbuild_core::fingerprint_identity::main::recorded_model_local_hash::recorded_model_local_hash;
use uuid::Uuid;

use crate::snapshots::models::SnapshotNode;
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::{ControlTransaction, Parameter};
use crate::warehouse_metadata::_helpers::ingestion::scoped_event_id;
use crate::warehouse_metadata::models::{
    DefinitionEvidence, FingerprintObservation, FingerprintQuery, PhysicalEvidence,
};

pub(crate) fn destination(node: &SnapshotNode) -> Option<FingerprintQuery> {
    let destination = node.definition.get("destination")?;
    Some(FingerprintQuery {
        node_type: node.kind.clone(),
        node_name: node.definition["name"].as_str()?.into(),
        database: destination["database"].as_str().map(str::to_owned),
        schema: Some(destination["schema"].as_str()?.into()),
        name: Some(destination["name"].as_str()?.into()),
    })
}

pub(crate) fn definition_evidence(
    node: &SnapshotNode,
    build: Option<&(FingerprintObservation, String)>,
) -> DefinitionEvidence {
    let current = node.definition["planner_identity"]["local_hash"].as_str();
    let recorded = build.and_then(|(record, metadata)| match node.kind.as_str() {
        "model" => recorded_model_local_hash(&record.definition_hash, metadata),
        _ => None,
    });
    match (current, recorded) {
        (Some(current), Some(recorded)) if current == recorded => DefinitionEvidence::Matched {
            local_hash: recorded,
        },
        (Some(current), Some(recorded)) => DefinitionEvidence::Changed {
            current_local_hash: current.into(),
            recorded_local_hash: recorded,
        },
        _ => DefinitionEvidence::Unavailable {
            reason: "comparable planner-local model identities are unavailable".into(),
            current_local_hash: current.map(str::to_owned),
        },
    }
}

pub(crate) async fn physical_evidence(
    transaction: ControlTransaction,
    (location_id, target, destination): (Uuid, &str, &FingerprintQuery),
) -> Result<(ControlTransaction, PhysicalEvidence), ControlError> {
    let relation_id = scoped_event_id(
        (location_id, target),
        (
            destination.database.as_deref(),
            destination.schema.as_deref().unwrap_or_default(),
            "catalog",
        ),
        destination.name.as_deref().unwrap_or_default(),
    )?;
    let (transaction, rows) = query(transaction,
        "SELECT event_id,observed_at,record_json AS record FROM sqb_warehouse_catalog WHERE event_id=$1 AND location_id=$2 AND target=$3 LIMIT 1",
        vec![Parameter::Id(relation_id), Parameter::Id(location_id), Parameter::Text(target.into())]).await?;
    let evidence = match rows.into_iter().next() {
        Some(row) => PhysicalEvidence::Observed {
            observation: decode_row(row)?,
        },
        None => PhysicalEvidence::Unavailable {
            reason: "no retained physical catalog observation for the exact destination".into(),
        },
    };
    Ok((transaction, evidence))
}
