//! What a plan asks of the warehouse state, each answered from an index.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlbuild_core::constants::{FINGERPRINT_TABLE_NAME, STATE_TABLE_NAMES};

use crate::consumption::models::{
    ConsumedInput, ConsumptionAppend, ConsumptionRow, NodeKey, RelationKey,
};
use crate::observations::constants::{CURSOR_MAX_KIND, SOURCE_COLUMNS_KIND};
use crate::warehouse_state::_helpers::indexes::folded;
use crate::warehouse_state::models::{
    DescribedColumns, InvalidObservation, RelationFilter, StateConsumption, StateFingerprint,
    StateLocation, StateRelation, WarehouseState, WarehouseStateParts,
};

fn same(left: &str, right: &str) -> bool {
    folded(left) == folded(right)
}

/// Whether a fact stored at `stored` answers a read of `wanted`; an unset database matches any.
fn database_matches(stored: Option<&str>, wanted: Option<&str>) -> bool {
    match (stored, wanted) {
        (Some(stored), Some(wanted)) => same(stored, wanted),
        _ => true,
    }
}

/// Whether `wanted` is unset or holds `value`, compared as the warehouse folds case.
fn any_same(wanted: Option<&[String]>, value: &str) -> bool {
    wanted.is_none_or(|wanted| wanted.iter().any(|item| same(value, item)))
}

/// Whether a listing hides the relation: state tables are planner inputs, not relations, except fingerprints.
fn hidden(relation: &StateRelation) -> bool {
    STATE_TABLE_NAMES.contains(&relation.name.as_str()) && relation.name != FINGERPRINT_TABLE_NAME
}

/// A stored consumption row as `--changed` reads it; a missing text column reads as empty.
fn consumption_row(record: &StateConsumption) -> ConsumptionRow {
    let text = |value: &Option<String>| value.clone().unwrap_or_default();
    ConsumptionRow {
        row: ConsumptionAppend {
            execution_id: text(&record.execution_id),
            phase: text(&record.phase),
            run_id: text(&record.run_id),
            node: NodeKey {
                kind: text(&record.node_type),
                name: text(&record.node_name),
            },
            relation: RelationKey {
                database: record.target_database.clone(),
                schema: text(&record.target_schema),
                name: text(&record.target_name),
            },
            version_hash: text(&record.version_hash),
            input_count: record.input_count.unwrap_or(-1),
            input: consumed_input(record),
        },
        recorded_at: record.recorded_at.timestamp_micros(),
    }
}

fn consumed_input(record: &StateConsumption) -> Option<ConsumedInput> {
    let kind: String = record.input_kind.clone()?;
    Some(ConsumedInput {
        kind,
        name: record.input_name.clone().unwrap_or_default(),
        version: record.input_version.clone().unwrap_or_default(),
    })
}

impl WarehouseState {
    /// The facts as received, for fixtures and parity checks.
    pub fn parts(&self) -> &WarehouseStateParts {
        &self.parts
    }

    /// When every required state table last synced cleanly; `None` before that.
    pub fn synced_at(&self) -> Option<DateTime<Utc>> {
        self.parts.provenance.synced_at
    }

    /// The mirror version the facts were read at, when the server versions its mirror.
    pub fn version(&self) -> Option<&str> {
        self.parts.provenance.version.as_deref()
    }

    /// Whether the facts include consumption history.
    pub fn has_consumption(&self) -> bool {
        self.parts.has_consumption
    }

    /// The relations `filter` names, in source order; state tables other than fingerprints are hidden.
    pub fn relations(&self, filter: &RelationFilter<'_>) -> Vec<&StateRelation> {
        let candidates: Vec<&StateRelation> = match filter.names {
            Some(names) => {
                let mut positions: Vec<usize> = names
                    .iter()
                    .filter_map(|name| self.relations_by_name.get(&folded(name)))
                    .flatten()
                    .copied()
                    .collect();
                positions.sort_unstable();
                positions.dedup();
                positions
                    .into_iter()
                    .filter_map(|position| self.parts.relations.get(position))
                    .collect()
            }
            None => self.parts.relations.iter().collect(),
        };
        candidates
            .into_iter()
            .filter(|relation| {
                !hidden(relation)
                    && filter.database.is_none_or(|database| {
                        database_matches(relation.database.as_deref(), Some(database))
                    })
                    && any_same(filter.schemas, &relation.schema)
            })
            .collect()
    }

    /// The newest fingerprint of each node in a state location: newest `ts`, then the greatest run ID.
    pub fn latest_fingerprints(&self, location: &StateLocation) -> Vec<&StateFingerprint> {
        let mut newest: BTreeMap<(&str, &str), &StateFingerprint> = BTreeMap::new();
        let positions = self
            .fingerprints_by_schema
            .get(&folded(&location.schema))
            .into_iter()
            .flatten();
        for fingerprint in positions.filter_map(|position| self.parts.fingerprints.get(*position)) {
            if !database_matches(
                fingerprint.location.database.as_deref(),
                location.database.as_deref(),
            ) {
                continue;
            }
            let key = (
                fingerprint.node_type.as_str(),
                fingerprint.node_name.as_str(),
            );
            let replaces = newest.get(&key).is_none_or(|current| {
                (fingerprint.ts, &fingerprint.run_id) > (current.ts, &current.run_id)
            });
            if replaces {
                newest.insert(key, fingerprint);
            }
        }
        newest.into_values().collect()
    }

    /// The model's cursor `MAX` text, only when observed by its newest successful build.
    pub fn target_max(&self, model: &str) -> Option<&str> {
        let observations: &[usize] = self
            .observations_by_node
            .get(&(CURSOR_MAX_KIND.to_owned(), model.to_owned()))
            .map_or(&[], Vec::as_slice);
        let runs = self.latest_model_runs.get(model).into_iter().flatten();
        for run in runs.filter_map(|position| self.parts.latest_runs.get(*position)) {
            let matching = observations
                .iter()
                .filter_map(|position| self.parts.observations.get(*position))
                .find(|observation| {
                    observation.run_id == run.run_id && observation.location == run.location
                });
            if let Some(observation) = matching {
                return Some(observation.value.as_str());
            }
        }
        None
    }

    /// The source's columns as last described, only when it described exactly `sql`.
    pub fn described_columns(
        &self,
        source: &str,
        sql: &str,
    ) -> Result<Option<Vec<String>>, InvalidObservation> {
        let hash: String = format!("{:x}", Sha256::digest(sql.as_bytes()));
        let positions = self
            .observations_by_node
            .get(&(SOURCE_COLUMNS_KIND.to_owned(), source.to_owned()))
            .into_iter()
            .flatten();
        for observation in positions.filter_map(|position| self.parts.observations.get(*position)) {
            let described: DescribedColumns =
                serde_json::from_str(&observation.value).map_err(|error| InvalidObservation {
                    source: source.to_owned(),
                    reason: error.to_string(),
                })?;
            if described.sql_hash == hash {
                return Ok(Some(described.columns));
            }
        }
        Ok(None)
    }

    /// Every consumption row of the state table at `location`, in source order.
    pub fn consumption_rows(&self, location: &StateLocation) -> Vec<ConsumptionRow> {
        self.consumption_by_schema
            .get(&folded(&location.schema))
            .into_iter()
            .flatten()
            .filter_map(|position| self.parts.consumption.get(*position))
            .filter(|row| {
                database_matches(
                    row.location.database.as_deref(),
                    location.database.as_deref(),
                )
            })
            .map(consumption_row)
            .collect()
    }
}
