//! Ad-hoc commands resolve on the server, need their own permission per gated flag and dry runs record nothing.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::runs::main::check_adhoc_run::check_adhoc_run;
use crate::runs::main::request_person_run::request_person_run;
use crate::runs::models::PersonRunRequest;
use crate::runs::tests::helpers::{
    adhoc_check, adhoc_fixture, adhoc_launch, check_projection, json_column, launch_outcome, rows,
};
use crate::runs::tests::test_types::{AdhocCommandTestCase, AdhocPerson, ExactKeyPlanner};
use crate::store::main::migrate_store::migrate_store;
use crate::store::models::Parameter;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

const AUDIT: &str = "SELECT event_id FROM sqb_security_audit";

#[test]
fn given_ad_hoc_commands_when_people_check_and_launch_them_then_each_gated_flag_needs_its_own_permission()
 {
    let test_cases = [
        AdhocCommandTestCase {
            description: "run.adhoc covers the resolved selection, not only its writers",
            person: AdhocPerson::Developer,
            command: &["build", "model:orders", "source:shop.orders"],
            priority: 0,
            reason: "Refresh orders",
            expected_check: json!({"selection": ["model:orders", "source:shop.orders"], "write_set": ["model:orders"],
                "permissions": [["run.adhoc", true]], "error": null, "launchable": true}),
            expected_launch: Ok(true),
        },
        AdhocCommandTestCase {
            description: "a build claims the managed source it loads for a selected model",
            person: AdhocPerson::Developer,
            command: &["build", "model:customers"],
            priority: 0,
            reason: "Refresh customers",
            expected_check: json!({"selection": ["model:customers"], "write_set": ["model:customers", "source:shop.customers"],
                "permissions": [["run.adhoc", true]], "error": null, "launchable": true}),
            expected_launch: Ok(true),
        },
        AdhocCommandTestCase {
            description: "a build that does not load sources claims only its own writers",
            person: AdhocPerson::Developer,
            command: &["build", "model:customers", "--no-load-sources"],
            priority: 0,
            reason: "Refresh customers only",
            expected_check: json!({"selection": ["model:customers"], "write_set": ["model:customers"],
                "permissions": [["run.adhoc", true]], "error": null, "launchable": true}),
            expected_launch: Ok(true),
        },
        AdhocCommandTestCase {
            description: "every gated flag is its own permission, and Developer holds none of them",
            person: AdhocPerson::Developer,
            command: &[
                "build",
                "model:orders",
                "--full-refresh",
                "--warehouse",
                "BUILD_XS",
            ],
            priority: 1,
            reason: "Rebuild orders now",
            expected_check: json!({"selection": ["model:orders"], "write_set": ["model:orders"],
                "permissions": [["run.adhoc", true], ["run.rewrite_history", false], ["run.warehouse", false], ["run.prioritise", false]],
                "error": null, "launchable": false}),
            expected_launch: Err(403),
        },
        AdhocCommandTestCase {
            description: "Developer lacks run.rewrite_history alone",
            person: AdhocPerson::Developer,
            command: &["build", "model:orders", "--full-refresh"],
            priority: 0,
            reason: "Rebuild orders",
            expected_check: json!({"selection": ["model:orders"], "write_set": ["model:orders"],
                "permissions": [["run.adhoc", true], ["run.rewrite_history", false]], "error": null, "launchable": false}),
            expected_launch: Err(403),
        },
        AdhocCommandTestCase {
            description: "Developer lacks run.warehouse alone",
            person: AdhocPerson::Developer,
            command: &["build", "model:orders", "--warehouse", "BUILD_XS"],
            priority: 0,
            reason: "Refresh orders",
            expected_check: json!({"selection": ["model:orders"], "write_set": ["model:orders"],
                "permissions": [["run.adhoc", true], ["run.warehouse", false]], "error": null, "launchable": false}),
            expected_launch: Err(403),
        },
        AdhocCommandTestCase {
            description: "Developer lacks run.prioritise alone",
            person: AdhocPerson::Developer,
            command: &["build", "model:orders"],
            priority: 1,
            reason: "Refresh orders",
            expected_check: json!({"selection": ["model:orders"], "write_set": ["model:orders"],
                "permissions": [["run.adhoc", true], ["run.prioritise", false]], "error": null, "launchable": false}),
            expected_launch: Err(403),
        },
        AdhocCommandTestCase {
            description: "Admin holds every gated permission",
            person: AdhocPerson::Admin,
            command: &[
                "build",
                "model:orders",
                "--full-refresh",
                "--warehouse",
                "BUILD_XS",
            ],
            priority: 1,
            reason: "Rebuild orders now",
            expected_check: json!({"selection": ["model:orders"], "write_set": ["model:orders"],
                "permissions": [["run.adhoc", true], ["run.rewrite_history", true], ["run.warehouse", true], ["run.prioritise", true]],
                "error": null, "launchable": true}),
            expected_launch: Ok(true),
        },
        AdhocCommandTestCase {
            description: "Operator may run jobs but not ad-hoc commands",
            person: AdhocPerson::Operator,
            command: &["build", "model:orders"],
            priority: 0,
            reason: "Refresh orders",
            expected_check: json!({"selection": ["model:orders"], "write_set": ["model:orders"],
                "permissions": [["run.adhoc", false]], "error": null, "launchable": false}),
            expected_launch: Err(403),
        },
        AdhocCommandTestCase {
            description: "without view the dry run reveals no node, and no role is the default",
            person: AdhocPerson::Stranger,
            command: &["build", "model:orders"],
            priority: 0,
            reason: "Refresh orders",
            expected_check: json!({"selection": [], "write_set": [], "permissions": [],
                "error": "view on every node of this target is required", "launchable": false}),
            expected_launch: Err(403),
        },
        AdhocCommandTestCase {
            description: "other commands launch with their own verb and write nothing",
            person: AdhocPerson::Developer,
            command: &["test", "model:orders"],
            priority: 0,
            reason: "Check orders",
            expected_check: json!({"selection": ["model:orders"], "write_set": [], "permissions": [["run.adhoc", true]],
                "error": null, "launchable": true}),
            expected_launch: Ok(true),
        },
        AdhocCommandTestCase {
            description: "a refused flag is refused before any permission",
            person: AdhocPerson::Admin,
            command: &["build", "--vars"],
            priority: 0,
            reason: "Refresh orders",
            expected_check: json!({"selection": [], "write_set": [], "permissions": [],
                "error": "--vars is not available for ad-hoc runs", "launchable": false}),
            expected_launch: Err(400),
        },
        AdhocCommandTestCase {
            description: "a launch needs a reason, which the dry run does not",
            person: AdhocPerson::Admin,
            command: &["build", "model:orders"],
            priority: 0,
            reason: " ",
            expected_check: json!({"selection": ["model:orders"], "write_set": ["model:orders"],
                "permissions": [["run.adhoc", true]], "error": null, "launchable": true}),
            expected_launch: Err(400),
        },
    ];
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
            let (location_id, snapshot_id, people) = adhoc_fixture(store, &context).await;
            let mut run_ids: Vec<Uuid> = Vec::new();
            for test_case in &test_cases {
                let run_id = Uuid::now_v7();
                run_ids.push(run_id);
                let person = people.of(test_case.person);
                let audited = rows(store, AUDIT, Vec::new()).await.len();
                let check = check_adhoc_run(
                    store,
                    (&context, person),
                    &ExactKeyPlanner,
                    &adhoc_check(location_id, test_case),
                )
                .await
                .expect("dry run");
                assert_eq!(
                    (
                        check_projection(&check),
                        rows(store, AUDIT, Vec::new()).await.len()
                    ),
                    (test_case.expected_check.clone(), audited),
                    "{backend:?}: {}; a dry run records nothing",
                    test_case.description
                );
                let launched = request_person_run(
                    store,
                    (&context, person),
                    PersonRunRequest::Adhoc(
                        &ExactKeyPlanner,
                        &adhoc_launch(location_id, test_case, run_id),
                    ),
                )
                .await;
                assert_eq!(
                    launch_outcome(launched),
                    test_case.expected_launch,
                    "{backend:?}: {}",
                    test_case.description
                );
            }
            let retried = request_person_run(
                store,
                (&context, people.of(test_cases[0].person)),
                PersonRunRequest::Adhoc(
                    &ExactKeyPlanner,
                    &adhoc_launch(location_id, &test_cases[0], run_ids[0]),
                ),
            )
            .await;
            assert_eq!(
                launch_outcome(retried),
                Ok(false),
                "{backend:?}: a retry of the same run ID is not a second run"
            );
            let frozen = rows(
                store,
                "SELECT request_json FROM sqb_runs WHERE run_id=$1",
                vec![Parameter::Id(run_ids[0])],
            )
            .await;
            let frozen = json_column(&frozen[0]["request_json"]);
            assert_eq!(
                [
                    &frozen["kind"],
                    &frozen["selection"],
                    &frozen["write_set"],
                    &frozen["snapshot_id"]
                ],
                [
                    &json!("adhoc"),
                    &json!(["model:orders", "source:shop.orders"]),
                    &json!(["model:orders"]),
                    &json!(snapshot_id)
                ],
                "{backend:?}: the worker receives the exact resolved selection of the pinned publication"
            );
            let tested = test_cases
                .iter()
                .position(|test_case| test_case.command[0] == "test")
                .expect("a test launch");
            let commands = rows(
                store,
                "SELECT request_json FROM sqb_runs WHERE run_id=$1 OR run_id=$2 ORDER BY run_id",
                vec![Parameter::Id(run_ids[0]), Parameter::Id(run_ids[tested])],
            )
            .await
            .iter()
            .map(|row| json_column(&row["request_json"]).get("command").cloned())
            .collect::<Vec<Option<Value>>>();
            assert_eq!(
                commands,
                vec![None, Some(json!("test"))],
                "{backend:?}: the worker receives the command, and a build request is serialized as before"
            );
            let audit = rows(
                store,
                "SELECT after_json FROM sqb_security_audit WHERE action='run.adhoc_requested'",
                Vec::new(),
            )
            .await;
            assert_eq!(
                audit
                    .iter()
                    .map(|row| json_column(&row["after_json"]))
                    .map(|event| json!([
                        event["verb"],
                        event["frozen"]["settings"],
                        event["request"]["reason"]
                    ]))
                    .collect::<Vec<Value>>(),
                vec![
                    json!(["build", {}, "Refresh orders"]),
                    json!(["build", {}, "Refresh customers"]),
                    json!(["build", {"load_sources": false}, "Refresh customers only"]),
                    json!(["build", {"full_refresh": true, "warehouse": "BUILD_XS"}, "Rebuild orders now"]),
                    json!(["test", {}, "Check orders"]),
                ],
                "{backend:?}: each launch is audited once with its command, reason and warehouse"
            );
            close_test_store(fixture).await;
        });
    }
}
