//! A page following a run audits its repeated output reads once per window; a search or download is audited each time.

use crate::credentials::models::DefaultRole;
use crate::run_output::main::publish_output_batch::publish_output_batch;
use crate::run_output::main::read_person_output_index::read_person_output_index;
use crate::run_output::main::read_person_output_page::read_person_output_page;
use crate::run_output::main::search_person_output::search_person_output;
use crate::run_output::models::{OutputSearchQuery, OutputStream};
use crate::run_output::tests::helpers::{
    batch, claimed_run, output_store, page_query, reader, segment,
};
use crate::run_output::tests::test_types::{OutputAuditTestCase, PageStart};
use crate::runs::tests::helpers::rows;
use crate::store::models::Parameter;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_page_following_a_run_when_it_reads_the_output_twice_then_only_searches_are_audited_twice()
 {
    let test_cases = [
        OutputAuditTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_audits: vec![
                ("output.index", 1),
                ("output.page", 1),
                ("output.search", 2),
            ],
        },
        OutputAuditTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_audits: vec![
                ("output.index", 1),
                ("output.page", 1),
                ("output.search", 2),
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
            let run_id = claim.request.run_id;
            publish_output_batch(
                store,
                &output,
                (run_id, claim.claim_id),
                batch(
                    0,
                    vec![segment(
                        Some("model:orders"),
                        OutputStream::Stdout,
                        "loaded\n",
                    )],
                ),
            )
            .await
            .expect("batch");
            let (context, admin) = reader(store).await;
            let person = (
                (&context, admin),
                (location, "development", run_id),
                DefaultRole::None,
            );
            let tail = page_query(
                (None, None),
                PageStart {
                    tail: true,
                    ..PageStart::default()
                },
                (10, false, None),
            );
            let search = OutputSearchQuery {
                node: "model:orders".into(),
                stream: None,
                text: "loaded".into(),
                from: None,
                direction: None,
            };
            for _ in 0..2 {
                read_person_output_index(store, person, None)
                    .await
                    .expect("index")
                    .expect("run");
                read_person_output_page(store, &output, person, &tail)
                    .await
                    .expect("page")
                    .expect("run");
                search_person_output(store, &output, person, &search)
                    .await
                    .expect("search")
                    .expect("run");
            }
            let mut audits: Vec<(&str, usize)> = Vec::new();
            for (action, _) in &test_case.expected_audits {
                let found = rows(
                    store,
                    "SELECT event_id FROM sqb_security_audit WHERE action=$1",
                    vec![Parameter::Text((*action).into())],
                )
                .await;
                audits.push((action, found.len()));
            }
            assert_eq!(
                audits, test_case.expected_audits,
                "{}: audit rows by action",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
