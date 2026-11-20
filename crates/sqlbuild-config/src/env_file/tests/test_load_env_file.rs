use std::ffi::OsString;
use std::path::Path;

use sqlbuild_core::process_environment::constants::ENV_FILE_MARKER_VARIABLE;
use sqlbuild_core::process_environment::main::install_environment_overlay::install_environment_overlay;
use sqlbuild_core::process_environment::models::EnvironmentOverlay;
use tempfile::TempDir;

use crate::env_file::main::load_env_file::load_env_file;
use crate::env_file::models::{EnvFileError, EnvFileLoad};
use crate::env_file::tests::test_types::EnvFileCase;

fn project(toml: &str, files: &[(&str, &str)]) -> TempDir {
    let directory = TempDir::new().expect("temporary directory");
    std::fs::write(
        directory.path().join("sqlbuild_project.toml"),
        format!("name = \"shop\"\nadapter = \"duckdb\"\n{toml}"),
    )
    .expect("project file");
    for (path, contents) in files {
        let path = directory.path().join(path);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("directory");
        std::fs::write(&path, contents).expect("env file");
        restrict(&path);
    }
    directory
}

#[cfg(unix)]
fn restrict(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).expect("chmod");
}

#[cfg(not(unix))]
fn restrict(_path: &Path) {}

fn set_pairs(load: &EnvFileLoad) -> Vec<(String, String)> {
    load.overlay
        .set
        .iter()
        .filter(|(name, _)| name != ENV_FILE_MARKER_VARIABLE)
        .map(|(name, value)| {
            (
                name.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            )
        })
        .collect()
}

#[test]
fn given_project_and_target_env_files_when_loading_then_the_chosen_file_fills_unset_variables() {
    let test_cases = [
        EnvFileCase {
            description: "the default .env loads, with quotes, comments and export",
            project: "",
            files: &[(
                ".env",
                "# provider settings\nSQB_T1_TOKEN=abc\nSQB_T1_QUOTED=\"two words\" # note\nexport SQB_T1_EXPORTED='x=y'\n",
            )],
            target: None,
            expected_display: ".env",
            expected_loaded: true,
            expected_set: &[
                ("SQB_T1_TOKEN", "abc"),
                ("SQB_T1_QUOTED", "two words"),
                ("SQB_T1_EXPORTED", "x=y"),
            ],
        },
        EnvFileCase {
            description: "a later duplicate replaces an earlier one",
            project: "",
            files: &[(".env", "SQB_T2_TOKEN=first\nSQB_T2_TOKEN=second\n")],
            target: None,
            expected_display: ".env",
            expected_loaded: true,
            expected_set: &[("SQB_T2_TOKEN", "second")],
        },
        EnvFileCase {
            description: "a missing default .env is fine",
            project: "",
            files: &[],
            target: None,
            expected_display: ".env",
            expected_loaded: false,
            expected_set: &[],
        },
        EnvFileCase {
            description: "the selected target's env_file replaces the project's",
            project: "env_file = \"project.env\"\n[targets.uat]\nenv_file = \"conf/uat.env\"\n",
            files: &[
                ("project.env", "SQB_T3_PROJECT=yes\n"),
                ("conf/uat.env", "SQB_T3_UAT=yes\n"),
            ],
            target: Some("uat"),
            expected_display: "conf/uat.env",
            expected_loaded: true,
            expected_set: &[("SQB_T3_UAT", "yes")],
        },
        EnvFileCase {
            description: "a target without env_file keeps the project's",
            project: "env_file = \"project.env\"\n[targets.dev]\nschema = \"dev\"\n",
            files: &[("project.env", "SQB_T4_PROJECT=yes\n")],
            target: Some("dev"),
            expected_display: "project.env",
            expected_loaded: true,
            expected_set: &[("SQB_T4_PROJECT", "yes")],
        },
        EnvFileCase {
            description: "env_file = \"\" turns loading off",
            project: "env_file = \"\"\n",
            files: &[(".env", "SQB_T5_TOKEN=abc\n")],
            target: None,
            expected_display: "",
            expected_loaded: false,
            expected_set: &[],
        },
        EnvFileCase {
            description: "a target's env_file = \"\" turns off the project's",
            project: "[targets.ci]\nenv_file = \"\"\n",
            files: &[(".env", "SQB_T6_TOKEN=abc\n")],
            target: Some("ci"),
            expected_display: "",
            expected_loaded: false,
            expected_set: &[],
        },
    ];
    for test_case in test_cases {
        let directory = project(test_case.project, test_case.files);
        let load = load_env_file(directory.path(), test_case.target)
            .expect(test_case.description)
            .expect("a project");
        assert_eq!(
            load.choice.display, test_case.expected_display,
            "{}",
            test_case.description
        );
        assert_eq!(
            load.marker.loaded, test_case.expected_loaded,
            "{}",
            test_case.description
        );
        let expected: Vec<(String, String)> = test_case
            .expected_set
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        assert_eq!(set_pairs(&load), expected, "{}", test_case.description);
        assert!(load.warning.is_none(), "{}", test_case.description);
    }
}

