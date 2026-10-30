//! Configure a file-backed SQLite control store without interpreting filenames as URIs.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::store::errors::ControlError;

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

pub(crate) fn open_sqlite(path: PathBuf) -> Result<(), ControlError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let connection = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
            | rusqlite::OpenFlags::SQLITE_OPEN_CREATE
            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(Duration::from_secs(30))?;
    connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL")?;
    Ok(())
}
