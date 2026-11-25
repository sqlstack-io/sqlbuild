use crate::health::models::ControlTarget;
use crate::health::tests::test_types::CodedError;
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

pub(crate) const DNS: &str = "Can't reach the control database: db.internal doesn't resolve (DNS)";

/// Record a failure to resolve the control database's host at `at`.
pub(crate) fn unresolved(
    health: &crate::health::models::ServerHealth,
    at: chrono::DateTime<chrono::Utc>,
) {
    let error = io(
        std::io::ErrorKind::Other,
        "failed to lookup address information: Name or service not known",
    );
    let cause = crate::health::main::control_cause::control_cause(&error, (&target(), false))
        .expect("a database cause");
    health.control_failed(&cause, (at, None));
}

/// Two active locations: one that reported 20 seconds ago, one silent for 20 minutes.
pub(crate) fn locations(
    health: &crate::health::models::ServerHealth,
    at: chrono::DateTime<chrono::Utc>,
) {
    health.locations_seen(vec![
        crate::health::models::StoredLocationReport {
            location_id: uuid::Uuid::from_u128(1),
            name: "orders".into(),
            last_seen_at: Some(at - chrono::TimeDelta::seconds(20)),
        },
        crate::health::models::StoredLocationReport {
            location_id: uuid::Uuid::from_u128(2),
            name: "finance".into(),
            last_seen_at: Some(at - chrono::TimeDelta::minutes(20)),
        },
    ]);
}
