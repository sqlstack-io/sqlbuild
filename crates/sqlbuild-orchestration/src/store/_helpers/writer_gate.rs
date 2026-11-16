//! One process's writers to a SQLite store take turns in arrival order, not SQLite's busy backoff.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use tokio::sync::OwnedMutexGuard;

use crate::store::errors::ControlError;

/// How long a writer waits for its turn; the same bound SQLite's busy handler gives it.
const WRITER_WAIT: std::time::Duration = std::time::Duration::from_secs(30);

/// Wait for this process's turn to write `path`; the guard ends the turn when it drops.
pub(crate) async fn writer_turn(path: &Path) -> Result<OwnedMutexGuard<()>, ControlError> {
    static GATES: OnceLock<Mutex<HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>>> = OnceLock::new();
    let gate = Arc::clone(
        GATES
            .get_or_init(Mutex::default)
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(path.canonicalize()?)
            .or_default(),
    );
    tokio::time::timeout(WRITER_WAIT, gate.lock_owned())
        .await
        .map_err(|_| {
            ControlError::Sqlite(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
                Some("another writer in this process held the control store".into()),
            ))
        })
}
