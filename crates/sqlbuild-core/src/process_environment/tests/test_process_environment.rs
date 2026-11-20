use std::ffi::OsString;

use crate::process_environment::constants::ENV_FILE_MARKER_VARIABLE;
use crate::process_environment::main::env_file_marker::env_file_marker;
use crate::process_environment::main::environment_var_os::environment_var_os;
use crate::process_environment::main::environment_vars_os::environment_vars_os;
use crate::process_environment::main::environment_without_env_file::environment_without_env_file;
use crate::process_environment::main::install_environment_overlay::install_environment_overlay;
use crate::process_environment::main::marker_json::marker_json;
use crate::process_environment::models::{EnvFileMarker, EnvironmentOverlay};
use crate::process_environment::tests::test_types::OverlayTestCase;

/// One test: the overlay is process-wide, so parallel tests would see each other's.
#[test]
fn given_an_overlay_when_reading_the_environment_then_it_sets_unsets_and_carries_the_marker() {
    let test_cases = [OverlayTestCase {
        description: "a file variable is set, an inherited one unset, and the marker reads back",
        set: ("SQLBUILD_TEST_FROM_FILE", "file value"),
        removed: "CARGO_MANIFEST_DIR",
        expected_value: "file value",
    }];
    for test_case in test_cases {
        let (name, value) = test_case.set;
        assert!(
            std::env::var_os(test_case.removed).is_some(),
            "{}: cargo sets the variable the overlay removes",
            test_case.description
        );
        let marker = EnvFileMarker {
            project_dir: "/work/shop".into(),
            target: Some("dev".into()),
            path: Some("/work/shop/.env".into()),
            loaded: true,
            names: vec![name.into()],
            overridden: vec!["HOME".into()],
        };
        install_environment_overlay(EnvironmentOverlay {
            set: vec![
                (OsString::from(name), OsString::from(value)),
                (
                    OsString::from(ENV_FILE_MARKER_VARIABLE),
                    OsString::from(marker_json(&marker)),
                ),
            ],
            remove: vec![OsString::from(test_case.removed)],
        });

        let variables: Vec<(OsString, OsString)> = environment_vars_os();
        let read: Option<OsString> = environment_var_os(name);
        let removed: Option<OsString> = environment_var_os(test_case.removed);
        let read_marker: Option<EnvFileMarker> = env_file_marker();
        let for_children: Vec<(OsString, OsString)> = environment_without_env_file();
        install_environment_overlay(EnvironmentOverlay::default());

        assert_eq!(
            read,
            Some(OsString::from(test_case.expected_value)),
            "{}",
            test_case.description
        );
        assert_eq!(removed, None, "{}", test_case.description);
        assert_eq!(read_marker, Some(marker), "{}", test_case.description);
        assert!(
            variables
                .iter()
                .any(|(listed, listed_value)| listed == name && listed_value == value),
            "{}: the effective environment lists set variables",
            test_case.description
        );
        assert!(
            variables
                .iter()
                .all(|(listed, _)| listed != test_case.removed),
            "{}: the effective environment leaves removed variables out",
            test_case.description
        );
        assert!(
            for_children
                .iter()
                .all(|(listed, _)| listed != name && listed != ENV_FILE_MARKER_VARIABLE),
            "{}: children start without the file's variables or its marker",
            test_case.description
        );
        assert_eq!(environment_var_os(name), None, "{}", test_case.description);
        assert_eq!(env_file_marker(), None, "{}", test_case.description);
    }
}
