//! With no setting output goes beside a SQLite store, or to the user's state folder with PostgreSQL whatever the working directory.

use crate::run_output::_helpers::folders::default_folder;
use crate::run_output::main::open_output_store::open_output_store;
use crate::run_output::models::OutputSettings;
use crate::run_output::tests::test_types::{OutputDefaultTestCase, OutputLocationTestCase};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

/// A filesystem root, so a case's `/`-led path is absolute on this platform too.
#[cfg(windows)]
const ROOT: &str = "C:\\";
#[cfg(not(windows))]
const ROOT: &str = "/";

/// The case's path, with a leading `/` read as this platform's root.
fn platform_path(path: &str) -> std::path::PathBuf {
    path.strip_prefix('/').map_or_else(
        || std::path::PathBuf::from(path),
        |rest| std::path::Path::new(ROOT).join(rest),
    )
}

#[test]
fn given_a_users_folders_when_no_store_is_set_then_the_default_never_depends_on_the_working_directory()
 {
    let test_cases = [
        OutputDefaultTestCase {
            description: "SQLite keeps it beside the control database",
            backend: Backend::Sqlite,
            state_home: Some("/srv/state"),
            home: Some("/home/sqlbuild"),
            expected_folder: Some("output"),
        },
        OutputDefaultTestCase {
            description: "SQLite needs no home",
            backend: Backend::Sqlite,
            state_home: None,
            home: None,
            expected_folder: Some("output"),
        },
        OutputDefaultTestCase {
            description: "PostgreSQL uses XDG_STATE_HOME",
            backend: Backend::Postgres,
            state_home: Some("/srv/state"),
            home: Some("/home/sqlbuild"),
            expected_folder: Some("/srv/state/sqlbuild/output"),
        },
        OutputDefaultTestCase {
            description: "PostgreSQL needs no home beside XDG_STATE_HOME",
            backend: Backend::Postgres,
            state_home: Some("/srv/state"),
            home: None,
            expected_folder: Some("/srv/state/sqlbuild/output"),
        },
        OutputDefaultTestCase {
            description: "PostgreSQL falls back to the state folder in the home",
            backend: Backend::Postgres,
            state_home: None,
            home: Some("/home/sqlbuild"),
            expected_folder: Some("/home/sqlbuild/.local/state/sqlbuild/output"),
        },
        OutputDefaultTestCase {
            description: "an empty XDG_STATE_HOME is ignored",
            backend: Backend::Postgres,
            state_home: Some(""),
            home: Some("/home/sqlbuild"),
            expected_folder: Some("/home/sqlbuild/.local/state/sqlbuild/output"),
        },
        OutputDefaultTestCase {
            description: "a relative XDG_STATE_HOME is ignored",
            backend: Backend::Postgres,
            state_home: Some("state"),
            home: Some("/home/sqlbuild"),
            expected_folder: Some("/home/sqlbuild/.local/state/sqlbuild/output"),
        },
        OutputDefaultTestCase {
            description: "PostgreSQL has no default without a home",
            backend: Backend::Postgres,
            state_home: None,
            home: None,
            expected_folder: None,
        },
        OutputDefaultTestCase {
            description: "a relative home never means the working directory",
            backend: Backend::Postgres,
            state_home: None,
            home: Some("sqlbuild"),
            expected_folder: None,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let beside = fixture
                .directory
                .as_ref()
                .map(|directory| directory.path().canonicalize().expect("store folder"))
                .unwrap_or_default();
            let folders = (
                test_case.state_home.map(platform_path),
                test_case.home.map(platform_path),
            );
            assert_eq!(
                default_folder(&fixture.store, folders).ok(),
                test_case
                    .expected_folder
                    .map(|folder| beside.join(platform_path(folder))),
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_no_output_setting_when_the_store_opens_then_sqlite_reports_an_absolute_folder_beside_it() {
    let test_cases = [OutputLocationTestCase {
        description: "SQLite keeps it beside the control database",
        backend: Backend::Sqlite,
        expected_folder: "output",
    }];
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
            close_test_store(fixture).await;
        });
    }
}