#[test]
fn given_a_variable_already_in_the_environment_when_loading_then_the_environment_wins() {
    let directory = project("", &[(".env", "PATH=/from/file\nSQB_T7_TOKEN=abc\n")]);

    let load = load_env_file(directory.path(), None)
        .expect("loads")
        .expect("a project");

    assert_eq!(
        set_pairs(&load),
        vec![("SQB_T7_TOKEN".to_owned(), "abc".to_owned())]
    );
    assert_eq!(load.marker.names, vec!["SQB_T7_TOKEN".to_owned()]);
    assert_eq!(load.marker.overridden, vec!["PATH".to_owned()]);
}

#[test]
fn given_a_configured_env_file_that_does_not_exist_when_loading_then_the_error_names_it() {
    let directory = project("[targets.uat]\nenv_file = \"missing/uat.env\"\n", &[]);

    let error = load_env_file(directory.path(), Some("uat")).expect_err("missing file");

    assert!(matches!(error, EnvFileError::Missing { .. }));
    assert_eq!(
        error.message(),
        "env file missing/uat.env does not exist (set by [targets.uat] env_file in sqlbuild_project.toml)"
    );
}

#[test]
fn given_an_invalid_line_when_loading_then_the_error_gives_its_number_but_never_its_text() {
    let directory = project("", &[(".env", "SQB_T8_OK=1\n\nnot a pair s3cr3t\n")]);

    let error = load_env_file(directory.path(), None).expect_err("invalid line");

    assert!(
        matches!(error, EnvFileError::Invalid { line: 3, .. }),
        "{error:?}"
    );
    assert!(!error.message().contains("s3cr3t"), "{}", error.message());
}

#[cfg(unix)]
#[test]
fn given_an_env_file_others_can_read_when_loading_then_it_warns() {
    use std::os::unix::fs::PermissionsExt;

    let directory = project("", &[(".env", "SQB_T9_TOKEN=abc\n")]);
    let path = directory.path().join(".env");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");

    let load = load_env_file(directory.path(), None)
        .expect("loads")
        .expect("a project");

    assert_eq!(
        load.warning.as_deref(),
        Some(
            "env file .env is readable by group or others (mode 644); restrict it with chmod 600 .env"
        )
    );
}

#[test]
fn given_variables_an_earlier_load_took_when_loading_again_then_the_file_replaces_or_unsets_them() {
    let directory = project("", &[(".env", "SQB_T10_KEPT=new\n")]);
    let first = load_env_file(directory.path(), None)
        .expect("loads")
        .expect("a project");
    let mut earlier: EnvironmentOverlay = first.overlay;
    earlier
        .set
        .retain(|(name, _)| name == ENV_FILE_MARKER_VARIABLE);
    let marker = earlier.set[0].1.to_string_lossy().replace(
        "\"names\":[\"SQB_T10_KEPT\"]",
        "\"names\":[\"SQB_T10_DROPPED\",\"SQB_T10_KEPT\"]",
    );
    install_environment_overlay(EnvironmentOverlay {
        set: vec![
            (OsString::from("SQB_T10_KEPT"), OsString::from("old")),
            (OsString::from("SQB_T10_DROPPED"), OsString::from("old")),
            (
                OsString::from(ENV_FILE_MARKER_VARIABLE),
                OsString::from(marker),
            ),
        ],
        remove: Vec::new(),
    });

    let reloaded = load_env_file(directory.path(), None);
    install_environment_overlay(EnvironmentOverlay::default());
    let reloaded = reloaded.expect("loads").expect("a project");

    assert_eq!(
        set_pairs(&reloaded),
        vec![("SQB_T10_KEPT".to_owned(), "new".to_owned())],
        "a value the earlier load took is replaced, not kept as if the environment set it"
    );
    assert_eq!(
        reloaded.overlay.remove,
        vec![OsString::from("SQB_T10_DROPPED")]
    );
}
