//! Where an opened control store connects, for the sentences that name it.

use crate::health::main::control_target::control_target;
use crate::health::models::ControlTarget;
use crate::store::models::ControlStore;

/// Where an opened `store` connects, with its pool's limits.
#[must_use]
pub(crate) fn store_target(store: &ControlStore) -> ControlTarget {
    match store {
        ControlStore::Postgres(pool) => {
            let options = pool.connect_options();
            ControlTarget {
                host: Some(options.get_host().to_owned()),
                port: Some(options.get_port()),
                user: Some(options.get_username().to_owned()),
                connections: pool.options().get_max_connections(),
                wait_seconds: pool.options().get_acquire_timeout().as_secs(),
            }
        }
        ControlStore::Sqlite(_) => control_target(""),
    }
}
