use crate::graph::tests::helpers::{failure, job_selection, names, needs_jobs};
use crate::graph::tests::test_types::{
    JobSelectorErrorTestCase, JobSelectorTestCase, NeedsJobsTestCase,
};

#[test]
fn given_attached_jobs_when_resolving_then_jobs_select_their_own_nodes() {
    let test_cases = [
        JobSelectorTestCase {
            description: "a job selects its selection and the functions it needs",
            select: &["job:nightly"],
            exclude: &[],
            expected_names: &["orders", "tax"],
        },
        JobSelectorTestCase {
            description: "a bare job name needs no prefix",
            select: &["nightly"],
            exclude: &[],
            expected_names: &["orders", "tax"],
        },
        JobSelectorTestCase {
            description: "a job's own exclusion holds",
            select: &["job:marts_without_items"],
            exclude: &[],
            expected_names: &["customer_orders", "orders", "tax"],
        },
        JobSelectorTestCase {
            description: "a group selects the union of its jobs",
            select: &["job:daily_group"],
            exclude: &[],
            expected_names: &["customers", "orders", "tax"],
        },
        JobSelectorTestCase {
            description: "a job intersects with other selectors",
            select: &["job:marts,tag:finance"],
            exclude: &[],
            expected_names: &["order_items", "tax"],
        },
        JobSelectorTestCase {
            description: "a command exclusion narrows a job",
            select: &["job:marts"],
            exclude: &["order_items"],
            expected_names: &["customer_orders", "orders", "tax"],
        },
        JobSelectorTestCase {
            description: "a job can be excluded",
            select: &["job:marts"],
            exclude: &["job:nightly"],
            expected_names: &["customer_orders", "order_items", "tax"],
        },
        JobSelectorTestCase {
            description: "expansion applies to the job's nodes",
            select: &["+job:crm+"],
            exclude: &[],
            expected_names: &["customer_orders", "customers", "tax"],
        },
        JobSelectorTestCase {
            description: "a job pattern matches job names",
            select: &["job:cr?"],
            exclude: &[],
            expected_names: &["customers"],
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            job_selection(true, test_case.select, test_case.exclude),
            Ok(names(test_case.expected_names)),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_job_selectors_when_unresolvable_then_fails_with_message() {
    let test_cases = [
        JobSelectorErrorTestCase {
            description: "jobs not loaded",
            attached: false,
            select: &["job:nightly"],
            expected_error: (
                "S010",
                "selector 'job:nightly' selects a job; jobs cannot be selected here",
                None,
            ),
        },
        JobSelectorErrorTestCase {
            description: "unknown job suggests a job",
            attached: true,
            select: &["job:nightyl"],
            expected_error: (
                "S007",
                "unknown job name 'nightyl'",
                Some("did you mean 'nightly'?"),
            ),
        },
        JobSelectorErrorTestCase {
            description: "unknown bare name suggests a job",
            attached: true,
            select: &["nightyl"],
            expected_error: (
                "S007",
                "unknown selector name 'nightyl'",
                Some("did you mean 'nightly'?"),
            ),
        },
        JobSelectorErrorTestCase {
            description: "a job's own selection failure surfaces",
            attached: true,
            select: &["job:broken"],
            expected_error: ("S007", "unknown selector name 'missing'", None),
        },
        JobSelectorErrorTestCase {
            description: "a job cannot select another job",
            attached: true,
            select: &["job:nested"],
            expected_error: (
                "S010",
                "selector 'job:nightly' selects a job; jobs cannot be selected here",
                None,
            ),
        },
        JobSelectorErrorTestCase {
            description: "a job cannot end a path selector",
            attached: true,
            select: &["orders~nightly"],
            expected_error: (
                "S003",
                "path selector endpoint 'nightly' is a job; path selectors join two nodes",
                None,
            ),
        },
        JobSelectorErrorTestCase {
            description: "a job: prefix cannot start a path selector",
            attached: false,
            select: &["job:nightly~orders"],
            expected_error: (
                "S003",
                "path selector endpoint 'job:nightly' is a job; path selectors join two nodes",
                None,
            ),
        },
        JobSelectorErrorTestCase {
            description: "a group with an unknown member",
            attached: true,
            select: &["job:ghost_group"],
            expected_error: (
                "S007",
                "job group 'ghost_group' names unknown job 'ghost'",
                None,
            ),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            job_selection(test_case.attached, test_case.select, &[]),
            Err(failure(test_case.expected_error)),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_selectors_when_scanning_then_only_job_references_need_jobs() {
    let test_cases = [
        NeedsJobsTestCase {
            description: "job prefix",
            selectors: &["job:nightly"],
            also_known: &[],
            expected_needs_jobs: true,
        },
        NeedsJobsTestCase {
            description: "graph name",
            selectors: &["+orders+"],
            also_known: &[],
            expected_needs_jobs: false,
        },
        NeedsJobsTestCase {
            description: "bare name outside the graph",
            selectors: &["nightly"],
            also_known: &[],
            expected_needs_jobs: true,
        },
        NeedsJobsTestCase {
            description: "job inside an intersection of a later token",
            selectors: &["orders", "tag:daily,job:nightly"],
            also_known: &[],
            expected_needs_jobs: true,
        },
        NeedsJobsTestCase {
            description: "name another selector knows",
            selectors: &["load_orders"],
            also_known: &["load_orders"],
            expected_needs_jobs: false,
        },
        NeedsJobsTestCase {
            description: "patterns, paths and folders",
            selectors: &["night*", "orders~nightly", "path:models/mart", "tag:daily"],
            also_known: &[],
            expected_needs_jobs: false,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            needs_jobs(test_case.selectors, test_case.also_known),
            test_case.expected_needs_jobs,
            "{}",
            test_case.description
        );
    }
}
