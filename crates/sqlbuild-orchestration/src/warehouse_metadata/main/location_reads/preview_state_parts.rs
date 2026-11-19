//! A location plans from the warehouse state its synced copy describes.

use sqlbuild_state::warehouse_state::models::{
    StateConsumption, StateFingerprint, StateLatestRun, StateObservation, SyncedProvenance,
    WarehouseStateParts,
};

use crate::warehouse_metadata::_helpers::mirror_pages::{location, relation};
use crate::warehouse_metadata::models::PreviewCopy;

/// The state facts of `copy`, in the copy's order; `has_consumption` when it was read with history.
pub fn preview_state_parts(copy: PreviewCopy, has_consumption: bool) -> WarehouseStateParts {
    WarehouseStateParts {
        provenance: SyncedProvenance {
            synced_at: copy.synced_at,
            version: Some(copy.version),
        },
        relations: copy.relations.into_iter().map(relation).collect(),
        fingerprints: copy
            .fingerprints
            .into_iter()
            .map(|record| StateFingerprint {
                location: location(record.state_database, record.state_schema),
                node_type: record.node_type,
                node_name: record.node_name,
                target_database: record.target_database,
                target_schema: record.target_schema,
                target_name: record.target_name,
                run_id: record.run_id,
                definition_hash: record.definition_hash,
                version_hash: record.version_hash,
                schema_fingerprint: record.schema_fingerprint,
                definition: record.definition,
                metadata_json: record.metadata_json,
                ts: record.ts,
            })
            .collect(),
        latest_runs: copy
            .latest_runs
            .into_iter()
            .map(|run| StateLatestRun {
                location: location(run.state_database, run.state_schema),
                node_type: run.node_type,
                node_name: run.node_name,
                run_id: run.run_id,
            })
            .collect(),
        observations: copy
            .observations
            .into_iter()
            .map(|record| StateObservation {
                location: location(record.state_database, record.state_schema),
                kind: record.kind,
                node: record.node,
                cursor_type: record.cursor_type,
                value: record.value,
                run_id: record.run_id,
                observed_at: record.observed_at,
            })
            .collect(),
        consumption: copy
            .consumption
            .into_iter()
            .map(|record| StateConsumption {
                location: location(record.state_database, record.state_schema),
                execution_id: record.execution_id,
                phase: record.phase,
                run_id: record.run_id,
                node_type: record.node_type,
                node_name: record.node_name,
                target_database: record.target_database,
                target_schema: record.target_schema,
                target_name: record.target_name,
                version_hash: record.version_hash,
                input_count: record.input_count,
                input_kind: record.input_kind,
                input_name: record.input_name,
                input_version: record.input_version,
                recorded_at: record.recorded_at,
            })
            .collect(),
        has_consumption,
    }
}
