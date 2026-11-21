//! A person reads a node's output numbered across its two streams, or the run's numbered in the order printed: pages either way, one stream, folded similar lines, two-way search and download.

use futures_util::StreamExt;
use uuid::Uuid;

use crate::credentials::models::DefaultRole;
use crate::run_output::main::download_person_output::download_person_output;
use crate::run_output::main::publish_output_batch::publish_output_batch;
use crate::run_output::main::read_person_output_index::read_person_output_index;
use crate::run_output::main::read_person_output_page::read_person_output_page;
use crate::run_output::main::search_person_output::search_person_output;
use crate::run_output::models::{
    OutputDownloadQuery, OutputLevel, OutputMark, OutputSearch, OutputSearchQuery, OutputStream,
    SearchDirection,
};
use crate::run_output::tests::helpers::{
    batch, claimed_run, line, line_row, output_store, page_query, reader, segment, similar_row,
    timeless,
};
use crate::run_output::tests::test_types::{
    OutputFoldTestCase, OutputReadTestCase, OutputSearchTestCase, PageRead, PageStart, SearchRead,
};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

const ORDERS: &str = "model:orders";
const CUSTOMERS: &str = "model:customers";

#[test]
fn given_worker_batches_when_a_person_reads_a_node_or_the_run_then_pages_streams_marks_and_downloads_agree()
 {
    let (out, err) = (OutputStream::Stdout, OutputStream::Stderr);
    let marks = || {
        vec![
            OutputMark {
                line: 4,
                stream: err,
                level: OutputLevel::Error,
                text: "Traceback (most recent call last):".into(),
            },
            OutputMark {
                line: 5,
                stream: err,
                level: OutputLevel::Error,
                text: "ValueError: bad amount".into(),
            },
            OutputMark {
                line: 7,
                stream: out,
                level: OutputLevel::Warning,
                text: "warning: slow load".into(),
            },
        ]
    };
    let pages = || {
        vec![
            PageRead {
                node: Some(ORDERS),
                stream: None,
                start: PageStart {
                    from: Some(2),
                    ..PageStart::default()
                },
                limit: 3,
                expected_rows: vec![
                    line_row(2, ORDERS, out, "line 2"),
                    line_row(3, ORDERS, out, "line 3"),
                    line_row(4, ORDERS, err, "Traceback (most recent call last):"),
                ],
                expected_links: (Some(5), Some(2)),
            },
            PageRead {
                node: Some(ORDERS),
                stream: Some(out),
                start: PageStart {
                    from: Some(1),
                    ..PageStart::default()
                },
                limit: 10,
                expected_rows: vec![
                    line_row(1, ORDERS, out, "line 1"),
                    line_row(2, ORDERS, out, "line 2"),
                    line_row(3, ORDERS, out, "line 3"),
                    line_row(6, ORDERS, out, "line 4"),
                    line_row(7, ORDERS, out, "warning: slow load"),
                    line_row(8, ORDERS, out, "line 6"),
                ],
                expected_links: (None, None),
            },
            PageRead {
                node: Some(ORDERS),
                stream: None,
                start: PageStart {
                    tail: true,
                    ..PageStart::default()
                },
                limit: 2,
                expected_rows: vec![
                    line_row(7, ORDERS, out, "warning: slow load"),
                    line_row(8, ORDERS, out, "line 6"),
                ],
                expected_links: (None, Some(7)),
            },
            PageRead {
                node: Some(ORDERS),
                stream: None,
                start: PageStart {
                    before: Some(4),
                    ..PageStart::default()
                },
                limit: 2,
                expected_rows: vec![
                    line_row(2, ORDERS, out, "line 2"),
                    line_row(3, ORDERS, out, "line 3"),
                ],
                expected_links: (Some(4), Some(2)),
            },
            PageRead {
                node: None,
                stream: None,
                start: PageStart {
                    tail: true,
                    ..PageStart::default()
                },
                limit: 3,
                expected_rows: vec![
                    line_row(8, ORDERS, out, "warning: slow load"),
                    line_row(9, ORDERS, out, "line 6"),
                    line_row(10, CUSTOMERS, out, "loaded customers"),
                ],
                expected_links: (None, Some(8)),
            },
            PageRead {
                node: None,
                stream: Some(err),
                start: PageStart {
                    from: Some(1),
                    ..PageStart::default()
                },
                limit: 5,
                expected_rows: vec![
                    line_row(5, ORDERS, err, "Traceback (most recent call last):"),
                    line_row(6, ORDERS, err, "ValueError: bad amount"),
                ],
                expected_links: (None, None),
            },
        ]
    };
    let downloads = (
        "line 1\nline 2\nline 3\nTraceback (most recent call last):\nValueError: bad amount\nline 4\nwarning: slow load\nline 6\n",
        "Traceback (most recent call last):\nValueError: bad amount\n",
    );
    let test_cases = [
        OutputReadTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_nodes: vec![(CUSTOMERS, 1, 1, 0), (ORDERS, 8, 6, 2), ("run", 1, 1, 0)],
            expected_run_lines: 10,
            expected_marks: marks(),
            pages: pages(),
            expected_downloads: downloads,
        },
        OutputReadTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_nodes: vec![(CUSTOMERS, 1, 1, 0), (ORDERS, 8, 6, 2), ("run", 1, 1, 0)],
            expected_run_lines: 10,
            expected_marks: marks(),
            pages: pages(),
            expected_downloads: downloads,
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
            let batches = [
                batch(
                    0,
                    vec![
                        segment(None, out, "starting\n"),
                        segment(Some(ORDERS), out, "line 1\nline 2\nline 3\n"),
                        segment(
                            Some(ORDERS),
                            err,
                            "Traceback (most recent call last):\nValueError: bad amount\n",
                        ),
                    ],
                ),
                batch(
                    1,
                    vec![
                        segment(Some(ORDERS), out, "line 4\nwarning: slow load\nline 6\n"),
                        segment(Some(CUSTOMERS), out, "loaded customers\n"),
                    ],
                ),
            ];
            for sent in batches.iter().chain(&batches[1..]) {
                publish_output_batch(store, &output, worker, sent.clone())
                    .await
                    .expect("batch");
            }
            let refused = publish_output_batch(
                store,
                &output,
                (claim.request.run_id, Uuid::now_v7()),
                batches[1].clone(),
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
            let index = read_person_output_index(store, person, Some(ORDERS))
                .await
                .expect("index")
                .expect("run");
            let nodes: Vec<(&str, i64, i64, i64)> = index
                .nodes
                .iter()
                .map(|node| {
                    (
                        node.node_key.as_str(),
                        node.line_count,
                        node.stdout_lines,
                        node.stderr_lines,
                    )
                })
                .collect();
            assert_eq!(
                (nodes, index.line_count, &index.marks),
                (
                    test_case.expected_nodes.clone(),
                    test_case.expected_run_lines,
                    &test_case.expected_marks
                ),
                "{}: a retried batch is stored once",
                test_case.description
            );
            for read in &test_case.pages {
                let page = read_person_output_page(
                    store,
                    &output,
                    person,
                    &page_query(
                        (read.node, read.stream),
                        read.start,
                        (read.limit, false, None),
                    ),
                )
                .await
                .expect("page")
                .expect("run");
                assert_eq!(
                    (timeless(&page.rows), (page.next, page.previous)),
                    (timeless(&read.expected_rows), read.expected_links),
                    "{}: {:?} {:?} from {:?}",
                    test_case.description,
                    read.node,
                    read.stream,
                    (read.start.from, read.start.before, read.start.tail)
                );
            }
            let mut downloads: Vec<String> = Vec::new();
            for stream in [None, Some(err)] {
                let parts: Vec<String> = download_person_output(
                    store,
                    &output,
                    person,
                    &OutputDownloadQuery {
                        node: ORDERS.into(),
                        stream,
                    },
                )
                .await
                .expect("download")
                .expect("run")
                .map(|part| part.expect("chunk"))
                .collect()
                .await;
                downloads.push(parts.concat());
            }
            assert_eq!(
                (downloads[0].as_str(), downloads[1].as_str()),
                test_case.expected_downloads,
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_runs_of_similar_lines_when_a_folding_page_is_read_then_four_or_more_fold_except_around_a_jump()
 {
    let out = OutputStream::Stdout;
    let printed: &str = "row 1 ok\nrow 2 ok\nrow 3 ok\nrow 4 ok\nrow 5 ok\nrow 6 ok\nrow 7 ok\nrow 8 ok\nrow 9 ok\nrow 10 ok\ndone\nx 1\nx 2\n";
    let row = |number: i64| line_row(number, ORDERS, out, &format!("row {number} ok"));
    let test_cases = [
        OutputFoldTestCase {
            description: "ten similar lines fold, two do not",
            backend: Backend::Sqlite,
            printed,
            start: PageStart {
                from: Some(1),
                ..PageStart::default()
            },
            limit: 10,
            around: None,
            expected_rows: vec![
                similar_row(10, line(1, ORDERS, out, "row 1 ok")),
                line_row(11, ORDERS, out, "done"),
                line_row(12, ORDERS, out, "x 1"),
                line_row(13, ORDERS, out, "x 2"),
            ],
            expected_span: (1, 14),
        },
        OutputFoldTestCase {
            description: "lines near a jump stay open and the four before them fold",
            backend: Backend::Postgres,
            printed,
            start: PageStart {
                from: Some(1),
                ..PageStart::default()
            },
            limit: 20,
            around: Some(8),
            expected_rows: vec![
                similar_row(4, line(1, ORDERS, out, "row 1 ok")),
                row(5),
                row(6),
                row(7),
                row(8),
                row(9),
                row(10),
                line_row(11, ORDERS, out, "done"),
                line_row(12, ORDERS, out, "x 1"),
                line_row(13, ORDERS, out, "x 2"),
            ],
            expected_span: (1, 14),
        },
        OutputFoldTestCase {
            description: "a one-row page reads its folded run to the end",
            backend: Backend::Sqlite,
            printed,
            start: PageStart {
                from: Some(1),
                ..PageStart::default()
            },
            limit: 1,
            around: None,
            expected_rows: vec![similar_row(10, line(1, ORDERS, out, "row 1 ok"))],
            expected_span: (1, 11),
        },
        OutputFoldTestCase {
            description: "a backward page folds the run before it",
            backend: Backend::Postgres,
            printed,
            start: PageStart {
                before: Some(11),
                ..PageStart::default()
            },
            limit: 1,
            around: None,
            expected_rows: vec![similar_row(10, line(1, ORDERS, out, "row 1 ok"))],
            expected_span: (1, 11),
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
            publish_output_batch(
                store,
                &output,
                (claim.request.run_id, claim.claim_id),
                batch(0, vec![segment(Some(ORDERS), out, test_case.printed)]),
            )
            .await
            .expect("batch");
            let (context, admin) = reader(store).await;
            let page = read_person_output_page(
                store,
                &output,
                (
                    (&context, admin),
                    (location, "development", claim.request.run_id),
                    DefaultRole::None,
                ),
                &page_query(
                    (Some(ORDERS), None),
                    test_case.start,
                    (test_case.limit, true, test_case.around),
                ),
            )
            .await
            .expect("page")
            .expect("run");
            assert_eq!(
                (timeless(&page.rows), (page.from, page.to)),
                (timeless(&test_case.expected_rows), test_case.expected_span),
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_node_output_when_searched_either_way_then_small_output_counts_and_wraps_and_large_output_continues()
 {
    let (out, err) = (OutputStream::Stdout, OutputStream::Stderr);
    let found = |line: i64, count: i64, rank: i64| OutputSearch {
        line: Some(line),
        count: Some(count),
        rank: Some(rank),
        continue_from: None,
    };
    let partial = |line: Option<i64>, continue_from: Option<i64>| OutputSearch {
        line,
        count: None,
        rank: None,
        continue_from,
    };
    let search = |text: &'static str, from: i64, direction: SearchDirection| SearchRead {
        text,
        from,
        direction,
    };
    let (forward, backward) = (SearchDirection::Forward, SearchDirection::Backward);
    let test_cases = [
        OutputSearchTestCase {
            description: "SQLite output small enough to scan whole",
            backend: Backend::Sqlite,
            filler_batches: 0,
            searches: vec![
                search("line", 1, forward),
                search("line", 4, forward),
                search("line", 5, backward),
                search("line", 9, forward),
                search("VALUEERROR", 1, forward),
                search("zzz", 1, forward),
            ],
            expected_results: vec![
                found(1, 5, 1),
                found(6, 5, 4),
                found(3, 5, 3),
                found(1, 5, 1),
                found(5, 1, 1),
                OutputSearch {
                    line: None,
                    count: Some(0),
                    rank: None,
                    continue_from: None,
                },
            ],
        },
        OutputSearchTestCase {
            description: "PostgreSQL output past the chunk budget continues in both directions",
            backend: Backend::Postgres,
            filler_batches: 70,
            searches: vec![
                search("line", 1, forward),
                search("line", 66, forward),
                search("nomatch", 200, backward),
                search("nomatch", 8, backward),
            ],
            expected_results: vec![
                partial(None, Some(66)),
                partial(Some(71), None),
                partial(None, Some(8)),
                partial(None, None),
            ],
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
            for number in 0..test_case.filler_batches {
                publish_output_batch(
                    store,
                    &output,
                    worker,
                    batch(
                        number,
                        vec![segment(
                            Some(ORDERS),
                            out,
                            &format!("filler {}\n", number + 1),
                        )],
                    ),
                )
                .await
                .expect("filler");
            }
            let first = test_case.filler_batches;
            for sent in [
                batch(
                    first,
                    vec![
                        segment(Some(ORDERS), out, "line 1\nline 2\nline 3\n"),
                        segment(
                            Some(ORDERS),
                            err,
                            "Traceback (most recent call last):\nValueError: bad amount\n",
                        ),
                    ],
                ),
                batch(
                    first + 1,
                    vec![segment(
                        Some(ORDERS),
                        out,
                        "line 4\nwarning: slow load\nline 6\n",
                    )],
                ),
            ] {
                publish_output_batch(store, &output, worker, sent)
                    .await
                    .expect("batch");
            }
            let (context, admin) = reader(store).await;
            let person = (
                (&context, admin),
                (location, "development", claim.request.run_id),
                DefaultRole::None,
            );
            let mut results: Vec<OutputSearch> = Vec::new();
            for read in &test_case.searches {
                let result = search_person_output(
                    store,
                    &output,
                    person,
                    &OutputSearchQuery {
                        node: ORDERS.into(),
                        stream: None,
                        text: read.text.into(),
                        from: Some(read.from),
                        direction: Some(read.direction),
                    },
                )
                .await
                .expect("search")
                .expect("run");
                results.push(result);
            }
            assert_eq!(
                results, test_case.expected_results,
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
