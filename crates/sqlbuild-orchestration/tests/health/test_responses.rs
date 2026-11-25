//! Each control database failure reaches the API by its own sentence, case, since and request ID; health records it.

use axum::routing::get;
use axum::{Router, middleware};
use tower::ServiceExt;

use crate::api::_helpers::resources::request_ids::identify_request;
use crate::api::errors::ApiError;
use crate::health::_helpers::details::health_details;
use crate::health::models::{ControlCauseCase, ServerHealth};
use crate::health::tests::helpers::{coded, io, sqlite_busy, target};
use crate::health::tests::test_types::CauseResponseTestCase;
use crate::store::errors::ControlError;
use crate::store::tests::store_helpers::run;

#[test]
fn given_each_control_database_failure_when_a_request_meets_it_then_the_response_names_its_cause() {
    let test_cases = [
        CauseResponseTestCase {
            description: "a host name that doesn't resolve",
            error: || {
                io(
                    std::io::ErrorKind::Other,
                    "failed to lookup address information: Temporary failure in name resolution",
                )
            },
            expected_status: 503,
            expected_message: "Can't reach the control database: db.internal doesn't resolve (DNS)",
            expected_case: ControlCauseCase::NameLookup,
            expected_unavailable: true,
        },
        CauseResponseTestCase {
            description: "a refused connection",
            error: || {
                io(
                    std::io::ErrorKind::ConnectionRefused,
                    "Connection refused (os error 111)",
                )
            },
            expected_status: 503,
            expected_message: "Can't reach the control database at db.internal:5432",
            expected_case: ControlCauseCase::Unreachable,
            expected_unavailable: true,
        },
        CauseResponseTestCase {
            description: "a timed-out connection",
            error: || io(std::io::ErrorKind::TimedOut, "connection timed out"),
            expected_status: 503,
            expected_message: "Can't reach the control database at db.internal:5432",
            expected_case: ControlCauseCase::Unreachable,
            expected_unavailable: true,
        },
        CauseResponseTestCase {
            description: "a pool wait that ran out while connecting",
            error: || ControlError::Database(sqlx_core::Error::PoolTimedOut),
            expected_status: 503,
            expected_message: "Can't reach the control database at db.internal:5432",
            expected_case: ControlCauseCase::Unreachable,
            expected_unavailable: true,
        },
        CauseResponseTestCase {
            description: "a rejected login",
            error: || coded("28P01", "password authentication failed for user \"sqb\""),
            expected_status: 503,
            expected_message: "The control database rejected the login for sqb",
            expected_case: ControlCauseCase::LoginRejected,
            expected_unavailable: true,
        },
        CauseResponseTestCase {
            description: "too many connections",
            error: || coded("53300", "sorry, too many clients already"),
            expected_status: 503,
            expected_message: "The control database has no free connections",
            expected_case: ControlCauseCase::NoFreeConnections,
            expected_unavailable: true,
        },
        CauseResponseTestCase {
            description: "a database shutting down",
            error: || {
                coded(
                    "57P01",
                    "terminating connection due to administrator command",
                )
            },
            expected_status: 503,
            expected_message: "The control database is restarting",
            expected_case: ControlCauseCase::Restarting,
            expected_unavailable: true,
        },
        CauseResponseTestCase {
            description: "a database starting up",
            error: || coded("57P03", "the database system is starting up"),
            expected_status: 503,
            expected_message: "The control database is restarting",
            expected_case: ControlCauseCase::Restarting,
            expected_unavailable: true,
        },
        CauseResponseTestCase {
            description: "a locked SQLite database",
            error: sqlite_busy,
            expected_status: 503,
            expected_message: "The control database is locked by another process",
            expected_case: ControlCauseCase::Locked,
            expected_unavailable: true,
        },
        CauseResponseTestCase {
            description: "an unknown database error keeps its own message",
            error: || {
                coded(
                    "XX001",
                    "could not read block 7 in file \"base/16384/2619\"",
                )
            },
            expected_status: 500,
            expected_message: "Control database error: could not read block 7 in file \"base/16384/2619\"",
            expected_case: ControlCauseCase::Other,
            expected_unavailable: false,
        },
        CauseResponseTestCase {
            description: "an unknown transient error keeps its own message",
            error: || ControlError::Database(sqlx_core::Error::WorkerCrashed),
            expected_status: 503,
            expected_message: "Control database error: attempted to communicate with a crashed background worker",
            expected_case: ControlCauseCase::Other,
            expected_unavailable: true,
        },
    ];
    run(async {
        for test_case in test_cases {
            let health = ServerHealth::new("1.0.0", false);
            health.set_target(target());
            let error = test_case.error;
            let application = Router::new()
                .route(
                    "/api/thing",
                    get(move || async move { Err::<(), ApiError>(error().into()) }),
                )
                .layer(middleware::from_fn_with_state(
                    health.clone(),
                    identify_request,
                ));
            let response = application
                .oneshot(
                    axum::http::Request::get("/api/thing")
                        .body(axum::body::Body::empty())
                        .expect("request"),
                )
                .await
                .expect("response");
            let status = response.status().as_u16();
            let request_id = response
                .headers()
                .get("x-request-id")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body");
            let body: serde_json::Value = serde_json::from_slice(&bytes).expect("JSON body");
            let failing_since = health
                .control_failing_since()
                .map(|since| serde_json::json!(since));
            let recorded_requests = health_details(&health).components[0]
                .request_ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            assert_eq!(
                (
                    status,
                    body["error"].clone(),
                    body["cause"]["case"].clone(),
                    body["cause"]["since"].is_string(),
                    body["request_id"].as_str().map(str::to_owned),
                    failing_since.is_some(),
                    failing_since
                        .clone()
                        .unwrap_or_else(|| body["cause"]["since"].clone())
                        == body["cause"]["since"],
                    recorded_requests.len() == usize::from(test_case.expected_unavailable),
                ),
                (
                    test_case.expected_status,
                    serde_json::json!(test_case.expected_message),
                    serde_json::json!(test_case.expected_case),
                    true,
                    request_id,
                    test_case.expected_unavailable,
                    true,
                    true,
                ),
                "{}",
                test_case.description
            );
        }
    });
}
