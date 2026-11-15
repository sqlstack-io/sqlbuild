use crate::lint_text::constants::{DIRECTIVE_FIX_REASON, DIRECTIVE_MESSAGE};
use crate::lint_text::errors::LintTextError;
use crate::lint_text::main::format_sql_bodies::format_sql_bodies;
use crate::lint_text::models::{FormatFile, SuppressionFinding};

const INDENT: &str = "\n            ";

/// The rejection finding for a directive on `line`, printing `rest` after the entry header.
pub(crate) fn directive_finding(line: usize, rule: &str, rest: &[&str]) -> SuppressionFinding {
    let mut remediation: String = format!(
        "Remove this comment, then fix the {rule} finding, or record it as a reviewed exception \
         in sqlbuild_project.toml:{INDENT}[[rules.rule_exceptions]]"
    );
    for line in rest {
        remediation.push_str(INDENT);
        remediation.push_str(line);
    }
    SuppressionFinding {
        file: 0,
        line,
        message: DIRECTIVE_MESSAGE.to_owned(),
        remediation,
        fault: true,
        fix: None,
        fix_unavailable_reason: Some(DIRECTIVE_FIX_REASON),
    }
}

/// A macro nesting `depth` calls, each with a comment that keeps it on several lines, `step` per level.
pub(crate) fn nested_macro(depth: usize, step: &str) -> String {
    let mut body: String = String::new();
    for level in 0..depth {
        body.push_str(&format!("{}COALESCE(\n", step.repeat(level)));
    }
    body.push_str(&format!("{}@value_text", step.repeat(depth)));
    for level in (0..depth).rev() {
        let inner: String = step.repeat(level + 1);
        body.push_str(&format!(
            ",\n{inner}-- Level {level}\n{inner}'level {level}'\n{})",
            step.repeat(level)
        ));
    }
    format!("MACRO (arguments (value_text VARCHAR), returns VARCHAR);\n\n{body}\n")
}

/// The project macro file `contents` as one `sqb format` run on `dialect` leaves it, without faults.
pub(crate) fn formatted_macro(contents: &str, dialect: &str) -> Result<String, LintTextError> {
    let files = [FormatFile {
        relative_parts: Some(vec![
            "macros".to_string(),
            "sql".to_string(),
            "m.sql".to_string(),
        ]),
        contents: contents.to_string(),
    }];
    let result = format_sql_bodies(&files, dialect, 100)?;
    assert_eq!(result.faults, Vec::new(), "{dialect}");
    Ok(result
        .formatted
        .into_iter()
        .next()
        .map_or_else(|| contents.to_string(), |(_, text)| text))
}
