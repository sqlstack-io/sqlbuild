use crate::assembly::analysis_session::_helpers::resolution_rows::with_name_findings;
use crate::assembly::analysis_session::tests::test_types::{NameFindingsTestCase, RowShape};
use crate::semantic_validation::models::DiagnosticRow;

/// SQLBuild's own B101 wording, a row at line 0 in these cases.
const SPELLING_ROW: &str = "Unknown function 'NVL2' for dialect PostgreSQL; did you mean COALESCE?";

fn row((code, line, severity): RowShape) -> DiagnosticRow {
    let message: String = match line {
        0 => SPELLING_ROW.to_owned(),
        _ => format!("{code} at {line}"),
    };
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
fn given_name_findings_when_merging_diagnostics_then_they_replace_polyglot_names_only() {
    let test_cases = [
        NameFindingsTestCase {
            description: "a name polyglot also reports stays an error; one it alone reports goes",
            polyglot: &[("B002", 2, "error"), ("B003", 3, "error")],
            sqlquack: &[("B002", 2, "error")],
            expected_rows: &[("B002", 2, "error")],
        },
        NameFindingsTestCase {
            description: "a name polyglot does not report is a warning",
            polyglot: &[("B002", 2, "error")],
            sqlquack: &[("B002", 2, "error"), ("B005", 6, "error")],
            expected_rows: &[("B002", 2, "error"), ("B005", 6, "warning")],
        },
        NameFindingsTestCase {
            description: "the same code on another line is not the same finding",
            polyglot: &[("B004", 4, "error")],
            sqlquack: &[("B004", 5, "error")],
            expected_rows: &[("B004", 5, "warning")],
        },
        NameFindingsTestCase {
            description: "polyglot keeps its type rows after the names, and other rows before",
            polyglot: &[
                ("B218", 4, "error"),
                ("B002", 2, "error"),
                ("B000", 1, "error"),
                ("B102", 3, "error"),
            ],
            sqlquack: &[("B002", 2, "error")],
            expected_rows: &[
                ("B000", 1, "error"),
                ("B002", 2, "error"),
                ("B218", 4, "error"),
                ("B102", 3, "error"),
            ],
        },
        NameFindingsTestCase {
            description: "SQLQuack's type findings are left out",
            polyglot: &[],
            sqlquack: &[("B212", 5, "error"), ("B230", 6, "error")],
            expected_rows: &[],
        },
        NameFindingsTestCase {
            description: "SQLBuild's own spelling check of a built-in stays before the names",
            polyglot: &[("B101", 0, "error"), ("B101", 7, "error")],
            sqlquack: &[("B003", 2, "error")],
            expected_rows: &[
                ("B101", 0, "error"),
                ("B003", 2, "warning"),
                ("B101", 7, "error"),
            ],
        },
    ];
    for test_case in test_cases {
        let merged: Vec<DiagnosticRow> = with_name_findings(
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
