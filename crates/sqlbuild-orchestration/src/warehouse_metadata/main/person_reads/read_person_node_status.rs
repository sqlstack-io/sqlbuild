//! One authorized current node, one physical observation and one latest build; never warehouse polling.

use uuid::Uuid;

use crate::authorization::main::begin_authorized::begin_authorized;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::snapshots::main::read_current_record::read_current_record;
use crate::snapshots::main::read_snapshot_nodes::read_snapshot_nodes;
use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::models::ControlStore;
use crate::warehouse_metadata::_helpers::fingerprint_reads::latest_fingerprint;
use crate::warehouse_metadata::_helpers::node_status::{
    destination, node_definition_evidence, physical_evidence,
};
use crate::warehouse_metadata::models::{
    DefinitionEvidence, NodeStatus, NodeStatusQuery, PhysicalEvidence, SourceDataStatus,
};

pub async fn read_person_node_status(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (location_id, target, options): (Uuid, &str, &NodeStatusQuery),
    default_role: DefaultRole,
) -> Result<Option<NodeStatus>, ControlError> {
    let (transaction, _) = begin_authorized(
        store,
        credentials,
        &PermissionScope {
            action: Action::View,
            location_id: Some(location_id),
            target: Some(target.into()),
            nodes: [options.node_key.clone()].into(),
        },
        default_role,
    )
    .await?;
    if options.node_key.trim().is_empty() {
        return Err(ControlError::Invalid(
            "node status requires an exact node key".into(),
        ));
    }
    let (transaction, snapshot) = read_current_record(transaction, (location_id, target)).await?;
    let Some(snapshot) = snapshot else {
        commit(transaction).await?;
        return Ok(None);
    };
    let (mut transaction, mut nodes) = read_snapshot_nodes(
        transaction,
        snapshot.snapshot_id,
        Some(&[options.node_key.clone()].into()),
    )
    .await?;
    let Some(node) = nodes.remove(&options.node_key) else {
        commit(transaction).await?;
        return Ok(None);
    };
    let mut build = None;
    let physical = if let Some(destination) = destination(&node) {
        let physical = if matches!(node.kind.as_str(), "model" | "seed") {
            let physical;
            (transaction, physical) =
                physical_evidence(transaction, (location_id, target, &destination)).await?;
            physical
        } else {
            PhysicalEvidence::Unavailable {
                reason: "node is not a physical relation".into(),
            }
        };
        (transaction, build) =
            latest_fingerprint(transaction, (location_id, target, &destination)).await?;
        physical
    } else {
        PhysicalEvidence::Unavailable {
            reason: "node has no physical relation destination".into(),
        }
    };
    let definition: DefinitionEvidence;
    (transaction, definition) =
        node_definition_evidence(transaction, &node, build.as_ref()).await?;
    let status = NodeStatus {
        snapshot_id: snapshot.snapshot_id,
        node_key: options.node_key.clone(),
        physical,
        definition,
        build: build.map(|(record, _, _)| record),
        source_data: SourceDataStatus::Unknown {
            reason: "consumed-input provenance is not recorded".into(),
        },
    };
    commit(transaction).await?;
    Ok(Some(status))
}
