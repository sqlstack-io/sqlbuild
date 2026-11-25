//! Whether a store's connection pool is full, which tells a busy database from one that can't be reached.

use crate::store::models::ControlStore;

/// Every pooled PostgreSQL connection is open and in use; a SQLite store has no pool.
pub(crate) fn pool_full(store: &ControlStore) -> bool {
    match store {
        ControlStore::Postgres(pool) => {
            pool.size() >= pool.options().get_max_connections() && pool.num_idle() == 0
        }
        ControlStore::Sqlite(_) => false,
    }
}
