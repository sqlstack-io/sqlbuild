use crate::orchestration_files::main::check_pools::check_pools;
use crate::orchestration_files::tests::helpers::{
    declaration_failure, inventory, limits, project, with_file,
};
use crate::orchestration_files::tests::test_types::PoolCheckTestCase;

const SUPPLIER_JOB: &str = "name = \"orders_export_job\"\ndescription = \"d\"\nselect = \"orders\"\npools = { supplier_api = 2 }\n";
const PRODUCTION_JOB: &str = "name = \"orders_export_job\"\ndescription = \"d\"\nselect = \"orders\"\ntargets = [\"production\"]\npools = { supplier_api = 2 }\n";
const MISSPELT_GROUP: &str = "name = \"orders_nightly_job_group\"\ndescription = \"d\"\njobs = [\"orders_export_job\"]\npools = { supplier_apy = 1 }\n";

#[test]
fn given_jobs_taking_pools_when_checking_then_an_undeclared_pool_or_an_amount_over_its_limit_fails()
{
    let test_cases = [
        PoolCheckTestCase {
            description: "a declared pool with room for the amount on every target",
            files: with_file("orchestration/jobs/orders/export.toml", SUPPLIER_JOB),
            limits: limits(
                &[("supplier_api", 2)],
                &[("development", &[("supplier_api", 2)])],
            ),
            expected_failure: None,
        },
        PoolCheckTestCase {
            description: "a project declaring no pools",
            files: with_file("orchestration/jobs/orders/export.toml", SUPPLIER_JOB),
            limits: limits(&[], &[]),
            expected_failure: Some((
                "orchestration/jobs/orders/export.toml:1:8: job 'orders_export_job' uses pool 'supplier_api', which [pools] in sqlbuild_project.toml does not declare",
                "the project declares no pools; declare it with its limit, such as supplier_api = 1",
            )),
        },
        PoolCheckTestCase {
            description: "a job group's misspelt pool, with the declared names",
            files: with_file(
                "orchestration/job_groups/orders/nightly.toml",
                MISSPELT_GROUP,
            ),
            limits: limits(&[("supplier_api", 1), ("warehouse", 2)], &[]),
            expected_failure: Some((
                "orchestration/job_groups/orders/nightly.toml:1:8: job group 'orders_nightly_job_group' uses pool 'supplier_apy', which [pools] in sqlbuild_project.toml does not declare",
                "declared pools: supplier_api, warehouse; or declare it with its limit, such as supplier_apy = 1",
            )),
        },
        PoolCheckTestCase {
            description: "an amount over the project's limit",
            files: with_file("orchestration/jobs/orders/export.toml", SUPPLIER_JOB),
            limits: limits(&[("supplier_api", 1)], &[]),
            expected_failure: Some((
                "orchestration/jobs/orders/export.toml:1:8: job 'orders_export_job' takes 2 slots of pool 'supplier_api', more than its limit of 1",
                "take at most 1, or raise the limit in [pools]",
            )),
        },
        PoolCheckTestCase {
            description: "an amount over one target's lowered limit",
            files: with_file("orchestration/jobs/orders/export.toml", PRODUCTION_JOB),
            limits: limits(
                &[("supplier_api", 4)],
                &[
                    ("development", &[("supplier_api", 1)]),
                    ("production", &[("supplier_api", 1)]),
                ],
            ),
            expected_failure: Some((
                "orchestration/jobs/orders/export.toml:1:8: job 'orders_export_job' takes 2 slots of pool 'supplier_api', more than its limit of 1 on target 'production'",
                "take at most 1, or raise the limit in [targets.production.pools]",
            )),
        },
        PoolCheckTestCase {
            description: "a job limited to a target that has room ignores another target's lower limit",
            files: with_file("orchestration/jobs/orders/export.toml", PRODUCTION_JOB),
            limits: limits(
                &[("supplier_api", 2)],
                &[("development", &[("supplier_api", 1)])],
            ),
            expected_failure: None,
        },
    ];
    for test_case in test_cases {
        let directory = project(&test_case.files);

        let checked = check_pools(
            &inventory(directory.path()).expect(test_case.description),
            &test_case.limits,
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
