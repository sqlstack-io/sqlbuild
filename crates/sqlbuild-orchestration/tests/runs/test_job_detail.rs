//! One job's declaration reads like the job list, and a full-refresh launch needs run.rewrite_history down to group children.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::main::identities::create_principal::create_principal;
use crate::credentials::models::{BuiltinRole, DefaultRole, RoleAssignmentRequest};
use crate::credentials::tests::role_helpers::{
    SUPPORT_ROLE, assign, credentials, orders_grant, role_fixture,
};
use crate::declarations::main::read_person_job::read_person_job;
use crate::runs::_helpers::requests::job::frozen_group_children;
use crate::runs::main::request_person_run::request_person_run;
use crate::runs::models::{JobRunRequest, PersonRunRequest, RunRequest, RunWindow};
use crate::runs::tests::helpers::{json_column, rows, snapshot};
use crate::runs::tests::test_types::JobDetailTestCase;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{Parameter, TransactionMode};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_published_jobs_when_people_read_and_launch_them_then_listing_view_and_rewrite_history_gate_them()
 {
    let test_cases = [
        JobDetailTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_order: ["daily_orders", "customers"],
        },
        JobDetailTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_order: ["daily_orders", "customers"],
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let roles = role_fixture(store).await;
            let mut input = snapshot(roles.location_id);
            input.generation = 2;
            input.documents.insert(
                "roles".into(),
                json!({SUPPORT_ROLE: {"description": "Answer order questions",
                    "grants": [orders_grant(json!(["view"]), &["orders"])]}}),
            );
            input.documents.insert("jobs".into(), json!({
                "daily_orders": {"description": "Build orders", "selection": ["orders"], "settings": {
                    "vars": "{\"api_key\": \"orders-key-123\"}",
                    "defer_to": "postgres://orders:orders-db-pass@warehouse.example.test/orders",
                    "concurrency": 4}},
                "customers": {"description": "Build customers", "selection": ["customers"]},
            }));
            input.documents.insert(
                "groups".into(),
                json!({"orders_group": {
                "description": "Orders, then customers", "selection": ["customers", "orders"],
                "jobs": {"customers": ["daily_orders"], "daily_orders": []}}}),
            );
            input
                .nodes
                .get_mut("orders")
                .expect("orders node")
                .definition = json!({"materialization": "table", "config": {"critical": true}});
            publish_snapshot(store, &input).await.expect("publish jobs");
            let administrator = (&roles.context, credentials(&roles.administrator));
            let location = roles.location_id;
            let read = |who, job: &'static str| {
                read_person_job(
                    store,
                    who,
                    (location, "development", job),
                    DefaultRole::None,
                )
            };

            let job = read(administrator, "daily_orders")
                .await
                .expect("administrator")
                .expect("published job");
            assert_eq!(
                (
                    json!(job.settings),
                    json!(job.critical_nodes),
                    json!(job.member_of),
                    job.members.len()
                ),
                (
                    json!({"concurrency": 4, "vars": "[REDACTED]",
                        "defer_to": "postgres://orders:[REDACTED]@warehouse.example.test/orders"}),
                    json!(["orders"]),
                    json!(["orders_group"]),
                    0
                ),
                "{}: free-form vars and URI passwords are withheld",
                test_case.description
            );
            let group = read(administrator, "orders_group")
                .await
                .expect("administrator")
                .expect("published group");
            assert_eq!(
                group
                    .members
                    .iter()
                    .map(|member| (member.job_key.as_str(), json!(member.waits_for)))
                    .collect::<Vec<_>>(),
                [
                    (test_case.expected_order[0], json!([])),
                    (test_case.expected_order[1], json!(["daily_orders"]))
                ],
                "{}: members in run order, not name order, with their waits",
                test_case.description
            );
            assert!(
                read(administrator, "nightly")
                    .await
                    .expect("administrator")
                    .is_none(),
                "{}: an unknown job is not found",
                test_case.description
            );
            let member = (&roles.context, roles.member_id);
            for job in ["daily_orders", "nightly"] {
                assert!(
                    matches!(read(member, job).await, Err(ControlError::Denied { .. })),
                    "{}: view on one node does not list the target, and refusal hides whether {job} exists",
                    test_case.description
                );
            }

            let operator = create_principal(store, &roles.context, "orders-operator")
                .await
                .expect("operator");
            assign(
                store,
                (&roles.context, &roles.administrator),
                RoleAssignmentRequest {
                    principal_id: operator,
                    role: BuiltinRole::Operator.into(),
                    location_id: None,
                    target: None,
                    revoke: false,
                },
            )
            .await;
            let launch = |job_key: &str| JobRunRequest {
                run_id: Uuid::now_v7(),
                location_id: location,
                target: "development".into(),
                job_key: job_key.into(),
                reason: "Rebuild orders from scratch".into(),
                priority: 0,
                window: None,
                full_refresh: true,
            };
            let refused = launch("daily_orders");
            assert!(
                matches!(
                    request_person_run(
                        store,
                        (&roles.context, operator),
                        PersonRunRequest::Job(&refused)
                    )
                    .await,
                    Err(ControlError::Denied { .. })
                ),
                "{}: an operator may run the job but not rewrite its history",
                test_case.description
            );
            let both = JobRunRequest {
                window: Some(RunWindow::Integer { start: 1, end: 2 }),
                ..launch("daily_orders")
            };
            assert!(
                matches!(
                    request_person_run(store, administrator, PersonRunRequest::Job(&both)).await,
                    Err(ControlError::Invalid(_))
                ),
                "{}: a window and a full refresh are refused together",
                test_case.description
            );
            let group_launch = launch("orders_group");
            for request in [launch("daily_orders"), group_launch.clone()] {
                assert!(
                    request_person_run(store, administrator, PersonRunRequest::Job(&request))
                        .await
                        .expect("administrator launch"),
                    "{}: an administrator may rebuild from scratch",
                    test_case.description
                );
                let stored = rows(
                    store,
                    "SELECT request_json FROM sqb_runs WHERE run_id=$1",
                    vec![Parameter::Id(request.run_id)],
                )
                .await;
                assert_eq!(
                    json_column(&stored[0]["request_json"])["settings"]["full_refresh"],
                    json!(true),
                    "{}: the frozen request carries the full refresh",
                    test_case.description
                );
            }
            let stored = rows(
                store,
                "SELECT request_json FROM sqb_runs WHERE run_id=$1",
                vec![Parameter::Id(group_launch.run_id)],
            )
            .await;
            let parent: RunRequest =
                serde_json::from_value(json_column(&stored[0]["request_json"])).expect("request");
            let transaction = begin(store, TransactionMode::Read).await.expect("read");
            let (transaction, children) = frozen_group_children(transaction, &parent)
                .await
                .expect("children");
            commit(transaction).await.expect("commit");
            assert!(
                children.len() == 2
                    && children
                        .iter()
                        .all(|child| child.settings.get("full_refresh") == Some(&json!(true))),
                "{}: every group child rebuilds from scratch",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
