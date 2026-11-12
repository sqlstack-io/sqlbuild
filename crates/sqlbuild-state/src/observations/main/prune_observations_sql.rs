//! Prune old observations from one observations table.

use crate::observations::constants::OBSERVED_AT_COLUMN;

/// Delete every observation ranked beyond the newest `retain` per kind and node.
pub fn prune_observations_sql(qualified_name: &str, retain: i64) -> String {
    format!(
        "DELETE FROM {qualified_name} WHERE observation_id IN (SELECT observation_id FROM \
(SELECT observation_id, ROW_NUMBER() OVER (PARTITION BY kind, node ORDER BY \
{OBSERVED_AT_COLUMN} DESC, observation_id DESC) AS observation_rank FROM {qualified_name}) \
AS ranked WHERE observation_rank > {retain})"
    )
}
