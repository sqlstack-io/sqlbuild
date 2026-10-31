//! Recall storage integration contracts for SQLite and PostgreSQL.

#[path = "store/recall/helpers.rs"]
mod helpers;
#[path = "store/replicas/helpers.rs"]
mod replica_helpers;
#[path = "store/helpers.rs"]
mod store_helpers;
#[path = "store/test_types.rs"]
mod store_test_types;
#[path = "store/recall/test_contract.rs"]
mod test_contract;
#[path = "store/recall/test_types.rs"]
mod test_types;
#[path = "store/recall/test_views.rs"]
mod test_views;
