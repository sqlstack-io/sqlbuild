use std::collections::BTreeSet;

use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::runs::_helpers::requests::past_runs::read_past_runs;
use crate::runs::models::PastRunResults;
use crate::runs::tests::helpers::{queue, request, set_node_status, snapshot};
use crate::runs::tests::test_types::PastRunLookupTestCase;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::migrate_store::migrate_store;
use crate::store::models::TransactionMode;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

const FIRST: u128 = 0x01a1_d3b9_0000_7000_8000_0000_0000_0001;
const SECOND: u128 = 0x01a1_d3b9_0000_7000_8000_0000_0000_0002;
const THIRD: u128 = 0x01a1_d3c0_0000_7000_8000_0000_0000_0003;

#[test]
fn given_runs_when_run_selectors_name_them_then_full_ids_and_unique_prefixes_read_their_results() {
    let test_cases = [
        PastRunLookupTestCase {
            description: "SQLite: a full ID and a unique 8-character prefix each name their run, with what it selected, failed and skipped",
            backend: Backend::Sqlite,
            ids: &["01a1d3b9-0000-7000-8000-000000000001", "01a1d3c0"],
            expected_runs: Ok(vec![
                (
                    "01a1d3b9-0000-7000-8000-000000000001",
                    FIRST,
                    [&["customers", "orders"], &["orders"], &["customers"]],
                ),
                ("01a1d3c0", THIRD, [&["customers", "orders"], &[], &[]]),
            ]),
        },
        PastRunLookupTestCase {
            description: "SQLite: an ambiguous prefix lists the runs it matches",
            backend: Backend::Sqlite,
            ids: &["01a1d3b9"],
            expected_runs: Err(
                "run ID '01a1d3b9' is ambiguous: it starts 01a1d3b9-0000-7000-8000-000000000001, 01a1d3b9-0000-7000-8000-000000000002; give more of the ID",
            ),
        },
        PastRunLookupTestCase {
            description: "SQLite: a prefix shorter than 8 characters is refused",
            backend: Backend::Sqlite,
            ids: &["01a1d3c"],
            expected_runs: Err(
                "run ID '01a1d3c' is too short: give the full ID or at least 8 characters of it",
            ),
        },
        PastRunLookupTestCase {
            description: "SQLite: an ID no run has is refused",
            backend: Backend::Sqlite,
            ids: &["ffffffff"],
            expected_runs: Err("no run 'ffffffff' in this location and target"),
        },
        PastRunLookupTestCase {
            description: "PostgreSQL: a full ID and a unique 8-character prefix each name their run, with what it selected, failed and skipped",
            backend: Backend::Postgres,
            ids: &["01a1d3b9-0000-7000-8000-000000000001", "01a1d3c0"],
            expected_runs: Ok(vec![
                (
                    "01a1d3b9-0000-7000-8000-000000000001",
                    FIRST,
                    [&["customers", "orders"], &["orders"], &["customers"]],
                ),
                ("01a1d3c0", THIRD, [&["customers", "orders"], &[], &[]]),
            ]),
        },
        PastRunLookupTestCase {
            description: "PostgreSQL: an ambiguous prefix lists the runs it matches",
            backend: Backend::Postgres,
            ids: &["01a1d3b9"],
            expected_runs: Err(
                "run ID '01a1d3b9' is ambiguous: it starts 01a1d3b9-0000-7000-8000-000000000001, 01a1d3b9-0000-7000-8000-000000000002; give more of the ID",
            ),
        },
        PastRunLookupTestCase {
            description: "PostgreSQL: a prefix shorter than 8 characters is refused",
            backend: Backend::Postgres,
            ids: &["01a1d3c"],
            expected_runs: Err(
                "run ID '01a1d3c' is too short: give the full ID or at least 8 characters of it",
            ),
        },
        PastRunLookupTestCase {
            description: "PostgreSQL: an ID no run has is refused",
            backend: Backend::Postgres,
            ids: &["ffffffff"],
            expected_runs: Err("no run 'ffffffff' in this location and target"),
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
            for id in [FIRST, SECOND, THIRD] {
                queue(
                    store,
                    &request(
                        (location, published.snapshot_id),
                        id,
                        1,
                        &["customers", "orders"],
                    ),
                )
                .await;
            }
            set_node_status(store, (Uuid::from_u128(FIRST), "orders"), "failed").await;
            set_node_status(store, (Uuid::from_u128(FIRST), "customers"), "skipped").await;
            let ids: BTreeSet<String> = test_case.ids.iter().map(|id| (*id).to_owned()).collect();
            let transaction = begin(store, TransactionMode::Read).await.expect("read");
            let (transaction, past) = read_past_runs(transaction, (location, "development"), &ids)
                .await
                .expect("read past runs");
            commit(transaction).await.expect("commit");
            let keys = |keys: &[&str]| keys.iter().map(|key| (*key).to_owned()).collect();
            assert_eq!(
                past,
                test_case
                    .expected_runs
                    .clone()
                    .map(|runs| runs
                        .into_iter()
                        .map(|(id, run_id, [selected, failed, skipped])| (
                            id.to_owned(),
                            PastRunResults {
                                run_id: Uuid::from_u128(run_id),
                                selected: keys(selected),
                                failed: keys(failed),
                                skipped: keys(skipped),
                            }
                        ))
                        .collect())
                    .map_err(str::to_owned),
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
