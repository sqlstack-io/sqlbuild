//! Real session, role, immutable-job and database paths enforce sensitive-run permissions.

use serde_json::json;
use uuid::Uuid;

use crate::authorization::main::{
    assign_builtin_role::assign_builtin_role, bootstrap_user::bootstrap_user,
    change_administrator::change_administrator,
};
use crate::credentials::main::{
    identities::create_principal::create_principal, sessions::login_password::login_password,
    sessions::set_password::set_password,
};
use crate::credentials::models::{
    AdministratorChange, AuditContext, BuiltinRole, DefaultRole, PasswordAuthority, PasswordLogin,
    RoleAssignmentRequest,
};
use crate::credentials::tests::location_helpers::create_location;
use crate::runs::main::{
    advance_groups::advance_groups, change_run::change_run, list_person_runs::list_person_runs,
    list_runs::list_runs, request_job_run::request_job_run, request_person_run::request_person_run,
    request_rerun::request_rerun,
};
use crate::runs::models::{
    CancelRunRequest, JobRunRequest, PersonRunRequest, RerunRequest, RunAuthority, RunChange,
    RunChangeRequest,
};
use crate::runs::tests::helpers::{request_run, snapshot};
use crate::runs::tests::test_types::PersonRunPermissionTestCase;
use crate::scheduler::tests::contracts::helpers::acquire_lease;
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
                    let person = create_principal(store, &context, "orders-person")
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
                    let credentials = sessions[1].identity.principal_id;
                    let location_id = create_location(store, "orders", "test-owner")
                        .await
                        .expect("location");
                    let mut input = snapshot(location_id);
                    input.documents.insert("jobs".into(), json!({
                    "daily_orders": {"description":"Build orders", "selection":["orders"], "settings":inputs["job_settings"][settings_index]},
                    "customers": {"description":"Build customers", "selection":["customers"], "settings":inputs["job_settings"][0]}
                }));
                    input.documents.insert("groups".into(), json!({"orders_group": {"description":"Build both jobs", "selection":["customers","orders"], "jobs":{"daily_orders":[],"customers":[]}}}));
                    let published = publish_snapshot(store, &input)
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
                        full_refresh: false,
                    };
                    assert!(
                        matches!(
                            request_person_run(
                                store,
                                (&context, credentials),
                                PersonRunRequest::Job(&request)
                            )
                            .await,
                            Err(ControlError::Denied { .. })
                        ),
                        "no role is the default"
                    );
                    let assignment = RoleAssignmentRequest {
                        principal_id: person,
                        role: test_case.role.into(),
                        location_id: None,
                        target: None,
                        revoke: false,
                    };
                    assert!(
                        assign_builtin_role(
                            store,
                            (&context, sessions[0].identity.principal_id),
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
                    let lease = acquire_lease(store, Uuid::now_v7())
                        .await
                        .expect("elect")
                        .expect("leader");
                    let mut reruns = Vec::new();
                    for job_key in ["daily_orders", "orders_group"] {
                        let result = request_person_run(
                            store,
                            (&context, credentials),
                            PersonRunRequest::Job(&JobRunRequest {
                                run_id: Uuid::now_v7(),
                                job_key: job_key.into(),
                                ..request.clone()
                            }),
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
                        let source_id = Uuid::now_v7();
                        request_job_run(
                            store,
                            &context,
                            &JobRunRequest {
                                run_id: source_id,
                                job_key: job_key.into(),
                                priority: 1,
                                ..request.clone()
                            },
                        )
                        .await
                        .expect("owner creates urgent source with frozen settings");
                        let cancellation = CancelRunRequest {
                            run_id: source_id,
                            event_id: Uuid::now_v7(),
                            reason: "Stop before worker launch".into(),
                        };
                        let can_cancel = test_case.role != BuiltinRole::Viewer;
                        for expected in [true, false] {
                            assert_eq!(
                                request_person_run(
                                    store,
                                    (&context, credentials),
                                    PersonRunRequest::Cancel(&cancellation),
                                )
                                .await
                                .ok(),
                                can_cancel.then_some(expected),
                                "cancellation permits stopping even sensitive or prioritised work"
                            );
                        }
                        let source_id = Uuid::now_v7();
                        request_job_run(
                            store,
                            &context,
                            &JobRunRequest {
                                run_id: source_id,
                                job_key: job_key.into(),
                                ..request.clone()
                            },
                        )
                        .await
                        .expect("owner creates independent rerun source");
                        change_run(
                            store,
                            RunAuthority::Operator(&context),
                            &RunChangeRequest {
                                event_id: Uuid::now_v7(),
                                run_id: source_id,
                                claim_id: None,
                                change: RunChange::CancelRequested {
                                    actor: context.actor.clone(),
                                    reason: cancellation.reason.clone(),
                                },
                            },
                        )
                        .await
                        .expect("owner stops independent rerun source");
                        for _ in 0..2 {
                            advance_groups(
                                store,
                                &lease,
                                (&Default::default(), (location_id, "development")),
                            )
                            .await
                            .expect("scheduler confirms stopped children and parent");
                        }
                        let rerun = RerunRequest {
                            run_id: Uuid::now_v7(),
                            source_run_id: source_id,
                            from_failure: false,
                            reason: "Rerun the original frozen execution".into(),
                            priority: 0,
                        };
                        for expected in [true, false] {
                            let result =
                                request_rerun(store, (&context, Some(credentials)), &rerun).await;
                            assert_eq!(
                                result.as_ref().ok().copied(),
                                test_case.expected_allowed.then_some(expected),
                                "{}: frozen job/group rerun and idempotent retry: {result:?}",
                                test_case.description,
                            );
                            assert_eq!(
                                matches!(result, Err(ControlError::Denied { .. })),
                                !test_case.expected_allowed,
                                "denial must be authorization, not invalid input"
                            );
                        }
                        reruns.push(rerun);
                    }
                    let mut adhoc = crate::runs::tests::helpers::request(
                        (location_id, published.snapshot_id),
                        Uuid::now_v7().as_u128(),
                        0,
                        &["orders"],
                    );
                    adhoc.settings =
                        serde_json::from_value(inputs["job_settings"][settings_index].clone())
                            .expect("ad-hoc frozen settings");
                    request_run(store, &adhoc)
                        .await
                        .expect("owner requests ad-hoc execution");
                    request_person_run(
                        store,
                        (&context, sessions[0].identity.principal_id),
                        PersonRunRequest::Cancel(&CancelRunRequest {
                            run_id: adhoc.run_id,
                            event_id: Uuid::now_v7(),
                            reason: "Stop ad-hoc source".into(),
                        }),
                    )
                    .await
                    .expect("administrator stops ad-hoc execution");
                    let adhoc_rerun = RerunRequest {
                        run_id: Uuid::now_v7(),
                        source_run_id: adhoc.run_id,
                        from_failure: false,
                        reason: "Repeat ad-hoc execution".into(),
                        priority: 0,
                    };
                    let can_rerun_adhoc = test_case.role == BuiltinRole::Admin
                        || (test_case.role == BuiltinRole::Developer && settings_index == 0);
                    let result =
                        request_rerun(store, (&context, Some(credentials)), &adhoc_rerun).await;
                    assert_eq!(
                        result.as_ref().ok().copied(),
                        can_rerun_adhoc.then_some(true),
                        "ad-hoc: {result:?}"
                    );
                    assert_eq!(
                        matches!(result, Err(ControlError::Denied { .. })),
                        !can_rerun_adhoc
                    );
                    reruns.push(adhoc_rerun);
                    assert_eq!(
                        list_runs(store, (location_id, "development"), 100)
                            .await
                            .expect("queue projection")
                            .len(),
                        9 + 8 * usize::from(test_case.expected_allowed)
                            + usize::from(can_rerun_adhoc),
                        "owner sources plus allowed manual/rerun jobs and expanded groups only"
                    );
                    let administrator_credentials = sessions[0].identity.principal_id;
                    assert!(
                        change_administrator(
                            store,
                            (&context, administrator_credentials),
                            (person, AdministratorChange::Disable),
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
                            (&context, administrator_credentials),
                            (person, AdministratorChange::Enable),
                        )
                        .await
                        .expect("enable person")
                    );
                    assert!(
                        matches!(
                            request_person_run(
                                store,
                                (&context, credentials),
                                PersonRunRequest::Job(&JobRunRequest {
                                    run_id: Uuid::now_v7(),
                                    ..request
                                })
                            )
                            .await,
                            Err(ControlError::Denied { .. })
                        ),
                        "reenabling cannot restore a revoked grant"
                    );
                    for rerun in reruns {
                        assert!(
                            matches!(
                                request_rerun(store, (&context, Some(credentials)), &rerun).await,
                                Err(ControlError::Denied { .. })
                            ),
                            "retry must reauthorize after revocation"
                        );
                    }
                    close_test_store(fixture).await;
                });
            }
        }
    }
}
