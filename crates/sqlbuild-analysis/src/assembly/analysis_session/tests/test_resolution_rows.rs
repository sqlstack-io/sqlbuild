use crate::assembly::analysis_session::_helpers::resolution_rows::with_resolution_rows;
use crate::assembly::analysis_session::tests::test_types::ResolutionRowsTestCase;
use crate::semantic_validation::models::DiagnosticRow;

fn row((code, line, severity): (&str, usize, &str)) -> DiagnosticRow {
    DiagnosticRow {
        code: code.to_owned(),
        message: format!("{code} at {line}"),
        line: Some(line),
        column: Some(1),
        start: None,
        end: None,
        severity: severity.to_owned(),
    }
}

#[test]
fn given_routed_findings_when_merging_diagnostics_then_only_shared_ones_stay_errors() {
    let test_cases = [
        ResolutionRowsTestCase {
            description: "a finding polyglot also reports stays an error in its place",
            polyglot: &[
                ("B101", 1, "error"),
                ("B002", 2, "error"),
                ("B210", 3, "error"),
            ],
            sqlquack: &[("B002", 2, "error")],
            expected_rows: &[
                ("B101", 1, "error"),
                ("B002", 2, "error"),
                ("B210", 3, "error"),
            ],
        },
        ResolutionRowsTestCase {
            description: "a finding polyglot does not report is a warning, after the rest",
            polyglot: &[("B210", 3, "error")],
            sqlquack: &[("B002", 2, "error")],
            expected_rows: &[("B210", 3, "error"), ("B002", 2, "warning")],
        },
        ResolutionRowsTestCase {
            description: "a polyglot finding SQLQuack does not report is dropped",
            polyglot: &[("B003", 4, "error"), ("B005", 5, "error")],
            sqlquack: &[],
            expected_rows: &[("B005", 5, "error")],
        },
    ];
    for test_case in test_cases {
        let merged: Vec<DiagnosticRow> = with_resolution_rows(
            test_case.polyglot.iter().copied().map(row).collect(),
            test_case.sqlquack.iter().copied().map(row).collect(),
        );
        assert_eq!(
            merged,
            test_case
                .expected_rows
                .iter()
                .copied()
                .map(row)
                .collect::<Vec<DiagnosticRow>>(),
            "{}",
            test_case.description
        );
    }
}
