//! The cause of a failed control database operation, in the words people read.

use crate::health::_helpers::cause::{chain, database_case, database_chain, sqlite_case};
use crate::health::_helpers::sentence::sentence;
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
