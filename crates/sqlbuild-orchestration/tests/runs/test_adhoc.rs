//! Ad-hoc commands resolve on the server, need their own permission per gated flag and dry runs record nothing.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::authorization::main::assign_builtin_role::assign_builtin_role;
use crate::authorization::main::bootstrap_user::bootstrap_user;
use crate::credentials::main::identities::create_principal::create_principal;
use crate::credentials::models::{AuditContext, BuiltinRole, RoleAssignmentRequest};
use crate::credentials::tests::location_helpers::create_location;
use crate::runs::main::check_adhoc_run::check_adhoc_run;
use crate::runs::main::request_person_run::request_person_run;
use crate::runs::models::{AdhocCheckRequest, AdhocRunRequest, PersonRunRequest};
use crate::runs::tests::helpers::{ExactKeyPlanner, snapshot};
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::snapshots::models::SnapshotNode;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::main::query::query;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, Parameter, TransactionMode};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

async fn rows(store: &ControlStore, sql: &str, parameters: Vec<Parameter>) -> Vec<Value> {
    let transaction = begin(store, TransactionMode::Read).await.expect("read");
    let (transaction, rows) = query(transaction, sql, parameters).await.expect("query");
    commit(transaction).await.expect("commit");
    rows.into_iter().map(|row| json!(row)).collect()
}

fn command(tokens: &[&str]) -> Vec<String> {
    tokens.iter().map(|token| (*token).to_owned()).collect()
}

