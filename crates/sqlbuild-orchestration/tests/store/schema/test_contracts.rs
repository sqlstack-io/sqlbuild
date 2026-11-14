//! Control-store schema contracts for SQLite and PostgreSQL.

#[path = "helpers.rs"]
pub(crate) mod helpers;
#[path = "test_foreign_key_indexes.rs"]
mod test_foreign_key_indexes;
#[path = "test_store_upgrade.rs"]
mod test_store_upgrade;
#[path = "test_types.rs"]
pub(crate) mod test_types;
