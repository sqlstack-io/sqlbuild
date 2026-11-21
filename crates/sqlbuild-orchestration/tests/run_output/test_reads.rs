//! A person pages, tails, searches and downloads one node's stream; a retried batch is stored once; another claim is refused.

use uuid::Uuid;

use crate::credentials::models::DefaultRole;
use crate::run_output::main::download_person_output::download_person_output;
use crate::run_output::main::publish_output_batch::publish_output_batch;
use crate::run_output::main::read_person_output_index::read_person_output_index;
use crate::run_output::main::read_person_output_page::read_person_output_page;
use crate::run_output::main::search_person_output::search_person_output;
use crate::run_output::models::{
    OutputDownloadQuery, OutputPageQuery, OutputSearchQuery, OutputStream,
};
use crate::run_output::tests::helpers::{batch, claimed_run, output_store, reader, segment};
use crate::run_output::tests::test_types::{OutputReadTestCase, StreamSummary};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use futures_util::StreamExt;

#[test]
fn given_worker_batches_when_a_person_reads_a_node_stream_then_pages_tail_search_and_download_agree()
 {
    let test_cases = [
        OutputReadTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_streams: vec![
                ("model:orders", "stderr", 2, vec![1, 2], vec![]),
                ("model:orders", "stdout", 6, vec![], vec![5]),
                ("run", "stdout", 1, vec![], vec![]),
            ],
            expected_page: vec![(2, "line 2"), (3, "line 3"), (4, "line 4")],
            expected_page_links: (Some(5), Some(1)),
            expected_tail: vec![5, 6],
            expected_matches: vec![1, 2, 3, 4, 6],
            expected_download: "line 1\nline 2\nline 3\nline 4\nwarning: slow load\nline 6\n",
        },
        OutputReadTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_streams: vec![
                ("model:orders", "stderr", 2, vec![1, 2], vec![]),
                ("model:orders", "stdout", 6, vec![], vec![5]),
                ("run", "stdout", 1, vec![], vec![]),
            ],
            expected_page: vec![(2, "line 2"), (3, "line 3"), (4, "line 4")],
            expected_page_links: (Some(5), Some(1)),
            expected_tail: vec![5, 6],
            expected_matches: vec![1, 2, 3, 4, 6],
            expected_download: "line 1\nline 2\nline 3\nline 4\nwarning: slow load\nline 6\n",
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
            let (location, claim) = claimed_run(store).await;
            let worker = (claim.request.run_id, claim.claim_id);
            let first = batch(
                0,
                vec![
                    segment(None, OutputStream::Stdout, "starting\n"),
                    segment(
                        Some("model:orders"),
                        OutputStream::Stdout,
                        "line 1\nline 2\nline 3\n",
                    ),
                    segment(
                        Some("model:orders"),
                        OutputStream::Stderr,
                        "Traceback (most recent call last):\nValueError: bad amount\n",
                    ),
                ],
            );
            let second = batch(
                1,
                vec![segment(
                    Some("model:orders"),
                    OutputStream::Stdout,
                    "line 4\nwarning: slow load\nline 6\n",
                )],
            );
            let stored: Vec<bool> = vec![
                publish_output_batch(store, &output, worker, first)
                    .await
                    .expect("first")
                    .stored,
                publish_output_batch(store, &output, worker, second.clone())
                    .await
                    .expect("second")
                    .stored,
                publish_output_batch(store, &output, worker, second.clone())
                    .await
                    .expect("retry")
                    .stored,
            ];
            assert_eq!(
                stored,
                vec![true, true, false],
                "{}: a retried batch is stored once",
                test_case.description
            );
            let refused = publish_output_batch(
                store,
                &output,
                (claim.request.run_id, Uuid::now_v7()),
                second,
            )
            .await;
            assert!(
                refused.is_err(),
                "{}: another claim cannot publish",
                test_case.description
            );
            let (context, admin) = reader(store).await;
            let person = (
                (&context, admin),
                (location, "development", claim.request.run_id),
                DefaultRole::None,
            );
            let index = read_person_output_index(store, person)
                .await
                .expect("index")
                .expect("run");
            let streams: Vec<StreamSummary<String>> = index
                .streams
                .iter()
                .map(|stream| {
                    (
                        stream.node_key.clone(),
                        stream.stream.as_str(),
                        stream.line_count,
                        stream.error_lines.clone(),
                        stream.warning_lines.clone(),
                    )
                })
                .collect();
            let expected: Vec<StreamSummary<String>> = test_case
                .expected_streams
                .iter()
                .map(|(node, stream, lines, errors, warnings)| {
                    (
                        (*node).to_owned(),
                        *stream,
                        *lines,
                        errors.clone(),
                        warnings.clone(),
                    )
                })
                .collect();
            assert_eq!(streams, expected, "{}", test_case.description);
            let page_query = |from: Option<i64>, limit: u32, tail: bool| OutputPageQuery {
                node: "model:orders".into(),
                stream: OutputStream::Stdout,
                from,
                limit: Some(limit),
                tail,
            };
            let page =
                read_person_output_page(store, &output, person, &page_query(Some(2), 3, false))
                    .await
                    .expect("page")
                    .expect("run");
            let lines: Vec<(i64, &str)> = page
                .lines
                .iter()
                .map(|line| (line.number, line.text.as_str()))
                .collect();
            assert_eq!(lines, test_case.expected_page, "{}", test_case.description);
            assert_eq!(
                (page.next, page.previous),
                test_case.expected_page_links,
                "{}",
                test_case.description
            );
            let tail = read_person_output_page(store, &output, person, &page_query(None, 2, true))
                .await
                .expect("tail")
                .expect("run");
            let numbers: Vec<i64> = tail.lines.iter().map(|line| line.number).collect();
            assert_eq!(
                numbers, test_case.expected_tail,
                "{}",
                test_case.description
            );
            let search = search_person_output(
                store,
                &output,
                person,
                &OutputSearchQuery {
                    node: "model:orders".into(),
                    stream: OutputStream::Stdout,
                    text: "LINE".into(),
                    after: None,
                    limit: None,
                },
            )
            .await
            .expect("search")
            .expect("run");
            let matches: Vec<i64> = search.matches.iter().map(|line| line.number).collect();
            assert_eq!(
                matches, test_case.expected_matches,
                "{}",
                test_case.description
            );
            let download = download_person_output(
                store,
                &output,
                person,
                &OutputDownloadQuery {
                    node: "model:orders".into(),
                    stream: OutputStream::Stdout,
                },
            )
            .await
            .expect("download")
            .expect("run");
            let parts: Vec<String> = download.map(|part| part.expect("chunk")).collect().await;
            assert_eq!(
                parts.concat(),
                test_case.expected_download,
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
