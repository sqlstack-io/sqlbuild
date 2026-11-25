//! A pool that waited with every connection busy is exhausted; one that waited with room was trying to connect.

use crate::health::main::control_cause::control_cause;
use crate::health::models::ControlCauseCase;
use crate::health::tests::helpers::target;
use crate::store::errors::ControlError;

#[test]
fn given_a_pool_timeout_when_described_then_a_full_pool_is_busy_and_a_pool_with_room_is_unreachable()
 {
    let described = [true, false].map(|pool_full| {
        control_cause(
            &ControlError::Database(sqlx_core::Error::PoolTimedOut),
            (&target(), pool_full),
        )
        .map(|cause| (cause.case, cause.message, cause.transient))
    });
    assert_eq!(
        described,
        [
            Some((
                ControlCauseCase::PoolExhausted,
                "All 10 control database connections were busy for 30 s".to_owned(),
                true,
            )),
            Some((
                ControlCauseCase::Unreachable,
                "Can't reach the control database at db.internal:5432".to_owned(),
                true,
            )),
        ]
    );
}

#[test]
fn given_an_error_that_is_not_the_databases_when_described_then_it_has_no_cause() {
    assert_eq!(
        control_cause(
            &ControlError::Invalid("bad request".into()),
            (&target(), false)
        ),
        None
    );
}
