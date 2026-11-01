use crate::format_command::models::{LintFinding, LintReport, RuleFixLine};

/// Owned copies of string literals.
pub(super) fn texts(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

/// A report with one excerpted fault, one location-only warning and one Rule fix.
pub(super) fn sample_report() -> LintReport {
    LintReport {
        files_checked: 3,
        formatted_files: texts(&["/p/models/orders.sql"]),
        rule_fixes: vec![RuleFixLine {
            file: "/p/models/orders.sql".to_owned(),
            code: "SQLCLEAN002".to_owned(),
            line: 4,
            status: "applied".to_owned(),
            reason: "ok".to_owned(),
        }],
        findings: vec![
            LintFinding {
                file: "/p/models/orders.sql".to_owned(),
                display_path: "models/orders.sql".to_owned(),
                line: 2,
                column: 8,
                end_line: Some(2),
                end_column: Some(15),
                code: "SQLCLEAN004".to_owned(),
                message: "Avoid LIMIT".to_owned(),
                fault: true,
                engine: "native".to_owned(),
                remediation: Some("Remove the LIMIT clause from the model body because the model must return every row it is expected to produce downstream.".to_owned()),
                source_line: Some("SELECT\t1 LIMIT 5".to_owned()),
            },
            LintFinding {
                file: "/other/x.sql".to_owned(),
                display_path: "/other/x.sql".to_owned(),
                line: 1,
                column: 1,
                code: "description-present".to_owned(),
                message: "Missing".to_owned(),
                engine: "sqlbuild".to_owned(),
                ..LintFinding::default()
            },
        ],
    }
}
