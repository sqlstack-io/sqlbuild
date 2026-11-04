//! Widened custom roles take effect only through another administrator or a confirmed sole one.

use serde_json::json;

use crate::credentials::models::Action;
use crate::credentials::tests::role_helpers::{
    SUPPORT_ROLE, approve, granting_role, orders_grant, publish_support_role, request, role_audit,
    role_fixture,
};
use crate::credentials::tests::test_types::CustomRoleTestCase;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_widened_custom_role_when_unapproved_then_new_grants_stay_refused() {
    let test_cases = [
        CustomRoleTestCase {
            description: "SQLite refuses an unapproved widening",
            backend: Backend::Sqlite,
            expected_role: None,
        },
        CustomRoleTestCase {
            description: "PostgreSQL refuses an unapproved widening",
            backend: Backend::Postgres,
            expected_role: None,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let roles = role_fixture(store, 2).await;
            let activation = request(store, &roles, 0).await;
            assert_eq!(
                granting_role(store, &roles, Action::View).await.as_deref(),
                test_case.expected_role,
                "{}: a request alone grants nothing",
                test_case.description
            );
            approve(store, &roles, (1, &activation, false))
                .await
                .expect("first approval");
            publish_support_role(
                store,
                roles.location_id,
                (
                    2,
                    json!([orders_grant(json!(["view", "job.run"]), &["orders"])]),
                ),
            )
            .await;
            assert_eq!(
                granting_role(store, &roles, Action::JobRun)
                    .await
                    .as_deref(),
                test_case.expected_role,
                "{}: the widened action waits for approval",
                test_case.description
            );
            assert_eq!(
                granting_role(store, &roles, Action::View).await.as_deref(),
                Some(SUPPORT_ROLE),
                "{}: unchanged grants stay effective",
                test_case.description
            );
            assert!(matches!(
                approve(store, &roles, (1, &activation, false)).await,
                Err(ControlError::Invalid(_))
            ));
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_widened_custom_role_when_another_administrator_approves_then_it_applies_and_narrowing_is_immediate()
 {
    let test_cases = [
        CustomRoleTestCase {
            description: "SQLite applies another administrator's approval",
            backend: Backend::Sqlite,
            expected_role: Some(SUPPORT_ROLE),
        },
        CustomRoleTestCase {
            description: "PostgreSQL applies another administrator's approval",
            backend: Backend::Postgres,
            expected_role: Some(SUPPORT_ROLE),
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
                "../../benchmarks/duckshop/edge_cases/orchestration/custom_role_approval.json",
            );
            let case: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(path).expect("role fixture"))
                    .expect("role case");
            let publication = |index: usize| case["publications"][index].clone();
            let roles = role_fixture(store, 2).await;
            let first = request(store, &roles, 0).await;
            assert!(
                approve(store, &roles, (1, &first, false))
                    .await
                    .expect("approve")
            );
            publish_support_role(store, roles.location_id, (2, publication(1))).await;
            let widened = request(store, &roles, 0).await;
            publish_support_role(store, roles.location_id, (3, publication(2))).await;
            assert!(
                approve(store, &roles, (1, &widened, false))
                    .await
                    .expect("approve widening")
            );
            assert_eq!(
                granting_role(store, &roles, Action::JobRun)
                    .await
                    .as_deref(),
                test_case.expected_role,
                "{}",
                test_case.description
            );
            publish_support_role(store, roles.location_id, (4, publication(3))).await;
            assert_eq!(
                json!({
                    "view": granting_role(store, &roles, Action::View).await,
                    "job.run": granting_role(store, &roles, Action::JobRun).await,
                }),
                case["expected_after_narrowing"],
                "{}: narrowing applies on publish",
                test_case.description
            );
            let audit = role_audit(store).await;
            assert_eq!(audit.len(), 4);
            assert_eq!(
                (
                    audit[3].0.as_str(),
                    &audit[3].1["signed"],
                    &audit[3].1["self_approval"]
                ),
                ("role.approved", &json!(false), &json!(false))
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_sole_administrator_when_self_approval_is_confirmed_then_it_applies_and_is_audited() {
    let test_cases = [
        CustomRoleTestCase {
            description: "SQLite sole administrator confirms a self-approval",
            backend: Backend::Sqlite,
            expected_role: Some(SUPPORT_ROLE),
        },
        CustomRoleTestCase {
            description: "PostgreSQL sole administrator confirms a self-approval",
            backend: Backend::Postgres,
            expected_role: Some(SUPPORT_ROLE),
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let roles = role_fixture(store, 1).await;
            let activation = request(store, &roles, 0).await;
            assert!(matches!(
                approve(store, &roles, (0, &activation, false)).await,
                Err(ControlError::Denied { .. })
            ));
            assert!(
                approve(store, &roles, (0, &activation, true))
                    .await
                    .expect("confirmed")
            );
            assert_eq!(
                granting_role(store, &roles, Action::View).await.as_deref(),
                test_case.expected_role,
                "{}",
                test_case.description
            );
            let audit = role_audit(store).await;
            assert_eq!(
                (
                    audit[1].0.as_str(),
                    &audit[1].1["signed"],
                    &audit[1].1["self_approval"]
                ),
                ("role.approved", &json!(false), &json!(true)),
                "{}: the approval is audited as an unsigned self-approval",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_two_administrators_when_requester_self_approves_then_it_is_refused() {
    let test_cases = [
        CustomRoleTestCase {
            description: "SQLite refuses self-approval with another administrator",
            backend: Backend::Sqlite,
            expected_role: None,
        },
        CustomRoleTestCase {
            description: "PostgreSQL refuses self-approval with another administrator",
            backend: Backend::Postgres,
            expected_role: None,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let roles = role_fixture(store, 2).await;
            let activation = request(store, &roles, 0).await;
            assert!(matches!(
                approve(store, &roles, (0, &activation, true)).await,
                Err(ControlError::Denied { .. })
            ));
            assert_eq!(
                granting_role(store, &roles, Action::View).await.as_deref(),
                test_case.expected_role,
                "{}",
                test_case.description
            );
            assert_eq!(role_audit(store).await.len(), 1);
            close_test_store(fixture).await;
        });
    }
}
