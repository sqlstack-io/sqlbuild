//! HTTP contracts compiled within the API owner.

#[path = "changes/helpers.rs"]
mod changes_helpers;
#[path = "helpers.rs"]
pub(crate) mod helpers;
#[path = "oidc/helpers.rs"]
mod oidc_helpers;
#[path = "oidc/test_types.rs"]
mod oidc_test_types;
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
#[path = "test_external_sessions.rs"]
mod test_external_sessions;
#[path = "test_location_health.rs"]
mod test_location_health;
#[path = "test_location_work.rs"]
mod test_location_work;
#[path = "test_oidc.rs"]
mod test_oidc;
#[path = "test_passwords.rs"]
mod test_passwords;
#[path = "test_proxy.rs"]
mod test_proxy;
#[path = "test_reads.rs"]
mod test_reads;
#[path = "test_types.rs"]
mod test_types;
#[path = "test_warehouse_metadata.rs"]
mod test_warehouse_metadata;
#[path = "test_worker.rs"]
mod test_worker;
#[path = "warehouse_metadata/catalog/helpers.rs"]
mod warehouse_catalog_helpers;
#[path = "warehouse_metadata/events/helpers.rs"]
mod warehouse_events_helpers;
#[path = "warehouse_metadata/helpers.rs"]
mod warehouse_metadata_helpers;
#[path = "warehouse_metadata/reconciliation/helpers.rs"]
mod warehouse_reconciliation_helpers;
#[path = "work/helpers.rs"]
mod work_helpers;
