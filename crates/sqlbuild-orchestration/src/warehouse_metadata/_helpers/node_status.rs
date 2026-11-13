//! Physical observations and planner-local definitions remain independent evidence dimensions.

use std::collections::{BTreeSet, HashMap};

use sqlbuild_core::fingerprint_identity::main::fingerprint_dialect::fingerprint_dialect;
use sqlbuild_core::fingerprint_identity::main::recorded_model_local_hash::recorded_model_local_hash;
use sqlquack_fingerprint::{FingerprintDialect, FingerprintError};
use uuid::Uuid;

use crate::snapshots::constants::MODEL_NODE_KIND;
use crate::snapshots::models::SnapshotNode;
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::{ControlTransaction, Parameter};
use crate::warehouse_metadata::_helpers::fingerprint_reads::definition_texts;
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

/// A node's newest build: its record, recorded metadata JSON and recorded definition text hash.
type RecordedBuild = (FingerprintObservation, String, String);

/// Each node's definition evidence, reading the recorded texts its comparison re-fingerprints.
pub(crate) async fn definition_evidences(
    transaction: ControlTransaction,
    builds: &[(&SnapshotNode, Option<&RecordedBuild>)],
) -> Result<(ControlTransaction, Vec<DefinitionEvidence>), ControlError> {
    let hashes: BTreeSet<String> = builds
        .iter()
        .filter_map(|(node, build)| rehash_text(node, *build))
        .map(str::to_owned)
        .collect();
    let (transaction, texts): (ControlTransaction, HashMap<String, String>) =
        definition_texts(transaction, &hashes).await?;
    let evidences: Vec<DefinitionEvidence> = builds
        .iter()
        .map(|(node, build)| {
            let recorded_text: Option<&str> = rehash_text(node, *build)
                .and_then(|hash| texts.get(hash))
                .map(String::as_str);
            definition_evidence(node, *build, recorded_text)
        })
        .collect();
    Ok((transaction, evidences))
}

/// One node's definition evidence, reading the recorded text its comparison re-fingerprints.
pub(crate) async fn node_definition_evidence(
    transaction: ControlTransaction,
    node: &SnapshotNode,
    build: Option<&RecordedBuild>,
) -> Result<(ControlTransaction, DefinitionEvidence), ControlError> {
    let (transaction, evidences) = definition_evidences(transaction, &[(node, build)]).await?;
    let evidence: DefinitionEvidence =
        evidences
            .into_iter()
            .next()
            .unwrap_or(DefinitionEvidence::Unavailable {
                reason: "no definition evidence was read".into(),
                current_local_hash: None,
            });
    Ok((transaction, evidence))
}

/// The recorded text hash to re-fingerprint, for a model that differs and has a snapshot dialect.
fn rehash_text<'a>(node: &SnapshotNode, build: Option<&'a RecordedBuild>) -> Option<&'a str> {
    let (_, _, text_hash) = build?;
    (node.kind == MODEL_NODE_KIND
        && node.definition["planner_identity"].get("dialect").is_some()
        && matches!(
            definition_evidence(node, build, None),
            DefinitionEvidence::Changed { .. }
        ))
    .then_some(text_hash.as_str())
}

/// The planner's query fingerprint of `sql` under `dialect`, or `recorded` when it does not tokenize.
fn query_hash(sql: &str, dialect: Option<&str>, recorded: &str) -> String {
    let dialect = FingerprintDialect::from_name(fingerprint_dialect(dialect))
        .unwrap_or(FingerprintDialect::Generic);
    sqlquack_fingerprint::query_fingerprint(sql, dialect)
        .unwrap_or_else(|_: FingerprintError| recorded.to_owned())
}

/// Compare planner-local identities, re-fingerprinting `recorded_text` under the snapshot's dialect.
pub(crate) fn definition_evidence(
    node: &SnapshotNode,
    build: Option<&RecordedBuild>,
    recorded_text: Option<&str>,
) -> DefinitionEvidence {
    let identity = &node.definition["planner_identity"];
    let current = identity["local_hash"].as_str();
    let recorded = build.and_then(|(record, metadata, _)| match node.kind.as_str() {
        "model" => {
            let definition_hash: String = recorded_text.zip(identity.get("dialect")).map_or_else(
                || record.definition_hash.clone(),
                |(text, dialect)| query_hash(text, dialect.as_str(), &record.definition_hash),
            );
            recorded_model_local_hash(&definition_hash, metadata)
        }
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
