use crate::refactor_command::main::compare_with_baseline::compare_with_baseline;
use crate::refactor_command::models::BaselineComparison;
use crate::refactor_command::tests::helpers::{diagnostics, labels};
use crate::refactor_command::tests::test_types::{BaselineComparisonTestCase, DiagnosticRow};
use crate::refactoring::models::FileChange;

const OVER_FULL: DiagnosticRow = (
    true,
    "XSQBRLAYOUT001",
    "sql/models/staging",
    "sql/models/staging holds 3 models; the maximum is 2",
);

#[test]
fn given_rule_failures_before_and_after_when_comparing_then_only_new_or_worse_ones_block() {
    let test_cases = [
        BaselineComparisonTestCase {
            description: "a folder over its limit that loses a model keeps an existing failure",
            before: &[OVER_FULL],
            after: &[(
                true,
                "XSQBRLAYOUT001",
                "sql/models/staging",
                "sql/models/staging holds 2 models; the maximum is 2",
            )],
            moves: &[("sql/models/staging/a.sql", "sql/models/finance/a.sql")],
            expected_remaining: &["XSQBRLAYOUT001 sql/models/staging"],
            expected_resolved: &[],
            expected_blocking: &[],
        },
        BaselineComparisonTestCase {
            description: "a failure the moves repair is resolved",
            before: &[OVER_FULL],
            after: &[],
            moves: &[("sql/models/staging/a.sql", "sql/models/finance/a.sql")],
            expected_remaining: &[],
            expected_resolved: &["XSQBRLAYOUT001 sql/models/staging"],
            expected_blocking: &[],
        },
        BaselineComparisonTestCase {
            description: "a folder over its limit that gains a model is worse and blocks",
            before: &[OVER_FULL],
            after: &[(
                true,
                "XSQBRLAYOUT001",
                "sql/models/staging",
                "sql/models/staging holds 4 models; the maximum is 2",
            )],
            moves: &[("sql/models/finance/a.sql", "sql/models/staging/a.sql")],
            expected_remaining: &[],
            expected_resolved: &[],
            expected_blocking: &["XSQBRLAYOUT001 sql/models/staging"],
        },
        BaselineComparisonTestCase {
            description: "a failure in a folder that had none is new and blocks",
            before: &[OVER_FULL],
            after: &[
                OVER_FULL,
                (
                    true,
                    "XSQBRLAYOUT001",
                    "sql/models/finance",
                    "sql/models/finance holds 3 models; the maximum is 2",
                ),
            ],
            moves: &[("sql/models/marts/a.sql", "sql/models/finance/a.sql")],
            expected_remaining: &["XSQBRLAYOUT001 sql/models/staging"],
            expected_resolved: &[],
            expected_blocking: &["XSQBRLAYOUT001 sql/models/finance"],
        },
        BaselineComparisonTestCase {
            description: "an unchanged failure on a moved file follows the file",
            before: &[(
                true,
                "SQBRMODEL101",
                "sql/models/staging/a.sql",
                "a has no tests",
            )],
            after: &[(
                true,
                "SQBRMODEL101",
                "sql/models/finance/a.sql",
                "a has no tests",
            )],
            moves: &[("sql/models/staging/a.sql", "sql/models/finance/a.sql")],
            expected_remaining: &["SQBRMODEL101 sql/models/finance/a.sql"],
            expected_resolved: &[],
            expected_blocking: &[],
        },
        BaselineComparisonTestCase {
            description: "a changed failure on an edited file blocks",
            before: &[(
                true,
                "XSQBRSIZE001",
                "sql/models/marts/totals.sql",
                "totals reads 2 models",
            )],
            after: &[(
                true,
                "XSQBRSIZE001",
                "sql/models/marts/totals.sql",
                "totals reads 3 models",
            )],
            moves: &[("sql/models/marts/totals.sql", "sql/models/marts/totals.sql")],
            expected_remaining: &[],
            expected_resolved: &[],
            expected_blocking: &["XSQBRSIZE001 sql/models/marts/totals.sql"],
        },
        BaselineComparisonTestCase {
            description: "a changed failure without a place blocks",
            before: &[(true, "XSQBRMART001", "", "the project has 2 marts")],
            after: &[(true, "XSQBRMART001", "", "the project has 3 marts")],
            moves: &[("sql/models/staging/a.sql", "sql/models/marts/a.sql")],
            expected_remaining: &[],
            expected_resolved: &[],
            expected_blocking: &["XSQBRMART001 "],
        },
        BaselineComparisonTestCase {
            description: "a reduced failure without a place is existing",
            before: &[(true, "XSQBRMART001", "", "the project has 3 marts")],
            after: &[(true, "XSQBRMART001", "", "the project has 2 marts")],
            moves: &[("sql/models/marts/a.sql", "sql/models/staging/a.sql")],
            expected_remaining: &["XSQBRMART001 "],
            expected_resolved: &[],
            expected_blocking: &[],
        },
        BaselineComparisonTestCase {
            description: "a failure whose words change blocks even with smaller numbers",
            before: &[(
                true,
                "SQBRPROJECT204",
                "sql/models/o/a.sql",
                "owner o has fin_a and fin_b",
            )],
            after: &[(
                true,
                "SQBRPROJECT204",
                "sql/models/o/a.sql",
                "owner o has fin_a and fin_c",
            )],
            moves: &[("sql/models/o/c/c.sql", "sql/models/o/fin_c/c.sql")],
            expected_remaining: &[],
            expected_resolved: &[],
            expected_blocking: &["SQBRPROJECT204 sql/models/o/a.sql"],
        },
        BaselineComparisonTestCase {
            description: "a count that grows at an untouched place blocks",
            before: &[(
                true,
                "SQBRPROJECT204",
                "sql/models/o/a.sql",
                "2 sibling owner directories",
            )],
            after: &[(
                true,
                "SQBRPROJECT204",
                "sql/models/o/a.sql",
                "3 sibling owner directories",
            )],
            moves: &[("sql/models/o/c/c.sql", "sql/models/o/fin_c/c.sql")],
            expected_remaining: &[],
            expected_resolved: &[],
            expected_blocking: &["SQBRPROJECT204 sql/models/o/a.sql"],
        },
        BaselineComparisonTestCase {
            description: "nothing counts as resolved when another error may have hidden rules",
            before: &[OVER_FULL],
            after: &[(
                false,
                "D016",
                "sql/models/staging/a.sql",
                "invalid model identity",
            )],
            moves: &[],
            expected_remaining: &[],
            expected_resolved: &[],
            expected_blocking: &["D016 sql/models/staging/a.sql"],
        },
        BaselineComparisonTestCase {
            description: "errors that are not rule failures always block",
            before: &[OVER_FULL],
            after: &[
                OVER_FULL,
                (false, "S006", "sql/models/staging/a.sql", "not visible"),
            ],
            moves: &[],
            expected_remaining: &["XSQBRLAYOUT001 sql/models/staging"],
            expected_resolved: &[],
            expected_blocking: &["S006 sql/models/staging/a.sql"],
        },
        BaselineComparisonTestCase {
            description: "two failures where the project had one: the second is new",
            before: &[OVER_FULL],
            after: &[OVER_FULL, OVER_FULL],
            moves: &[],
            expected_remaining: &["XSQBRLAYOUT001 sql/models/staging"],
            expected_resolved: &[],
            expected_blocking: &["XSQBRLAYOUT001 sql/models/staging"],
        },
    ];
    for test_case in test_cases {
        let changes: Vec<FileChange> = test_case
            .moves
            .iter()
            .map(|(original_path, path)| FileChange {
                path: (*path).to_owned(),
                original_path: (*original_path).to_owned(),
                edits: Vec::new(),
            })
            .collect();

        let comparison: BaselineComparison = compare_with_baseline(
            &diagnostics(test_case.before),
            &diagnostics(test_case.after),
            &changes,
        );

        assert_eq!(
            (
                labels(&comparison.remaining),
                labels(&comparison.resolved),
                labels(&comparison.blocking),
            ),
            (
                test_case
                    .expected_remaining
                    .iter()
                    .map(|label| (*label).to_owned())
                    .collect(),
                test_case
                    .expected_resolved
                    .iter()
                    .map(|label| (*label).to_owned())
                    .collect(),
                test_case
                    .expected_blocking
                    .iter()
                    .map(|label| (*label).to_owned())
                    .collect(),
            ),
            "{}",
            test_case.description
        );
    }
}
