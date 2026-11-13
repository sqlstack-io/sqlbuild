//! Read the newest run row of every run whose cost a later build may still re-read.

use crate::resource_costs::constants::{
    LATEST_RUN_COST_COLUMNS, OBSERVED_AT_COLUMN, REFRESHABLE_COST_STATUSES, RUN_RESOURCE_TYPE,
};

/// The columns of `LATEST_RUN_COST_COLUMNS` for each run whose newest run row is refreshable.
pub fn latest_run_costs_sql(qualified_name: &str) -> String {
    let statuses: String = REFRESHABLE_COST_STATUSES
        .iter()
        .map(|status| format!("'{status}'"))
        .collect::<Vec<_>>()
        .join(", ");
    let columns: String = LATEST_RUN_COST_COLUMNS.join(", ");
    format!(
        "SELECT {columns} FROM (SELECT {columns}, ROW_NUMBER() OVER (PARTITION BY run_id ORDER BY attempt DESC, \
{OBSERVED_AT_COLUMN} DESC, observation_id DESC) AS cost_rank FROM {qualified_name} WHERE \
resource_type = '{RUN_RESOURCE_TYPE}') AS ranked WHERE cost_rank = 1 AND status IN ({statuses})"
    )
}
