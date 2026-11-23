use std::path::{Path, PathBuf};

use crate::rule_evidence::main::import_closure::import_closure;
use crate::rule_evidence::main::project_import_closure::project_import_closure;
use crate::rule_evidence::main::test_evidence::test_evidence;
use crate::rule_evidence::tests::helpers::{project_with, rule_project};
use crate::rule_evidence::tests::test_types::{EvidenceTestCase, ProjectClosureTestCase};

const PYTHON: (u8, u8) = (3, 12);

#[test]
fn given_rule_project_when_reading_evidence_then_matches_python_closure_and_cases() {
    let test_cases = [
        EvidenceTestCase {
            description: "parametrized cases, a dead branch and a skipped test",
            project: "parametrized",
            test_source: "import pytest\nfrom sqlbuild.rules.testing import RuleCase, evaluate_rule\nfrom rules.custom import check\n\nCASES = [RuleCase(description='a', source='', expected_finding_count=0), RuleCase(description='b', source='', expected_finding_count=0)]\n\n@pytest.mark.parametrize('test_case', CASES)\ndef test_cases(test_case):\n    evaluate_rule(rule=check, test_case=test_case)\n    if False:\n        evaluate_rule(rule=check, test_case=RuleCase(description='dead', source='', expected_finding_count=0))\n\n@pytest.mark.skip\ndef test_skipped():\n    evaluate_rule(rule=check, test_case=RuleCase(description='skip', source='', expected_finding_count=0))\n",
            expected_closure: vec![
                "rules/__init__.py",
                "rules/shared/__init__.py",
                "rules/shared/util.py",
            ],
            expected_cases: vec![
                (5, 10, "tests/test_rule.py:test_cases"),
                (5, 74, "tests/test_rule.py:test_cases"),
            ],
        },
        EvidenceTestCase {
            description: "a harness call outside a test function counts nothing",
            project: "module_level",
            test_source: "from sqlbuild.rules.testing import RuleCase, evaluate_rule\nfrom rules.custom import check\n\nevaluate_rule(rule=check, test_case=RuleCase(description='a', source='', expected_finding_count=0))\n",
            expected_closure: vec![
                "rules/__init__.py",
                "rules/shared/__init__.py",
                "rules/shared/util.py",
            ],
            expected_cases: vec![],
        },
    ];

    for test_case in test_cases {
        let root: PathBuf = rule_project(test_case.project, test_case.test_source);
        let source: PathBuf = root.join("rules/custom.py");
        let closure: Vec<PathBuf> = import_closure(&source, &root, PYTHON);
        let cases: Vec<(usize, usize, String)> =
            test_evidence((&source, "check"), (&root, &root), PYTHON)
                .unwrap_or_default()
                .into_iter()
                .map(|case| (case.line, case.column, case.owner))
                .collect();
        let relative: Vec<&Path> = closure
            .iter()
            .map(|path| path.strip_prefix(&root).unwrap_or(path))
            .collect();

        assert_eq!(
            relative,
            test_case
                .expected_closure
                .iter()
                .map(Path::new)
                .collect::<Vec<_>>(),
            "{}",
            test_case.description
        );
        assert_eq!(
            cases,
            test_case
                .expected_cases
                .iter()
                .map(|(line, column, owner)| (*line, *column, (*owner).to_owned()))
                .collect::<Vec<_>>(),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_project_module_when_reading_project_closure_then_lists_imported_project_files() {
    let test_cases = [
        ProjectClosureTestCase {
            description: "helper modules across directories, transitively",
            files: vec![
                (
                    "sql/macros/money.py",
                    "import os\nfrom macros._rates import RATE\n\ndef cents(x):\n    return x\n",
                ),
                (
                    "sql/macros/_rates.py",
                    "from python import shared\nRATE = 100\n",
                ),
                ("python/__init__.py", ""),
                ("python/nodes/shared.py", "VALUE = 1\n"),
                ("python/nodes/unused.py", "VALUE = 2\n"),
            ],
            source: "sql/macros/money.py",
            expected_closure: vec![
                "sql/macros/_rates.py",
                "python/__init__.py",
                "python/nodes/shared.py",
            ],
        },
        ProjectClosureTestCase {
            description: "a module importing nothing local has an empty closure",
            files: vec![("sql/macros/plain.py", "import json\n")],
            source: "sql/macros/plain.py",
            expected_closure: vec![],
        },
    ];

    for test_case in test_cases {
        let root: PathBuf = project_with(test_case.description, &test_case.files);
        let closure: Vec<PathBuf> =
            project_import_closure(&root.join(test_case.source), &root, PYTHON);
        let relative: Vec<&Path> = closure
            .iter()
            .map(|path| path.strip_prefix(&root).unwrap_or(path))
            .collect();

        assert_eq!(
            relative,
            test_case
                .expected_closure
                .iter()
                .map(Path::new)
                .collect::<Vec<&Path>>(),
            "{}",
            test_case.description
        );
    }
}
