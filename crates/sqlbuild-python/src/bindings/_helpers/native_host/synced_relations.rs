//! The synced copy's relations, indexed by name as the warehouse folds case, so lookups skip a scan.

use std::collections::HashMap;

use sqlbuild_core::constants::{FINGERPRINT_TABLE_NAME, STATE_TABLE_NAMES};
use sqlbuild_orchestration::warehouse_metadata::models::WarehouseCatalogRecord;

/// What a listing asks for; each unset part matches every relation.
pub(crate) struct RelationFilter<'filter> {
    pub(crate) database: Option<&'filter str>,
    pub(crate) schemas: Option<&'filter [String]>,
    pub(crate) names: Option<&'filter [String]>,
}

/// The copy's relations in copy order, with their positions under each folded name.
pub(crate) struct SyncedRelations {
    records: Vec<WarehouseCatalogRecord>,
    positions_by_name: HashMap<String, Vec<usize>>,
}

fn folded(value: &str) -> String {
    value.to_lowercase()
}

fn same(left: &str, right: &str) -> bool {
    folded(left) == folded(right)
}

/// Whether `wanted` is unset or holds `value`, compared as the warehouse folds case.
fn any_same(wanted: Option<&[String]>, value: &str) -> bool {
    wanted.is_none_or(|wanted| wanted.iter().any(|item| same(value, item)))
}

impl SyncedRelations {
    pub(crate) fn new(records: Vec<WarehouseCatalogRecord>) -> Self {
        let mut positions_by_name: HashMap<String, Vec<usize>> = HashMap::new();
        for (position, record) in records.iter().enumerate() {
            positions_by_name
                .entry(folded(&record.name))
                .or_default()
                .push(position);
        }
        Self {
            records,
            positions_by_name,
        }
    }

    /// The relations `filter` names, in copy order; state tables other than fingerprints are hidden.
    pub(crate) fn listed(&self, filter: &RelationFilter<'_>) -> Vec<&WarehouseCatalogRecord> {
        let candidates: Vec<&WarehouseCatalogRecord> = match filter.names {
            Some(names) => self.named(names),
            None => self.records.iter().collect(),
        };
        candidates
            .into_iter()
            .filter(|record| {
                !(STATE_TABLE_NAMES.contains(&record.name.as_str())
                    && record.name != FINGERPRINT_TABLE_NAME)
                    && filter.database.is_none_or(|database| {
                        record
                            .state_database
                            .as_deref()
                            .is_none_or(|state_database| same(state_database, database))
                    })
                    && any_same(filter.schemas, &record.state_schema)
            })
            .collect()
    }

    /// The relations carrying any of `names`, once each, in copy order.
    fn named(&self, names: &[String]) -> Vec<&WarehouseCatalogRecord> {
        let mut positions: Vec<usize> = names
            .iter()
            .filter_map(|name| self.positions_by_name.get(&folded(name)))
            .flatten()
            .copied()
            .collect();
        positions.sort_unstable();
        positions.dedup();
        positions
            .into_iter()
            .filter_map(|position| self.records.get(position))
            .collect()
    }
}
