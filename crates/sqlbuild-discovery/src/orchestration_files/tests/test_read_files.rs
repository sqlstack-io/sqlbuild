use std::collections::BTreeMap;

use crate::orchestration_files::_helpers::read_job_file::read_job_file;
use crate::orchestration_files::models::{
    InitialStatus, OnFailure, Overlap, ScheduleFile, ScheduleSource,
};
use crate::orchestration_files::tests::helpers::{at, group_members, written};
use crate::orchestration_files::tests::test_types::{
    FileErrorTestCase, GroupReadTestCase, JobReadTestCase,
};

const FULL_JOB: &str = r#"name = "orders_export_job"
description = "Write the daily partner file."
select = "python/orders/partner_export"
exclude = "tag:slow"
targets = ["production"]
owner = "Orders team"
overlap = "skip"
concurrency_groups = ["warehouse"]
schedule = { calendar = "trading_days_calendar", timezone = "Europe/London", default_status = "paused" }

[on_failure]
retries = 2

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
            expected_on_failure: Some(OnFailure {
                retries: 2,
                retry_delay_seconds: 0,
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
            expected_on_failure: None,
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
            job.run.on_failure, test_case.expected_on_failure,
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
            description: "retries out of range",
            text: "name = \"a_job\"\ndescription = \"d\"\nselect = \"orders\"\non_failure = { retries = 0 }\n",
            expected_message: "on_failure retries must be a whole number from 1 to 10",
            expected_position: at(4, 14),
            expected_help: None,
        },
        FileErrorTestCase {
            description: "retry delay beyond a day",
            text: "name = \"a_job\"\ndescription = \"d\"\nselect = \"orders\"\non_failure = { retries = 1, retry_delay_seconds = 86401 }\n",
            expected_message: "on_failure retry_delay_seconds must be a whole number from 0 to 86400",
            expected_position: at(4, 14),
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
