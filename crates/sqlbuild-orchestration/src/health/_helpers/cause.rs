//! How a control database error is classified, and each cause's own message.

use std::error::Error;

use crate::health::constants::{
    CONNECTION_CLASS, LOGIN_REJECTED_CODES, NAME_LOOKUP_MESSAGES, RESTARTING_CODES,
    TOO_MANY_CONNECTIONS_CODE,
};
use crate::health::models::ControlCauseCase;

/// A database error's case; a pool that waited with room to spare was trying to connect, as sqlx retries a refused connection until the wait ends.
pub(crate) fn database_case(error: &sqlx_core::Error, pool_full: bool) -> ControlCauseCase {
    match error {
        sqlx_core::Error::Io(error) if name_lookup(&error.to_string()) => {
            ControlCauseCase::NameLookup
        }
        sqlx_core::Error::Io(_) => ControlCauseCase::Unreachable,
        sqlx_core::Error::PoolTimedOut if pool_full => ControlCauseCase::PoolExhausted,
        sqlx_core::Error::PoolTimedOut => ControlCauseCase::Unreachable,
        sqlx_core::Error::Database(error) => error
            .code()
            .map_or(ControlCauseCase::Other, |code| code_case(&code)),
        _ => ControlCauseCase::Other,
    }
}

/// A SQLSTATE's case.
pub(crate) fn code_case(code: &str) -> ControlCauseCase {
    if LOGIN_REJECTED_CODES.contains(&code) {
        return ControlCauseCase::LoginRejected;
    }
    if code == TOO_MANY_CONNECTIONS_CODE {
        return ControlCauseCase::NoFreeConnections;
    }
    if RESTARTING_CODES.contains(&code) {
        return ControlCauseCase::Restarting;
    }
    if code.starts_with(CONNECTION_CLASS) {
        return ControlCauseCase::Unreachable;
    }
    ControlCauseCase::Other
}

/// A SQLite error's case: busy or locked, or any other.
pub(crate) fn sqlite_case(error: &rusqlite::Error) -> ControlCauseCase {
    match error {
        rusqlite::Error::SqliteFailure(failure, _)
            if matches!(
                failure.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            ) =>
        {
            ControlCauseCase::Locked
        }
        _ => ControlCauseCase::Other,
    }
}

/// Whether a resolver message says the name doesn't resolve.
pub(crate) fn name_lookup(message: &str) -> bool {
    let message: String = message.to_ascii_lowercase();
    NAME_LOOKUP_MESSAGES
        .iter()
        .any(|fragment| message.contains(fragment))
}

/// A database error by the database's own message, then its sources.
pub(crate) fn database_chain(error: &sqlx_core::Error) -> Vec<String> {
    match error {
        sqlx_core::Error::Database(database) => {
            let mut messages: Vec<String> = vec![database.message().to_owned()];
            messages.extend(sources(database.source()));
            messages
        }
        sqlx_core::Error::Io(io) => chain(io),
        error => chain(error),
    }
}

/// An error by its own message, then each source's.
pub(crate) fn chain(error: &dyn Error) -> Vec<String> {
    let mut messages: Vec<String> = vec![error.to_string()];
    messages.extend(sources(error.source()));
    messages
}

/// Each source's own message, without repeats.
pub(crate) fn sources(mut source: Option<&dyn Error>) -> Vec<String> {
    let mut messages: Vec<String> = Vec::new();
    while let Some(error) = source {
        let message: String = error.to_string();
        if messages.last() != Some(&message) {
            messages.push(message);
        }
        source = error.source();
    }
    messages
}
