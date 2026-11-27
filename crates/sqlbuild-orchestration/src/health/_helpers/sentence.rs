//! The sentence each case of control database failure reads as.

use crate::health::constants::{UNNAMED_HOST, UNNAMED_USER};
use crate::health::models::{ControlCauseCase, ControlTarget};

/// What people read for `case` at `target`; `own` is the error's own message, for errors with no case of their own.
pub(crate) fn sentence(case: ControlCauseCase, target: &ControlTarget, own: &str) -> String {
    let host: &str = target.host.as_deref().unwrap_or(UNNAMED_HOST);
    match case {
        ControlCauseCase::NameLookup => {
            format!("Can't reach the control database: {host} doesn't resolve (DNS)")
        }
        ControlCauseCase::Unreachable => match target.port {
            Some(port) => format!("Can't reach the control database at {host}:{port}"),
            None => format!("Can't reach the control database at {host}"),
        },
        ControlCauseCase::LoginRejected => format!(
            "The control database rejected the login for {}",
            target.user.as_deref().unwrap_or(UNNAMED_USER)
        ),
        ControlCauseCase::NoFreeConnections => {
            "The control database has no free connections".to_owned()
        }
        ControlCauseCase::Restarting => "The control database is restarting".to_owned(),
        ControlCauseCase::PoolExhausted => format!(
            "All {} control database connections were busy for {} s",
            target.connections, target.wait_seconds
        ),
        ControlCauseCase::Locked => "The control database is locked by another process".to_owned(),
        ControlCauseCase::SessionCutOff => {
            format!("The control database cut off a server session that ran past its limit: {own}")
        }
        ControlCauseCase::Other => format!("Control database error: {own}"),
    }
}
