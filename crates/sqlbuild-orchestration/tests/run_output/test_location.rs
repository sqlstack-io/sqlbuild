//! An output store with no setting defaults to an `output` folder for either control store, reported as an absolute location.

use crate::run_output::main::open_output_store::open_output_store;
use crate::run_output::models::OutputSettings;
use crate::run_output::tests::test_types::OutputLocationTestCase;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_no_output_setting_when_the_store_opens_then_it_defaults_to_an_absolute_output_folder() {
    let test_cases = [
        OutputLocationTestCase {
            description: "SQLite keeps it beside the control database",
            backend: Backend::Sqlite,
            expected_folder: "output",
        },
        OutputLocationTestCase {
            description: "PostgreSQL keeps it in the working directory",
            backend: Backend::Postgres,
            expected_folder: "output",
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let opened = open_output_store(&OutputSettings::default(), &fixture.store)
                .expect("the default output store");
            let folder = std::path::Path::new(
                opened
                    .location()
                    .strip_prefix("file://")
                    .expect("a local store"),
            );
            assert_eq!(
                (
                    folder.is_absolute(),
                    folder.is_dir(),
                    folder.ends_with(test_case.expected_folder)
                ),
                (true, true, true),
                "{}: {}",
                test_case.description,
                opened.location()
            );
            drop(std::fs::remove_dir(folder));
            close_test_store(fixture).await;
        });
    }
}
