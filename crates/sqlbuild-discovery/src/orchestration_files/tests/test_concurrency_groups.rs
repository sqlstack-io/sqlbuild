use std::collections::BTreeMap;

use crate::orchestration_files::main::check_concurrency_groups::check_concurrency_groups;
use crate::orchestration_files::tests::helpers::{
    declaration_failure, inventory, project, with_file,
};
use crate::orchestration_files::tests::test_types::ConcurrencyGroupCheckTestCase;

const WINDOW_JOB: &str = "name = \"orders_export_job\"\ndescription = \"d\"\nselect = \"orders\"\nconcurrency_groups = [\"nightly_window\"]\n";
const WINDOW_GROUP: &str = "name = \"orders_nightly_job_group\"\ndescription = \"d\"\njobs = [\"orders_export_job\"]\nconcurrency_groups = [\"nightly_windw\"]\n";

#[test]
fn given_jobs_naming_concurrency_groups_when_checking_then_an_undeclared_group_fails_at_its_job() {
    let test_cases = [
        ConcurrencyGroupCheckTestCase {
            description: "a declared group",
            files: with_file("orchestration/jobs/orders/export.toml", WINDOW_JOB),
            declared: &[("nightly_window", 1)],
            expected_failure: None,
        },
        ConcurrencyGroupCheckTestCase {
            description: "a project declaring no groups",
            files: with_file("orchestration/jobs/orders/export.toml", WINDOW_JOB),
            declared: &[],
            expected_failure: Some((
                "orchestration/jobs/orders/export.toml:1:8: job 'orders_export_job' names concurrency group 'nightly_window', which [concurrency_groups] in sqlbuild_project.toml does not declare",
                "the project declares no concurrency groups; declare it with its most runs at once, such as nightly_window = 1",
            )),
        },
        ConcurrencyGroupCheckTestCase {
            description: "a job group's misspelt group, with the declared names",
            files: with_file("orchestration/job_groups/orders/nightly.toml", WINDOW_GROUP),
            declared: &[("nightly_window", 1), ("warehouse", 2)],
            expected_failure: Some((
                "orchestration/job_groups/orders/nightly.toml:1:8: job group 'orders_nightly_job_group' names concurrency group 'nightly_windw', which [concurrency_groups] in sqlbuild_project.toml does not declare",
                "declared groups: nightly_window, warehouse; or declare it with its most runs at once, such as nightly_windw = 1",
            )),
        },
    ];
    for test_case in test_cases {
        let directory = project(&test_case.files);
        let declared: BTreeMap<String, u32> = test_case
            .declared
            .iter()
            .map(|(name, limit)| ((*name).to_owned(), *limit))
            .collect();

        let checked = check_concurrency_groups(
            &inventory(directory.path()).expect(test_case.description),
            &declared,
        );

        assert_eq!(
            checked.err(),
            test_case
                .expected_failure
                .map(|(message, help)| declaration_failure(message, Some(help))),
            "{}",
            test_case.description
        );
    }
}
