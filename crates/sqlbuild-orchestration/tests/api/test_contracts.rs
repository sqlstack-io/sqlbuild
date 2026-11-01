//! HTTP contracts compiled within the API owner.

#[path = "changes/helpers.rs"]
mod changes_helpers;
#[path = "helpers.rs"]
pub(crate) mod helpers;
#[path = "output/helpers.rs"]
mod output_helpers;
#[path = "recall/helpers.rs"]
mod recall_helpers;
#[path = "test_changes.rs"]
mod test_changes;
#[path = "test_contract.rs"]
mod test_contract;
#[path = "test_development.rs"]
mod test_development;
#[path = "test_location_health.rs"]
mod test_location_health;
#[path = "test_location_work.rs"]
mod test_location_work;
#[path = "test_reads.rs"]
mod test_reads;
#[path = "test_types.rs"]
mod test_types;
#[path = "test_warehouse_metadata.rs"]
mod test_warehouse_metadata;
#[path = "test_worker.rs"]
mod test_worker;
#[path = "warehouse_metadata/helpers.rs"]
mod warehouse_metadata_helpers;
#[path = "work/helpers.rs"]
mod work_helpers;
