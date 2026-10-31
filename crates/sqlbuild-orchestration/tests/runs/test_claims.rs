use std::collections::BTreeMap;
use std::num::NonZeroU32;

use sqlbuild_orchestration::runs::main::claim_next_run::claim_next_run;
use sqlbuild_orchestration::runs::main::list_run_claims::list_run_claims;
use sqlbuild_orchestration::runs::main::list_runs::list_runs;
use sqlbuild_orchestration::runs::models::RunStatus;
use sqlbuild_orchestration::scheduler::main::acquire_lease::acquire_lease;
use sqlbuild_orchestration::snapshots::main::publish_snapshot::publish_snapshot;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use sqlbuild_orchestration::store::main::read_recall_events::read_recall_events;
use uuid::Uuid;

use crate::helpers::{queue, request, snapshot};
use crate::location_helpers::create_location;
use crate::replica_helpers::second_store;
use crate::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store_test_types::Backend;
use crate::terminal_helpers::finish;
use crate::test_types::{
    BacklogClaimTestCase, GroupLimitTestCase, PriorityClaimsTestCase, ScopedClaimTestCase,
};

#[test]
fn given_higher_priority_work_for_other_scopes_when_a_local_launcher_claims_then_only_its_location_and_target_are_reserved()
 {
    let test_cases = [
        ScopedClaimTestCase {
            description: "SQLite local dispatch cannot reserve another location or target",
            backend: Backend::Sqlite,
            expected_selected_run: 1,
            expected_unscoped_run: 3,
        },
        ScopedClaimTestCase {
            description: "PostgreSQL local dispatch cannot reserve another location or target",
            backend: Backend::Postgres,
            expected_selected_run: 1,
            expected_unscoped_run: 3,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let other = create_location(store, "inventory", "test-admin")
                .await
                .expect("other location");
            let published = publish_snapshot(store, &snapshot(location))
                .await
                .expect("snapshot");
            let other_published = publish_snapshot(store, &snapshot(other))
                .await
                .expect("other location snapshot");
            let mut secondary = snapshot(location);
            secondary.target = "secondary".into();
            let secondary = publish_snapshot(store, &secondary)
                .await
                .expect("other target snapshot");
            let wanted = request((location, published.snapshot_id), 1, 0, &["orders"]);
            let mut other_target = request((location, secondary.snapshot_id), 2, 10, &["orders"]);
            other_target.target = "secondary".into();
            let other_location = request((other, other_published.snapshot_id), 3, 20, &["orders"]);
            for input in [&wanted, &other_target, &other_location] {
                queue(store, input).await;
            }
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let claim = claim_next_run(
                store,
                &lease,
                &BTreeMap::new(),
                Some((location, "development")),
            )
            .await
            .expect("scoped claim")
            .expect("matching work");
            assert_eq!(
                claim.request.run_id,
                Uuid::from_u128(test_case.expected_selected_run),
                "{}",
                test_case.description
            );
            assert_eq!(
                list_run_claims(store, (location, "development"), None)
                    .await
                    .expect("recover queued claim"),
                vec![claim.clone()],
                "recovery includes a committed claim before launch"
            );
            assert!(
                list_run_claims(store, (location, "development"), Some(claim.request.run_id))
                    .await
                    .expect("page after claimed run")
                    .is_empty()
            );
            assert!(
                list_run_claims(store, (other, "development"), None)
                    .await
                    .expect("unclaimed neighboring location")
                    .is_empty()
            );
            assert!(
                list_run_claims(store, (location, "secondary"), None)
                    .await
                    .expect("unclaimed neighboring target")
                    .is_empty()
            );
            assert!(
                claim_next_run(
                    store,
                    &lease,
                    &BTreeMap::new(),
                    Some((location, "development"))
                )
                .await
                .expect("scope exhausted")
                .is_none()
            );
            finish(store, &lease, &claim).await;
            assert!(
                list_run_claims(store, (location, "development"), None)
                    .await
                    .expect("terminal claims are not recoverable")
                    .is_empty()
            );
            let next = claim_next_run(store, &lease, &BTreeMap::new(), None)
                .await
                .expect("unrestricted claim")
                .expect("other work was not reserved");
            assert_eq!(
                next.request.run_id,
                Uuid::from_u128(test_case.expected_unscoped_run),
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_overlapping_priority_queues_when_two_launchers_claim_then_conflicts_rollback_and_terminal_facts_release_claims()
 {
    let test_cases = [
        PriorityClaimsTestCase {
            description: "SQLite claims prioritize independent write sets and release atomically",
            backend: Backend::Sqlite,
            expected_initial_claims: [2, 3],
            expected_claim_events: 4,
            expected_terminal_events: 2,
        },
        PriorityClaimsTestCase {
            description: "PostgreSQL claims prioritize independent write sets and release atomically",
            backend: Backend::Postgres,
            expected_initial_claims: [2, 3],
            expected_claim_events: 4,
            expected_terminal_events: 2,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate run store");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("create location");
            let published = publish_snapshot(store, &snapshot(location))
                .await
                .expect("publish snapshot");
            let scope = (location, published.snapshot_id);
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            for input in [
                request(scope, 1, 1, &["orders"]),
                request(scope, 2, 3, &["orders"]),
                request(scope, 3, 2, &["products"]),
            ] {
                queue(store, &input).await;
            }
            let second = second_store(store).await;
            let limits = BTreeMap::new();
            let (first, other) = tokio::join!(
                claim_next_run(store, &lease, &limits, None),
                claim_next_run(&second, &lease, &limits, None)
            );
            let first = first.expect("first claimant").expect("claim available");
            let other = other
                .expect("second claimant")
                .expect("independent claim available");
            let claimed: std::collections::BTreeSet<_> =
                [first.request.run_id, other.request.run_id]
                    .into_iter()
                    .collect();
            assert_eq!(
                claimed,
                test_case
                    .expected_initial_claims
                    .into_iter()
                    .map(Uuid::from_u128)
                    .collect(),
                "{}",
                test_case.description
            );
            assert!(
                claim_next_run(store, &lease, &limits, None)
                    .await
                    .expect("blocked run stays queued")
                    .is_none()
            );
            queue(store, &request(scope, 4, 9, &["customers", "orders"])).await;
            queue(store, &request(scope, 5, 0, &["customers"])).await;
            let independent = claim_next_run(store, &lease, &limits, None)
                .await
                .expect("try partially conflicting candidate")
                .expect("independent work proceeds");
            assert_eq!(
                independent.request.run_id,
                Uuid::from_u128(5),
                "customers was not leaked by the failed customers/orders claim"
            );
            let orders = [&first, &other]
                .into_iter()
                .find(|claim| claim.request.run_id == Uuid::from_u128(2))
                .expect("orders owner");
            finish(store, &lease, orders).await;
            finish(store, &lease, &independent).await;
            let next = claim_next_run(store, &lease, &limits, None)
                .await
                .expect("terminal cleanup committed")
                .expect("next priority run claims both nodes");
            assert_eq!(next.request.run_id, Uuid::from_u128(4));
            let events = read_recall_events(store, 0, 100)
                .await
                .expect("read immutable run stream");
            assert_eq!(
                events
                    .iter()
                    .filter(|event| event["event_type"] == "run_claimed")
                    .count(),
                test_case.expected_claim_events,
                "{}",
                test_case.description
            );
            assert_eq!(
                events
                    .iter()
                    .filter(|event| event["event_type"] == "run_terminated")
                    .count(),
                test_case.expected_terminal_events,
                "{}",
                test_case.description
            );
            let runs = list_runs(store, (location, "development"), 100)
                .await
                .expect("read atomic projections");
            assert_eq!(
                runs.iter()
                    .filter(|run| run.status == RunStatus::Failed)
                    .count(),
                2
            );
            close_store(second).await;
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_named_group_limits_when_capacity_is_lowered_then_active_high_slots_still_count_and_terminal_cleanup_unblocks_work()
 {
    let test_cases = [
        GroupLimitTestCase {
            description: "SQLite occupied high slots still count after lowering a group limit",
            backend: Backend::Sqlite,
            expected_unblocked_run: 3,
        },
        GroupLimitTestCase {
            description: "PostgreSQL occupied high slots still count after lowering a group limit",
            backend: Backend::Postgres,
            expected_unblocked_run: 3,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate run store");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("create location");
            let published = publish_snapshot(store, &snapshot(location))
                .await
                .expect("publish snapshot");
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let mut limits = BTreeMap::from([(
                "warehouse".into(),
                NonZeroU32::new(2).expect("nonzero limit"),
            )]);
            for id in 1..=3 {
                let mut input = request((location, published.snapshot_id), id, 0, &["orders"]);
                input.write_set.clear();
                input.concurrency_groups.insert("warehouse".into());
                queue(store, &input).await;
            }
            assert!(
                claim_next_run(store, &lease, &BTreeMap::new(), None)
                    .await
                    .is_err(),
                "unknown groups cannot silently become unlimited"
            );
            let first = claim_next_run(store, &lease, &limits, None)
                .await
                .expect("first slot")
                .expect("available");
            let second = claim_next_run(store, &lease, &limits, None)
                .await
                .expect("second slot")
                .expect("available");
            assert_eq!(first.request.run_id, Uuid::from_u128(1));
            assert_eq!(second.request.run_id, Uuid::from_u128(2));
            assert!(
                claim_next_run(store, &lease, &limits, None)
                    .await
                    .expect("group full")
                    .is_none()
            );
            finish(store, &lease, &first).await;
            limits.insert(
                "warehouse".into(),
                NonZeroU32::new(1).expect("nonzero lowered limit"),
            );
            assert!(
                claim_next_run(store, &lease, &limits, None)
                    .await
                    .expect("high slot counts after limit reduction")
                    .is_none()
            );
            finish(store, &lease, &second).await;
            let third = claim_next_run(store, &lease, &limits, None)
                .await
                .expect("released group")
                .expect("third queued run");
            assert_eq!(
                third.request.run_id,
                Uuid::from_u128(test_case.expected_unblocked_run),
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_more_than_one_page_of_blocked_runs_when_claiming_then_lower_priority_independent_work_is_not_starved()
 {
    let test_cases = [
        BacklogClaimTestCase {
            description: "SQLite scans past more than a full page of blocked work",
            backend: Backend::Sqlite,
            expected_independent_run: 105,
        },
        BacklogClaimTestCase {
            description: "PostgreSQL scans past more than a full page of blocked work",
            backend: Backend::Postgres,
            expected_independent_run: 105,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let published = publish_snapshot(store, &snapshot(location))
                .await
                .expect("snapshot");
            let scope = (location, published.snapshot_id);
            let mut lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let limits = BTreeMap::new();
            queue(store, &request(scope, 1, 0, &["orders"])).await;
            claim_next_run(store, &lease, &limits, None)
                .await
                .expect("claim blocker")
                .expect("available");
            for batch in (2..=104).collect::<Vec<_>>().chunks(20) {
                for id in batch {
                    queue(store, &request(scope, *id, 0, &["orders"])).await;
                }
                lease = sqlbuild_orchestration::scheduler::main::renew_lease::renew_lease(
                    store, &lease,
                )
                .await
                .expect("keep leader live while preparing backlog");
            }
            queue(store, &request(scope, 105, -1, &["customers"])).await;
            let started = std::time::Instant::now();
            let independent = claim_next_run(store, &lease, &limits, None)
                .await
                .expect("scan past a full blocked page")
                .expect("independent work");
            assert_eq!(
                independent.request.run_id,
                Uuid::from_u128(test_case.expected_independent_run),
                "{}",
                test_case.description
            );
            assert!(
                started.elapsed() < std::time::Duration::from_secs(5),
                "claim scan exceeded five seconds: {}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
