use std::collections::BTreeMap;
use std::num::NonZeroU32;

use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::models::PoolSlots;
use crate::runs::tests::claim_helpers::claim_exactly;
use crate::runs::tests::claim_helpers::slot_counts;
use crate::runs::tests::helpers::pool_slots;
use crate::runs::tests::helpers::{queue, request, snapshot};
use crate::runs::tests::test_types::{PoolAmountTestCase, ProjectLimitTestCase};
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_project_limit_and_a_server_cap_when_claiming_then_each_location_runs_up_to_the_lower_of_them()
 {
    let test_cases = [
        ProjectLimitTestCase {
            description: "the project's limit holds without a server cap",
            backend: Backend::Sqlite,
            project_limit: 2,
            server_cap: None,
            expected_running_per_location: 2,
        },
        ProjectLimitTestCase {
            description: "a lower server cap lowers the project's limit",
            backend: Backend::Sqlite,
            project_limit: 2,
            server_cap: Some(1),
            expected_running_per_location: 1,
        },
        ProjectLimitTestCase {
            description: "a higher server cap never raises the project's limit",
            backend: Backend::Postgres,
            project_limit: 1,
            server_cap: Some(3),
            expected_running_per_location: 1,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate run store");
            let lease = acquire_lease(store, (Uuid::now_v7(), None))
                .await
                .expect("elect")
                .expect("leader");
            let limit = NonZeroU32::new(test_case.project_limit).expect("positive limit");
            let mut locations: Vec<Uuid> = Vec::new();
            for (index, name) in ["orders", "returns"].into_iter().enumerate() {
                let location = create_location(store, name, "test-admin")
                    .await
                    .expect("location");
                let published = publish_snapshot(store, &snapshot(location))
                    .await
                    .expect("snapshot");
                for run_index in 0..3 {
                    let id = u128::try_from(index * 10 + run_index + 1).expect("run id");
                    let mut input = request((location, published.snapshot_id), id, 0, &["orders"]);
                    input.write_set.clear();
                    input.pools.insert(
                        "warehouse".into(),
                        PoolSlots {
                            amount: NonZeroU32::MIN,
                            limit,
                        },
                    );
                    queue(store, &input).await;
                }
                locations.push(location);
            }
            let caps: BTreeMap<String, NonZeroU32> = test_case
                .server_cap
                .map(|cap| ("warehouse".to_owned(), NonZeroU32::new(cap).expect("cap")))
                .into_iter()
                .collect();
            let mut running: BTreeMap<Uuid, usize> = BTreeMap::new();
            let claims = claim_exactly(
                store,
                &lease,
                (&caps, test_case.expected_running_per_location * 2),
            )
            .await;
            for claim in claims {
                *running.entry(claim.request.location_id).or_default() += 1;
            }
            assert_eq!(
                locations
                    .iter()
                    .map(|location| running.get(location).copied().unwrap_or_default())
                    .collect::<Vec<_>>(),
                vec![test_case.expected_running_per_location; 2],
                "{}: each location's group has its own slots",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_pool_amounts_when_claiming_then_a_run_takes_all_its_slots_at_once_or_none() {
    let test_cases = [
        PoolAmountTestCase {
            description: "a run holding two of three slots leaves no room for another taking two",
            runs: &[
                (1, 0, &[("supplier_api", 2, 3)]),
                (2, 0, &[("supplier_api", 2, 3)]),
            ],
            caps: &[],
            expected_claimed: &[1],
            expected_slots: &[("supplier_api", 2)],
        },
        PoolAmountTestCase {
            description: "a run using two pools waits when one is full, takes nothing of the other, and goes before later runs of it",
            runs: &[
                (1, 5, &[("warehouse", 1, 1)]),
                (2, 0, &[("supplier_api", 1, 1), ("warehouse", 1, 1)]),
                (3, 0, &[("supplier_api", 1, 1)]),
            ],
            caps: &[],
            expected_claimed: &[1],
            expected_slots: &[("warehouse", 1)],
        },
        PoolAmountTestCase {
            description: "a server cap below a run's amount gives it the whole capped pool",
            runs: &[(1, 0, &[("supplier_api", 3, 3)])],
            caps: &[("supplier_api", 2)],
            expected_claimed: &[1],
            expected_slots: &[("supplier_api", 2)],
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(Backend::Sqlite).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate run store");
            let lease = acquire_lease(store, (Uuid::now_v7(), None))
                .await
                .expect("elect")
                .expect("leader");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let published = publish_snapshot(store, &snapshot(location))
                .await
                .expect("snapshot");
            for (id, priority, pools) in test_case.runs {
                let mut input = request(
                    (location, published.snapshot_id),
                    *id,
                    *priority,
                    &["orders"],
                );
                input.write_set.clear();
                for (pool, amount, limit) in *pools {
                    input
                        .pools
                        .insert((*pool).to_owned(), pool_slots(*amount, *limit));
                }
                queue(store, &input).await;
            }
            let caps: BTreeMap<String, NonZeroU32> = test_case
                .caps
                .iter()
                .map(|(pool, cap)| ((*pool).to_owned(), NonZeroU32::new(*cap).expect("cap")))
                .collect();

            let claims =
                claim_exactly(store, &lease, (&caps, test_case.expected_claimed.len())).await;

            assert_eq!(
                claims
                    .iter()
                    .map(|claim| claim.request.run_id.as_u128())
                    .collect::<Vec<_>>(),
                test_case.expected_claimed,
                "{}",
                test_case.description
            );
            assert_eq!(
                slot_counts(store).await.into_iter().collect::<Vec<_>>(),
                test_case
                    .expected_slots
                    .iter()
                    .map(|(pool, count)| ((*pool).to_owned(), *count))
                    .collect::<Vec<_>>(),
                "{}: a run holds as many slots as it takes",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
