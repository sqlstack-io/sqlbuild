//! Past `max_bytes_per_run` a run keeps its first part and its newest part, and counts the lines left out between them.

use crate::credentials::models::DefaultRole;
use crate::run_output::main::publish_output_batch::publish_output_batch;
use crate::run_output::main::read_person_output_index::read_person_output_index;
use crate::run_output::main::read_person_output_page::read_person_output_page;
use crate::run_output::models::OutputStream;
use crate::run_output::tests::helpers::{
    batch, claimed_run, object_count, output_store, page_query, reader, segment,
};
use crate::run_output::tests::test_types::{OutputCapTestCase, PageStart};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_run_past_its_byte_cap_when_its_output_is_read_then_the_first_and_last_parts_remain_with_a_gap()
 {
    let test_cases = [
        OutputCapTestCase {
            description: "SQLite keeps two first and two last batches of six",
            backend: Backend::Sqlite,
            max_bytes_per_run: 60,
            batches: 6,
            expected_lines: vec![(1, 0), (2, 0), (5, 2), (6, 0)],
            expected_omitted: (30, 2),
            expected_objects: 4,
            expected_mark_lines: vec![6],
        },
        OutputCapTestCase {
            description: "PostgreSQL keeps two first and two last batches of six",
            backend: Backend::Postgres,
            max_bytes_per_run: 60,
            batches: 6,
            expected_lines: vec![(1, 0), (2, 0), (5, 2), (6, 0)],
            expected_omitted: (30, 2),
            expected_objects: 4,
            expected_mark_lines: vec![6],
        },
        OutputCapTestCase {
            description: "a run under its cap keeps everything",
            backend: Backend::Sqlite,
            max_bytes_per_run: 1_000,
            batches: 3,
            expected_lines: vec![(1, 0), (2, 0), (3, 0)],
            expected_omitted: (0, 0),
            expected_objects: 3,
            expected_mark_lines: vec![3],
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            let folder = tempfile::tempdir().expect("output folder");
            let output = output_store(store, folder.path(), Some(test_case.max_bytes_per_run));
            let (location, claim) = claimed_run(store).await;
            for number in 0..test_case.batches {
                let word: &str = ["ok   ", "error"][usize::from([2, 3, 5].contains(&number))];
                let text = format!("batch {number:02} {word}\n");
                publish_output_batch(
                    store,
                    &output,
                    (claim.request.run_id, claim.claim_id),
                    batch(
                        number as u64,
                        vec![segment(Some("model:orders"), OutputStream::Stdout, &text)],
                    ),
                )
                .await
                .expect("batch");
            }
            let (context, admin) = reader(store).await;
            let person = (
                (&context, admin),
                (location, "development", claim.request.run_id),
                DefaultRole::None,
            );
            let page = read_person_output_page(
                store,
                &output,
                person,
                &page_query(
                    (Some("model:orders"), None),
                    PageStart {
                        from: Some(1),
                        ..PageStart::default()
                    },
                    (100, false, None),
                ),
            )
            .await
            .expect("page")
            .expect("run");
            let rows: serde_json::Value = serde_json::to_value(&page.rows).expect("rows");
            let lines: Vec<(i64, i64)> = rows
                .as_array()
                .expect("rows")
                .iter()
                .map(|row| {
                    (
                        row["line"]["number"].as_i64().unwrap_or_default(),
                        row["line"]["omitted_before"].as_i64().unwrap_or_default(),
                    )
                })
                .collect();
            assert_eq!(lines, test_case.expected_lines, "{}", test_case.description);
            let index = read_person_output_index(store, person, Some("model:orders"))
                .await
                .expect("index")
                .expect("run");
            let mark_lines: Vec<i64> = index.marks.iter().map(|mark| mark.line).collect();
            assert_eq!(
                ((index.omitted_bytes, index.omitted_lines), mark_lines),
                (
                    test_case.expected_omitted,
                    test_case.expected_mark_lines.clone()
                ),
                "{}",
                test_case.description
            );
            assert_eq!(
                object_count(&output).await,
                test_case.expected_objects,
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
