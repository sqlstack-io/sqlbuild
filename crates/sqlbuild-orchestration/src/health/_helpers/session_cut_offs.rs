//! The sessions PostgreSQL cut off, kept per database until the health probe reports them.

use std::sync::PoisonError;

use chrono::Utc;

use crate::health::models::{SessionCutOff, SessionCutOffs};
use crate::store::constants::{IDLE_IN_TRANSACTION_CODE, PENDING_CUT_OFFS, QUERY_CANCELED_CODE};
use crate::store::models::ControlStore;

/// This process's cut-off sessions, for every store it opens, told apart by database.
pub(crate) static SESSION_CUT_OFFS: SessionCutOffs = SessionCutOffs::new();

/// Keep `error` in `cut_offs` for `store` when PostgreSQL cut its session off; the oldest beyond the limit are dropped.
pub(crate) fn note(cut_offs: &SessionCutOffs, store: &ControlStore, error: &sqlx_core::Error) {
    let Some(idle) = cut_off(error) else {
        return;
    };
    let mut pending = cut_offs
        .pending
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    pending.push((
        database(store),
        SessionCutOff {
            at: Utc::now(),
            idle,
            error: error.to_string(),
        },
    ));
    let excess = pending.len().saturating_sub(PENDING_CUT_OFFS);
    pending.drain(..excess);
}

/// `store`'s cut-off sessions in `cut_offs`, which the caller now owns.
pub(crate) fn take(cut_offs: &SessionCutOffs, store: &ControlStore) -> Vec<SessionCutOff> {
    let key = database(store);
    let mut pending = cut_offs
        .pending
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let (taken, kept): (Vec<_>, Vec<_>) = pending.drain(..).partition(|(of, _)| *of == key);
    *pending = kept;
    taken.into_iter().map(|(_, cut_off)| cut_off).collect()
}

/// Keep `unreported` for `store`'s next report, ahead of any cut off since.
pub(crate) fn restore(
    cut_offs: &SessionCutOffs,
    store: &ControlStore,
    unreported: Vec<SessionCutOff>,
) {
    let key = database(store);
    let mut pending = cut_offs
        .pending
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let mut restored: Vec<(String, SessionCutOff)> = unreported
        .into_iter()
        .map(|cut_off| (key.clone(), cut_off))
        .collect();
    restored.append(&mut pending);
    let excess = restored.len().saturating_sub(PENDING_CUT_OFFS);
    restored.drain(..excess);
    *pending = restored;
}

/// Whether PostgreSQL cut the session off: `Some(true)` for a transaction left idle, `Some(false)` for a statement past its timeout.
fn cut_off(error: &sqlx_core::Error) -> Option<bool> {
    let sqlx_core::Error::Database(error) = error else {
        return None;
    };
    match error.code()?.as_ref() {
        IDLE_IN_TRANSACTION_CODE => Some(true),
        QUERY_CANCELED_CODE if error.message().contains("statement timeout") => Some(false),
        _ => None,
    }
}

/// Which database a store is: its PostgreSQL host, port and database, or its SQLite file.
fn database(store: &ControlStore) -> String {
    match store {
        ControlStore::Postgres(pool) => {
            let options = pool.connect_options();
            format!(
                "{}:{}/{}",
                options.get_host(),
                options.get_port(),
                options.get_database().unwrap_or_default()
            )
        }
        ControlStore::Sqlite(sqlite) => sqlite.path.display().to_string(),
    }
}
