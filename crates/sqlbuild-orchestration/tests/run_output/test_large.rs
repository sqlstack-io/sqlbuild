//! A node printing about 1 GB leaves the control store holding only its small index.

use std::time::Instant;

use crate::credentials::models::DefaultRole;
use crate::run_output::constants::BATCH_TEXT_BYTES;
use crate::run_output::main::publish_output_batch::publish_output_batch;
use crate::run_output::main::read_person_output_page::read_person_output_page;
use crate::run_output::models::{OutputPageQuery, OutputStream};
use crate::run_output::tests::helpers::{batch, claimed_run, output_store, reader, segment};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

const GIGABYTE: usize = 1 << 30;
const CONTROL_STORE_LIMIT: u64 = 16 * 1024 * 1024;

#[test]
fn given_a_node_printing_a_gigabyte_when_it_is_stored_then_the_control_store_holds_only_the_index()
{
    run(async {
        let Some(fixture) = open_test_store(Backend::Sqlite).await else {
            return;
        };
        let store = &fixture.store;
        let folder = tempfile::tempdir().expect("output folder");
        let output = output_store(store, folder.path(), None);
        let (location, claim) = claimed_run(store).await;
        let line =
            "2026-11-21T10:00:00Z model orders processed partition 000123 rows=48210 status=ok\n";
        let text: String = line.repeat(BATCH_TEXT_BYTES / line.len());
        let batches: usize = GIGABYTE.div_ceil(text.len());
        let started = Instant::now();
        for number in 0..batches {
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
        let directory = fixture.directory.as_ref().expect("SQLite folder");
        let control_bytes: u64 = std::fs::read_dir(directory.path())
            .expect("control folder")
            .flatten()
            .map(|entry| {
                entry
                    .metadata()
                    .map(|metadata| metadata.len())
                    .unwrap_or_default()
            })
            .sum();
        assert!(
            control_bytes < CONTROL_STORE_LIMIT,
            "the control store holds {control_bytes} bytes after {batches} batches in {:?}",
            started.elapsed()
        );
        let lines_per_batch = (text.len() / line.len()) as i64;
        let (context, admin) = reader(store).await;
        let page = read_person_output_page(
            store,
            &output,
            (
                (&context, admin),
                (location, "development", claim.request.run_id),
                DefaultRole::None,
            ),
            &OutputPageQuery {
                node: "model:orders".into(),
                stream: OutputStream::Stdout,
                from: None,
                limit: Some(2),
                tail: true,
            },
        )
        .await
        .expect("tail")
        .expect("run");
        assert_eq!(page.line_count, lines_per_batch * batches as i64);
        assert_eq!(page.lines.len(), 2);
        close_test_store(fixture).await;
    });
}
