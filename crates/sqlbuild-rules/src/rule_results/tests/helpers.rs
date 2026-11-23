use crate::rule_results::models::RuleFinding;

/// One unevaluated finding of `code` in `sql/models/a.sql` at `line`.
pub(crate) fn unevaluated(code: &str, line: i64) -> RuleFinding {
    RuleFinding {
        code: code.into(),
        path: "sql/models/a.sql".into(),
        line,
        column: 1,
        message: format!("Rule {code} could not be evaluated: too deep"),
        unevaluated: true,
        ..RuleFinding::default()
    }
}
