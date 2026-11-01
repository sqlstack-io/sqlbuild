use crate::assembly::analysis_session::_helpers::resolution_rows::with_resolution_rows;
use crate::assembly::analysis_session::tests::test_types::ResolutionRowsTestCase;
use crate::semantic_validation::models::DiagnosticRow;

/// SQLBuild's own B101 wording, a row at line 0 in these cases.
const SPELLING_ROW: &str = "Unknown function 'NVL2' for dialect DuckDB; did you mean COALESCE?";

fn row((code, line, severity): (&str, usize, &str)) -> DiagnosticRow {
    let messages: [String; 2] = [format!("{code} at {line}"), SPELLING_ROW.to_owned()];
    let message: String = messages[usize::from(line == 0)].clone();
    DiagnosticRow {
        code: code.to_owned(),
        message,
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
            description: "a finding polyglot also reports stays an error; unowned rows stay",
            polyglot: &[
                ("B000", 1, "error"),
                ("B002", 2, "error"),
                ("B210", 3, "error"),
            ],
            sqlquack: &[(1, ("B002", 2, "error")), (2, ("B210", 3, "error"))],
            names_only: false,
            expected_rows: &[
                ("B000", 1, "error"),
                ("B002", 2, "error"),
                ("B210", 3, "error"),
            ],
        },
        ResolutionRowsTestCase {
            description: "scope rules come before polyglot's other rows, names and types after",
            polyglot: &[
                ("B230", 4, "error"),
                ("B103", 1, "error"),
                ("B002", 2, "error"),
            ],
            sqlquack: &[(1, ("B002", 2, "error")), (0, ("B230", 4, "error"))],
            names_only: false,
            expected_rows: &[
                ("B230", 4, "error"),
                ("B103", 1, "error"),
                ("B002", 2, "error"),
            ],
        },
        ResolutionRowsTestCase {
            description: "a finding polyglot does not report is a warning, after the rest",
            polyglot: &[("B103", 3, "error")],
            sqlquack: &[(2, ("B212", 2, "error"))],
            names_only: false,
            expected_rows: &[("B103", 3, "error"), ("B212", 2, "warning")],
        },
        ResolutionRowsTestCase {
            description: "polyglot's unknown functions and other owned rows SQLQuack lacks are dropped",
            polyglot: &[
                ("B101", 4, "error"),
                ("B005", 5, "error"),
                ("B217", 6, "error"),
            ],
            sqlquack: &[],
            names_only: false,
            expected_rows: &[],
        },
        ResolutionRowsTestCase {
            description: "SQLBuild's own spelling check of a built-in stays",
            polyglot: &[("B101", 0, "error")],
            sqlquack: &[],
            names_only: false,
            expected_rows: &[("B101", 0, "error")],
        },
        ResolutionRowsTestCase {
            description: "names only: polyglot keeps its type rows and SQLQuack's are left out",
            polyglot: &[
                ("B002", 2, "error"),
                ("B102", 3, "error"),
                ("B218", 4, "error"),
            ],
            sqlquack: &[
                (1, ("B002", 2, "error")),
                (2, ("B212", 5, "error")),
                (1, ("B005", 6, "error")),
            ],
            names_only: true,
            expected_rows: &[
                ("B002", 2, "error"),
                ("B005", 6, "warning"),
                ("B102", 3, "error"),
                ("B218", 4, "error"),
            ],
        },
    ];
    for test_case in test_cases {
        let merged: Vec<DiagnosticRow> = with_resolution_rows(
            test_case.polyglot.iter().copied().map(row).collect(),
            test_case
                .sqlquack
                .iter()
                .map(|(phase, found)| (*phase, row(*found)))
                .collect(),
            test_case.names_only,
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
