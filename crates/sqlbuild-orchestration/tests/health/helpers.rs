use std::borrow::Cow;

use crate::health::models::ControlTarget;
use crate::store::errors::ControlError;

/// The control database the tests' sentences name.
pub(crate) fn target() -> ControlTarget {
    ControlTarget {
        host: Some("db.internal".into()),
        port: Some(5432),
        user: Some("sqb".into()),
        connections: 10,
        wait_seconds: 30,
    }
}

/// A database error with a SQLSTATE, as PostgreSQL returns one.
#[derive(Debug)]
pub(crate) struct CodedError {
    pub(crate) code: &'static str,
    pub(crate) message: &'static str,
}

impl std::fmt::Display for CodedError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for CodedError {}

impl sqlx_core::error::DatabaseError for CodedError {
    fn message(&self) -> &str {
        self.message
    }

    fn code(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed(self.code))
    }

    fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
        self
    }

    fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
        self
    }

    fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
        self
    }

    fn kind(&self) -> sqlx_core::error::ErrorKind {
        sqlx_core::error::ErrorKind::Other
    }
}

pub(crate) fn coded(code: &'static str, message: &'static str) -> ControlError {
    ControlError::Database(sqlx_core::Error::Database(Box::new(CodedError {
        code,
        message,
    })))
}

pub(crate) fn io(kind: std::io::ErrorKind, message: &'static str) -> ControlError {
    ControlError::Database(sqlx_core::Error::Io(std::io::Error::new(kind, message)))
}

pub(crate) fn sqlite_busy() -> ControlError {
    ControlError::Sqlite(rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
        Some("database is locked".into()),
    ))
}
