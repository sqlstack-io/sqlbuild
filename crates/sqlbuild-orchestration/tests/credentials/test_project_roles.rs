//! Project role grants take effect on publish and follow selector coverage per snapshot.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;

use crate::authorization::main::compile_roles::compile_roles;
use crate::authorization::main::list_role_definitions::list_role_definitions;
use crate::credentials::models::{Action, RoleAssignmentRequest, RoleKey};
use crate::credentials::tests::role_helpers::{
    SUPPORT_ROLE, assign, authorize_member, developer_grants, member_listings,
    member_trigger_change, orders_grant, publish_customers_trigger, publish_support_role,
    role_changes, role_fixture,
};
use crate::credentials::tests::test_types::{ProjectRoleTestCase, RolesFileTestCase};
use crate::declarations::models::TriggerChange;
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

const ADHOC_DENIED: &str = "missing run.adhoc permission for this target";

#[test]
fn given_published_selector_when_it_matches_more_nodes_then_coverage_grows_on_publish() {
    let test_cases = [
        ProjectRoleTestCase {
            description: "SQLite widens coverage of an unchanged selector on publish",
            backend: Backend::Sqlite,
            expected_before: Err(ADHOC_DENIED),
            expected_after: Ok(SUPPORT_ROLE),
        },
        ProjectRoleTestCase {
            description: "PostgreSQL widens coverage of an unchanged selector on publish",
            backend: Backend::Postgres,
            expected_before: Err(ADHOC_DENIED),
            expected_after: Ok(SUPPORT_ROLE),
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
            let grants = |nodes: &[&str]| developer_grants("tag:orders", nodes);
            publish_support_role(store, roles.location_id, (2, grants(&["orders"]))).await;
            let adhoc = (Action::RunAdhoc, ["customers", "orders"].as_slice());
            assert_eq!(
                authorize_member(store, &roles, adhoc)
                    .await
                    .as_deref()
                    .map_err(String::as_str),
                test_case.expected_before,
                "{}: an ad-hoc run needs every written node",
                test_case.description
            );
            publish_support_role(
                store,
                roles.location_id,
                (3, grants(&["customers", "orders"])),
            )
            .await;
            assert_eq!(
                authorize_member(store, &roles, adhoc)
                    .await
                    .as_deref()
                    .map_err(String::as_str),
                test_case.expected_after,
                "{}",
                test_case.description
            );
            assert_eq!(
                authorize_member(store, &roles, (Action::ViewSql, &["customers"]))
                    .await
                    .as_deref()
                    .map_err(String::as_str),
                test_case.expected_after,
                "{}: view.sql follows the same coverage",
                test_case.description
            );
            assert_eq!(
                role_changes(store).await,
                [
                    (1, "added".to_owned()),
                    (2, "changed".to_owned()),
                    (3, "changed".to_owned())
                ],
                "{}: the new role, the dropped view grant and the grown coverage enter the change feed",
                test_case.description
            );
            let definitions = list_role_definitions(
                store,
                (&roles.context, roles.administrator.identity.principal_id),
                (roles.location_id, "development"),
            )
            .await
            .expect("role definitions");
            assert_eq!(
                definitions
                    .iter()
                    .map(|role| (role.key.as_str(), role.builtin, role.holders))
                    .collect::<Vec<_>>(),
                [
                    ("viewer", true, 0),
                    ("operator", true, 0),
                    ("developer", true, 0),
                    ("admin", true, 1),
                    (SUPPORT_ROLE, false, 1)
                ],
                "{}: built-in roles, then the published role, each with its active holders here",
                test_case.description
            );
            assert_eq!(
                definitions[0].grants[0].permissions,
                [Action::View],
                "a viewer views every node"
            );
            let support = &definitions[4];
            assert_eq!(
                support.description.as_deref(),
                Some("Answer order questions")
            );
            assert_eq!(
                support.grants[0].matched,
                BTreeMap::from([(
                    "tag:orders".to_owned(),
                    vec!["customers".to_owned(), "orders".to_owned()]
                )]),
                "each selector shows the nodes it matched at the latest publish"
            );
            assert!(
                matches!(
                    list_role_definitions(
                        store,
                        (&roles.context, roles.member_id),
                        (roles.location_id, "development"),
                    )
                    .await,
                    Err(ControlError::Denied { .. })
                ),
                "people without role.assign everywhere cannot read role definitions"
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_selector_resolving_to_no_nodes_when_published_then_it_grants_nothing() {
    let test_cases = [
        ProjectRoleTestCase {
            description: "SQLite grants nothing for an empty resolution",
            backend: Backend::Sqlite,
            expected_before: Ok(SUPPORT_ROLE),
            expected_after: Err("missing view permission for this target"),
        },
        ProjectRoleTestCase {
            description: "PostgreSQL grants nothing for an empty resolution",
            backend: Backend::Postgres,
            expected_before: Ok(SUPPORT_ROLE),
            expected_after: Err("missing view permission for this target"),
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
            assert_eq!(
                authorize_member(store, &roles, (Action::View, &[]))
                    .await
                    .as_deref()
                    .map_err(String::as_str),
                test_case.expected_before,
                "{}: the resolved grant allows view",
                test_case.description
            );
            publish_support_role(
                store,
                roles.location_id,
                (2, json!([orders_grant(json!(["view", "run.adhoc"]), &[])])),
            )
            .await;
            assert_eq!(
                authorize_member(store, &roles, (Action::View, &[]))
                    .await
                    .as_deref()
                    .map_err(String::as_str),
                test_case.expected_after,
                "{}: a location-scoped action needs a grant that resolved to a node",
                test_case.description
            );
            assert_eq!(
                authorize_member(store, &roles, (Action::RunAdhoc, &["orders"]))
                    .await
                    .as_deref()
                    .map_err(String::as_str),
                Err("missing run.adhoc permission for this target"),
                "{}: the denial names no node",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_grant_outside_a_triggers_job_when_the_member_overrides_it_then_both_trigger_actions_are_denied()
 {
    let test_cases = [
        ProjectRoleTestCase {
            description: "SQLite authorizes trigger overrides on the job's write set",
            backend: Backend::Sqlite,
            expected_before: Err("missing trigger.toggle permission for this target"),
            expected_after: Err("missing sensor.reset_cursor permission for this target"),
        },
        ProjectRoleTestCase {
            description: "PostgreSQL authorizes trigger overrides on the job's write set",
            backend: Backend::Postgres,
            expected_before: Err("missing trigger.toggle permission for this target"),
            expected_after: Err("missing sensor.reset_cursor permission for this target"),
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
            let grants = |nodes: &[&str]| {
                json!([orders_grant(
                    json!(["view", "trigger.toggle", "sensor.reset_cursor"]),
                    nodes
                )])
            };
            publish_customers_trigger(store, roles.location_id, (2, grants(&["orders"]))).await;
            assert_eq!(
                member_trigger_change(store, &roles, TriggerChange::Pause)
                    .await
                    .map(|_| SUPPORT_ROLE)
                    .as_ref()
                    .map(|role| *role)
                    .map_err(String::as_str),
                test_case.expected_before,
                "{}: an orders-only grant cannot pause a customers-only trigger",
                test_case.description
            );
            assert_eq!(
                member_trigger_change(store, &roles, TriggerChange::ResetCursor)
                    .await
                    .map(|_| SUPPORT_ROLE)
                    .as_ref()
                    .map(|role| *role)
                    .map_err(String::as_str),
                test_case.expected_after,
                "{}: nor reset its cursor",
                test_case.description
            );
            publish_customers_trigger(
                store,
                roles.location_id,
                (3, grants(&["customers", "orders"])),
            )
            .await;
            assert_eq!(
                member_trigger_change(store, &roles, TriggerChange::Pause).await,
                Ok(true),
                "{}: coverage of the job's write set allows the pause",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_view_grant_on_part_of_the_location_when_listing_then_each_listing_is_denied_until_full_coverage()
 {
    let test_cases = [
        ProjectRoleTestCase {
            description: "SQLite location listings need view on every node",
            backend: Backend::Sqlite,
            expected_before: Err("missing view permission for this target"),
            expected_after: Ok(SUPPORT_ROLE),
        },
        ProjectRoleTestCase {
            description: "PostgreSQL location listings need view on every node",
            backend: Backend::Postgres,
            expected_before: Err("missing view permission for this target"),
            expected_after: Ok(SUPPORT_ROLE),
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
            assert_eq!(
                member_listings(store, &roles).await,
                vec![test_case.expected_before.map(|_| ()).map_err(str::to_owned); 3],
                "{}: catalog, statistics and runs are denied, not filtered",
                test_case.description
            );
            publish_support_role(
                store,
                roles.location_id,
                (
                    2,
                    json!([orders_grant(
                        json!(["view"]),
                        &["customers", "inventory", "orders", "products"]
                    )]),
                ),
            )
            .await;
            assert_eq!(
                member_listings(store, &roles).await,
                vec![test_case.expected_after.map(|_| ()).map_err(str::to_owned); 3],
                "{}: full coverage allows every listing",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_role_removed_from_code_when_its_assignment_remains_then_it_grants_nothing() {
    let test_cases = [
        ProjectRoleTestCase {
            description: "SQLite stale assignment grants nothing",
            backend: Backend::Sqlite,
            expected_before: Ok(SUPPORT_ROLE),
            expected_after: Err("missing run.adhoc permission for this target"),
        },
        ProjectRoleTestCase {
            description: "PostgreSQL stale assignment grants nothing",
            backend: Backend::Postgres,
            expected_before: Ok(SUPPORT_ROLE),
            expected_after: Err("missing run.adhoc permission for this target"),
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
            publish_support_role(
                store,
                roles.location_id,
                (2, developer_grants("tag:orders", &["orders"])),
            )
            .await;
            let adhoc = (Action::RunAdhoc, ["orders"].as_slice());
            assert_eq!(
                authorize_member(store, &roles, adhoc)
                    .await
                    .as_deref()
                    .map_err(String::as_str),
                test_case.expected_before,
                "{}: the published role grants ad-hoc runs",
                test_case.description
            );
            let mut input = snapshot(roles.location_id);
            input.generation = 3;
            input.documents.insert("roles".into(), json!({}));
            publish_snapshot(store, &input)
                .await
                .expect("publish without roles");
            assert_eq!(
                authorize_member(store, &roles, adhoc)
                    .await
                    .as_deref()
                    .map_err(String::as_str),
                test_case.expected_after,
                "{}: the removed role's assignment grants nothing on nodes",
                test_case.description
            );
            assert_eq!(
                authorize_member(store, &roles, (Action::View, &[]))
                    .await
                    .as_deref()
                    .map_err(String::as_str),
                Err("missing view permission for this target"),
                "{}: nor location-wide",
                test_case.description
            );
            assert_eq!(
                role_changes(store).await.last(),
                Some(&(3, "removed".to_owned())),
                "{}: the removal enters the change feed",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_two_custom_roles_when_each_covers_part_of_a_run_then_their_union_authorizes_it() {
    let test_cases = [
        ProjectRoleTestCase {
            description: "SQLite authorizes the union of two custom roles",
            backend: Backend::Sqlite,
            expected_before: Err(ADHOC_DENIED),
            expected_after: Ok("orders_returns+orders_support"),
        },
        ProjectRoleTestCase {
            description: "PostgreSQL authorizes the union of two custom roles",
            backend: Backend::Postgres,
            expected_before: Err(ADHOC_DENIED),
            expected_after: Ok("orders_returns+orders_support"),
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
                json!({
                    SUPPORT_ROLE: {"description": "Answer order questions", "grants": developer_grants("tag:orders", &["orders"])},
                    "orders_returns": {"description": "Handle returns", "grants": developer_grants("tag:returns", &["customers"])},
                }),
            );
            publish_snapshot(store, &input)
                .await
                .expect("publish roles");
            let adhoc = (Action::RunAdhoc, ["customers", "orders"].as_slice());
            assert_eq!(
                authorize_member(store, &roles, adhoc)
                    .await
                    .as_deref()
                    .map_err(String::as_str),
                test_case.expected_before,
                "{}: one role alone does not cover the run",
                test_case.description
            );
            assign(
                store,
                (&roles.context, &roles.administrator),
                RoleAssignmentRequest {
                    principal_id: roles.member_id,
                    role: RoleKey::Custom("orders_returns".into()),
                    location_id: Some(roles.location_id),
                    target: None,
                    revoke: false,
                },
            )
            .await;
            assert_eq!(
                authorize_member(store, &roles, adhoc)
                    .await
                    .as_deref()
                    .map_err(String::as_str),
                test_case.expected_after,
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_roles_file_when_compiled_then_selectors_resolve_and_empty_grants_warn() {
    let roles = r#"
[roles.orders_support]
description = "Answer order questions"
[[roles.orders_support.grants]]
nodes = ["tag:orders"]
permissions = ["view.sql", "run.adhoc"]
[[roles.orders_support.grants]]
nodes = ["tag:orders,tag:refunds"]
permissions = ["view.evidence"]
"#;
    let test_cases = [
        RolesFileTestCase {
            description: "resolved grants and a warning for the empty intersection",
            text: roles,
            expected_compiled: Ok((
                json!({"orders_support": {"description": "Answer order questions", "grants": [
                    {"nodes": ["tag:orders"], "permissions": ["view.sql", "run.adhoc"],
                        "resolved_nodes": {"tag:orders": ["model:orders", "model:returns"]}},
                    {"nodes": ["tag:orders,tag:refunds"], "permissions": ["view.evidence"],
                        "resolved_nodes": {"tag:orders,tag:refunds": []}},
                ]}}),
                vec![
                    "role orders_support grant 2 selects no nodes (tag:orders,tag:refunds); it grants nothing in this snapshot",
                ],
            )),
        },
        RolesFileTestCase {
            description: "an administrative permission is never granted from code",
            text: "[roles.orders_support]\ndescription = \"Answer order questions\"\n[[roles.orders_support.grants]]\nnodes = [\"tag:orders\"]\npermissions = [\"role.assign\"]\n",
            expected_compiled: Err(
                "roles.toml: invalid control-plane data: role orders_support needs a description, a key that is not a built-in role, and grants of node selectors with only non-administrative permissions",
            ),
        },
        RolesFileTestCase {
            description: "a selector the project rejects fails publication",
            text: "[roles.orders_support]\ndescription = \"Answer order questions\"\n[[roles.orders_support.grants]]\nnodes = [\"tag:unknown\"]\npermissions = [\"view.sql\"]\n",
            expected_compiled: Err(
                "roles.toml: role orders_support selector 'tag:unknown': no models found with tag 'unknown'",
            ),
        },
    ];
    let catalog: BTreeMap<&str, Result<Vec<&str>, &str>> = BTreeMap::from([
        ("tag:orders", Ok(vec!["model:orders", "model:returns"])),
        ("tag:orders,tag:refunds", Ok(vec![])),
        ("tag:unknown", Err("no models found with tag 'unknown'")),
    ]);
    let resolve = |selector: &str| {
        catalog[selector]
            .clone()
            .map(|keys| keys.into_iter().map(str::to_owned).collect::<BTreeSet<_>>())
            .map_err(str::to_owned)
    };
    for test_case in test_cases {
        let compiled = compile_roles(test_case.text, resolve);
        assert_eq!(
            compiled
                .as_ref()
                .map(|(document, warnings)| (
                    document.clone(),
                    warnings.iter().map(String::as_str).collect::<Vec<_>>()
                ))
                .map_err(String::as_str),
            test_case.expected_compiled,
            "{}",
            test_case.description
        );
    }
}
