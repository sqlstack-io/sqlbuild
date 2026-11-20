use crate::project::errors::ConfigFailure;
use crate::project::main::load_changes_config::load_changes_config;
use crate::project::main::load_control_config::load_control_config;
use crate::project::main::load_dynamic_config::load_dynamic_config;
use crate::project::main::load_git_base_ref::load_git_base_ref;
use crate::project::main::load_pool_limits::load_pool_limits;
use crate::project::main::load_review_config::load_review_config;
use crate::project::models::ConfigFile;
use crate::project::models::{ChangesConfig, ControlConfig, DynamicConfig, ReviewConfig};
use crate::project::tests::helpers::{failure_at, first_failure, project_directory};
use crate::project::tests::test_types::{
    ChangesConfigTestCase, ControlConfigTestCase, DynamicConfigErrorTestCase,
    DynamicConfigTestCase, GitBaseRefTestCase, PoolsErrorTestCase, PoolsTestCase,
    ProjectFailureTestCase, ReviewConfigErrorTestCase, ReviewConfigTestCase,
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
            description: "a blank catalog connection is rejected",
            project: "name = 'orders'\nadapter = 'duckdb'\n[targets.dev]\ncatalog_connection = ' '\n",
            local: "",
            expected_failure: Some(ConfigFailure::plain(
                "Expected 'catalog_connection' to be a non-empty string when provided",
            )),
        },
        ProjectFailureTestCase {
            description: "a catalog connection must be a name, not credential overrides",
            project: "name = 'orders'\nadapter = 'duckdb'\n",
            local: "[targets.dev]\ncatalog_connection = { user = 'catalog-reader' }\n",
            expected_failure: Some(ConfigFailure::plain(
                "Expected 'catalog_connection' to be a non-empty string when provided",
            )),
        },
        ProjectFailureTestCase {
            description: "a target may plan from its synced state",
            project: "name = 'orders'\nadapter = 'duckdb'\n[targets.dev]\nplan_from = 'synced_state'\n",
            local: "[targets.dev]\nplan_from = 'live'\n",
            expected_failure: None,
        },
        ProjectFailureTestCase {
            description: "plan_from names only live or synced_state",
            project: "name = 'orders'\nadapter = 'duckdb'\n[targets.dev]\nplan_from = 'cache'\n",
            local: "",
            expected_failure: Some(ConfigFailure::plain(
                "Expected 'plan_from' to be one of: live, synced_state",
            )),
        },
        ProjectFailureTestCase {
            description: "a local target's plan_from names only live or synced_state",
            project: "name = 'orders'\nadapter = 'duckdb'\n",
            local: "[targets.dev]\nplan_from = 'cache'\n",
            expected_failure: Some(ConfigFailure::plain(
                "Expected 'plan_from' to be one of: live, synced_state",
            )),
        },
        ProjectFailureTestCase {
            description: "a target may keep its dynamic models paused between builds",
            project: "name = 'orders'\nadapter = 'snowflake'\n[targets.dev]\npause_dynamic = true\n",
            local: "[targets.dev]\npause_dynamic = false\n",
            expected_failure: None,
        },
        ProjectFailureTestCase {
            description: "pause_dynamic is a boolean",
            project: "name = 'orders'\nadapter = 'snowflake'\n[targets.dev]\npause_dynamic = 'yes'\n",
            local: "",
            expected_failure: Some(ConfigFailure::plain(
                "Expected 'pause_dynamic' to be a boolean when provided",
            )),
        },
        ProjectFailureTestCase {
            description: "an invalid dynamic section fails the project load",
            project: "name = 'orders'\nadapter = 'snowflake'\n[dynamic]\nheartbeat_grace = 'soon'\n",
            local: "",
            expected_failure: Some(ConfigFailure::at(
                ConfigFile::Project,
                "dynamic.heartbeat_grace must be a fixed duration such as 30m or 2h",
            )),
        },
        ProjectFailureTestCase {
            description: "a grace no longer than the heartbeat interval would expire every build",
            project: "name = 'orders'\nadapter = 'snowflake'\n[dynamic]\nheartbeat_grace = '1m'\n",
            local: "",
            expected_failure: Some(ConfigFailure::at(
                ConfigFile::Project,
                "dynamic.heartbeat_grace must be longer than 1m: a build writes a heartbeat every minute, so a shorter grace expires builds that are still running",
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
            description: "a local file cannot allow warehouse round trips",
            project: PROJECT,
            local: "[settings]\nallow_warehouse_round_trips = true\n",
            expected_failure: failure_at(
                ConfigFile::Local,
                "cannot set [settings] allow_warehouse_round_trips; it is a shared project policy \
                 that only sqlbuild_project.toml can set",
                Some(
                    "remove allow_warehouse_round_trips from sqlbuild_local.toml\n  = help: to \
                     change the policy for everyone, set this in sqlbuild_project.toml:\n            \
                     [settings]\n\x20           allow_warehouse_round_trips = false",
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

#[test]
fn given_control_section_when_reading_then_returns_its_values_and_interval_seconds() {
    let control = |sync: &str| ControlConfig {
        server: Some("https://sqlbuild.example.com".to_owned()),
        location: Some("orders-warehouse".to_owned()),
        sync_at_least_every_seconds: sync.parse().ok(),
    };
    let test_cases = [
        ControlConfigTestCase {
            description: "every control key with a compound interval",
            project: "name = 'orders'\nadapter = 'duckdb'\n[control]\n\
                      server = 'https://sqlbuild.example.com'\nlocation = 'orders-warehouse'\n\
                      sync_at_least_every = '1h30m'\n",
            expected_config: control("5400"),
        },
        ControlConfigTestCase {
            description: "exactly the one-minute minimum",
            project: "name = 'orders'\nadapter = 'duckdb'\n[control]\n\
                      server = 'https://sqlbuild.example.com'\nlocation = 'orders-warehouse'\n\
                      sync_at_least_every = '60s'\n",
            expected_config: control("60"),
        },
        ControlConfigTestCase {
            description: "off turns safety syncs off",
            project: "name = 'orders'\nadapter = 'duckdb'\n[control]\n\
                      server = 'https://sqlbuild.example.com'\nlocation = 'orders-warehouse'\n\
                      sync_at_least_every = 'off'\n",
            expected_config: control("0"),
        },
        ControlConfigTestCase {
            description: "a bare zero turns safety syncs off",
            project: "name = 'orders'\nadapter = 'duckdb'\n[control]\n\
                      server = 'https://sqlbuild.example.com'\nlocation = 'orders-warehouse'\n\
                      sync_at_least_every = '0'\n",
            expected_config: control("0"),
        },
        ControlConfigTestCase {
            description: "no control section",
            project: PROJECT,
            expected_config: ControlConfig::default(),
        },
    ];
    for test_case in test_cases {
        let (_directory, paths) = project_directory(test_case.project, "");

        let config: ControlConfig = load_control_config(&paths).expect(test_case.description);

        assert_eq!(
            config, test_case.expected_config,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_invalid_control_section_when_loading_then_project_loading_rejects_it() {
    let interval = "control.sync_at_least_every must be a duration of at least '1m'";
    let test_cases = [
        ReviewConfigErrorTestCase {
            description: "an interval one second under the minimum",
            project: "name = 'orders'\nadapter = 'duckdb'\n[control]\nsync_at_least_every = '59s'\n",
            expected_fragment: interval,
        },
        ReviewConfigErrorTestCase {
            description: "a negative interval",
            project: "name = 'orders'\nadapter = 'duckdb'\n[control]\nsync_at_least_every = '-30m'\n",
            expected_fragment: interval,
        },
        ReviewConfigErrorTestCase {
            description: "an interval that is not a duration",
            project: "name = 'orders'\nadapter = 'duckdb'\n[control]\nsync_at_least_every = 'hourly'\n",
            expected_fragment: interval,
        },
        ReviewConfigErrorTestCase {
            description: "a calendar interval",
            project: "name = 'orders'\nadapter = 'duckdb'\n[control]\nsync_at_least_every = '1mo'\n",
            expected_fragment: interval,
        },
        ReviewConfigErrorTestCase {
            description: "an integer interval",
            project: "name = 'orders'\nadapter = 'duckdb'\n[control]\nsync_at_least_every = 1800\n",
            expected_fragment: interval,
        },
        ReviewConfigErrorTestCase {
            description: "a misspelt control key",
            project: "name = 'orders'\nadapter = 'duckdb'\n[control]\nlocaton = 'orders-warehouse'\n",
            expected_fragment: "control contains unknown key(s): locaton",
        },
    ];
    for test_case in test_cases {
        let (_directory, paths) = project_directory(test_case.project, "");

        let failures: [String; 2] = [
            format!(
                "{:?}",
                load_control_config(&paths).expect_err(test_case.description)
            ),
            format!("{:?}", first_failure(&paths).expect(test_case.description)),
        ];

        for failure in failures {
            assert!(
                failure.contains(test_case.expected_fragment),
                "{}: {failure}",
                test_case.description
            );
        }
    }
}

#[test]
fn given_changes_section_when_reading_then_returns_counts_and_reach() {
    let test_cases = [
        ChangesConfigTestCase {
            description: "both kinds of change and every node downstream",
            project: "name = 'orders'\nadapter = 'duckdb'\n[changes]\n\
                      counts = ['definition', 'source_data']\nreach = 'all'\n",
            expected_config: ChangesConfig {
                counts: Some(vec!["definition".to_owned(), "source_data".to_owned()]),
                reach: Some("all".to_owned()),
            },
        },
        ChangesConfigTestCase {
            description: "only new source data, reach left to the default",
            project: "name = 'orders'\nadapter = 'duckdb'\n[changes]\ncounts = ['source_data']\n",
            expected_config: ChangesConfig {
                counts: Some(vec!["source_data".to_owned()]),
                reach: None,
            },
        },
        ChangesConfigTestCase {
            description: "no changes section",
            project: PROJECT,
            expected_config: ChangesConfig::default(),
        },
    ];
    for test_case in test_cases {
        let (_directory, paths) = project_directory(test_case.project, "");

        let config: ChangesConfig = load_changes_config(&paths).expect(test_case.description);

        assert_eq!(
            config, test_case.expected_config,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_invalid_changes_section_when_loading_then_project_loading_rejects_it() {
    let counts = "changes.counts must list one or more of: definition, source_data";
    let test_cases = [
        ReviewConfigErrorTestCase {
            description: "an empty counts list",
            project: "name = 'orders'\nadapter = 'duckdb'\n[changes]\ncounts = []\n",
            expected_fragment: counts,
        },
        ReviewConfigErrorTestCase {
            description: "an unknown kind of change",
            project: "name = 'orders'\nadapter = 'duckdb'\n[changes]\ncounts = ['schedule']\n",
            expected_fragment: counts,
        },
        ReviewConfigErrorTestCase {
            description: "counts as one string",
            project: "name = 'orders'\nadapter = 'duckdb'\n[changes]\ncounts = 'definition'\n",
            expected_fragment: "changes.counts must be a list of strings",
        },
        ReviewConfigErrorTestCase {
            description: "an unknown reach",
            project: "name = 'orders'\nadapter = 'duckdb'\n[changes]\nreach = 'group'\n",
            expected_fragment: "changes.reach must be one of: job, all",
        },
        ReviewConfigErrorTestCase {
            description: "a misspelt changes key",
            project: "name = 'orders'\nadapter = 'duckdb'\n[changes]\ncount = ['definition']\n",
            expected_fragment: "changes contains unknown key(s): count",
        },
    ];
    for test_case in test_cases {
        let (_directory, paths) = project_directory(test_case.project, "");

        let failures: [String; 2] = [
            format!(
                "{:?}",
                load_changes_config(&paths).expect_err(test_case.description)
            ),
            format!("{:?}", first_failure(&paths).expect(test_case.description)),
        ];

        for failure in failures {
            assert!(
                failure.contains(test_case.expected_fragment),
                "{}: {failure}",
                test_case.description
            );
        }
    }
}

#[test]
fn given_pools_when_reading_for_a_target_then_its_overrides_replace_the_project_limits() {
    let project = "name = 'orders'\nadapter = 'duckdb'\n[pools]\nsupplier_api = 1\n\
                   warehouse = 4\n[targets.prod.pools]\nwarehouse = 2\n[targets.dev]\n\
                   schema = 'dev'\n";
    let test_cases = [
        PoolsTestCase {
            description: "the declared pools, for no target",
            project,
            target: None,
            expected_pools: &[("supplier_api", 1), ("warehouse", 4)],
        },
        PoolsTestCase {
            description: "a target's override",
            project,
            target: Some("prod"),
            expected_pools: &[("supplier_api", 1), ("warehouse", 2)],
        },
        PoolsTestCase {
            description: "a target without overrides",
            project,
            target: Some("dev"),
            expected_pools: &[("supplier_api", 1), ("warehouse", 4)],
        },
        PoolsTestCase {
            description: "a project without pools",
            project: PROJECT,
            target: Some("dev"),
            expected_pools: &[],
        },
    ];
    for test_case in test_cases {
        let (_directory, paths) = project_directory(test_case.project, "");

        let limits = load_pool_limits(&paths).expect(test_case.description);
        let pools = test_case
            .target
            .map_or(&limits.declared, |target| limits.on(target));

        assert_eq!(
            pools
                .iter()
                .map(|(name, limit)| (name.as_str(), *limit))
                .collect::<Vec<_>>(),
            test_case.expected_pools,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_invalid_pools_when_loading_then_project_loading_rejects_them() {
    let test_cases = [
        PoolsErrorTestCase {
            description: "a limit of zero",
            project: "name = 'orders'\nadapter = 'duckdb'\n[pools]\nnightly = 0\n",
            local: "",
            expected_fragment: "pools.nightly must be the pool's limit, a whole number of slots from 1 to 10000",
        },
        PoolsErrorTestCase {
            description: "a limit that is not a whole number",
            project: "name = 'orders'\nadapter = 'duckdb'\n[pools]\nnightly = 'one'\n",
            local: "",
            expected_fragment: "pools.nightly must be the pool's limit",
        },
        PoolsErrorTestCase {
            description: "a target overriding a group the project does not declare",
            project: "name = 'orders'\nadapter = 'duckdb'\n[pools]\nnightly = 1\n\
                      [targets.prod.pools]\nnightly_windw = 1\n",
            local: "",
            expected_fragment: "targets.prod.pools.nightly_windw changes a pool [pools] does not declare",
        },
        PoolsErrorTestCase {
            description: "a local file setting a project rule",
            project: "name = 'orders'\nadapter = 'duckdb'\n[pools]\nnightly = 1\n",
            local: "[targets.prod.pools]\nnightly = 2\n",
            expected_fragment: "targets.prod.pools is project policy; set it in \
                                sqlbuild_project.toml",
        },
    ];
    for test_case in test_cases {
        let (_directory, paths) = project_directory(test_case.project, test_case.local);

        let failure = format!("{:?}", first_failure(&paths).expect(test_case.description));

        assert!(
            failure.contains(test_case.expected_fragment),
            "{}: {failure}",
            test_case.description
        );
    }
}

#[test]
fn given_dynamic_section_when_reading_then_returns_its_values_or_defaults() {
    let test_cases = [
        DynamicConfigTestCase {
            description: "a longer grace that may be ignored",
            project: "name = 'orders'\nadapter = 'snowflake'\n[dynamic]\n\
                      heartbeat_grace = '45m'\nallow_ignore_grace = true\n",
            expected_config: DynamicConfig {
                heartbeat_grace_seconds: 2_700,
                allow_ignore_grace: true,
            },
        },
        DynamicConfigTestCase {
            description: "any grace longer than the heartbeat interval",
            project: "name = 'orders'\nadapter = 'snowflake'\n[dynamic]\n\
                      heartbeat_grace = '61s'\n",
            expected_config: DynamicConfig {
                heartbeat_grace_seconds: 61,
                allow_ignore_grace: false,
            },
        },
        DynamicConfigTestCase {
            description: "no dynamic section",
            project: PROJECT,
            expected_config: DynamicConfig {
                heartbeat_grace_seconds: 1_800,
                allow_ignore_grace: false,
            },
        },
    ];
    for test_case in test_cases {
        let (_directory, paths) = project_directory(test_case.project, "");

        let config: DynamicConfig = load_dynamic_config(&paths).expect(test_case.description);

        assert_eq!(
            config, test_case.expected_config,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_invalid_dynamic_section_when_reading_then_it_is_rejected() {
    let test_cases = [
        DynamicConfigErrorTestCase {
            description: "a grace that is not a duration",
            project: "name = 'orders'\nadapter = 'snowflake'\n[dynamic]\n\
                      heartbeat_grace = 'soon'\n",
            expected_fragment: "dynamic.heartbeat_grace must be a fixed duration such as 30m or 2h",
        },
        DynamicConfigErrorTestCase {
            description: "a grace in months",
            project: "name = 'orders'\nadapter = 'snowflake'\n[dynamic]\n\
                      heartbeat_grace = '1mo'\n",
            expected_fragment: "dynamic.heartbeat_grace must be a fixed duration such as 30m or 2h",
        },
        DynamicConfigErrorTestCase {
            description: "allow_ignore_grace written as text",
            project: "name = 'orders'\nadapter = 'snowflake'\n[dynamic]\n\
                      allow_ignore_grace = 'yes'\n",
            expected_fragment: "dynamic.allow_ignore_grace must be true or false",
        },
        DynamicConfigErrorTestCase {
            description: "a misspelt dynamic key",
            project: "name = 'orders'\nadapter = 'snowflake'\n[dynamic]\n\
                      heartbeat_grase = '30m'\n",
            expected_fragment: "dynamic contains unknown key(s): heartbeat_grase",
        },
    ];
    for test_case in test_cases {
        let (_directory, paths) = project_directory(test_case.project, "");

        let failure: String = format!(
            "{:?}",
            load_dynamic_config(&paths).expect_err(test_case.description)
        );

        assert!(
            failure.contains(test_case.expected_fragment),
            "{}: {failure}",
            test_case.description
        );
    }
}
