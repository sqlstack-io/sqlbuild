//! A publication is indexed once per snapshot, shared by concurrent first requests and replaced by newer publications.

use std::cell::Cell;
use std::sync::{Arc, Mutex};

use serde_json::json;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::runs::_helpers::lifecycle::publications::cached;
use crate::runs::main::requests::check_adhoc_run::check_adhoc_run;
use crate::runs::models::{AdhocCheckRequest, SnapshotSlots};
use crate::runs::tests::helpers::{
    adhoc_fixture, cache_id, cache_lookup, check_projection, snapshot,
};
use crate::runs::tests::test_types::{
    CacheLookupTestCase, ConcurrentLookupTestCase, ExactKeyPlanner, FailedBuildTestCase,
    NewerPublicationTestCase,
};
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::snapshots::models::SnapshotNode;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_snapshot_lookups_when_cached_then_each_is_built_once_until_two_newer_snapshots_of_its_target_replace_it()
 {
    let test_cases = [
        CacheLookupTestCase {
            description: "a repeated lookup reuses the value its first lookup built",
            lookups: &[(1, 1), (1, 1)],
            expected_built: &[true, false],
        },
        CacheLookupTestCase {
            description: "the current snapshot and the one before it stay cached together",
            lookups: &[(1, 1), (1, 2), (1, 1), (1, 2)],
            expected_built: &[true, true, false, false],
        },
        CacheLookupTestCase {
            description: "a third snapshot of one target evicts its least recently used",
            lookups: &[(1, 1), (1, 2), (1, 3), (1, 2), (1, 1)],
            expected_built: &[true, true, true, false, true],
        },
        CacheLookupTestCase {
            description: "another target's publications never evict this target's",
            lookups: &[(1, 1), (2, 2), (2, 3), (2, 4), (1, 1)],
            expected_built: &[true, true, true, true, false],
        },
    ];
    for test_case in test_cases {
        run(async {
            let slots = Mutex::new(SnapshotSlots::new());
            let mut values: Vec<u8> = Vec::new();
            let mut built: Vec<bool> = Vec::new();
            for lookup in test_case.lookups {
                let (value, was_built) = cache_lookup(&slots, *lookup).await;
                values.push(value);
                built.push(was_built);
            }
            let snapshots: Vec<u8> = test_case
                .lookups
                .iter()
                .map(|(_, snapshot)| *snapshot)
                .collect();
            assert_eq!(
                (values, built.as_slice()),
                (snapshots, test_case.expected_built),
                "{}: each lookup gets its snapshot's value",
                test_case.description
            );
        });
    }
}

#[test]
fn given_concurrent_first_lookups_when_one_is_building_then_the_others_wait_for_its_value() {
    let test_cases = [ConcurrentLookupTestCase {
        description: "the second lookup waits for the first build and shares its value",
        first_value: 7,
        second_value: 8,
        expected_outcome: (7, 7, 1, true),
    }];
    for test_case in test_cases {
        run(async {
            let slots: Mutex<SnapshotSlots<u8>> = Mutex::new(SnapshotSlots::new());
            let builds = Cell::new(0);
            let (release, released) = oneshot::channel::<()>();
            let key = || ((cache_id(1), "development".to_owned()), cache_id(1));
            let first = cached(&slots, key(), (), |()| async {
                builds.set(builds.get() + 1);
                released.await.expect("release the build");
                Ok(((), test_case.first_value))
            });
            let second = cached(&slots, key(), (), |()| async {
                builds.set(builds.get() + 1);
                Ok(((), test_case.second_value))
            });
            let releasing = async {
                tokio::task::yield_now().await;
                release.send(()).expect("build still waiting");
            };
            let (first, second, ()) = tokio::join!(first, second, releasing);
            let ((), first) = first.expect("first lookup");
            let ((), second) = second.expect("second lookup");
            assert_eq!(
                (*first, *second, builds.get(), Arc::ptr_eq(&first, &second)),
                test_case.expected_outcome,
                "{}",
                test_case.description
            );
        });
    }
}

#[test]
fn given_a_failed_build_when_looked_up_again_then_nothing_was_cached() {
    let test_cases = [FailedBuildTestCase {
        description: "a failed build leaves the next lookup to build again",
        failure: "store unavailable",
        retried_value: 3,
        expected_outcome: (
            Some("invalid control-plane data: store unavailable"),
            Some(3),
        ),
    }];
    for test_case in test_cases {
        run(async {
            let slots: Mutex<SnapshotSlots<u8>> = Mutex::new(SnapshotSlots::new());
            let key = || ((cache_id(1), "development".to_owned()), cache_id(1));
            let failed = cached(&slots, key(), (), |()| async {
                Err::<((), u8), _>(ControlError::Invalid(test_case.failure.into()))
            })
            .await;
            let retried = cached(&slots, key(), (), |()| async {
                Ok(((), test_case.retried_value))
            })
            .await;
            assert_eq!(
                (
                    failed.err().map(|error| error.to_string()),
                    retried.ok().map(|((), value)| *value)
                ),
                (
                    test_case.expected_outcome.0.map(str::to_owned),
                    test_case.expected_outcome.1
                ),
                "{}",
                test_case.description
            );
        });
    }
}

#[test]
fn given_a_newer_publication_when_a_command_is_checked_again_then_it_resolves_over_the_new_snapshot()
 {
    let test_cases = [NewerPublicationTestCase {
        description: "a node the newer publication adds is selectable once it is current",
        command: &["build", "model:invoices"],
        added_node: "model:invoices",
        expected_before_error: "model:invoices is not available for ad-hoc runs",
        expected_after_selection: &["model:invoices"],
    }];
    for test_case in test_cases {
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
                let (location_id, _, people) = adhoc_fixture(store, &context).await;
                let request = AdhocCheckRequest {
                    location_id,
                    target: "development".into(),
                    command: test_case
                        .command
                        .iter()
                        .map(|token| (*token).to_owned())
                        .collect(),
                    priority: 0,
                };
                let check = || async {
                    check_adhoc_run(store, (&context, people.admin), &ExactKeyPlanner, &request)
                        .await
                        .expect("dry run")
                };
                let before = check().await;
                let mut input = snapshot(location_id);
                input.generation = 2;
                input.nodes.insert(
                    test_case.added_node.into(),
                    SnapshotNode {
                        kind: "model".into(),
                        definition: json!({"materialization": "table"}),
                        raw_sql: None,
                        compiled_sql: Some("select 1 as id".into()),
                        python_source: None,
                    },
                );
                let newer = publish_snapshot(store, &input)
                    .await
                    .expect("publish")
                    .snapshot_id;
                let after = check().await;
                assert_eq!(
                    (
                        check_projection(&before)["error"].clone(),
                        after.snapshot_id,
                        check_projection(&after)["selection"].clone(),
                    ),
                    (
                        json!(test_case.expected_before_error),
                        Some(newer),
                        json!(test_case.expected_after_selection),
                    ),
                    "{backend:?}: {}",
                    test_case.description
                );
            });
        }
    }
}
