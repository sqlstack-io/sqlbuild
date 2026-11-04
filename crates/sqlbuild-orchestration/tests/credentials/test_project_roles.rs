//! Project role grants follow selector coverage per snapshot; a rewritten selector waits for approval.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;

use crate::authorization::_helpers::custom_roles::set_effective_role;
use crate::authorization::main::compile_roles::compile_roles;
use crate::credentials::models::{Action, RoleAssignmentRequest, RoleGrant, RoleKey};
use crate::credentials::tests::role_helpers::{
    SUPPORT_ROLE, activate, assign, authorize_member, developer_grants, member_listings,
    member_trigger_change, orders_grant, publish_customers_trigger, publish_support_role,
    role_changes, role_fixture, try_request,
};
use crate::credentials::tests::test_types::{ProjectRoleTestCase, RolesFileTestCase};
use crate::declarations::models::TriggerChange;
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, migrate_store::migrate_store};
use crate::store::models::TransactionMode;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

const ADHOC_DENIED: &str = "missing run.adhoc on nodes customers; all requested nodes denied";

#[test]
fn given_approved_selector_when_it_matches_more_nodes_then_coverage_grows_without_approval() {
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
            let roles = role_fixture(store, 2).await;
            activate(store, &roles).await;
            let grants = |nodes: &[&str]| developer_grants("tag:orders", nodes);
            publish_support_role(store, roles.location_id, (2, grants(&["orders"]))).await;
            activate(store, &roles).await;
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
            assert!(
                matches!(
                    try_request(store, &roles, 0).await,
                    Err(ControlError::Invalid(_))
                ),
                "{}: coverage growth is not a widening to approve",
                test_case.description
            );
            assert_eq!(
                role_changes(store).await,
                [(2, "changed".to_owned()), (3, "changed".to_owned())],
                "{}: the dropped view grant and the grown coverage enter the change feed",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_rewritten_selector_when_published_then_its_coverage_waits_for_approval() {
    let test_cases = [
        ProjectRoleTestCase {
            description: "SQLite holds a rewritten selector for approval",
            backend: Backend::Sqlite,
            expected_before: Err(
                "missing run.adhoc on nodes customers, orders; all requested nodes denied",
            ),
            expected_after: Ok(SUPPORT_ROLE),
        },
        ProjectRoleTestCase {
            description: "PostgreSQL holds a rewritten selector for approval",
            backend: Backend::Postgres,
            expected_before: Err(
                "missing run.adhoc on nodes customers, orders; all requested nodes denied",
            ),
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
            let roles = role_fixture(store, 2).await;
            activate(store, &roles).await;
            publish_support_role(
                store,
                roles.location_id,
                (2, developer_grants("tag:orders", &["orders"])),
            )
            .await;
            activate(store, &roles).await;
            publish_support_role(
                store,
                roles.location_id,
                (3, developer_grants("tag:support", &["customers", "orders"])),
            )
            .await;
            let adhoc = (Action::RunAdhoc, ["customers", "orders"].as_slice());
            assert_eq!(
                authorize_member(store, &roles, adhoc)
                    .await
                    .as_deref()
                    .map_err(String::as_str),
                test_case.expected_before,
                "{}: the rewritten selector grants nothing before approval",
                test_case.description
            );
            assert_eq!(
                authorize_member(store, &roles, (Action::RunAdhoc, &["orders"]))
                    .await
                    .as_deref()
                    .map_err(String::as_str),
                Err("missing run.adhoc on nodes orders; all requested nodes denied"),
                "{}: the replaced selector stops applying at once",
                test_case.description
            );
            activate(store, &roles).await;
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
fn given_selector_resolving_to_no_nodes_when_approved_then_it_grants_nothing() {
    let test_cases = [
        ProjectRoleTestCase {
            description: "SQLite grants nothing for an empty resolution",
            backend: Backend::Sqlite,
            expected_before: Ok(SUPPORT_ROLE),
            expected_after: Err("missing permission in this scope; all requested nodes denied"),
        },
        ProjectRoleTestCase {
            description: "PostgreSQL grants nothing for an empty resolution",
            backend: Backend::Postgres,
            expected_before: Ok(SUPPORT_ROLE),
            expected_after: Err("missing permission in this scope; all requested nodes denied"),
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
            activate(store, &roles).await;
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
            activate(store, &roles).await;
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
                Err("missing run.adhoc on nodes orders; all requested nodes denied"),
                "{}: the denial names the uncovered node",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_removal_and_rewrite_in_one_publish_when_widening_waits_then_the_removal_applies_at_once() {
    let test_cases = [
        ProjectRoleTestCase {
            description: "SQLite removes a permission while a new selector waits",
            backend: Backend::Sqlite,
            expected_before: Err("missing run.adhoc on nodes orders; all requested nodes denied"),
            expected_after: Err("missing view.sql on nodes customers; all requested nodes denied"),
        },
        ProjectRoleTestCase {
            description: "PostgreSQL removes a permission while a new selector waits",
            backend: Backend::Postgres,
            expected_before: Err("missing run.adhoc on nodes orders; all requested nodes denied"),
            expected_after: Err("missing view.sql on nodes customers; all requested nodes denied"),
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
            activate(store, &roles).await;
            publish_support_role(
                store,
                roles.location_id,
                (2, developer_grants("tag:orders", &["orders"])),
            )
            .await;
            activate(store, &roles).await;
            publish_support_role(
                store,
                roles.location_id,
                (
                    3,
                    json!([
                        orders_grant(json!(["view.sql"]), &["orders"]),
                        {"nodes": ["tag:support"], "permissions": ["view.sql"],
                            "resolved_nodes": {"tag:support": ["customers"]}},
                    ]),
                ),
            )
            .await;
            assert_eq!(
                authorize_member(store, &roles, (Action::RunAdhoc, &["orders"]))
                    .await
                    .as_deref()
                    .map_err(String::as_str),
                test_case.expected_before,
                "{}: the removed permission is denied at once",
                test_case.description
            );
            assert_eq!(
                authorize_member(store, &roles, (Action::ViewSql, &["customers"]))
                    .await
                    .as_deref()
                    .map_err(String::as_str),
                test_case.expected_after,
                "{}: the rewritten selector's coverage waits for approval",
                test_case.description
            );
            assert_eq!(
                authorize_member(store, &roles, (Action::ViewSql, &["orders"]))
                    .await
                    .as_deref()
                    .map_err(String::as_str),
                Ok(SUPPORT_ROLE),
                "{}: the kept pair stays effective",
                test_case.description
            );
            activate(store, &roles).await;
            assert_eq!(
                authorize_member(store, &roles, (Action::ViewSql, &["customers", "orders"]))
                    .await
                    .as_deref()
                    .map_err(String::as_str),
                Ok(SUPPORT_ROLE),
                "{}: approval adds the new coverage",
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
            expected_before: Err(
                "missing trigger.toggle on nodes customers; all requested nodes denied",
            ),
            expected_after: Err(
                "missing sensor.reset_cursor on nodes customers; all requested nodes denied",
            ),
        },
        ProjectRoleTestCase {
            description: "PostgreSQL authorizes trigger overrides on the job's write set",
            backend: Backend::Postgres,
            expected_before: Err(
                "missing trigger.toggle on nodes customers; all requested nodes denied",
            ),
            expected_after: Err(
                "missing sensor.reset_cursor on nodes customers; all requested nodes denied",
            ),
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
            activate(store, &roles).await;
            let grants = |nodes: &[&str]| {
                json!([orders_grant(
                    json!(["view", "trigger.toggle", "sensor.reset_cursor"]),
                    nodes
                )])
            };
            publish_customers_trigger(store, roles.location_id, (2, grants(&["orders"]))).await;
            activate(store, &roles).await;
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
            expected_before: Err(
                "missing view on nodes customers, inventory, products; all requested nodes denied",
            ),
            expected_after: Ok(SUPPORT_ROLE),
        },
        ProjectRoleTestCase {
            description: "PostgreSQL location listings need view on every node",
            backend: Backend::Postgres,
            expected_before: Err(
                "missing view on nodes customers, inventory, products; all requested nodes denied",
            ),
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
            let roles = role_fixture(store, 2).await;
            activate(store, &roles).await;
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
            let roles = role_fixture(store, 2).await;
            activate(store, &roles).await;
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
            activate(store, &roles).await;
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
            let returns = serde_json::from_value::<Vec<RoleGrant>>(developer_grants(
                "tag:returns",
                &["customers"],
            ))
            .expect("returns grants");
            let transaction = set_effective_role(
                begin(store, TransactionMode::Write).await.expect("begin"),
                (roles.location_id, "development", "orders_returns"),
                ("synthetic-returns-digest", &returns),
            )
            .await
            .expect("effective returns role");
            commit(transaction).await.expect("commit");
            assign(
                store,
                (&roles.context, &roles.administrators[0]),
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
