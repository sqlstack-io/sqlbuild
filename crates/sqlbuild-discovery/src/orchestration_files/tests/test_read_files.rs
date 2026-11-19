use std::collections::BTreeMap;

use crate::orchestration_files::_helpers::read_group_file::read_group_file;
use crate::orchestration_files::_helpers::read_job_file::read_job_file;
use crate::orchestration_files::models::{
    DependencyFailure, DependencySettings, GroupDependencies, InitialStatus, Overlap, RetryPolicy,
    ScheduleFile, ScheduleSource, WaitFor,
};
use crate::orchestration_files::tests::helpers::{at, group_members, written};
use crate::orchestration_files::tests::test_types::{
    FileErrorTestCase, GroupDependenciesTestCase, GroupReadTestCase, JobReadTestCase,
};

const DEPENDENCY_GROUP: &str = "name = \"nightly_job_group\"\ndescription = \"d\"\njobs = [\"orders_job\", \"partner_export_job\"]\n\n";

const FULL_JOB: &str = r#"name = "orders_export_job"
description = "Write the daily partner file."
select = "python/orders/partner_export"
exclude = "tag:slow"
targets = ["production"]
owner = "Orders team"
overlap = "skip"
pools = { warehouse = 2, supplier_api = 1 }
schedule = { calendar = "trading_days_calendar", timezone = "Europe/London", default_status = "paused" }

[retry]
attempts = 3

[resources]
memory = "2g"
cpus = 1.5

[settings]
vars = { batch = 23 }
concurrency = 4
"#;

