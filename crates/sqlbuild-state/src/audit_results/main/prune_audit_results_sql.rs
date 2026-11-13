//! Prune old audit results from one audit results table.

/// Delete every audit result ranked beyond the newest `retain` per audit binding.
pub fn prune_audit_results_sql(qualified_name: &str, retain: i64) -> String {
    format!(
        "DELETE FROM {qualified_name} WHERE result_id IN (SELECT result_id FROM \
(SELECT result_id, ROW_NUMBER() OVER (PARTITION BY binding_key ORDER BY \
occurred_at DESC, result_id DESC) AS result_rank FROM {qualified_name}) \
AS ranked WHERE result_rank > {retain})"
    )
}
