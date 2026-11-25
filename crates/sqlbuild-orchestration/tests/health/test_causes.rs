//! A pool that waited full is exhausted; one that waited with room was trying to connect.

use crate::health::main::control_cause::control_cause;
use crate::health::models::ControlCauseCase;
use crate::health::tests::helpers::target;
use crate::health::tests::test_types::{ForeignErrorTestCase, PoolTimeoutTestCase};
use crate::store::errors::ControlError;

#[test]
fn given_a_pool_timeout_when_described_then_a_full_pool_is_busy_and_a_pool_with_room_is_unreachable()
 {
    let test_cases = [
        PoolTimeoutTestCase {
            description: "every connection was busy",
            pool_full: true,
            expected_cause: Some((
                ControlCauseCase::PoolExhausted,
                "All 10 control database connections were busy for 30 s",
                true,
            )),
        },
        PoolTimeoutTestCase {
            description: "the pool had room, so it was connecting",
            pool_full: false,
            expected_cause: Some((
                ControlCauseCase::Unreachable,
                "Can't reach the control database at db.internal:5432",
                true,
            )),
        },
    ];
    for test_case in test_cases {
        let described = control_cause(
            &ControlError::Database(sqlx_core::Error::PoolTimedOut),
            (&target(), test_case.pool_full),
        )
        .map(|cause| (cause.case, cause.message, cause.transient));
        assert_eq!(
            described,
            test_case.expected_cause.map(|(case, message, transient)| (
                case,
                message.to_owned(),
                transient
            )),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_an_error_that_is_not_the_databases_when_described_then_it_has_no_cause() {
    let test_cases = [ForeignErrorTestCase {
        description: "a refused request",
        error: || ControlError::Invalid("bad request".into()),
        expected_cause: None,
    }];
    for test_case in test_cases {
        assert_eq!(
            control_cause(&(test_case.error)(), (&target(), false)).map(|cause| cause.case),
            test_case.expected_cause,
            "{}",
            test_case.description
        );
    }
}
