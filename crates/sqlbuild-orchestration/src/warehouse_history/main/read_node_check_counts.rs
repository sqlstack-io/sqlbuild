//! Each node's check counts from every check's newest result, keyed `kind:name` like the node keys.

use std::collections::HashMap;

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;
use crate::warehouse_history::_helpers::checks::latest_results;
use crate::warehouse_metadata::models::CatalogNodeChecks;

pub(crate) async fn read_node_check_counts(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
) -> Result<(ControlTransaction, HashMap<String, CatalogNodeChecks>), ControlError> {
    let (transaction, latest) = latest_results(transaction, (location_id, target)).await?;
    let mut counts: HashMap<String, CatalogNodeChecks> = HashMap::new();
    for row in latest {
        let record = row.record;
        let (Some(kind), Some(name)) = (record.attached_target_kind, record.attached_target_name)
        else {
            continue;
        };
        let checks = counts.entry(format!("{kind}:{name}")).or_default();
        checks.total += 1;
        match record.outcome.as_str() {
            "pass" => checks.passing += 1,
            "warn" => checks.warning += 1,
            "fail" | "error" => checks.failing += 1,
            _ => {}
        }
    }
    Ok((transaction, counts))
}
