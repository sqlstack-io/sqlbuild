//! Expire or hold the persisted lease without introducing a fake control-store clock.

use chrono::{SecondsFormat, TimeDelta, Utc};

use crate::scheduler::models::SchedulerLease;
use crate::store::models::ControlStore;

pub(crate) async fn expire_lease(store: &ControlStore) {
    let ControlStore::Postgres(pool) = store else {
        let ControlStore::Sqlite(path) = store else {
            unreachable!()
        };
        rusqlite::Connection::open(path)
            .expect("open lease expiry connection")
            .execute_batch(
                "UPDATE sqb_scheduler_lease SET expires_at='2000-01-01T00:00:00.000000Z'",
            )
            .expect("expire SQLite scheduler lease");
        return;
    };
    sqlx_core::query::query(
        "UPDATE sqb_scheduler_lease SET expires_at='2000-01-01T00:00:00.000000Z'",
    )
    .execute(pool)
    .await
    .expect("expire PostgreSQL scheduler lease");
}

/// Keep `lease` live for a day without a renewer task, so slow setup cannot fence the test.
pub(crate) async fn extend_lease_through_test(
    store: &ControlStore,
    lease: SchedulerLease,
) -> SchedulerLease {
    let expires_at = Utc::now() + TimeDelta::days(1);
    let ControlStore::Postgres(pool) = store else {
        let ControlStore::Sqlite(path) = store else {
            unreachable!()
        };
        let held = rusqlite::Connection::open(path)
            .expect("open lease hold connection")
            .execute(
                "UPDATE sqb_scheduler_lease SET expires_at=?1 WHERE name='scheduler' AND holder=?2 AND epoch=?3",
                rusqlite::params![
                    expires_at.to_rfc3339_opts(SecondsFormat::Micros, true),
                    lease.holder.to_string(),
                    lease.epoch
                ],
            )
            .expect("hold SQLite scheduler lease");
        assert_eq!(held, 1, "the held lease is the current one");
        return SchedulerLease {
            expires_at,
            ..lease
        };
    };
    let held = sqlx_core::query::query(
        "UPDATE sqb_scheduler_lease SET expires_at=$1 WHERE name='scheduler' AND holder=$2 AND epoch=$3",
    )
    .bind(expires_at)
    .bind(lease.holder)
    .bind(lease.epoch)
    .execute(pool)
    .await
    .expect("hold PostgreSQL scheduler lease");
    assert_eq!(held.rows_affected(), 1, "the held lease is the current one");
    SchedulerLease {
        expires_at,
        ..lease
    }
}
