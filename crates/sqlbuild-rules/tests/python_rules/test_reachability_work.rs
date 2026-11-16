use serde_json::json;

use crate::helpers::{counted_codes, importing_modules};
use crate::test_types::ReachabilityWorkTestCase;

#[test]
fn given_four_times_the_modules_when_finding_unreachable_definitions_then_work_grows_near_linearly()
{
    let test_cases = [ReachabilityWorkTestCase {
        description: "unreachable definitions walking every import, 500 against 2000 modules",
        modules: 500,
        growth: 4,
        config: json!({ "select": ["SQBRPYTHON", "SQBRPYTHON306"] }),
        expected_unreachable_definitions: 2001,
        expected_max_ratio: 5.0,
    }];
    for test_case in test_cases {
        let small = importing_modules(test_case.modules);
        let large = importing_modules(test_case.growth * test_case.modules);

        let (_, small_work) = counted_codes(&small, &test_case.config);
        let (codes, large_work) = counted_codes(&large, &test_case.config);

        let unreachable: usize = codes.iter().filter(|code| *code == "SQBRPYTHON306").count();
        assert_eq!(
            unreachable, test_case.expected_unreachable_definitions,
            "{}",
            test_case.description
        );
        let ratio: f64 = large_work as f64 / small_work as f64;
        assert!(
            ratio < test_case.expected_max_ratio,
            "{}: growing the project {}x multiplied the allocations by {ratio:.2} ({small_work} to {large_work})",
            test_case.description,
            test_case.growth
        );
    }
}
