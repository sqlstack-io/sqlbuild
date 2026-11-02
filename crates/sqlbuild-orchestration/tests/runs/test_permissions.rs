//! Real session, role, immutable-job and database paths enforce sensitive-run permissions.

use serde_json::json;
use uuid::Uuid;

use crate::authorization::main::{
    assign_builtin_role::assign_builtin_role, bootstrap_user::bootstrap_user,
    change_administrator::change_administrator,
};
use crate::credentials::main::{
    create_principal::create_principal, login_password::login_password, set_password::set_password,
};
use crate::credentials::models::{
    AdministratorChange, AuditContext, BuiltinRole, DefaultRole, PasswordAuthority, PasswordLogin,
    PrincipalKind, RoleAssignmentRequest,
};
use crate::credentials::tests::location_helpers::create_location;
use crate::runs::main::{
    list_person_runs::list_person_runs, list_runs::list_runs,
    request_person_job_run::request_person_job_run,
};
use crate::runs::models::JobRunRequest;
use crate::runs::tests::helpers::snapshot;
use crate::runs::tests::test_types::PersonRunPermissionTestCase;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_scoped_person_roles_when_jobs_request_sensitive_settings_then_only_administrators_can_rewrite_history()
 {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks/duckshop/edge_cases/orchestration/administrator_lifecycle.json");
    let inputs: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("DuckShop person lifecycle"))
            .expect("lifecycle settings");
    let settings_count = inputs["job_settings"].as_array().expect("settings").len();
    let test_cases = [
        PersonRunPermissionTestCase {
            description: "viewer cannot run ordinary jobs",
            role: BuiltinRole::Viewer,
            settings_range: 0..1,
            expected_allowed: false,
        },
        PersonRunPermissionTestCase {
            description: "operator can run ordinary jobs",
            role: BuiltinRole::Operator,
            settings_range: 0..1,
            expected_allowed: true,
        },
        PersonRunPermissionTestCase {
            description: "developer can run ordinary jobs",
            role: BuiltinRole::Developer,
            settings_range: 0..1,
            expected_allowed: true,
        },
        PersonRunPermissionTestCase {
            description: "operator cannot rewrite history",
            role: BuiltinRole::Operator,
            settings_range: 1..settings_count,
            expected_allowed: false,
        },
        PersonRunPermissionTestCase {
            description: "developer cannot rewrite history",
            role: BuiltinRole::Developer,
            settings_range: 1..settings_count,
            expected_allowed: false,
        },
        PersonRunPermissionTestCase {
            description: "installation administrator can rewrite history",
            role: BuiltinRole::Admin,
            settings_range: 1..settings_count,
            expected_allowed: true,
        },
    ];
    for backend in [Backend::Sqlite, Backend::Postgres] {
        for test_case in &test_cases {
            for settings_index in test_case.settings_range.clone() {
                run(async {
                    let fixture = open_test_store(backend).await.expect("real database");
                    let store = &fixture.store;
                    migrate_store(store).await.expect("migrate");
                    let context = AuditContext {
                        actor: "local-control-store-owner".into(),
                        granting_role: "control-store-owner".into(),
                        request_id: Uuid::now_v7(),
                        ip_address: Some("127.0.0.1".parse().expect("trusted peer")),
                    };
                    let admin = bootstrap_user(store, &context, "orders-admin")
                        .await
                        .expect("bootstrap");
                    let person =
                        create_principal(store, &context, ("orders-person", PrincipalKind::Person))
                            .await
                            .expect("role-free person");
                    let mut sessions = Vec::new();
                    for (id, name) in [(admin, "orders-admin"), (person, "orders-person")] {
                        set_password(
                            store,
                            &context,
                            PasswordAuthority::Operator(id),
                            "orders-password-2026",
                        )
                        .await
                        .expect("provision password");
                        let PasswordLogin::Authenticated(session) =
                            login_password(store, &context, (name, "orders-password-2026"))
                                .await
                                .expect("login")
                        else {
                            panic!("valid session")
                        };
                        sessions.push(session);
                    }
                    let credentials = (sessions[1].bearer.as_str(), sessions[1].csrf.as_str());
                    let location_id = create_location(store, "orders", "test-owner")
                        .await
                        .expect("location");
                    let mut input = snapshot(location_id);
                    input.documents.insert("jobs".into(), json!({
                    "daily_orders": {"description":"Build orders", "selection":["orders"], "settings":inputs["job_settings"][settings_index]},
                    "customers": {"description":"Build customers", "selection":["customers"], "settings":inputs["job_settings"][0]}
                }));
                    input.documents.insert("groups".into(), json!({"orders_group": {"description":"Build both jobs", "selection":["customers","orders"], "jobs":{"daily_orders":[],"customers":[]}}}));
                    publish_snapshot(store, &input)
                        .await
                        .expect("publish immutable job");
                    let request = JobRunRequest {
                        run_id: Uuid::now_v7(),
                        location_id,
                        target: "development".into(),
                        job_key: "daily_orders".into(),
                        reason: "Refresh order totals".into(),
                        priority: 0,
                        window: None,
                    };
                    assert!(
                        matches!(
                            request_person_job_run(store, (&context, credentials), &request).await,
                            Err(ControlError::Denied { .. })
                        ),
                        "no role is the default"
                    );
                    let assignment = RoleAssignmentRequest {
                        principal_id: person,
                        role: test_case.role,
                        location_id: None,
                        target: None,
                        revoke: false,
                    };
                    assert!(
                        assign_builtin_role(
                            store,
                            (
                                &context,
                                (sessions[0].bearer.as_str(), sessions[0].csrf.as_str())
                            ),
                            &assignment
                        )
                        .await
                        .expect("authenticated assignment")
                    );
                    assert!(
                        list_person_runs(
                            store,
                            (&context, credentials),
                            (location_id, "development", 10),
                            DefaultRole::None,
                        )
                        .await
                        .expect("built-in metadata access")
                        .is_empty()
                    );
                    for job_key in ["daily_orders", "orders_group"] {
                        let result = request_person_job_run(
                            store,
                            (&context, credentials),
                            &JobRunRequest {
                                run_id: Uuid::now_v7(),
                                job_key: job_key.into(),
                                ..request.clone()
                            },
                        )
                        .await;
                        assert_eq!(
                            matches!(&result, Err(ControlError::Denied { .. })),
                            !test_case.expected_allowed,
                            "{}: {result:?}",
                            test_case.description
                        );
                        assert_eq!(
                            result.ok(),
                            test_case.expected_allowed.then_some(true),
                            "{}",
                            test_case.description
                        );
                    }
                    assert_eq!(
                        list_runs(store, (location_id, "development"), 10)
                            .await
                            .expect("queue projection")
                            .len(),
                        4 * usize::from(test_case.expected_allowed),
                        "denial cannot publish work; success queues the job, group and two children"
                    );
                    let administrator_credentials =
                        (sessions[0].bearer.as_str(), sessions[0].csrf.as_str());
                    assert!(
                        change_administrator(
                            store,
                            &context,
                            administrator_credentials,
                            (person, AdministratorChange::Disable)
                        )
                        .await
                        .expect("disable person")
                    );
                    assert!(
                        assign_builtin_role(
                            store,
                            (&context, administrator_credentials),
                            &RoleAssignmentRequest {
                                revoke: true,
                                ..assignment
                            }
                        )
                        .await
                        .expect("revoke a disabled person's grant before reenabling")
                    );
                    assert!(
                        change_administrator(
                            store,
                            &context,
                            administrator_credentials,
                            (person, AdministratorChange::Enable)
                        )
                        .await
                        .expect("enable person")
                    );
                    assert!(
                        matches!(
                            request_person_job_run(
                                store,
                                (&context, credentials),
                                &JobRunRequest {
                                    run_id: Uuid::now_v7(),
                                    ..request
                                }
                            )
                            .await,
                            Err(ControlError::Denied { .. })
                        ),
                        "reenabling cannot restore a revoked grant"
                    );
                    close_test_store(fixture).await;
                });
            }
        }
    }
}
