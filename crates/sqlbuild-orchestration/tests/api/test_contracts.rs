//! HTTP contracts compiled within the API owner.

#[path = "changes/helpers.rs"]
mod changes_helpers;
#[path = "denials/helpers.rs"]
mod denials_helpers;
#[path = "fingerprint_reads/helpers.rs"]
mod fingerprint_reads_helpers;
#[path = "helpers.rs"]
pub(crate) mod helpers;
#[path = "node_sql/helpers.rs"]
mod node_sql_helpers;
#[path = "node_status/helpers.rs"]
mod node_status_helpers;
#[path = "oidc/helpers.rs"]
mod oidc_helpers;
#[path = "oidc/test_types.rs"]
mod oidc_test_types;
#[path = "output/helpers.rs"]
mod output_helpers;
#[path = "person_sessions/helpers.rs"]
mod person_sessions_helpers;
#[path = "person_sessions/test_types.rs"]
mod person_sessions_test_types;
#[path = "recall/helpers.rs"]
mod recall_helpers;
#[path = "statistics/eligibility/helpers.rs"]
mod statistics_eligibility;
#[path = "statistics/helpers.rs"]
mod statistics_helpers;
#[path = "statistics/retention/helpers.rs"]
mod statistics_retention;
#[path = "test_catalog_reads.rs"]
mod test_catalog_reads;
#[path = "test_changes.rs"]
mod test_changes;
#[path = "test_checks.rs"]
mod test_checks;
#[path = "test_cli_login.rs"]
mod test_cli_login;
#[path = "test_column_lineage.rs"]
mod test_column_lineage;
#[path = "test_contract.rs"]
mod test_contract;
#[path = "test_custom_roles.rs"]
mod test_custom_roles;
#[path = "test_denials.rs"]
mod test_denials;
#[path = "test_development.rs"]
mod test_development;
#[path = "test_external_sessions.rs"]
mod test_external_sessions;
#[path = "test_location_health.rs"]
mod test_location_health;
#[path = "test_location_work.rs"]
mod test_location_work;
#[path = "test_maintenance.rs"]
mod test_maintenance;
#[path = "test_microbatch_coverage.rs"]
mod test_microbatch_coverage;
#[path = "test_oidc.rs"]
mod test_oidc;
#[path = "test_oidc_groups.rs"]
mod test_oidc_groups;
#[path = "test_passwords.rs"]
mod test_passwords;
#[path = "test_person_live.rs"]
mod test_person_live;
#[path = "test_person_location_work.rs"]
mod test_person_location_work;
#[path = "test_person_sessions.rs"]
mod test_person_sessions;
#[path = "test_proxy.rs"]
mod test_proxy;
#[path = "test_publish_encoding.rs"]
mod test_publish_encoding;
#[path = "test_reads.rs"]
mod test_reads;
#[path = "test_statistics_recovery.rs"]
mod test_statistics_recovery;
#[path = "test_types.rs"]
mod test_types;
#[path = "test_ui_assets.rs"]
mod test_ui_assets;
#[path = "test_warehouse_metadata.rs"]
mod test_warehouse_metadata;
#[path = "test_worker.rs"]
mod test_worker;
#[path = "warehouse_metadata/catalog/helpers.rs"]
mod warehouse_catalog_helpers;
#[path = "warehouse_metadata/events/helpers.rs"]
mod warehouse_events_helpers;
#[path = "warehouse_history/helpers.rs"]
pub(crate) mod warehouse_history_helpers;
#[path = "warehouse_metadata/helpers.rs"]
mod warehouse_metadata_helpers;
#[path = "warehouse_metadata/reconciliation/helpers.rs"]
mod warehouse_reconciliation_helpers;
#[path = "work/helpers.rs"]
mod work_helpers;
