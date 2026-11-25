//! Where a control database URL points, for the sentences that name it.

use std::str::FromStr;

use crate::health::models::ControlTarget;
use crate::store::constants::{POOL_CONNECTIONS, POOL_WAIT_SECONDS};
use crate::store::models::ControlStore;

/// The host, port and user of a PostgreSQL `url`, and the pool's limits; a SQLite path names none of them.
#[must_use]
pub fn control_target(url: &str) -> ControlTarget {
    let limits = ControlTarget {
        connections: POOL_CONNECTIONS,
        wait_seconds: POOL_WAIT_SECONDS,
        ..ControlTarget::default()
    };
    if !(url.starts_with("postgres://") || url.starts_with("postgresql://")) {
        return limits;
    }
    match sqlx_postgres::PgConnectOptions::from_str(url) {
        Ok(options) => ControlTarget {
            host: Some(options.get_host().to_owned()),
            port: Some(options.get_port()),
            user: Some(options.get_username().to_owned()),
            ..limits
        },
        Err(_) => limits,
    }
}

/// Where an opened `store` connects, with its pool's limits.
#[must_use]
pub fn store_target(store: &ControlStore) -> ControlTarget {
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
