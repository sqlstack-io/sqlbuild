//! When a location asks for named work next, and what a failed work request does.

use sqlbuild_orchestration::api::errors::ClientError;
use sqlbuild_orchestration::store::errors::ControlError;
use tokio::time::Instant;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::constants::LOCATION_WORK_IDLE_POLL;
use crate::native_commands::models::LocationWorkTurn;

/// A misconfigured or rejected sign-in stops the location instead of being retried.
pub(crate) fn refuse_rejected_sign_in(error: &ClientError) -> Result<(), ControlError> {
    if matches!(error, ClientError::Configuration(_))
        || matches!(error, ClientError::Rejected(status) if matches!(status.as_u16(), 401 | 403))
    {
        return Err(ControlError::Invalid(error.to_string()));
    }
    Ok(())
}

/// A failed work request: a rejected sign-in stops the location, anything else is logged and retried.
pub(crate) fn work_error(error: &ClientError) -> Result<(), ControlError> {
    refuse_rejected_sign_in(error)?;
    drop(write_output(&[(
        Stream::Stderr,
        &format!("Location work request failed: {error}\n"),
    )]));
    Ok(())
}

/// When to check for named work next: at once after a turn that claimed work, else after the idle interval.
pub(in crate::native_commands) fn next_work_check(
    turn: &LocationWorkTurn,
    now: Instant,
) -> Instant {
    if turn.claimed {
        now
    } else {
        now + LOCATION_WORK_IDLE_POLL
    }
}
