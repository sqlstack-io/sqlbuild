use crate::env_file::main::load_env_file::load_env_file;
use crate::env_file::tests::helpers::{earlier_load, project, set_pairs, with_mode};
use crate::env_file::tests::test_types::{
    EnvFileFailureTestCase, EnvFileTestCase, EnvironmentWinsTestCase, ReloadTestCase,
    SharedReadTestCase,
};
use sqlbuild_core::process_environment::main::install_environment_overlay::install_environment_overlay;
use sqlbuild_core::process_environment::models::EnvironmentOverlay;

#[test]
fn given_project_and_target_env_files_when_loading_then_the_chosen_file_fills_unset_variables() {
    let test_cases = [
        EnvFileTestCase {
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
        EnvFileTestCase {
            description: "a later duplicate replaces an earlier one",
            project: "",
            files: &[(".env", "SQB_T2_TOKEN=first\nSQB_T2_TOKEN=second\n")],
            target: None,
            expected_display: ".env",
            expected_loaded: true,
            expected_set: &[("SQB_T2_TOKEN", "second")],
        },
        EnvFileTestCase {
            description: "a missing default .env is fine",
            project: "",
            files: &[],
            target: None,
            expected_display: ".env",
            expected_loaded: false,
            expected_set: &[],
        },
        EnvFileTestCase {
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
        EnvFileTestCase {
            description: "a target without env_file keeps the project's",
            project: "env_file = \"project.env\"\n[targets.dev]\nschema = \"dev\"\n",
            files: &[("project.env", "SQB_T4_PROJECT=yes\n")],
            target: Some("dev"),
            expected_display: "project.env",
            expected_loaded: true,
            expected_set: &[("SQB_T4_PROJECT", "yes")],
        },
        EnvFileTestCase {
            description: "env_file = \"\" turns loading off",
            project: "env_file = \"\"\n",
            files: &[(".env", "SQB_T5_TOKEN=abc\n")],
            target: None,
            expected_display: "",
            expected_loaded: false,
            expected_set: &[],
        },
        EnvFileTestCase {
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
    let test_cases = [EnvironmentWinsTestCase {
        description: "PATH stays the environment's; the file fills the rest",
        file: "PATH=/from/file\nSQB_T7_TOKEN=abc\n",
        expected_set: &[("SQB_T7_TOKEN", "abc")],
        expected_names: &["SQB_T7_TOKEN"],
        expected_overridden: &["PATH"],
    }];
    for test_case in test_cases {
        let directory = project("", &[(".env", test_case.file)]);

        let load = load_env_file(directory.path(), None)
            .expect(test_case.description)
            .expect(test_case.description);

        let expected: Vec<(String, String)> = test_case
            .expected_set
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        assert_eq!(set_pairs(&load), expected, "{}", test_case.description);
        assert_eq!(
            load.marker.names, test_case.expected_names,
            "{}",
            test_case.description
        );
        assert_eq!(
            load.marker.overridden, test_case.expected_overridden,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_an_env_file_that_cannot_load_when_loading_then_the_error_names_it_but_never_a_value() {
    let test_cases = [
        EnvFileFailureTestCase {
            description: "a configured env file that does not exist",
            project: "[targets.uat]\nenv_file = \"missing/uat.env\"\n",
            files: &[],
            target: Some("uat"),
            expected_message: "env file missing/uat.env does not exist (set by [targets.uat] env_file in sqlbuild_project.toml)",
        },
        EnvFileFailureTestCase {
            description: "an invalid line, reported by its number only",
            project: "",
            files: &[(".env", "SQB_T8_OK=1\n\nnot a pair s3cr3t\n")],
            target: None,
            expected_message: "env file .env line 3 is not KEY=value (set by the default env_file = \".env\")",
        },
    ];
    for test_case in test_cases {
        let directory = project(test_case.project, test_case.files);

        let error =
            load_env_file(directory.path(), test_case.target).expect_err(test_case.description);

        assert_eq!(
            error.message(),
            test_case.expected_message,
            "{}",
            test_case.description
        );
    }
}

#[cfg(unix)]
#[test]
fn given_env_file_permissions_when_loading_then_a_file_others_can_read_warns() {
    let test_cases = [
        SharedReadTestCase {
            description: "readable by group and others",
            mode: 0o644,
            expected_warning: Some(
                "env file .env is readable by group or others (mode 644); restrict it with chmod 600 .env",
            ),
        },
        SharedReadTestCase {
            description: "readable by its owner only",
            mode: 0o600,
            expected_warning: None,
        },
    ];
    for test_case in test_cases {
        let directory = project("", &[(".env", "SQB_T9_TOKEN=abc\n")]);
        with_mode(&directory.path().join(".env"), test_case.mode);

        let load = load_env_file(directory.path(), None)
            .expect(test_case.description)
            .expect(test_case.description);

        assert_eq!(
            load.warning.as_deref(),
            test_case.expected_warning,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_variables_an_earlier_load_took_when_loading_again_then_the_file_replaces_or_unsets_them() {
    let test_cases = [ReloadTestCase {
        description: "a value the earlier load took is replaced, and one the file dropped is unset",
        file: "SQB_T10_KEPT=new\n",
        earlier: &[("SQB_T10_KEPT", "old"), ("SQB_T10_DROPPED", "old")],
        expected_set: &[("SQB_T10_KEPT", "new")],
        expected_remove: &["SQB_T10_DROPPED"],
    }];
    for test_case in test_cases {
        let directory = project("", &[(".env", test_case.file)]);
        install_environment_overlay(earlier_load(directory.path(), test_case.earlier));

        let reloaded = load_env_file(directory.path(), None);
        install_environment_overlay(EnvironmentOverlay::default());
        let reloaded = reloaded
            .expect(test_case.description)
            .expect(test_case.description);

        let expected: Vec<(String, String)> = test_case
            .expected_set
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        assert_eq!(set_pairs(&reloaded), expected, "{}", test_case.description);
        let removed: Vec<String> = reloaded
            .overlay
            .remove
            .iter()
            .map(|name| name.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            removed, test_case.expected_remove,
            "{}",
            test_case.description
        );
    }
}
