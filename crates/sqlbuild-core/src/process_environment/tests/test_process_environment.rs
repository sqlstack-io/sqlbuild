use std::ffi::OsString;

use crate::process_environment::constants::ENV_FILE_MARKER_VARIABLE;
use crate::process_environment::main::env_file_marker::env_file_marker;
use crate::process_environment::main::environment_var_os::environment_var_os;
use crate::process_environment::main::environment_vars_os::environment_vars_os;
use crate::process_environment::main::install_environment_overlay::install_environment_overlay;
use crate::process_environment::main::marker_json::marker_json;
use crate::process_environment::models::{EnvFileMarker, EnvironmentOverlay};

fn os(text: &str) -> OsString {
    OsString::from(text)
}

/// One test: the overlay is process-wide, so parallel tests would see each other's.
#[test]
fn given_an_overlay_when_reading_the_environment_then_it_sets_unsets_and_carries_the_marker() {
    let inherited: Option<(OsString, OsString)> = std::env::vars_os().next();
    let marker = EnvFileMarker {
        project_dir: "/work/shop".into(),
        target: Some("dev".into()),
        path: Some("/work/shop/.env".into()),
        loaded: true,
        names: vec!["SQLBUILD_TEST_FROM_FILE".into()],
        overridden: vec!["HOME".into()],
    };
    let mut remove: Vec<OsString> = Vec::new();
    if let Some((name, _)) = &inherited {
        remove.push(name.clone());
    }
    install_environment_overlay(EnvironmentOverlay {
        set: vec![
            (os("SQLBUILD_TEST_FROM_FILE"), os("file value")),
            (os(ENV_FILE_MARKER_VARIABLE), os(&marker_json(&marker))),
        ],
        remove,
    });

    assert_eq!(
        environment_var_os("SQLBUILD_TEST_FROM_FILE"),
        Some(os("file value")),
        "a set variable is read from the overlay"
    );
    assert_eq!(
        env_file_marker(),
        Some(marker),
        "the marker reads back as written"
    );
    let variables: Vec<(OsString, OsString)> = environment_vars_os();
    assert!(
        variables
            .iter()
            .any(|(name, value)| name == "SQLBUILD_TEST_FROM_FILE" && value == "file value"),
        "the effective environment lists set variables"
    );
    if let Some((name, _)) = inherited {
        assert_eq!(
            environment_var_os(&name),
            None,
            "a removed variable is unset"
        );
        assert!(
            !variables.iter().any(|(listed, _)| *listed == name),
            "the effective environment leaves removed variables out"
        );
    }

    install_environment_overlay(EnvironmentOverlay::default());
    assert_eq!(environment_var_os("SQLBUILD_TEST_FROM_FILE"), None);
    assert_eq!(env_file_marker(), None, "no marker without a loaded file");
}
