//! PostgreSQL holds every server session to a statement and an idle-in-transaction limit, and a session it cuts off is reported.

use crate::health::models::ControlCauseCase;
use crate::store::models::TransactionMode;
use crate::store::tests::session_limits::helpers::{cut_off_session, session_limits};
use crate::store::tests::session_limits::test_types::{
    CutOffReport, CutOffTestCase, SessionLimitTestCase,
};
use crate::store::tests::store_helpers::run;

#[test]
fn given_a_pooled_session_when_a_transaction_begins_then_postgres_holds_it_to_the_server_limits() {
    let test_cases = [
        SessionLimitTestCase {
            description: "a read keeps the session limits",
            mode: TransactionMode::Read,
            expected_limits: ("5min", "2min"),
        },
        SessionLimitTestCase {
            description: "a write keeps the session limits",
            mode: TransactionMode::Write,
            expected_limits: ("5min", "2min"),
        },
        SessionLimitTestCase {
            description: "a migration gets the long limits",
            mode: TransactionMode::Migrate,
            expected_limits: ("30min", "30min"),
        },
    ];
    for test_case in test_cases {
        let Some((statement, idle)) = run(session_limits(test_case.mode)) else {
            continue;
        };
        assert_eq!(
            (statement.as_str(), idle.as_str()),
            test_case.expected_limits,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_session_past_its_limit_when_postgres_cuts_it_off_then_health_and_a_platform_event_report_it()
 {
    let test_cases = [
        CutOffTestCase {
            description: "a transaction left idle past its limit",
            session: (
                "SET LOCAL idle_in_transaction_session_timeout = 300",
                1500,
                "SELECT 1 AS one",
            ),
            expected_report: CutOffReport {
                cause: Some(ControlCauseCase::SessionCutOff),
                reported: true,
                health: Some(ControlCauseCase::SessionCutOff),
                events: vec![true],
            },
        },
        CutOffTestCase {
            description: "a statement that ran past its limit",
            session: ("SET LOCAL statement_timeout = 300", 0, "SELECT pg_sleep(3)"),
            expected_report: CutOffReport {
                cause: Some(ControlCauseCase::SessionCutOff),
                reported: true,
                health: Some(ControlCauseCase::SessionCutOff),
                events: vec![false],
            },
        },
    ];
    for test_case in test_cases {
        let Some(observed) = run(cut_off_session(test_case.session)) else {
            continue;
        };
        assert_eq!(
            observed, test_case.expected_report,
            "{}",
            test_case.description
        );
    }
}
