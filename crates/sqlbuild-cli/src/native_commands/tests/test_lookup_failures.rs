//! A location that checks in but can't get its next generation number reports a failed load at the third failure in a row, once per streak.

use sqlbuild_orchestration::platform::models::LocationHealth;

use crate::native_commands::_helpers::location::lookup_failure::lookup_failure_health;
use crate::native_commands::models::GenerationLookups;
use crate::native_commands::tests::helpers::{answered, failed};
use crate::native_commands::tests::test_types::{
    LookupFailureHealthTestCase, LookupFailuresTestCase,
};

#[test]
fn given_failed_generation_lookups_when_counted_then_the_third_in_a_row_is_reported_once() {
    let test_cases = [
        LookupFailuresTestCase {
            description: "two failures report nothing",
            lookups: &[failed, failed],
            expected_reports: &[false, false],
        },
        LookupFailuresTestCase {
            description: "the third failure in a row reports, later ones in the streak don't",
            lookups: &[failed, failed, failed, failed, failed],
            expected_reports: &[false, false, true, false, false],
        },
        LookupFailuresTestCase {
            description: "an answered lookup ends the streak",
            lookups: &[failed, failed, answered, failed, failed, failed],
            expected_reports: &[false, false, false, false, false, true],
        },
    ];
    for test_case in test_cases {
        let mut lookups = GenerationLookups::default();
        let reports: Vec<bool> = test_case
            .lookups
            .iter()
            .map(|lookup| lookup(&mut lookups))
            .collect();
        assert_eq!(
            reports, test_case.expected_reports,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_reported_lookup_failure_when_built_then_it_is_a_failed_load_of_the_next_generation() {
    let test_cases = [
        LookupFailureHealthTestCase {
            description: "after generation 4, the location can't load 5",
            answered: Some(4),
            expected_generation: 5,
        },
        LookupFailureHealthTestCase {
            description: "before any generation, it can't load the first",
            answered: None,
            expected_generation: 1,
        },
    ];
    for test_case in test_cases {
        let mut lookups = GenerationLookups::default();
        test_case
            .answered
            .into_iter()
            .for_each(|generation| lookups.answered(generation));
        assert_eq!(
            lookup_failure_health(lookups, "timed out".into()),
            LocationHealth::CompileFailed {
                generation: test_case.expected_generation,
                rules: None,
                error: Some("timed out".into()),
            },
            "{}: a failed load, which the server reads as falling_back",
            test_case.description
        );
    }
}
