//! Retention removes a finished run's output objects and index once it is older than `run_output_days`.

use chrono::TimeDelta;

use crate::run_output::main::prune_run_output::prune_run_output;
use crate::run_output::main::publish_output_batch::publish_output_batch;
use crate::run_output::models::OutputStream;
use crate::run_output::tests::helpers::{
    batch, claimed_run, object_count, output_store, segment, set_chunks_writable,
};
use crate::run_output::tests::test_types::{
    OutputRetentionFailureTestCase, OutputRetentionTestCase, RetentionState,
};
use crate::runs::tests::helpers::rows;
use crate::store::main::query::query;
use crate::store::main::write::write;
use crate::store::models::Parameter;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_finished_runs_when_retention_runs_then_only_output_past_its_days_is_removed() {
    let test_cases = [
        OutputRetentionTestCase {
            description: "SQLite run that ended ten days ago",
            backend: Backend::Sqlite,
            ended_days_ago: Some(10),
            expected_pruned: 1,
            expected_objects: 0,
            expected_marks: 0,
        },
        OutputRetentionTestCase {
            description: "PostgreSQL run that ended ten days ago",
            backend: Backend::Postgres,
            ended_days_ago: Some(10),
            expected_pruned: 1,
            expected_objects: 0,
            expected_marks: 0,
        },
        OutputRetentionTestCase {
            description: "a run that ended yesterday keeps its output",
            backend: Backend::Sqlite,
            ended_days_ago: Some(1),
            expected_pruned: 0,
            expected_objects: 2,
            expected_marks: 4,
        },
        OutputRetentionTestCase {
            description: "a running run keeps its output",
            backend: Backend::Sqlite,
            ended_days_ago: None,
            expected_pruned: 0,
            expected_objects: 2,
            expected_marks: 4,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            let folder = tempfile::tempdir().expect("output folder");
            let output = output_store(store, folder.path(), None);
            let (_, claim) = claimed_run(store).await;
            let run_id = claim.request.run_id;
            for number in 0..2_u64 {
                publish_output_batch(
                    store,
                    &output,
                    (run_id, claim.claim_id),
                    batch(
                        number,
                        vec![segment(
                            Some("model:orders"),
                            OutputStream::Stdout,
                            "loaded\nerror: a row was refused\nwarning: slow load\n",
                        )],
                    ),
                )
                .await
                .expect("batch");
            }
            let days_ago = test_case.ended_days_ago;
            write(store, |transaction| async move {
                let ended = days_ago.map(|days| transaction.now - TimeDelta::days(days));
                query(
                    transaction,
                    "UPDATE sqb_runs SET ended_at=$1 WHERE run_id=$2",
                    vec![Parameter::OptionalTime(ended), Parameter::Id(run_id)],
                )
                .await
            })
            .await
            .expect("end the run");
            let pruned = prune_run_output(store, &output, 7).await.expect("prune");
            assert_eq!(
                pruned, test_case.expected_pruned,
                "{}",
                test_case.description
            );
            assert_eq!(
                object_count(&output).await,
                test_case.expected_objects,
                "{}",
                test_case.description
            );
            let indexed = rows(
                store,
                "SELECT chunk FROM sqb_run_output_chunks WHERE run_id=$1",
                vec![Parameter::Id(run_id)],
            )
            .await;
            assert_eq!(
                indexed.len(),
                test_case.expected_objects,
                "{}: the index goes with the objects",
                test_case.description
            );
            let marks = rows(
                store,
                "SELECT node_line FROM sqb_run_output_marks WHERE run_id=$1",
                vec![Parameter::Id(run_id)],
            )
            .await;
            assert_eq!(
                marks.len(),
                test_case.expected_marks,
                "{}: marked lines go with the index",
                test_case.description
            );
            assert_eq!(
                prune_run_output(store, &output, 7).await.expect("repeat"),
                0,
                "{}: a repeated pass finds nothing",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_objects_that_cannot_be_deleted_when_retention_runs_then_the_index_points_at_nothing_and_the_next_pass_finishes()
 {
    let test_cases = [
        OutputRetentionFailureTestCase {
            description: "SQLite run whose objects are read-only at first",
            backend: Backend::Sqlite,
            expected_states: [(0, 2, 0, 0, 1), (1, 0, 0, 0, 0)],
        },
        OutputRetentionFailureTestCase {
            description: "PostgreSQL run whose objects are read-only at first",
            backend: Backend::Postgres,
            expected_states: [(0, 2, 0, 0, 1), (1, 0, 0, 0, 0)],
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            let folder = tempfile::tempdir().expect("output folder");
            let output = output_store(store, folder.path(), None);
            let (_, claim) = claimed_run(store).await;
            let run_id = claim.request.run_id;
            for number in 0..2_u64 {
                publish_output_batch(
                    store,
                    &output,
                    (run_id, claim.claim_id),
                    batch(
                        number,
                        vec![segment(
                            Some("model:orders"),
                            OutputStream::Stderr,
                            "error: a row was refused\n",
                        )],
                    ),
                )
                .await
                .expect("batch");
            }
            write(store, |transaction| async move {
                let ended = transaction.now - TimeDelta::days(10);
                query(
                    transaction,
                    "UPDATE sqb_runs SET ended_at=$1 WHERE run_id=$2",
                    vec![Parameter::Time(ended), Parameter::Id(run_id)],
                )
                .await
            })
            .await
            .expect("end the run");
            let mut states: Vec<RetentionState> = Vec::new();
            for writable in [false, true] {
                set_chunks_writable(&output, folder.path(), writable).await;
                let pruned = prune_run_output(store, &output, 7).await.expect("a pass never fails for one run");
                let count = async |sql: &str| rows(store, sql, vec![Parameter::Id(run_id)]).await.len();
                states.push((
                    pruned,
                    object_count(&output).await,
                    count("SELECT chunk FROM sqb_run_output_chunks WHERE run_id=$1").await,
                    count("SELECT node_line FROM sqb_run_output_marks WHERE run_id=$1").await,
                    count("SELECT run_id FROM sqb_run_output WHERE run_id=$1").await,
                ));
            }
            assert_eq!(
                states,
                test_case.expected_states.to_vec(),
                "{}: (pruned, objects, chunks, marks, output rows) after each pass",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
