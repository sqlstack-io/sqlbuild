//! Existing human authentication mechanisms share live authority, never machine authority.

use reqwest::{Method, StatusCode};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::tests::helpers::stop_api;
use crate::api::tests::oidc_helpers::{start_provider, stop_provider};
use crate::api::tests::person_sessions_helpers::{browser_sessions, queue_counts, request};
use crate::api::tests::person_sessions_test_types::SessionRoleCase;
use crate::api::tests::test_types::LocationWorkTestCase;
use crate::authorization::main::{
    assign_builtin_role::assign_builtin_role, bootstrap_user::bootstrap_user,
};
use crate::credentials::main::{
    identities::create_principal::create_principal, sessions::login_password::login_password,
    sessions::set_password::set_password,
};
use crate::credentials::models::{
    AuditContext, PasswordAuthority, PasswordLogin, RoleAssignmentRequest,
};
use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::list_triggers::list_triggers;
use crate::platform::main::read_location_work::read_location_work;
use crate::platform::models::LocationWork;
use crate::runs::tests::helpers::{request as run_request, request_run};
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::sensors::main::begin_sensor_evaluation::begin_sensor_evaluation;
use crate::sensors::tests::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::snapshots::models::SnapshotNode;
use crate::store::main::{begin::begin, commit::commit, migrate_store::migrate_store};
use crate::store::models::TransactionMode;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_browser_session_roles_when_work_is_requested_then_scope_revocation_and_work_kind_remain_enforced()
 {
    let test_cases = [
        LocationWorkTestCase {
            description: "SQLite browser session matrix",
            backend: Backend::Sqlite,
            expected_generation: 1,
        },
        LocationWorkTestCase {
            description: "PostgreSQL browser session matrix",
            backend: Backend::Postgres,
            expected_generation: 1,
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend).await.expect("database");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let replica = second_store(store).await;
            let context = AuditContext {
                actor: "control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: Some(std::net::Ipv4Addr::LOCALHOST.into()),
            };
            let admin = bootstrap_user(store, &context, "orders-admin")
                .await
                .expect("administrator");
            let person = create_principal(store, &context, "orders-person")
                .await
                .expect("person");
            for id in [admin, person] {
                set_password(
                    store,
                    &context,
                    PasswordAuthority::Operator(id),
                    "orders-password-2026",
                )
                .await
                .expect("password");
            }
            let PasswordLogin::Authenticated(admin_session) =
                login_password(store, &context, ("orders-admin", "orders-password-2026"))
                    .await
                    .expect("admin login")
            else {
                panic!("valid admin")
            };
            let location = create_location(store, "orders", "test-owner")
                .await
                .expect("location");
            let mut input = snapshot(location);
            input.documents.get_mut("triggers").expect("triggers")["health_orders"]["when"] =
                json!({"kind":"custom", "function":"orders_ready", "code_hash":"orders_v1"});
            let sql_input: Value = serde_json::from_str(
                &std::fs::read_to_string(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("../../benchmarks/duckshop/edge_cases/orchestration/node_sql.json"),
                )
                .expect("SQL fixture"),
            )
            .expect("SQL inputs");
            let orders = input.nodes.get_mut("orders").expect("orders node");
            orders.raw_sql = Some(sql_input["raw_sql"].as_str().expect("raw SQL").into());
            orders.compiled_sql = Some(
                sql_input["compiled_sql"]
                    .as_str()
                    .expect("compiled SQL")
                    .into(),
            );
            orders.python_source = Some(
                sql_input["python_source"]
                    .as_str()
                    .expect("Python source")
                    .into(),
            );
            input.nodes.insert(
                "udf:orders".into(),
                SnapshotNode {
                    kind: "udf".into(),
                    definition: json!({"name":"orders", "arguments":[], "returns":"INTEGER"}),
                    raw_sql: None,
                    compiled_sql: Some(
                        sql_input["function_sql"]
                            .as_str()
                            .expect("function SQL")
                            .into(),
                    ),
                    python_source: None,
                },
            );
            let published = publish_snapshot(store, &input).await.expect("publication");
            assert_eq!(
                published.generation, test_case.expected_generation,
                "{}",
                test_case.description
            );
            let seed_run = run_request((location, published.snapshot_id), 100, 0, &["orders"]);
            request_run(store, &seed_run)
                .await
                .expect("nonempty metadata control");
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("election")
                .expect("leader");
            let trigger = list_triggers(store, location, "development")
                .await
                .expect("triggers")
                .remove(0);
            let evaluation = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                .await
                .expect("fenced sensor request")
                .expect("requested");
            let transaction = begin(store, TransactionMode::Read)
                .await
                .expect("inspect sensor");
            let (transaction, stored) = read_location_work(transaction, evaluation.evaluation_id)
                .await
                .expect("sensor work exists");
            assert!(matches!(
                stored.expect("actual work").record.request_json.work,
                LocationWork::EvaluateSensor { .. }
            ));
            commit(transaction).await.expect("close read");
            let provider = start_provider().await;
            let sessions = browser_sessions(store, &provider, person).await;
            assert_eq!(sessions.len(), 3, "password, proxy and OIDC all execute");
            let path =
                format!("/api/control/locations/{location}/targets/development/connection-tests");
            let metadata = format!("/api/control/locations/{location}/targets/development/runs");
            let sql_path =
                format!("/api/control/locations/{location}/targets/development/nodes/orders/sql");
            let inputs: Value = serde_json::from_str(&std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../benchmarks/duckshop/edge_cases/orchestration/administrator_lifecycle.json")).expect("DuckShop matrix")).expect("JSON");
            let roles: Vec<SessionRoleCase> =
                serde_json::from_value(inputs["session_roles"].clone())
                    .expect("independent expectations");
            let mut expected_work = 1;
            let mut expected_runs = 1;
            let mut last_request = Value::Null;
            for role_case in roles {
                for role in role_case.role.iter().copied() {
                    assign_builtin_role(
                        &replica,
                        (&context, (&admin_session.bearer, &admin_session.csrf)),
                        &RoleAssignmentRequest {
                            principal_id: person,
                            role: role.into(),
                            location_id: None,
                            target: None,
                            revoke: false,
                        },
                    )
                    .await
                    .expect("assign role through independent store");
                }
                for session in &sessions {
                    let body =
                        json!({"request_id":Uuid::now_v7(), "reason":"Check orders connectivity"});
                    last_request = body.clone();
                    let connection = request(
                        &provider.client,
                        session,
                        (Method::POST, &path),
                        (&body, &session.csrf),
                    )
                    .await;
                    assert_eq!(connection.status().as_u16(), role_case.expected_connection);
                    assert_eq!(connection.headers()["cache-control"], "no-store");
                    let read = request(
                        &provider.client,
                        session,
                        (Method::GET, &format!("{path}/{}", evaluation.evaluation_id)),
                        (&Value::Null, &session.csrf),
                    )
                    .await;
                    assert_eq!(read.status().as_u16(), role_case.expected_connection);
                    let read_body: Value = read.json().await.expect("sensor response");
                    assert_eq!(
                        read_body.is_null(),
                        role_case.expected_connection == StatusCode::OK.as_u16(),
                        "authorized reads hide actual sensor work as null"
                    );
                    let page = request(
                        &provider.client,
                        session,
                        (Method::GET, &metadata),
                        (&Value::Null, &session.csrf),
                    )
                    .await;
                    assert_eq!(page.status().as_u16(), role_case.expected_metadata);
                    let page: Value = page.json().await.expect("metadata response");
                    assert_eq!(
                        page.is_array(),
                        role_case.expected_metadata == StatusCode::OK.as_u16()
                    );
                    assert!(!page.to_string().contains("select 1 as id"));
                    let sql = request(
                        &provider.client,
                        session,
                        (Method::GET, &sql_path),
                        (&Value::Null, ""),
                    )
                    .await;
                    assert_eq!(sql.status().as_u16(), role_case.expected_sql);
                    assert_eq!(sql.headers()["cache-control"], "no-store");
                    let sql: Value = sql.json().await.expect("SQL response");
                    for (key, expected) in [
                        ("snapshot_id", json!(published.snapshot_id)),
                        ("node_key", json!("orders")),
                        ("raw_sql", sql_input["raw_sql"].clone()),
                        ("compiled_sql", sql_input["compiled_sql"].clone()),
                    ] {
                        assert_eq!(
                            sql.get(key),
                            [None, Some(&expected)][usize::from(role_case.expected_sql == 200)]
                        );
                    }
                    assert_eq!(
                        sql.get("decision_id").is_some(),
                        role_case.expected_sql != 200
                    );
                    assert!(sql.get("python_source").is_none());
                    let unpublished = request(
                        &provider.client,
                        session,
                        (Method::GET, &sql_path.replace("development", "preview")),
                        (&Value::Null, ""),
                    )
                    .await;
                    assert_eq!(unpublished.status().as_u16(), role_case.expected_sql);
                    assert_eq!(
                        unpublished
                            .json::<Value>()
                            .await
                            .expect("unpublished scope")
                            .is_null(),
                        role_case.expected_sql == 200
                    );
                    let job = json!({"run_id":Uuid::now_v7(), "location_id":location, "target":"development", "job_key":"orders", "reason":"Refresh orders", "priority":0});
                    assert_eq!(
                        request(
                            &provider.client,
                            session,
                            (Method::POST, "/api/control/runs"),
                            (&job, &session.csrf)
                        )
                        .await
                        .status()
                        .as_u16(),
                        role_case.expected_job
                    );
                    for (endpoint, request_body) in
                        [(&path[..], &body), ("/api/control/runs", &job)]
                    {
                        assert_eq!(
                            request(
                                &provider.client,
                                session,
                                (Method::POST, endpoint),
                                (request_body, "wrong-csrf")
                            )
                            .await
                            .status(),
                            StatusCode::UNAUTHORIZED
                        );
                    }
                    for machine_path in [
                        format!("/api/locations/{location}/targets/development/snapshots"),
                        format!(
                            "/api/locations/{location}/work/{}",
                            evaluation.evaluation_id
                        ),
                        format!(
                            "/api/locations/{location}/targets/development/runs/{}/recall/output_tail?invocation=scheduler/{}",
                            seed_run.run_id, seed_run.run_id
                        ),
                        format!(
                            "/api/locations/{location}/targets/development/runs/{}/recall/artifacts?invocation=scheduler/{}",
                            seed_run.run_id, seed_run.run_id
                        ),
                    ] {
                        assert_eq!(
                            request(
                                &provider.client,
                                session,
                                (Method::GET, &machine_path),
                                (&Value::Null, &session.csrf)
                            )
                            .await
                            .status(),
                            StatusCode::UNAUTHORIZED
                        );
                    }
                }
                expected_work += usize::from(role_case.expected_connection == 200) * sessions.len();
                expected_runs += usize::from(role_case.expected_job == 200) * sessions.len();
                assert_eq!(
                    queue_counts(store).await,
                    json!({"work":expected_work,"runs":expected_runs,"claims":0})
                );
                for role in role_case.role.iter().copied() {
                    assign_builtin_role(
                        &replica,
                        (&context, (&admin_session.bearer, &admin_session.csrf)),
                        &RoleAssignmentRequest {
                            principal_id: person,
                            role: role.into(),
                            location_id: None,
                            target: None,
                            revoke: true,
                        },
                    )
                    .await
                    .expect("revoke through independent store");
                }
                for session in &sessions {
                    assert_eq!(
                        request(
                            &provider.client,
                            session,
                            (Method::GET, &sql_path),
                            (&Value::Null, "")
                        )
                        .await
                        .status(),
                        StatusCode::FORBIDDEN
                    );
                    assert_eq!(
                        request(
                            &provider.client,
                            session,
                            (Method::POST, &path),
                            (&last_request, &session.csrf)
                        )
                        .await
                        .status(),
                        StatusCode::FORBIDDEN
                    );
                    assert_eq!(
                        request(
                            &provider.client,
                            session,
                            (
                                Method::GET,
                                &format!(
                                    "{path}/{}",
                                    last_request["request_id"].as_str().expect("request ID")
                                )
                            ),
                            (&Value::Null, &session.csrf)
                        )
                        .await
                        .status(),
                        StatusCode::FORBIDDEN
                    );
                    assert_eq!(
                        request(
                            &provider.client,
                            session,
                            (Method::GET, &metadata),
                            (&Value::Null, &session.csrf)
                        )
                        .await
                        .status(),
                        StatusCode::FORBIDDEN
                    );
                }
                assert_eq!(
                    queue_counts(store).await,
                    json!({"work":expected_work,"runs":expected_runs,"claims":0})
                );
            }
            assign_builtin_role(
                &replica,
                (&context, (&admin_session.bearer, &admin_session.csrf)),
                &RoleAssignmentRequest {
                    principal_id: person,
                    role: crate::credentials::models::BuiltinRole::Developer.into(),
                    location_id: Some(location),
                    target: Some("development".into()),
                    revoke: false,
                },
            )
            .await
            .expect("scoped developer");
            crate::api::tests::node_sql_helpers::node_sql_boundaries(
                store,
                (&provider.client, &sessions[0]),
                (location, published.snapshot_id),
                (&input, &sql_input),
            )
            .await;
            for session in &sessions {
                let result = request(
                    &provider.client,
                    session,
                    (
                        Method::GET,
                        &format!(
                            "{path}/{}",
                            last_request["request_id"].as_str().expect("accepted ID")
                        ),
                    ),
                    (&Value::Null, &session.csrf),
                )
                .await;
                assert_eq!(result.status(), StatusCode::OK);
                assert_eq!(
                    result
                        .json::<Value>()
                        .await
                        .expect("actual connection record")["request_json"]["request_id"],
                    last_request["request_id"]
                );
                for other in [
                    path.replace("development", "preview"),
                    path.replace(&location.to_string(), &Uuid::now_v7().to_string()),
                ] {
                    assert_eq!(
                        request(
                            &provider.client,
                            session,
                            (Method::POST, &other),
                            (&last_request, &session.csrf)
                        )
                        .await
                        .status(),
                        StatusCode::FORBIDDEN
                    );
                    assert_eq!(
                        request(
                            &provider.client,
                            session,
                            (
                                Method::GET,
                                &format!(
                                    "{other}/{}",
                                    last_request["request_id"].as_str().expect("ID")
                                )
                            ),
                            (&Value::Null, &session.csrf)
                        )
                        .await
                        .status(),
                        StatusCode::FORBIDDEN
                    );
                }
                assert_eq!(
                    request(
                        &provider.client,
                        session,
                        (Method::GET, &format!("{path}/{}", evaluation.evaluation_id)),
                        (&Value::Null, "wrong-csrf")
                    )
                    .await
                    .status(),
                    StatusCode::FORBIDDEN
                );
            }
            assert_eq!(
                queue_counts(store).await,
                json!({"work":expected_work,"runs":expected_runs,"claims":0})
            );
            for session in sessions {
                stop_api(session.api).await;
            }
            stop_provider(provider).await;
            close_store(replica).await;
            close_test_store(fixture).await;
        });
    }
}
