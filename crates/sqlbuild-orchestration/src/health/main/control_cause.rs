//! The cause of a failed control database operation, in the words people read.

use std::error::Error;

use crate::health::_helpers::sentence::sentence;
use crate::health::constants::{
    CONNECTION_CLASS, LOGIN_REJECTED_CODES, NAME_LOOKUP_MESSAGES, RESTARTING_CODES,
    TOO_MANY_CONNECTIONS_CODE,
};
use crate::health::models::{ControlCause, ControlCauseCase, ControlTarget};
use crate::store::errors::ControlError;

/// What `error` says about the control database at `target`, whose pool is `pool_full` or not; `None` for errors that aren't the database's.
#[must_use]
pub fn control_cause(
    error: &ControlError,
    (target, pool_full): (&ControlTarget, bool),
) -> Option<ControlCause> {
    let (case, chain): (ControlCauseCase, Vec<String>) = match error {
        ControlError::Database(error) => (database_case(error, pool_full), database_chain(error)),
        ControlError::Sqlite(error) => (sqlite_case(error), chain(error)),
        ControlError::Worker(error) => (ControlCauseCase::Other, chain(error)),
        ControlError::Io(error) => (ControlCauseCase::Other, chain(error)),
        _ => return None,
    };
    Some(ControlCause {
        case,
        message: sentence(case, target, &chain[0]),
        chain,
        transient: error.transient(),
        waited_for_pool: matches!(
            error,
            ControlError::Database(sqlx_core::Error::PoolTimedOut)
        ),
    })
}

fn database_case(error: &sqlx_core::Error, pool_full: bool) -> ControlCauseCase {
    match error {
        sqlx_core::Error::Io(error) if name_lookup(&error.to_string()) => {
            ControlCauseCase::NameLookup
        }
        sqlx_core::Error::Io(_) => ControlCauseCase::Unreachable,
        // A pool that waited with room to spare was trying to connect: sqlx retries a refused connection until the wait ends.
        sqlx_core::Error::PoolTimedOut if pool_full => ControlCauseCase::PoolExhausted,
        sqlx_core::Error::PoolTimedOut => ControlCauseCase::Unreachable,
        sqlx_core::Error::Database(error) => error
            .code()
            .map_or(ControlCauseCase::Other, |code| code_case(&code)),
        _ => ControlCauseCase::Other,
    }
}

fn code_case(code: &str) -> ControlCauseCase {
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

fn sqlite_case(error: &rusqlite::Error) -> ControlCauseCase {
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
fn name_lookup(message: &str) -> bool {
    let message: String = message.to_ascii_lowercase();
    NAME_LOOKUP_MESSAGES
        .iter()
        .any(|fragment| message.contains(fragment))
}

/// A database error by the database's own message, then its sources.
fn database_chain(error: &sqlx_core::Error) -> Vec<String> {
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

fn chain(error: &dyn Error) -> Vec<String> {
    let mut messages: Vec<String> = vec![error.to_string()];
    messages.extend(sources(error.source()));
    messages
}

fn sources(mut source: Option<&dyn Error>) -> Vec<String> {
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
