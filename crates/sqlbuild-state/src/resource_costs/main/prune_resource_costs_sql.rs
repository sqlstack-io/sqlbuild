//! Prune superseded observations from one resource costs table.

use crate::resource_costs::constants::OBSERVED_AT_COLUMN;

/// Delete every observation ranked beyond the newest `retain` per run, resource and warehouse.
pub fn prune_resource_costs_sql(qualified_name: &str, retain: i64) -> String {
    format!(
        "DELETE FROM {qualified_name} WHERE observation_id IN (SELECT observation_id FROM \
(SELECT observation_id, ROW_NUMBER() OVER (PARTITION BY run_id, resource_type, resource_name, \
warehouse_name, warehouse_size, warehouse_type, cluster_number ORDER BY attempt DESC, \
{OBSERVED_AT_COLUMN} DESC, observation_id DESC) AS cost_rank FROM {qualified_name}) AS ranked \
WHERE cost_rank > {retain})"
    )
}
