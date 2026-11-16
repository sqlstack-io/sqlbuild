use crate::tests::helpers::install_test_identity;
use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::json;

use crate::python_rules::tests::helpers::{
    median_growth, purity_chain_project, scaling_project, serial_timing, timed_python_codes,
    unit_scaling_project,
};
use crate::python_rules::tests::test_types::{
    PurityScalingTestCase, PythonGrowthTestCase, PythonScalingTestCase, UnitScalingTestCase,
};

#[test]
fn given_many_modules_and_dense_findings_when_evaluating_python_rules_then_finishes_within_budget()
{
    install_test_identity();
    let _serial = serial_timing();
    let test_cases = [PythonScalingTestCase {
        description: "default rules over 4000 modules and 40 hooks of 300 findings each",
        units: 4000,
        noisy_files: 40,
        functions: 150,
        config: json!({ "select": ["SQBRPYTHON"] }),
        expected_annotation_faults: 12000,
        expected_max_duration: Duration::from_secs(5),
    }];
    for test_case in test_cases {
        let files = scaling_project(test_case.units, test_case.noisy_files, test_case.functions);
        let (codes, elapsed) = timed_python_codes(&files, test_case.config).expect("rules run");
        let annotation_faults: usize = codes.iter().filter(|code| *code == "SQBRPYTHON101").count();
        assert_eq!(
            annotation_faults, test_case.expected_annotation_faults,
            "{}",
            test_case.description
        );
        assert!(
            elapsed < test_case.expected_max_duration,
            "{}: took {elapsed:?}",
            test_case.description
        );
    }
}

#[test]
fn given_four_times_the_modules_when_finding_unreachable_definitions_then_time_grows_near_linearly()
{
    install_test_identity();
    let _serial = serial_timing();
    let test_cases = [PythonGrowthTestCase {
        description: "unreachable definitions walking every import, 500 against 2000 modules",
        units: 500,
        noisy_files: 5,
        functions: 150,
        config: json!({ "select": ["SQBRPYTHON", "SQBRPYTHON306"] }),
        growth: 4,
        pairs: 5,
        expected_unreachable_definitions: 2001,
        expected_max_ratio: 6.5,
    }];
    for test_case in test_cases {
        let small = scaling_project(test_case.units, test_case.noisy_files, test_case.functions);
        let large = scaling_project(
            test_case.growth * test_case.units,
            test_case.growth * test_case.noisy_files,
            test_case.functions,
        );
        let (codes, _) = timed_python_codes(&large, test_case.config.clone()).expect("rules run");
        let unreachable: usize = codes.iter().filter(|code| *code == "SQBRPYTHON306").count();
        assert_eq!(
            unreachable, test_case.expected_unreachable_definitions,
            "{}",
            test_case.description
        );
        let ratio: f64 = median_growth(&small, &large, &test_case.config, test_case.pairs);
        assert!(
            ratio < test_case.expected_max_ratio,
            "{}: growing the project {}x multiplied the time by {ratio:.2}",
            test_case.description,
            test_case.growth
        );
    }
}

#[test]
fn given_many_linked_node_units_when_evaluating_unit_rules_then_finishes_within_budget_and_grows_near_linearly()
 {
    install_test_identity();
    let _serial = serial_timing();
    let test_cases = [UnitScalingTestCase {
        description: "default rules over a chain of 2000 units of node, main and checks, and 200 undescribed units",
        units: 2000,
        undescribed: 200,
        config: json!({ "select": ["SQBRPYTHON"] }),
        expected_codes: vec![("SQBRPYTHON011", 200)],
        expected_max_duration: Duration::from_secs(5),
        growth: 4,
        pairs: 5,
        expected_max_ratio: 6.5,
    }];
    for test_case in test_cases {
        let large = unit_scaling_project(test_case.units, test_case.undescribed);
        let (codes, elapsed) =
            timed_python_codes(&large, test_case.config.clone()).expect("rules run");
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for code in &codes {
            *counts.entry(code.as_str()).or_default() += 1;
        }
        assert_eq!(
            counts.into_iter().collect::<Vec<_>>(),
            test_case.expected_codes,
            "{}",
            test_case.description
        );
        assert!(
            elapsed < test_case.expected_max_duration,
            "{}: took {elapsed:?}",
            test_case.description
        );
        let small = unit_scaling_project(
            test_case.units / test_case.growth,
            test_case.undescribed / test_case.growth,
        );
        let ratio: f64 = median_growth(&small, &large, &test_case.config, test_case.pairs);
        assert!(
            ratio < test_case.expected_max_ratio,
            "{}: growing the project {}x multiplied the time by {ratio:.2}",
            test_case.description,
            test_case.growth
        );
    }
}

#[test]
fn given_many_roots_sharing_deep_call_chains_when_checking_compile_time_purity_then_finishes_within_budget()
 {
    install_test_identity();
    let _serial = serial_timing();
    let test_cases = [PurityScalingTestCase {
        description: "5000 module-scope roots into 10 helper chains of 400 calls each",
        roots: 5000,
        chains: 10,
        depth: 400,
        expected_faults: 5000,
        expected_max_duration: Duration::from_secs(3),
    }];
    for test_case in test_cases {
        let files = purity_chain_project(test_case.roots, test_case.chains, test_case.depth);
        let (codes, elapsed) =
            timed_python_codes(&files, json!({ "select": ["SQBRPYTHON022"] })).expect("rules run");
        assert_eq!(
            codes.len(),
            test_case.expected_faults,
            "{}",
            test_case.description
        );
        assert!(
            elapsed < test_case.expected_max_duration,
            "{}: took {elapsed:?}",
            test_case.description
        );
    }
}
