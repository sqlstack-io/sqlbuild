//! Configure a file-backed SQLite control store without interpreting filenames as URIs.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

pub(crate) fn sqlite_path(url: &str) -> Result<PathBuf, ControlError> {
    let path = Path::new(url.strip_prefix("sqlite://").unwrap_or(url)).to_path_buf();
    if path.as_os_str().is_empty()
        || path == Path::new(":memory:")
        || url.contains("://") && !url.starts_with("sqlite://")
    {
        return Err(ControlError::Invalid(
            "control-store URL must name PostgreSQL or a SQLite file".into(),
        ));
    }
    Ok(path)
}

pub(crate) fn open_sqlite(path: PathBuf, offline: bool) -> Result<ControlStore, ControlError> {
    if offline && !path.is_file() {
        return Err(ControlError::Invalid(
            "offline recovery requires an existing initialized control store".into(),
        ));
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let absolute = std::env::current_dir()?.join(path);
    let path =
        if absolute.exists() {
            absolute.canonicalize()?
        } else {
            absolute
                .parent()
                .ok_or_else(|| ControlError::Invalid("control store requires a filename".into()))?
                .canonicalize()?
                .join(absolute.file_name().ok_or_else(|| {
                    ControlError::Invalid("control store requires a filename".into())
                })?)
        };
    let mut lock_path = path.as_os_str().to_owned();
    lock_path.push(".access.lock");
    let access = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)?;
    let locked = if offline {
        fs4::FileExt::try_lock(&access)
    } else {
        fs4::FileExt::try_lock_shared(&access)
    };
    locked.map_err(|error| ControlError::Invalid(format!(
        "control store is in use; offline recovery requires exclusive access with all server and administration processes stopped: {error}"
    )))?;
    let connection = rusqlite::Connection::open_with_flags(
        &path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
            | rusqlite::OpenFlags::SQLITE_OPEN_CREATE
            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(Duration::from_secs(30))?;
    connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL")?;
    Ok(ControlStore::Sqlite(path, Arc::new(access)))
}
