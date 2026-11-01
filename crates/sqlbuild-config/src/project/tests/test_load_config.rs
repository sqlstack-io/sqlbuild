use crate::project::errors::ConfigFailure;
use crate::project::main::load_git_base_ref::load_git_base_ref;
use crate::project::main::load_review_config::load_review_config;
use crate::project::models::ConfigFile;
use crate::project::models::ReviewConfig;
use crate::project::tests::helpers::{failure_at, first_failure, project_directory};
use crate::project::tests::test_types::{
    GitBaseRefTestCase, ProjectFailureTestCase, ReviewConfigErrorTestCase, ReviewConfigTestCase,
};

const PROJECT: &str = "name = 'orders'\nadapter = 'duckdb'\n";

#[test]
fn given_project_files_when_loading_then_rejections_match_python_loader() {
    let test_cases = [
        ProjectFailureTestCase {
            description: "a valid project and local file load",
            project: "name = 'orders'\nadapter = 'duckdb'\n[settings]\nconcurrency = 4\n\
                      [targets.dev]\nschema = 'analytics'\n[sinks.lifecycle]\nshutdown_timeout = '10m'\n",
            local: "target = 'dev'\n[settings]\nmax_concurrency = 2\n",
            expected_failure: None,
        },
        ProjectFailureTestCase {
            description: "strict takes report-only mode",
            project: "name = 'orders'\nadapter = 'duckdb'\n[settings]\nstrict = 'report'\n",
            local: "",
            expected_failure: None,
        },
        ProjectFailureTestCase {
            description: "strict takes only true, false or report",
            project: "name = 'orders'\nadapter = 'duckdb'\n[settings]\nstrict = 'always'\n",
            local: "",
            expected_failure: Some(ConfigFailure::plain(
                "settings.strict must be true, false or \"report\"",
            )),
        },
        ProjectFailureTestCase {
            description: "an unknown key suggests the nearest supported key",
            project: "name = 'orders'\nadapter = 'duckdb'\n[scopes]\nenforce_placment = true\n",
            local: "",
            expected_failure: failure_at(
                ConfigFile::Project,
                "scopes contains unknown key(s): enforce_placment. Allowed keys: enforce_placement",
                Some("did you mean 'enforce_placement'?"),
            ),
        },
        ProjectFailureTestCase {
            description: "the janitor requires query change tracking to delete tracked relations",
            project: "name = 'orders'\nadapter = 'duckdb'\n[settings]\nquery_change_tracking = false\n\
                      [janitor]\nenabled = true\n",
            local: "",
            expected_failure: failure_at(
                ConfigFile::Project,
                "[janitor] delete_tracked_only = true requires query change tracking; \
                 sqlbuild_project.toml sets [settings] query_change_tracking = false",
                Some(
                    "to track which relations SQLBuild created, set this in sqlbuild_project.toml:\n\
                     \x20           [settings]\n            query_change_tracking = true",
                ),
            ),
        },
        ProjectFailureTestCase {
            description: "a janitor usage window and keep_in_use load",
            project: "name = 'orders'\nadapter = 'duckdb'\n[janitor]\nenabled = true\n\
                      in_use_window = '36h'\nkeep_in_use = true\n",
            local: "",
            expected_failure: None,
        },
        ProjectFailureTestCase {
            description: "a calendar janitor usage window is rejected",
            project: "name = 'orders'\nadapter = 'duckdb'\n[janitor]\nin_use_window = '1mo'\n",
            local: "",
            expected_failure: failure_at(
                ConfigFile::Project,
                "janitor.in_use_window must be a positive fixed duration such as '7d'",
                None,
            ),
        },
        ProjectFailureTestCase {
            description: "a janitor usage window that is not a duration is rejected",
            project: "name = 'orders'\nadapter = 'duckdb'\n[janitor]\nin_use_window = 7\n",
            local: "",
            expected_failure: failure_at(
                ConfigFile::Project,
                "janitor.in_use_window must be a positive fixed duration such as '7d'",
                None,
            ),
        },
        ProjectFailureTestCase {
            description: "a git base ref loads",
            project: "name = 'orders'\nadapter = 'duckdb'\n[git]\nbase_ref = 'origin/main'\n",
            local: "",
            expected_failure: None,
        },
        ProjectFailureTestCase {
            description: "an empty git base ref is rejected",
            project: "name = 'orders'\nadapter = 'duckdb'\n[git]\nbase_ref = ''\n",
            local: "",
            expected_failure: failure_at(
                ConfigFile::Project,
                "git.base_ref must be a non-empty git ref such as 'origin/main'",
                None,
            ),
        },
        ProjectFailureTestCase {
            description: "a misspelt git key suggests base_ref",
            project: "name = 'orders'\nadapter = 'duckdb'\n[git]\nbase_rf = 'origin/main'\n",
            local: "",
            expected_failure: failure_at(
                ConfigFile::Project,
                "git contains unknown key(s): base_rf. Allowed keys: base_ref",
                Some("did you mean 'base_ref'?"),
            ),
        },
        ProjectFailureTestCase {
            description: "a shutdown timeout over ten minutes is rejected",
            project: "name = 'orders'\nadapter = 'duckdb'\n[sinks.lifecycle]\nshutdown_timeout = '601s'\n",
            local: "",
            expected_failure: failure_at(
                ConfigFile::Project,
                "sinks.lifecycle.shutdown_timeout must be a fixed duration from '0s' to '10m' such as '30s'",
                None,
            ),
        },
        ProjectFailureTestCase {
            description: "a local file cannot set the shared SQL analysis policy",
            project: PROJECT,
            local: "[settings]\nrequire_sql_analysis = true\n",
            expected_failure: failure_at(
                ConfigFile::Local,
                "cannot set [settings] require_sql_analysis; it is a shared project policy that \
                 only sqlbuild_project.toml can set",
                Some(
                    "remove require_sql_analysis from sqlbuild_local.toml\n  = help: to change the \
                     policy for everyone, set this in sqlbuild_project.toml:\n            [settings]\n\
                     \x20           require_sql_analysis = false",
                ),
            ),
        },
        ProjectFailureTestCase {
            description: "a path default normalizing to an existing key is rejected",
            project: "name = 'orders'\nadapter = 'duckdb'\n[path_defaults.staging]\n[path_defaults.' staging ']\n",
            local: "",
            expected_failure: failure_at(
                ConfigFile::Project,
                "path_defaults defines both ' staging ' and another key that normalize to \
                 'staging'. Use only the canonical model-relative form without a leading \
                 'models/' prefix.",
                None,
            ),
        },
    ];

    for test_case in test_cases {
        let (_directory, paths) = project_directory(test_case.project, test_case.local);

        let failure = first_failure(&paths);

        assert_eq!(
            failure, test_case.expected_failure,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_git_section_when_reading_base_ref_then_returns_configured_ref() {
    let test_cases = [
        GitBaseRefTestCase {
            description: "a configured base ref",
            project: "name = 'orders'\nadapter = 'duckdb'\n[git]\nbase_ref = 'origin/main'\n",
            expected_base_ref: Some("origin/main"),
        },
        GitBaseRefTestCase {
            description: "no git section",
            project: PROJECT,
            expected_base_ref: None,
        },
    ];
    for test_case in test_cases {
        let (_directory, paths) = project_directory(test_case.project, "");

        let base_ref = load_git_base_ref(&paths).expect(test_case.description);

        assert_eq!(
            base_ref.as_deref(),
            test_case.expected_base_ref,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_review_section_when_reading_then_returns_its_values_or_defaults() {
    let test_cases = [
        ReviewConfigTestCase {
            description: "every review key",
            project: "name = 'orders'\nadapter = 'duckdb'\n[review]\nagainst = 'prod'\n\
                      downstream = 'all'\nschema_prefix = 'CI_REVIEW'\ncritical_tags = ['finance']\n",
            expected_config: ReviewConfig {
                against: Some("prod".to_owned()),
                downstream: Some("all".to_owned()),
                schema_prefix: Some("CI_REVIEW".to_owned()),
                critical_tags: Some(vec!["finance".to_owned()]),
            },
        },
        ReviewConfigTestCase {
            description: "no review section",
            project: PROJECT,
            expected_config: ReviewConfig::default(),
        },
    ];
    for test_case in test_cases {
        let (_directory, paths) = project_directory(test_case.project, "");

        let config: ReviewConfig = load_review_config(&paths).expect(test_case.description);

        assert_eq!(
            config, test_case.expected_config,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_invalid_review_section_when_reading_then_it_is_rejected() {
    let test_cases = [
        ReviewConfigErrorTestCase {
            description: "an unknown downstream policy",
            project: "name = 'orders'\nadapter = 'duckdb'\n[review]\ndownstream = 'some'\n",
            expected_fragment: "review.downstream must be one of: critical, all, none",
        },
        ReviewConfigErrorTestCase {
            description: "a schema prefix that is not an identifier",
            project: "name = 'orders'\nadapter = 'duckdb'\n[review]\nschema_prefix = 'review-ci'\n",
            expected_fragment: "review.schema_prefix must start with a letter",
        },
        ReviewConfigErrorTestCase {
            description: "a misspelt review key",
            project: "name = 'orders'\nadapter = 'duckdb'\n[review]\nagaints = 'prod'\n",
            expected_fragment: "review contains unknown key(s): againts",
        },
    ];
    for test_case in test_cases {
        let (_directory, paths) = project_directory(test_case.project, "");

        let failure: String = format!(
            "{:?}",
            load_review_config(&paths).expect_err(test_case.description)
        );

        assert!(
            failure.contains(test_case.expected_fragment),
            "{}: {failure}",
            test_case.description
        );
    }
}