#[test]
fn given_ad_hoc_commands_when_people_check_and_launch_them_then_each_gated_flag_needs_its_own_permission()
 {
    for backend in [Backend::Sqlite, Backend::Postgres] {
        run(async {
            let Some(fixture) = open_test_store(backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let context = AuditContext {
                actor: "local-control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let location_id = create_location(store, "orders", "test-owner")
                .await
                .expect("location");
            let mut input = snapshot(location_id);
            input.nodes = [
                "model:orders",
                "model:customers",
                "seed:currencies",
                "source:shop.orders",
            ]
            .into_iter()
            .map(|key| {
                let kind = key.split_once(':').expect("kind").0;
                (
                    key.to_owned(),
                    SnapshotNode {
                        kind: kind.into(),
                        definition: json!({"materialization": "table"}),
                        raw_sql: None,
                        compiled_sql: Some("select 1 as id".into()),
                        python_source: None,
                    },
                )
            })
            .collect();
            let published = publish_snapshot(store, &input).await.expect("publish");
            let admin = bootstrap_user(store, &context, "orders-admin")
                .await
                .expect("administrator");
            let mut people = Vec::new();
            for (name, role) in [
                ("orders-developer", &[BuiltinRole::Developer][..]),
                ("orders-operator", &[BuiltinRole::Operator][..]),
                ("orders-stranger", &[][..]),
            ] {
                let person = create_principal(store, &context, name)
                    .await
                    .expect("person");
                for role in role.iter().copied() {
                    assign_builtin_role(
                        store,
                        (&context, admin),
                        &RoleAssignmentRequest {
                            principal_id: person,
                            role: role.into(),
                            location_id: None,
                            target: None,
                            revoke: false,
                        },
                    )
                    .await
                    .expect("assign role");
                }
                people.push(person);
            }
            let (developer, operator, stranger) = (people[0], people[1], people[2]);
            let audited_before = rows(store, "SELECT event_id FROM sqb_security_audit", Vec::new())
                .await
                .len();
            let context = &context;
            let check = |person: Uuid, tokens: &[&str], priority: i64| {
                let request = AdhocCheckRequest {
                    location_id,
                    target: "development".into(),
                    command: command(tokens),
                    priority,
                };
                async move {
                    let check =
                        check_adhoc_run(store, (context, person), &ExactKeyPlanner, &request)
                            .await
                            .expect("dry run");
                    json!({
                        "selection": check.selection,
                        "write_set": check.write_set,
                        "permissions": check.permissions.iter().map(|permission| json!([permission.action, permission.allowed])).collect::<Vec<Value>>(),
                        "error": check.error,
                        "launchable": check.launchable,
                    })
                }
            };
            assert_eq!(
                check(
                    developer,
                    &["build", "model:orders", "source:shop.orders"],
                    0
                )
                .await,
                json!({"selection": ["model:orders", "source:shop.orders"], "write_set": ["model:orders"],
                       "permissions": [["run.adhoc", true]], "error": null, "launchable": true}),
                "{backend:?}: run.adhoc covers the resolved selection, not only its writers"
            );
            assert_eq!(
                check(
                    developer,
                    &[
                        "build",
                        "model:orders",
                        "--full-refresh",
                        "--warehouse",
                        "BUILD_XS"
                    ],
                    1
                )
                .await["permissions"],
                json!([
                    ["run.adhoc", true],
                    ["run.rewrite_history", false],
                    ["run.warehouse", false],
                    ["run.prioritise", false]
                ]),
                "{backend:?}: every gated flag is its own permission, and Developer holds none of them"
            );
            assert_eq!(
                check(
                    admin,
                    &[
                        "build",
                        "model:orders",
                        "--full-refresh",
                        "--warehouse",
                        "BUILD_XS"
                    ],
                    1
                )
                .await["launchable"],
                json!(true),
                "{backend:?}: Admin holds every gated permission"
            );
            assert_eq!(
                check(operator, &["build", "model:orders"], 0).await["permissions"],
                json!([["run.adhoc", false]]),
                "{backend:?}: Operator may run jobs but not ad-hoc commands"
            );
            assert_eq!(
                check(stranger, &["build", "model:orders"], 0).await,
                json!({"selection": [], "write_set": [], "permissions": [],
                       "error": "view on every node of this target is required", "launchable": false}),
                "{backend:?}: without view the dry run reveals no node"
            );
            assert_eq!(
                check(developer, &["test", "model:orders"], 0).await["error"],
                json!("workers do not run sqb test yet; only build can launch"),
                "{backend:?}: verbs workers cannot run yet are refused at launch"
            );
            assert_eq!(
                rows(store, "SELECT event_id FROM sqb_security_audit", Vec::new())
                    .await
                    .len(),
                audited_before,
                "{backend:?}: dry runs record no denial and no request"
            );
            let launch =
                |person: Uuid, tokens: &[&str], (reason, priority): (&str, i64), run_id: Uuid| {
                    let request = AdhocRunRequest {
                        run_id,
                        location_id,
                        target: "development".into(),
                        command: command(tokens),
                        reason: reason.into(),
                        priority,
                    };
                    async move {
                        request_person_run(
                            store,
                            (context, person),
                            PersonRunRequest::Adhoc(&ExactKeyPlanner, &request),
                        )
                        .await
                    }
                };
            let launched = Uuid::now_v7();
            for expected in [true, false] {
                assert_eq!(
                    launch(
                        developer,
                        &["build", "model:orders"],
                        ("Refresh orders", 0),
                        launched
                    )
                    .await
                    .ok(),
                    Some(expected),
                    "{backend:?}: a retry of the same run ID is not a second run"
                );
            }
            let frozen = rows(
                store,
                "SELECT request_json FROM sqb_runs WHERE run_id=$1",
                vec![Parameter::Id(launched)],
            )
            .await;
            let frozen: Value = serde_json::from_value(frozen[0]["request_json"].clone())
                .map(|text: Value| {
                    text.as_str().map_or(text.clone(), |text| {
                        serde_json::from_str(text).expect("request JSON")
                    })
                })
                .expect("frozen request");
            assert_eq!(
                (
                    &frozen["kind"],
                    &frozen["selection"],
                    &frozen["write_set"],
                    &frozen["snapshot_id"]
                ),
                (
                    &json!("adhoc"),
                    &json!(["model:orders"]),
                    &json!(["model:orders"]),
                    &json!(published.snapshot_id)
                ),
                "{backend:?}: the worker receives the exact resolved selection of the pinned publication"
            );
            for (person, tokens, priority, description) in [
                (
                    operator,
                    &["build", "model:orders"][..],
                    0,
                    "Operator lacks run.adhoc",
                ),
                (
                    developer,
                    &["build", "model:orders", "--full-refresh"][..],
                    0,
                    "Developer lacks run.rewrite_history",
                ),
                (
                    developer,
                    &["build", "model:orders", "--warehouse", "BUILD_XS"][..],
                    0,
                    "Developer lacks run.warehouse",
                ),
                (
                    developer,
                    &["build", "model:orders"][..],
                    1,
                    "Developer lacks run.prioritise",
                ),
                (
                    stranger,
                    &["build", "model:orders"][..],
                    0,
                    "no role is the default",
                ),
            ] {
                assert!(
                    matches!(
                        launch(person, tokens, ("Refresh orders", priority), Uuid::now_v7()).await,
                        Err(ControlError::Denied { .. })
                    ),
                    "{backend:?}: {description}"
                );
            }
            for (tokens, reason, description) in [
                (&["build", "model:orders"][..], " ", "a reason is required"),
                (
                    &["test", "model:orders"][..],
                    "Check orders",
                    "workers run only build",
                ),
                (&["build", "--vars"][..], "Refresh orders", "refused flag"),
            ] {
                assert!(
                    matches!(
                        launch(admin, tokens, (reason, 0), Uuid::now_v7()).await,
                        Err(ControlError::Invalid(_))
                    ),
                    "{backend:?}: {description}"
                );
            }
            let urgent = Uuid::now_v7();
            assert_eq!(
                launch(
                    admin,
                    &[
                        "build",
                        "model:orders",
                        "--full-refresh",
                        "--warehouse",
                        "BUILD_XS"
                    ],
                    ("Rebuild orders now", 1),
                    urgent
                )
                .await
                .ok(),
                Some(true),
                "{backend:?}: Admin launches every gated flag"
            );
            let audit = rows(store, "SELECT granting_role, after_json FROM sqb_security_audit WHERE action='run.adhoc_requested'", Vec::new()).await;
            let after: Vec<Value> = audit
                .iter()
                .map(|row| {
                    let after = &row["after_json"];
                    after.as_str().map_or(after.clone(), |text| {
                        serde_json::from_str(text).expect("audit JSON")
                    })
                })
                .collect();
            assert_eq!(
                after
                    .iter()
                    .map(|event| (
                        event["verb"].clone(),
                        event["frozen"]["settings"].clone(),
                        event["request"]["reason"].clone()
                    ))
                    .collect::<Vec<_>>(),
                vec![
                    (json!("build"), json!({}), json!("Refresh orders")),
                    (
                        json!("build"),
                        json!({"full_refresh": true, "warehouse": "BUILD_XS"}),
                        json!("Rebuild orders now")
                    ),
                ],
                "{backend:?}: each launch is audited once with its command, reason and warehouse"
            );
            close_test_store(fixture).await;
        });
    }
}
