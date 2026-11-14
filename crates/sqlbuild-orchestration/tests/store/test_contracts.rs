//! Recall storage integration contracts for SQLite and PostgreSQL.

#[path = "faults/helpers.rs"]
pub(crate) mod fault_helpers;
#[path = "faults/test_types.rs"]
pub(crate) mod fault_types;
#[path = "recall/helpers.rs"]
pub(crate) mod helpers;
#[path = "leases/helpers.rs"]
pub(crate) mod lease_helpers;
#[path = "logical_state/helpers.rs"]
pub(crate) mod logical_state;
#[path = "replicas/helpers.rs"]
pub(crate) mod replica_helpers;
#[path = "helpers.rs"]
pub(crate) mod store_helpers;
#[path = "test_types.rs"]
pub(crate) mod store_test_types;
#[path = "recall/test_contract.rs"]
mod test_contract;
#[path = "schema/test_foreign_key_indexes.rs"]
mod test_foreign_key_indexes;
#[path = "recall/test_types.rs"]
mod test_types;
#[path = "recall/test_views.rs"]
mod test_views;