#[test]
fn given_job_files_when_reading_then_types_every_field_and_keeps_positions() {
    let test_cases = [
        JobReadTestCase {
            description: "a job using every key",
            text: FULL_JOB,
            expected_name: written("orders_export_job", 1, 8),
            expected_select: written("python/orders/partner_export", 3, 10),
            expected_exclude: Some("tag:slow"),
            expected_overlap: Overlap::Skip,
            expected_pools: &[("supplier_api", 1), ("warehouse", 2)],
            expected_retry: Some(RetryPolicy {
                attempts: 3,
                delay_seconds: 0,
            }),
            expected_schedule: Some(ScheduleFile {
                source: ScheduleSource::Calendar(written("trading_days_calendar", 9, 25)),
                timezone: written("Europe/London", 9, 61),
                starts_at: None,
                default_status: InitialStatus::Paused,
            }),
            expected_cpus: Some(serde_json::json!(1.5)),
            expected_settings: BTreeMap::from([
                ("concurrency".to_owned(), serde_json::json!(4)),
                ("vars".to_owned(), serde_json::json!({"batch": 23})),
            ]),
        },
        JobReadTestCase {
            description: "a job with only the required keys takes the defaults",
            text: "name = \"a_job\"\ndescription = \"d\"\nselect = \"orders\"\nexclude = \" \"\n",
            expected_name: written("a_job", 1, 8),
            expected_select: written("orders", 3, 10),
            expected_exclude: None,
            expected_overlap: Overlap::Queue,
            expected_pools: &[],
            expected_retry: None,
            expected_schedule: None,
            expected_cpus: None,
            expected_settings: BTreeMap::new(),
        },
        JobReadTestCase {
            description: "a job with a retry table uses the current names",
            text: "name = \"a_job\"\ndescription = \"d\"\nselect = \"orders\"\n\n[retry]\nattempts = 3\ndelay_seconds = 900\n",
            expected_name: written("a_job", 1, 8),
            expected_select: written("orders", 3, 10),
            expected_exclude: None,
            expected_overlap: Overlap::Queue,
            expected_pools: &[],
            expected_retry: Some(RetryPolicy {
                attempts: 3,
                delay_seconds: 900,
            }),
            expected_schedule: None,
            expected_cpus: None,
            expected_settings: BTreeMap::new(),
        },
    ];
    for test_case in test_cases {
        let job = read_job_file(test_case.text).expect(test_case.description);
        assert_eq!(
            job.run.name, test_case.expected_name,
            "{}",
            test_case.description
        );
        assert_eq!(
            job.select, test_case.expected_select,
            "{}",
            test_case.description
        );
        assert_eq!(
            job.exclude.map(|exclude| exclude.value).as_deref(),
            test_case.expected_exclude,
            "{}",
            test_case.description
        );
        assert_eq!(
            job.run.overlap, test_case.expected_overlap,
            "{}",
            test_case.description
        );
        assert_eq!(
            job.run
                .pools
                .iter()
                .map(|(pool, amount)| (pool.as_str(), *amount))
                .collect::<Vec<_>>(),
            test_case.expected_pools,
            "{}",
            test_case.description
        );
        assert_eq!(
            job.run.retry, test_case.expected_retry,
            "{}",
            test_case.description
        );
        assert_eq!(
            job.run.schedule, test_case.expected_schedule,
            "{}",
            test_case.description
        );
        assert_eq!(
            job.resources
                .and_then(|resources| resources.value.get("cpus").cloned()),
            test_case.expected_cpus,
            "{}",
            test_case.description
        );
        assert_eq!(
            job.settings, test_case.expected_settings,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_invalid_job_files_when_reading_then_reports_message_position_and_help() {
    let test_cases = [
        FileErrorTestCase {
            description: "misspelt key",
            text: "name = \"a_job\"\ndescription = \"d\"\nselct = \"orders\"\n",
            expected_message: "unknown job key 'selct'",
            expected_position: at(3, 1),
            expected_help: Some("did you mean 'select'?"),
        },
        FileErrorTestCase {
            description: "top-level concurrency",
            text: "name = \"a_job\"\ndescription = \"d\"\nselect = \"orders\"\nconcurrency = 2\n",
            expected_message: "unknown job key 'concurrency'",
            expected_position: at(4, 1),
            expected_help: Some("move it under [settings] as `concurrency = ...`"),
        },
        FileErrorTestCase {
            description: "the removed on_failure table names its replacement",
            text: "name = \"a_job\"\ndescription = \"d\"\nselect = \"orders\"\n\n[on_failure]\nretries = 2\n",
            expected_message: "unknown job key 'on_failure'",
            expected_position: at(5, 2),
            expected_help: Some(
                "`[on_failure]` is now `[retry]`; `attempts = retries + 1`, \
                 `delay_seconds = retry_delay_seconds`",
            ),
        },
        FileErrorTestCase {
            description: "a pool amount of zero",
            text: "name = \"a_job\"\ndescription = \"d\"\nselect = \"orders\"\npools = { supplier_api = 0 }\n",
            expected_message: "pools.supplier_api must take a whole number of slots from 1 to 10000, such as supplier_api = 1",
            expected_position: at(4, 26),
            expected_help: None,
        },
        FileErrorTestCase {
            description: "a pool written as a list",
            text: "name = \"a_job\"\ndescription = \"d\"\nselect = \"orders\"\npools = [\"supplier_api\"]\n",
            expected_message: "invalid type: sequence, expected a map",
            expected_position: at(4, 9),
            expected_help: None,
        },
        FileErrorTestCase {
            description: "blank select",
            text: "name = \"a_job\"\ndescription = \"d\"\nselect = \"  \"\n",
            expected_message: "select must not be blank",
            expected_position: at(3, 10),
            expected_help: None,
        },
        FileErrorTestCase {
            description: "both cron and calendar",
            text: "name = \"a_job\"\ndescription = \"d\"\nselect = \"orders\"\nschedule = { cron = \"0 2 * * *\", calendar = \"c_calendar\", timezone = \"UTC\" }\n",
            expected_message: "schedule needs exactly one of `cron` or `calendar`",
            expected_position: at(4, 12),
            expected_help: None,
        },
        FileErrorTestCase {
            description: "retry delay beyond a day",
            text: "name = \"a_job\"\ndescription = \"d\"\nselect = \"orders\"\nretry = { attempts = 2, delay_seconds = 86401 }\n",
            expected_message: "retry delay_seconds must be a whole number from 0 to 86400",
            expected_position: at(4, 41),
            expected_help: None,
        },
        FileErrorTestCase {
            description: "attempts out of range",
            text: "name = \"a_job\"\ndescription = \"d\"\nselect = \"orders\"\n[retry]\nattempts = 12\n",
            expected_message: "retry attempts must be a whole number from 2 to 11",
            expected_position: at(5, 12),
            expected_help: None,
        },
        FileErrorTestCase {
            description: "a single attempt never retries",
            text: "name = \"a_job\"\ndescription = \"d\"\nselect = \"orders\"\n[retry]\nattempts = 1\n",
            expected_message: "retry attempts must be a whole number from 2 to 11",
            expected_position: at(5, 12),
            expected_help: None,
        },
        FileErrorTestCase {
            description: "retry without attempts",
            text: "name = \"a_job\"\ndescription = \"d\"\nselect = \"orders\"\nretry = { delay_seconds = 60 }\n",
            expected_message: "retry needs `attempts`",
            expected_position: at(4, 9),
            expected_help: None,
        },
        FileErrorTestCase {
            description: "the removed retry key names fail",
            text: "name = \"a_job\"\ndescription = \"d\"\nselect = \"orders\"\nretry = { retries = 2 }\n",
            expected_message: "unknown field `retries`, expected `attempts` or `delay_seconds`",
            expected_position: at(4, 11),
            expected_help: None,
        },
        FileErrorTestCase {
            description: "unknown retry key",
            text: "name = \"a_job\"\ndescription = \"d\"\nselect = \"orders\"\nretry = { attempts = 1, delay = 5 }\n",
            expected_message: "unknown field `delay`, expected `attempts` or `delay_seconds`",
            expected_position: at(4, 25),
            expected_help: None,
        },
        FileErrorTestCase {
            description: "missing description",
            text: "name = \"a_job\"\nselect = \"orders\"\n",
            expected_message: "missing field `description`",
            expected_position: at(1, 1),
            expected_help: None,
        },
    ];
    for test_case in test_cases {
        let error = read_job_file(test_case.text).expect_err(test_case.description);
        assert_eq!(
            error.message, test_case.expected_message,
            "{}",
            test_case.description
        );
        assert_eq!(
            error.position, test_case.expected_position,
            "{}",
            test_case.description
        );
        assert_eq!(
            error.help.as_deref(),
            test_case.expected_help,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_group_files_when_reading_then_keeps_member_positions_and_refuses_repeats() {
    let test_cases = [
        GroupReadTestCase {
            description: "members keep their positions",
            text: "name = \"nightly_job_group\"\ndescription = \"d\"\njobs = [\"export_job\", \"carrier_job\"]\n",
            expected_members: Ok(vec![
                ("export_job".to_owned(), at(3, 9)),
                ("carrier_job".to_owned(), at(3, 23)),
            ]),
        },
        GroupReadTestCase {
            description: "a repeated member is refused at the repeat",
            text: "name = \"nightly_job_group\"\ndescription = \"d\"\njobs = [\"export_job\", \"export_job\"]\n",
            expected_members: Err(("job 'export_job' is listed twice".to_owned(), at(3, 23))),
        },
        GroupReadTestCase {
            description: "groups do not select",
            text: "name = \"nightly_job_group\"\ndescription = \"d\"\njobs = [\"export_job\"]\nselect = \"orders\"\n",
            expected_members: Err(("unknown job group key 'select'".to_owned(), at(4, 1))),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            group_members(test_case.text),
            test_case.expected_members,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_group_dependencies_when_reading_then_types_defaults_and_member_overrides() {
    let test_cases = [
        GroupDependenciesTestCase {
            description: "no dependencies table keeps today's behaviour",
            dependencies: "",
            expected_dependencies: Ok(GroupDependencies::default()),
        },
        GroupDependenciesTestCase {
            description: "defaults and a member override",
            dependencies: "[dependencies]\nwait_for = \"job\"\non_failure = \"run\"\n\n[dependencies.partner_export_job]\non_failure = \"skip\"\n",
            expected_dependencies: Ok(GroupDependencies {
                defaults: DependencySettings {
                    wait_for: Some(WaitFor::Job),
                    on_failure: Some(DependencyFailure::Run),
                },
                members: [(
                    "partner_export_job".to_owned(),
                    DependencySettings {
                        wait_for: None,
                        on_failure: Some(DependencyFailure::Skip),
                    },
                )]
                .into(),
            }),
        },
        GroupDependenciesTestCase {
            description: "partial is the explicit default",
            dependencies: "dependencies = { on_failure = \"partial\" }\n",
            expected_dependencies: Ok(GroupDependencies {
                defaults: DependencySettings {
                    wait_for: None,
                    on_failure: Some(DependencyFailure::Partial),
                },
                members: [].into(),
            }),
        },
        GroupDependenciesTestCase {
            description: "an override must name a member",
            dependencies: "[dependencies.inventory_job]\non_failure = \"skip\"\n",
            expected_dependencies: Err((
                "dependencies.inventory_job is neither `wait_for`, `on_failure` nor a job in `jobs`",
                at(5, 15),
            )),
        },
        GroupDependenciesTestCase {
            description: "an unknown failure policy",
            dependencies: "[dependencies]\non_failure = \"retry\"\n",
            expected_dependencies: Err((
                "on_failure must be one of \"partial\", \"skip\", \"run\"",
                at(6, 14),
            )),
        },
        GroupDependenciesTestCase {
            description: "an unknown override key",
            dependencies: "[dependencies.orders_job]\nretries = 2\n",
            expected_dependencies: Err((
                "unknown dependencies.orders_job key 'retries'; expected `wait_for` or `on_failure`",
                at(6, 1),
            )),
        },
        GroupDependenciesTestCase {
            description: "waiting node by node is not available yet",
            dependencies: "[dependencies]\nwait_for = \"node\"\n",
            expected_dependencies: Err((
                "wait_for = \"node\" is not supported yet; it is planned, so use \"job\" for now",
                at(6, 12),
            )),
        },
    ];
    for test_case in test_cases {
        let text = format!("{DEPENDENCY_GROUP}{}", test_case.dependencies);
        assert_eq!(
            read_group_file(&text)
                .map(|group| group.dependencies)
                .map_err(|error| (error.message, error.position)),
            test_case
                .expected_dependencies
                .map_err(|(message, position)| (message.to_owned(), position)),
            "{}",
            test_case.description
        );
    }
}
