//! Axum's 5xx rejections name Rust types, so they reach clients as the generic JSON error; 4xx text describes the request.

use axum::extract::Path;
use axum::routing::get;
use axum::{Extension, Router, middleware};
use uuid::Uuid;

use crate::api::_helpers::resources::server_errors::generic_server_errors;
use crate::api::errors::ApiError;
use crate::api::tests::test_types::ServerErrorTestCase;
use crate::store::errors::ControlError;
use crate::store::tests::store_helpers::run;

#[test]
fn given_framework_rejections_when_served_then_server_errors_hide_their_text_and_request_errors_keep_it()
 {
    let test_cases = [
        ServerErrorTestCase {
            description: "a missing extension names a Rust type",
            path: "/missing",
            expected_status: 500,
            expected_body: r#"{"error":"internal server error"}"#,
        },
        ServerErrorTestCase {
            description: "a malformed path value describes the caller's own request",
            path: "/runs/not-a-run",
            expected_status: 400,
            expected_body: "Invalid URL: Cannot parse `run_id` with value `not-a-run`",
        },
        ServerErrorTestCase {
            description: "a handler's JSON error is already generic",
            path: "/failed",
            expected_status: 500,
            expected_body: r#"{"error":"control-store operation failed"}"#,
        },
    ];
    run(async {
        let application = Router::new()
            .route("/missing", get(|Extension(_): Extension<String>| async {}))
            .route("/runs/{run_id}", get(|Path(_): Path<Uuid>| async {}))
            .route(
                "/failed",
                get(|| async {
                    Err::<(), ApiError>(ControlError::Schema("synthetic".into()).into())
                }),
            )
            .layer(middleware::map_response(generic_server_errors));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("socket");
        let url = format!("http://{}", listener.local_addr().expect("address"));
        let server = tokio::spawn(async move { axum::serve(listener, application).await });
        for test_case in test_cases {
            let response = reqwest::get(format!("{url}{}", test_case.path))
                .await
                .expect("request");
            let status = response.status().as_u16();
            let body = response.text().await.expect("body");
            assert!(
                status == test_case.expected_status && body.starts_with(test_case.expected_body),
                "{}: {status} {body}",
                test_case.description
            );
        }
        server.abort();
    });
}
