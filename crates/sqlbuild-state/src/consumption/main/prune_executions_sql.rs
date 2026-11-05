//! Prune old executions from one consumption table.

use crate::consumption::_helpers::ranked_executions::ranked_executions;

/// Delete every row of executions ranked beyond the newest `retain` per relation, whole executions at a time.
pub fn prune_executions_sql(qualified_name: &str, retain: i64) -> String {
    format!(
        "DELETE FROM {qualified_name} WHERE {executions}",
        executions = ranked_executions(qualified_name, &format!("> {retain}")),
    )
}
