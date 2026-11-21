//! A SQLite control store keeps output beside itself by default; a PostgreSQL one must name a store.

use crate::run_output::main::open_output_store::open_output_store;
use crate::run_output::models::OutputSettings;
use crate::run_output::tests::test_types::OutputStoreSettingTestCase;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_control_store_when_no_output_store_is_named_then_only_sqlite_has_a_default() {
    let folder = tempfile::tempdir().expect("output folder");
    let test_cases = [
        OutputStoreSettingTestCase {
            description: "SQLite with no store named",
            backend: Backend::Sqlite,
            store: None,
            expected_error: None,
        },
        OutputStoreSettingTestCase {
            description: "PostgreSQL with no store named",
            backend: Backend::Postgres,
            store: None,
            expected_error: Some(
                "invalid control-plane data: a PostgreSQL control store needs [output] store in server.toml: an s3:// bucket, or a file:// folder every server replica shares",
            ),
        },
        OutputStoreSettingTestCase {
            description: "PostgreSQL with a store named",
            backend: Backend::Postgres,
            store: Some(format!("file://{}", folder.path().display())),
            expected_error: None,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let opened = open_output_store(
                &OutputSettings {
                    store: test_case.store.clone(),
                    max_bytes_per_run: None,
                },
                &fixture.store,
            );
            assert_eq!(
                opened.err().map(|error| error.to_string()),
                test_case.expected_error.map(str::to_owned),
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
